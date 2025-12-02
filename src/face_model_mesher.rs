use bevy::{asset::RenderAssetUsages, prelude::*, render::storage::ShaderStorageBuffer};

use crate::{
    chunk::LOW_NIBBLE,
    chunk_mesh::{ChunkMesh, Face},
    chunks_refs::ChunksRefs,
    constants::{ADJACENT_AO_DIRS, CHUNK_SIZE},
    lod::Lod,
    models::{
        IndexedModel, IndexedModelRegistry,
        model::{TexturedBlockModel, VoxelTexturingType},
    },
    utils::{generate_indices, index_to_ivec3},
    voxel::{BlockId, BlockRegistry},
};

const DIRECTION_OFFSET: [IVec3; 6] = [
    IVec3::X,     // Right
    IVec3::NEG_X, // Left
    IVec3::Y,     // Up
    IVec3::NEG_Y, // Down
    IVec3::Z,     // Back
    IVec3::NEG_Z, // Forward
];

pub fn build_chunk_mesh(
    chunks_refs: &ChunksRefs,
    lod: Lod,
    block_registry: &BlockRegistry,
    model_registry: &IndexedModelRegistry,
    flag_to_build: u8,
    cull_face_flag: u8,
    calculate_ao: bool,
) -> Option<ChunkMesh> {
    let _span = info_span!("Meshing Chunk.").entered();

    // early exit, if all faces are culled
    if chunks_refs.is_all_voxels_same() {
        return None;
    }

    let mut mesh = ChunkMesh::default();

    let mut add_voxel = |i: usize, voxel: BlockId| {
        // Skip non-solid blocks
        if !block_registry.has_flag(voxel, flag_to_build) {
            return;
        }
        let pos = index_to_ivec3(i);

        let offset_pos = pos + IVec3::splat(CHUNK_SIZE as i32);

        let mut visible_faces = 0;
        for (i, &offset) in DIRECTION_OFFSET.iter().enumerate() {
            let neighbor_pos = offset_pos + offset;
            let neighbor_block = chunks_refs.get_block_pre_offset(neighbor_pos);
            if !block_registry.has_flag(neighbor_block, cull_face_flag) {
                visible_faces |= 1 << i;
            }
        }

        let textured_model = &block_registry.block_model[voxel.0 as usize];
        let model = &model_registry.models[textured_model.model.0 as usize];

        let ao = if calculate_ao {
            let ao_directions = model.always_required_face_directions | visible_faces;

            compute_voxel_ao(chunks_refs, offset_pos, block_registry, ao_directions)
        } else {
            [0; 6]
        };

        add_block_to_mesh(
            &mut mesh,
            visible_faces,
            textured_model,
            model,
            ao,
            i as u32,
        );
    };

    let center_chunk = &chunks_refs.chunks[13];
    match center_chunk.index_size {
        crate::chunk::IndexSize::Nibble => {
            for (i, &nibbles) in center_chunk.voxels.iter().enumerate() {
                let block_a = center_chunk.palette[(nibbles & LOW_NIBBLE) as usize];
                let block_b = center_chunk.palette[(nibbles >> 4) as usize];

                add_voxel(i * 2, block_a);
                add_voxel(i * 2 + 1, block_b);
            }
        }
        crate::chunk::IndexSize::Byte => {
            for (i, &byte) in center_chunk.voxels.iter().enumerate() {
                let block = center_chunk.palette[byte as usize];

                add_voxel(i, block);
            }
        }
        crate::chunk::IndexSize::Short => {
            for (i, bytes) in center_chunk.voxels.chunks_exact(2).enumerate() {
                let bytes = u16::from_ne_bytes([bytes[0], bytes[1]]);
                let block = center_chunk.palette[bytes as usize];

                add_voxel(i, block);
            }
        }
    }

    if mesh.faces.is_empty() {
        None
    } else {
        mesh.indices = generate_indices(mesh.faces.len());
        Some(mesh)
    }
}

pub fn build_single_block_mesh(
    block_id: BlockId,
    block_registry: &BlockRegistry,
    model_registry: &IndexedModelRegistry,
) -> (Mesh, ShaderStorageBuffer) {
    let mut chunk_mesh = ChunkMesh::default();

    let textured_model = &block_registry.block_model[block_id.0 as usize];
    let model = &model_registry.models[textured_model.model.0 as usize];

    add_block_to_mesh(&mut chunk_mesh, 0b111111, textured_model, model, [0; 6], 0);

    chunk_mesh.indices = generate_indices(chunk_mesh.faces.len());

    let (mesh, face_buffer, _) = chunk_mesh.into_bevy_mesh();

    (mesh, face_buffer)
}

fn add_block_to_mesh(
    mesh: &mut ChunkMesh,
    mut visible_faces: u8,
    textured_model: &TexturedBlockModel,
    model: &IndexedModel,
    ao: [u8; 6],
    packed_pos: u32,
) {
    // Add always visible faces
    mesh.faces.extend(
        (model.always_visible_faces.start..model.always_visible_faces.end)
            .zip(&model.always_visible_faces.ao_direction)
            .enumerate()
            .map(|(i, (quad, direction))| Face {
                pos_ao: packed_pos | (ao[*direction as usize] as u32) << 15,
                model_id: quad,
                texture_id: match &textured_model.texture_ids {
                    VoxelTexturingType::SingleTexture(id) => *id,
                    VoxelTexturingType::MultiTexture { all_faces } => {
                        all_faces[6].get(i).copied().unwrap_or(0)
                    }
                },
            }),
    );

    while visible_faces != 0 {
        let i = visible_faces.trailing_zeros() as usize;
        visible_faces &= !(1 << i);

        let quad_range = &model.occluded_faces[i];

        mesh.faces.extend(
            (quad_range.start..quad_range.end)
                .zip(&quad_range.ao_direction)
                .enumerate()
                .map(|(j, (quad, face))| Face {
                    pos_ao: packed_pos | (ao[*face as usize] as u32) << 15,
                    model_id: quad,
                    texture_id: match &textured_model.texture_ids {
                        VoxelTexturingType::SingleTexture(id) => *id,
                        VoxelTexturingType::MultiTexture { all_faces } => {
                            all_faces[i].get(j).copied().unwrap_or(0)
                        }
                    },
                }),
        );
    }
}

/// Computes the AO for all 24 voxel face corners.
fn compute_voxel_ao(
    chunks: &ChunksRefs,
    voxel_pos: IVec3,
    registry: &BlockRegistry,
    directions: u8,
) -> [u8; 6] {
    if directions == 0 {
        return [0; 6];
    }

    // Step 1. Pack filled blocks into a u32. 1 is filled, 0 is empty.
    // We use these to count the number of filled neighbors for each corner.
    let mut ao_filled_per_axis = [0u16; 6];

    for (i, axis_val) in ao_filled_per_axis.iter_mut().enumerate() {
        // We only need to calculate AO for the faces that are visible.
        if (directions & (1 << i)) == 0 {
            continue;
        }

        let mut ao_index = 0u16;

        for (ao_i, ao_offset) in ADJACENT_AO_DIRS.iter().enumerate() {
            // ambient occlusion is sampled based on axis(ascent or descent)
            let ao_sample_offset = match i {
                0 => IVec3::new(1, ao_offset.y, ao_offset.x),  // +X
                1 => IVec3::new(-1, ao_offset.y, ao_offset.x), // -X
                2 => IVec3::new(ao_offset.x, 1, ao_offset.y),  // +Y
                3 => IVec3::new(ao_offset.x, -1, ao_offset.y), // -Y
                4 => IVec3::new(ao_offset.x, ao_offset.y, 1),  // +Z
                5 => IVec3::new(ao_offset.x, ao_offset.y, -1), // -Z,
                _ => unreachable!(),
            };
            let ao_voxel_pos = voxel_pos + ao_sample_offset;
            let ao_block = chunks.get_block_pre_offset(ao_voxel_pos);
            if registry.is_solid(ao_block) {
                ao_index |= 1 << ao_i;
            }
        }

        *axis_val = ao_index;
    }

    // Step 2 pack the 24 corners into u64.
    // Each corner is 2 bits, so 24 corners = 48 bits.

    let mut ao_per_face = [0; 6];

    for axis in 0..6 {
        let ao = ao_filled_per_axis[axis];

        let v0ao = ((ao >> 3) & 1) + ((ao >> 1) & 1) + ((ao >> 0) & 1);
        let v1ao = ((ao >> 5) & 1) + ((ao >> 1) & 1) + ((ao >> 2) & 1);
        let v2ao = ((ao >> 5) & 1) + ((ao >> 7) & 1) + ((ao >> 8) & 1);
        let v3ao = ((ao >> 3) & 1) + ((ao >> 7) & 1) + ((ao >> 6) & 1);

        let packed_ao = (v0ao << 0) | (v1ao << 2) | (v2ao << 4) | (v3ao << 6);

        ao_per_face[axis] = packed_ao as u8;
    }

    ao_per_face
}
