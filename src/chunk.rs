use bevy::prelude::*;
use bracket_noise::prelude::*;

use crate::{
    constants::CHUNK_SIZE3, utils::index_to_ivec3, voxel::{BlockData, BlockId}
};

#[derive(Clone)]
pub struct ChunkData {
    pub voxels: Vec<BlockData>,
}

impl ChunkData {
    #[inline]
    pub fn get_block(&self, index: usize) -> &BlockData {
        if self.voxels.len() == 1 {
            &self.voxels[0]
        } else {
            &self.voxels[index]
        }
    }

    // returns the block type if all voxels are the same
    #[inline]
    pub fn get_block_if_filled(&self) -> Option<&BlockData> {
        if self.voxels.len() == 1 {
            Some(&self.voxels[0])
        } else {
            None
        }
    }

    /// shape our voxel data based on the chunk_pos
    pub fn generate(chunk_pos: IVec3) -> Self {
        // hardcoded extremity check
        if chunk_pos.y * 32 + 32 > 21 + 32 {
            return Self {
                voxels: vec![BlockData {
                    block_type: BlockId(0),
                }],
            };
        }
        // hardcoded extremity check
        if chunk_pos.y * 32 < -21 - 32 {
            return Self {
                voxels: vec![BlockData {
                    block_type: BlockId(2),
                }],
            };
        }
        let mut voxels = Vec::with_capacity(CHUNK_SIZE3);
        let mut fast_noise = FastNoise::new();
        fast_noise.set_frequency(0.0254);
        for i in 0..32 * 32 * 32 {
            let voxel_pos = (chunk_pos * 32) + index_to_ivec3(i);
            let scale = 1.0;
            fast_noise.set_frequency(0.0254);
            let overhang = fast_noise.get_noise3d(
                voxel_pos.x as f32 * scale,
                voxel_pos.y as f32,
                voxel_pos.z as f32 * scale,
            ) * 55.0;
            fast_noise.set_frequency(0.002591);
            let noise_2 =
                fast_noise.get_noise(voxel_pos.x as f32 + overhang, voxel_pos.z as f32 * scale);
            let h = noise_2 * 30.0;
            let solid = h > voxel_pos.y as f32;

            let block_type = match solid {
                true => match (h - voxel_pos.y as f32) > 1.0 {
                    true => BlockId(1),
                    false => BlockId(2),
                },
                false => {
                    let on_x = voxel_pos.x % 16 == 0;
                    let on_z = voxel_pos.z % 16 == 0;

                    let on_x_chunk = chunk_pos.x % 4 == 0;
                    let on_z_chunk = chunk_pos.z % 4 == 0;

                    if on_x ^ on_z && on_x_chunk ^ on_z_chunk { BlockId(3) } else { BlockId(0) }
                },
            };
            voxels.push(BlockData { block_type });
        }

        Self { voxels }
    }
}
