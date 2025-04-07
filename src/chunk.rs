use std::sync::Arc;

use bevy::prelude::*;
use bracket_noise::prelude::*;

use crate::{
    constants::CHUNK_SIZE, voxel::BlockData
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

fn bilinear_interpolation(
    alpha: f32,
    beta: f32,
    x00: f32,
    x10: f32,
    x01: f32,
    x11: f32,
) -> f32 {
    (1.0 - alpha) * (1.0 - beta) * x00 +
    alpha * (1.0 - beta) * x10 +
    (1.0 - alpha) * beta * x01 +
    alpha * beta * x11
}

fn trilinear_interpolation(
    alpha: f32,
    beta: f32,
    gamma: f32,
    x000: f32, x100: f32, x010: f32, x110: f32,
    x001: f32, x101: f32, x011: f32, x111: f32,
) -> f32 {
    let c00 = (1.0 - alpha) * x000 + alpha * x100;
    let c01 = (1.0 - alpha) * x001 + alpha * x101;
    let c10 = (1.0 - alpha) * x010 + alpha * x110;
    let c11 = (1.0 - alpha) * x011 + alpha * x111;

    let c0 = (1.0 - beta) * c00 + beta * c10;
    let c1 = (1.0 - beta) * c01 + beta * c11;

    (1.0 - gamma) * c0 + gamma * c1
}

#[test]
fn test_interpolate() {
    let mut continental_noise = FastNoise::seeded(37);
    continental_noise.set_frequency(0.0002591);

    /*let continental_noise_downsampler = NoiseDownSampler2D::new(1, &continental_noise, IVec2::new(0, 0), 55.0);

    let n0 = continental_noise_downsampler.get_noise(IVec2::new(0, 0));
    println!("{n0} - {}", continental_noise.get_noise(0.0, 0.0) * 55.0);
    let n1 = continental_noise_downsampler.get_noise(IVec2::new(1, 0));
    println!("{n1} - {}", continental_noise.get_noise(1.0, 0.0) * 55.0);
    let n2 = continental_noise_downsampler.get_noise(IVec2::new(2, 0));
    println!("{n2} - {}", continental_noise.get_noise(2.0, 0.0) * 55.0);
    let n3 = continental_noise_downsampler.get_noise(IVec2::new(3, 0));
    println!("{n3} - {}", continental_noise.get_noise(3.0, 0.0) * 55.0);*/

    continental_noise.set_frequency(0.0254);
    continental_noise.set_seed(388);
    let continental_noise_downsampler = NoiseDownSampler3D::new(2, &continental_noise, IVec3::ZERO, 55.0, None);

    //let n0 = continental_noise_downsampler.get_noise(IVec3::new(0, 0,0));
    //println!("{n0} - {}", continental_noise.get_noise3d(0.0, 0.0, 0.0) * 55.0);
    
    let n1 = continental_noise_downsampler.get_noise(IVec3::new(0, 1, 0));
    println!("{n1} - {}", continental_noise.get_noise3d(0.0, 1.0, 0.0) * 55.0);
    
    //let n2 = continental_noise_downsampler.get_noise(IVec3::new(2, 0, 0));
    //println!("{n2} - {}", continental_noise.get_noise3d(2.0, 0.0, 0.0) * 55.0);
    
    //let n3 = continental_noise_downsampler.get_noise(IVec3::new(31, 31, 31));
    //println!("{n3} - {} - S{}", continental_noise.get_noise3d(31.0, 31.0, 31.0) * 55.0, continental_noise_downsampler.samples.last().unwrap());
    

}

#[derive(Debug, Clone)]
pub struct NoiseDownSampler2D {
    samples: Box<[f32]>,
    upsampling: i32,
    min_point: IVec2,
    edge_length: i32
}
impl NoiseDownSampler2D {
    pub fn new(upsampling: i32, noise: &FastNoise, chunk_origin: IVec2, scale: f32, buffer: Option<i16>, unitised: bool) -> Self {
        let buffer = buffer.unwrap_or(0) as i32;

        let min_point: IVec2 = (chunk_origin >> upsampling) - buffer;
        let max_point: IVec2 = ((chunk_origin + IVec2::splat(CHUNK_SIZE as i32)) >> upsampling) + 1 + buffer;

        let edge_length = max_point.x - min_point.x; 
        let mut samples = vec![0.0; (edge_length * edge_length) as usize].into_boxed_slice();

        for sample_point_z in min_point.y..max_point.y {
            for sample_point_x in min_point.x..max_point.x {
                let sample_point = IVec2::new(sample_point_x, sample_point_z);
                let world_point: IVec2 = sample_point << upsampling;

                let index = sample_point - min_point;
                let index = index.x + index.y * edge_length;

                let noise_value = noise.get_noise(
                    world_point.x as f32,
                    world_point.y as f32,
                );

                let sample_value = if unitised {
                    noise_value * 0.5 + 0.5
                } else {
                    noise_value
                };

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

        let sample_value_00 = self.samples[index as usize];
        let sample_value_10 = self.samples[(index + 1) as usize];
        let sample_value_01 = self.samples[(index + self.edge_length) as usize];
        let sample_value_11 = self.samples[(index + self.edge_length + 1) as usize];

        let world_sample_point: IVec2 = world_sample_point << self.upsampling;
        let sample_point = (world_pos - world_sample_point).as_vec2() / (1 << self.upsampling) as f32;
        
        bilinear_interpolation(sample_point.x, sample_point.y, sample_value_00, sample_value_10, sample_value_01, sample_value_11)
    }
}

#[derive(Debug, Clone)]
pub struct NoiseDownSampler3D {
    samples: Box<[f32]>,
    upsampling: i32,
    min_point: IVec3,
    edge_length: IVec3
}
impl NoiseDownSampler3D {
    pub fn new(upsampling: i32, noise: &FastNoise, chunk_origin: IVec3, scale: f32, buffer: Option<IVec3>) -> Self {
        let min_point: IVec3 = (chunk_origin - buffer.unwrap_or(IVec3::ZERO)) >> upsampling;
        let max_point: IVec3 = ((chunk_origin + IVec3::splat(CHUNK_SIZE as i32) + buffer.unwrap_or(IVec3::ZERO)) >> upsampling) + 1;

        let edge_length = max_point - min_point;
        let total_size = (edge_length.x * edge_length.y * edge_length.z) as usize;
        let mut samples = vec![0.0; total_size].into_boxed_slice();

        for sample_point_y in min_point.y..max_point.y {
            for sample_point_z in min_point.z..max_point.z {
                for sample_point_x in min_point.x..max_point.x {
                    let sample_point = IVec3::new(sample_point_x, sample_point_y, sample_point_z);
                    let world_point = sample_point << upsampling;

                    let index = (sample_point_x - min_point.x)
                              + (sample_point_z - min_point.z) * edge_length.x
                              + (sample_point_y - min_point.y) * edge_length.x * edge_length.z;

                    let sample_value = noise.get_noise3d(
                        world_point.x as f32,
                        world_point.y as f32,
                        world_point.z as f32,
                    );

                    samples[index as usize] = sample_value * scale;
                }
            }
        }

        Self {
            samples,
            upsampling,
            min_point,
            edge_length,
        }
    }

    pub fn get_noise(&self, world_pos: IVec3) -> f32 {
        let world_sample_point = world_pos >> self.upsampling;
        let local_sample_point = world_sample_point - self.min_point;

        let index = local_sample_point.x + local_sample_point.z * self.edge_length.x + local_sample_point.y * self.edge_length.x * self.edge_length.z;
        let layer_offset = self.edge_length.x * self.edge_length.z;
        
        let sample_value_000 = self.samples[index as usize];
        let sample_value_100 = self.samples[(index + 1) as usize];
        let sample_value_010 = self.samples[(index + self.edge_length.x) as usize];
        let sample_value_110 = self.samples[(index + self.edge_length.x + 1) as usize];
    
        let sample_value_001 = self.samples[(index + layer_offset) as usize];
        let sample_value_101 = self.samples[(index + 1 + layer_offset) as usize];
        let sample_value_011 = self.samples[(index + self.edge_length.x + layer_offset) as usize];
        let sample_value_111 = self.samples[(index + self.edge_length.x + 1 + layer_offset) as usize];

        
        let world_sample_point = world_sample_point << self.upsampling;
        let sample_point = (world_pos - world_sample_point).as_vec3() / (1 << self.upsampling) as f32;
        
        trilinear_interpolation(
            sample_point.x,
            sample_point.z,
            sample_point.y,
            sample_value_000,
            sample_value_100,
            sample_value_010,
            sample_value_110,
            sample_value_001,
            sample_value_101,
            sample_value_011,
            sample_value_111
        )
    }
}