use bevy::prelude::*;

use crate::{
    chunk_mesh::{ChunkMesh, Face}, chunks_refs::ChunksRefs, constants::CHUNK_SIZE, lod::Lod, models::{model::{VoxelTexturingType, AO_CORNERS}, IndexedModelRegistry}, utils::generate_indices, voxel::{BlockFlags, BlockRegistry}
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
                    0
                };

                let packed_pos_ao = (pos.x as u32) | (pos.y as u32) << 5 | (pos.z as u32) << 10 | (ao << 15);

                // Add always visible faces
                mesh.faces.extend((model.always_visible_faces.start..model.always_visible_faces.end).enumerate().map(|(i, quad)| {
                    Face {
                        pos_ao: packed_pos_ao,
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
                        mesh.faces.extend((quad_range.start..quad_range.end).enumerate().map(|(j, quad)| {
                            Face {
                                pos_ao: packed_pos_ao,
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

fn compute_voxel_ao(
    chunks: &ChunksRefs,
    base_pos: IVec3,
    registry: &BlockRegistry,
) -> u32 {
    let mut packed = 0u32;

    for (i, &corner) in AO_CORNERS.iter().enumerate() {
        // For each corner, check the 3 neighbor voxels that share it:
        // They are offset -1 along each axis from the corner position.
        let neighbor1 = base_pos + IVec3::new(corner[0] - 1, corner[1], corner[2]);
        let neighbor2 = base_pos + IVec3::new(corner[0], corner[1] - 1, corner[2]);
        let neighbor3 = base_pos + IVec3::new(corner[0], corner[1], corner[2] - 1);

        let mut occ = 0;
        if registry.is_solid(chunks.get_block(neighbor1).block_type) { occ += 1; }
        if registry.is_solid(chunks.get_block(neighbor2).block_type) { occ += 1; }
        if registry.is_solid(chunks.get_block(neighbor3).block_type) { occ += 1; }

        // Pack 2 bits per corner (values 0..=3), total 8 corners → 16 bits
        packed |= (occ as u32 & 0b11) << (i * 2);
    }

    packed
}
