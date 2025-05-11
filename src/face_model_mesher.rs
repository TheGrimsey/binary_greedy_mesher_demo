use bevy::prelude::*;

use crate::{
    chunk_mesh::{ChunkMesh, Face}, chunks_refs::ChunksRefs, constants::{ADJACENT_AO_DIRS, CHUNK_SIZE}, lod::Lod, models::{model::VoxelTexturingType, IndexedModelRegistry}, utils::generate_indices, voxel::{BlockFlags, BlockRegistry}
};

const DIRECTION_OFFSET: [IVec3; 6] = [
    IVec3::new(1, 0, 0),  // Right
    IVec3::new(-1, 0, 0), // Left
    IVec3::new(0, 1, 0),  // Up
    IVec3::new(0, -1, 0), // Down
    IVec3::new(0, 0, 1),  // Back
    IVec3::new(0, 0, -1), // Forward
];

pub fn build_chunk_mesh(chunks_refs: &ChunksRefs, lod: Lod, block_registry: &BlockRegistry, model_registry: &IndexedModelRegistry, flag_to_build: BlockFlags, calculate_ao: bool) -> Option<ChunkMesh> {
    // early exit, if all faces are culled
    if chunks_refs.is_all_voxels_same() {
        return None;
    }
    
    let mut mesh = ChunkMesh::default();

    for z in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let pos = IVec3::new(x as i32, y as i32, z as i32);

                // Get the block at the current voxel position
                let voxel = chunks_refs.get_block(pos);
                
                // Skip non-solid blocks
                if !block_registry.has_flag(voxel.block_type, flag_to_build) {
                    continue;
                }

                let textured_model = &block_registry.block_model[voxel.block_type.0 as usize];
                let model = &model_registry.models[textured_model.model.0 as usize];

                let ao = if calculate_ao {
                    compute_voxel_ao(chunks_refs, pos, block_registry)
                } else {
                    [0; 6]
                };

                let packed_pos = pos.x as u32 | (pos.y as u32) << 5 | (pos.z as u32) << 10;

                // Add always visible faces
                mesh.faces.extend((model.always_visible_faces.start..model.always_visible_faces.end).zip(&model.always_visible_faces.ao_direction).enumerate().map(|(i, (quad, face))| {
                    Face {
                        pos_ao: packed_pos | (ao[*face as usize] as u32) << 15,
                        model_id: quad,
                        texture_id: match &textured_model.texture_ids {
                            VoxelTexturingType::SingleTexture(id) => *id,
                            VoxelTexturingType::MultiTexture { all_faces } => all_faces[6].get(i).copied().unwrap_or(0),
                        },
                    }
                }));

                for (i, (&offset, quad_range)) in DIRECTION_OFFSET.iter().zip(&model.occluded_faces).enumerate() {
                    // Check if the neighbor in the direction is solid
                    if !block_registry.is_solid(chunks_refs.get_block(pos + offset).block_type) {
                        mesh.faces.extend((quad_range.start..quad_range.end).zip(&quad_range.ao_direction).enumerate().map(|(j, (quad, face))| {
                            Face {
                                pos_ao: packed_pos | (ao[*face as usize] as u32) << 15,
                                model_id: quad,
                                texture_id: match &textured_model.texture_ids {
                                    VoxelTexturingType::SingleTexture(id) => *id,
                                    VoxelTexturingType::MultiTexture { all_faces } => all_faces[i].get(j).copied().unwrap_or(0),
                                },
                            }
                        }));
                    }
                }
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

/// Computes the AO for all 24 voxel face corners.
fn compute_voxel_ao(
    chunks: &ChunksRefs,
    voxel_pos: IVec3,
    registry: &BlockRegistry,
) -> [u8; 6] {
    // Step 1. Pack filled blocks into a u32. 1 is filled, 0 is empty.
    // We use these to count the number of filled neighbors for each corner.

    let mut ao_filled_per_axis = [0u16; 6];

    for (i, axis_val) in ao_filled_per_axis.iter_mut().enumerate() {
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
            let ao_block = chunks.get_block(ao_voxel_pos);
            if registry.is_solid(ao_block.block_type) {
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
