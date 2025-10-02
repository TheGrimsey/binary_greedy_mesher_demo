use std::sync::Arc;

use bevy::{
    math::{IVec3, UVec3},
    platform::collections::HashMap,
};
/*use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;*/

use crate::{
    chunk::ChunkData,
    utils::{CHUNK_POWER, index_to_ivec3_bounds, vec3_to_index, vec3_to_index_in_chunk},
    voxel::BlockId,
};

// pointers to chunk data, a middle one with all their neighbours
#[derive(Clone)]
pub struct ChunksRefs {
    pub chunks: Vec<Arc<ChunkData>>,
}

impl ChunksRefs {
    /// construct a ChunkRefs at middle_chunk position
    /// safety: panics if ChunkData doesn't exist in input world_data
    pub fn try_new(
        world_data: &HashMap<IVec3, Arc<ChunkData>>,
        middle_chunk: IVec3,
    ) -> Option<Self> {
        let mut chunks = vec![];
        for i in 0..3 * 3 * 3 {
            let offset = index_to_ivec3_bounds(i, 3) + IVec3::splat(-1);
            chunks.push(Arc::clone(
                world_data.get(&(middle_chunk + offset)).unwrap(),
            ))
        }
        Some(Self { chunks })
    }
    // returns if all the voxels are the same
    // this is an incredibly fast approximation (1 sample per chunk) all = voxels[0]
    // so may be inacurate, but the odds are incredibly low
    pub fn is_all_voxels_same(&self) -> bool {
        let first_block = self.chunks[0].get_block_if_filled();
        if first_block.is_none() {
            return false;
        };

        self.chunks
            .iter()
            .skip(1)
            .all(|chunk| chunk.get_block_if_filled() == first_block)
    }

    /// helper function to get block data that may exceed the bounds of the middle chunk
    /// input position is local pos to middle chunk
    pub fn get_block(&self, pos: IVec3) -> BlockId {
        let x = (pos.x + 32) as u32;
        let y = (pos.y + 32) as u32;
        let z = (pos.z + 32) as u32;

        self.get_block_pre_offset(IVec3::new(x as i32, y as i32, z as i32))
    }
    pub fn get_block_pre_offset(&self, pos: IVec3) -> BlockId {
        let chunk = pos >> CHUNK_POWER;
        let local_pos = pos & ((1 << CHUNK_POWER) - 1);

        let chunk_index = vec3_to_index(chunk, 3);
        let chunk_data = &self.chunks[chunk_index];
        let i = vec3_to_index_in_chunk(local_pos.as_uvec3());
        chunk_data.get_block(i)
    }

    pub fn get_block_in_center_chunk(&self, pos: UVec3) -> BlockId {
        let i = vec3_to_index_in_chunk(pos);
        self.chunks[13].get_block(i)
    }
}
