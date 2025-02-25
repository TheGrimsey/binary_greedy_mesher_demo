use std::sync::Arc;

use bevy::prelude::*;
use bracket_noise::prelude::*;

use crate::{
    constants::{CHUNK_SIZE, CHUNK_SIZE3}, utils::index_to_ivec3, voxel::{BlockData, BlockId}
};

#[derive(Resource)]
pub struct ChunkGenerator {
    pub generate: Arc<dyn Fn(IVec3) -> ChunkData + Send + Sync>,
}

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
}


/// shape our voxel data based on the chunk_pos
pub fn generate(chunk_pos: IVec3) -> ChunkData {

    // hardcoded extremity check
    let chunk_height_limit = 3;

    if chunk_pos.y > chunk_height_limit {
        return ChunkData {
            voxels: vec![BlockData {
                block_type: BlockId(0),
            }],
        };
    }
    // hardcoded extremity check
    if chunk_pos.y < -chunk_height_limit {
        return ChunkData {
            voxels: vec![BlockData {
                block_type: BlockId(2),
            }],
        };
    }

    let _span = info_span!("Generating chunk data").entered();

    let chunk_origin = chunk_pos * 32;
    let mut voxels = Vec::with_capacity(CHUNK_SIZE3);

    let mut continental_noise = FastNoise::seeded(37);
    continental_noise.set_frequency(0.0002591);

    let continental_noise_downsampler = NoiseDownSampler::new(2, &continental_noise, chunk_origin.xz(), 55.0);

    let mut errosion = FastNoise::seeded(549);
    errosion.set_frequency(0.004891);

    let errosion_downsampler = NoiseDownSampler::new(1, &errosion, chunk_origin.xz(), 1.0);

    let mut fast_noise = FastNoise::new();
    fast_noise.set_frequency(0.0254);
    for i in 0..CHUNK_SIZE3 {
        let voxel_pos = chunk_origin + index_to_ivec3(i);
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

        let errosion_noise = errosion_downsampler.get_noise(voxel_pos.xz());
        let continental_noise = continental_noise_downsampler.get_noise(voxel_pos.xz());
        
        let surface_height = continental_noise + (noise_2 * 30.0 * (1.0 - errosion_noise));
        let solid = surface_height > voxel_pos.y as f32;

        let block_type = match solid {
            true => match surface_height - voxel_pos.y as f32 { // Distance from surface
                y if y > 3.0 => BlockId(4), // Stone
                y if y > 1.0 => BlockId(1), // Dirt  // TODO: Top soiling by checking if X blocks above are solid.
                _ => BlockId(2), // Grass
            },
            false => {
                if voxel_pos.y < 0 {
                    BlockId(3) // Glass pretending to be water :)
                } else {
                    BlockId(0)
                }
            },
        };
        voxels.push(BlockData { block_type });
    }

    ChunkData { voxels }
}

fn bilinear_interpolation(
    x: f32,
    y: f32,
    q11: f32,
    q12: f32,
    q21: f32,
    q22: f32,
) -> f32 {
    let r1 = (1.0 - x) * q11 + x * q21;
    let r2 = (1.0 - x) * q12 + x * q22;
 
    (1.0 - y) * r1 + y * r2
}

#[test]
fn test_generate() {
    let _ = generate(IVec3::new(0, 0, 0));
}

struct NoiseDownSampler {
    samples: Box<[f32]>,
    upsampling: i32,
    min_point: IVec2,
    edge_length: i32
}
impl NoiseDownSampler {
    pub fn new(upsampling: i32, noise: &FastNoise, chunk_origin: IVec2, scale: f32) -> Self {
        let min_point: IVec2 = chunk_origin >> upsampling;
        let max_point: IVec2 = ((chunk_origin + IVec2::splat(CHUNK_SIZE as i32)) >> upsampling) + 1;

        let edge_length = max_point.x - min_point.x; 
        let mut samples = vec![0.0; (edge_length * edge_length) as usize].into_boxed_slice();

        for sample_point_y in min_point.y..max_point.y {
            for sample_point_x in min_point.x..max_point.x {
                let sample_point = IVec2::new(sample_point_x, sample_point_y);
                let world_point: IVec2 = sample_point << upsampling;

                let index = sample_point - min_point;
                let index = index.x + index.y * edge_length;

                let sample_value = noise.get_noise(
                    world_point.x as f32,
                    world_point.y as f32,
                );

                samples[index as usize] = sample_value * scale;
            }
        }

        Self {
            samples,
            upsampling,
            min_point,
            edge_length
        }
    }

    pub fn get_noise(&self, world_pos: IVec2) -> f32 {
        let world_sample_point = world_pos >> self.upsampling;

        let local_sample_point = world_sample_point - self.min_point;
        let index = local_sample_point.x + local_sample_point.y * self.edge_length;

        let sample_value = self.samples[index as usize];
        let sample_value_x1 = self.samples[(index + 1) as usize];
        let sample_value_y1 = self.samples[(index + self.edge_length) as usize];
        let sample_value_xy1 = self.samples[(index + self.edge_length + 1) as usize];

        let world_sample_point: IVec2 = world_sample_point << self.upsampling;
        let sample_point = (world_pos - world_sample_point).as_vec2() / (1 << self.upsampling) as f32;
        
        bilinear_interpolation(sample_point.x, sample_point.y, sample_value, sample_value_x1, sample_value_y1, sample_value_xy1)
    }
}