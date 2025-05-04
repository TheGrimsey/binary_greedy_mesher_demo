use bevy::prelude::*;

use crate::{
    chunk_mesh::{ChunkMesh, Face}, chunks_refs::ChunksRefs, constants::CHUNK_SIZE, lod::Lod, models::IndexedModelRegistry, utils::generate_indices, voxel::{BlockFlags, BlockRegistry}
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

                let model = &model_registry.models[block_registry.block_model[voxel.block_type.0 as usize].0 as usize];

                let packed_pos_ao = (pos.x as u32) | (pos.y as u32) << 5 | (pos.z as u32) << 10;

                // Add always visible faces
                mesh.faces.extend((model.always_visible_faces.start..model.always_visible_faces.end).map(|quad| {
                    Face {
                        pos_ao: packed_pos_ao,
                        model_id: quad,
                        texture_id: 0,
                    }
                }));

                for (&offset, quad_range) in DIRECTION_OFFSET.iter().zip(&model.occluded_faces) {
                    // Check if the neighbor in the direction is solid
                    if !block_registry.is_solid(chunks_refs.get_block(pos + offset).block_type) {
                        mesh.faces.extend((quad_range.start..quad_range.end).map(|quad| {
                            Face {
                                pos_ao: packed_pos_ao,
                                model_id: quad,
                                texture_id: 0,
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
