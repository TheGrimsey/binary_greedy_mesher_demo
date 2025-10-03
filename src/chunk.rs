use std::sync::Arc;

use bevy::prelude::*;
use bracket_noise::prelude::*;

use crate::{
    constants::{CHUNK_SIZE, CHUNK_SIZE3},
    voxel::BlockId,
};

#[derive(Resource)]
pub struct ChunkGenerator {
    pub generate: Arc<dyn Fn(IVec3) -> ChunkData + Send + Sync>,
}

pub const HIGH_NIBBLE: u8 = 0xF0;
pub const LOW_NIBBLE: u8 = 0x0F;

#[derive(Clone)]
pub struct ChunkData {
    pub palette: Vec<BlockId>,
    pub voxels: Box<[u8]>,
    pub index_size: IndexSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexSize {
    Nibble, // 4 bits
    Byte,   // 8 bits
    Short,  // 16 bits
}
impl IndexSize {
    pub const fn max_palette_size(&self) -> usize {
        match self {
            IndexSize::Nibble => 16,
            IndexSize::Byte => u8::MAX as usize + 1,
            IndexSize::Short => u16::MAX as usize + 1,
        }
    }

    pub const fn palette_to_index_size(palette_size: usize) -> Option<IndexSize> {
        if palette_size <= IndexSize::Nibble.max_palette_size() {
            Some(IndexSize::Nibble)
        } else if palette_size <= IndexSize::Byte.max_palette_size() {
            Some(IndexSize::Byte)
        } else if palette_size <= IndexSize::Short.max_palette_size() {
            Some(IndexSize::Short)
        } else {
            None
        }
    }

    pub const fn next_larger(&self) -> Option<IndexSize> {
        match self {
            IndexSize::Nibble => Some(IndexSize::Byte),
            IndexSize::Byte => Some(IndexSize::Short),
            IndexSize::Short => None,
        }
    }

    pub const fn chunk_size_in_bytes(&self) -> usize {
        match self {
            IndexSize::Nibble => CHUNK_SIZE3 / 2,
            IndexSize::Byte => CHUNK_SIZE3,
            IndexSize::Short => CHUNK_SIZE3 * 2,
        }
    }
}

impl ChunkData {
    #[inline]
    pub fn get_block(&self, index: usize) -> BlockId {
        match self.index_size {
            IndexSize::Nibble => {
                if self.voxels.len() == 1 {
                    return self.palette[self.voxels[0] as usize].clone();
                }

                let byte = self.voxels[index / 2];
                let nibble = if index % 2 == 0 {
                    byte & LOW_NIBBLE
                } else {
                    (byte >> 4) & LOW_NIBBLE
                };
                self.palette[nibble as usize].clone()
            }
            IndexSize::Byte => {
                self.palette[self
                    .voxels
                    .get(index)
                    .or(self.voxels.first())
                    .cloned()
                    .unwrap() as usize]
            }
            IndexSize::Short => {
                let bytes = if self.voxels.len() == 2 {
                    u16::from_ne_bytes([self.voxels[0], self.voxels[1]])
                } else {
                    let byte_index = index * 2;

                    u16::from_ne_bytes([self.voxels[byte_index], self.voxels[byte_index + 1]])
                };

                self.palette[bytes as usize]
            }
        }
    }

    /// returns the block type if all voxels are the same
    #[inline]
    pub fn get_block_if_filled(&self) -> Option<BlockId> {
        match self.index_size {
            IndexSize::Nibble => {
                if self.voxels.len() == 1 {
                    return Some(self.palette[(self.voxels[0] & LOW_NIBBLE) as usize].clone());
                }
            }
            IndexSize::Byte => {
                if self.voxels.len() == 1 {
                    return Some(self.palette[self.voxels[0] as usize].clone());
                }
            }
            IndexSize::Short => {
                if self.voxels.len() == 2 {
                    let bytes = u16::from_ne_bytes([self.voxels[0], self.voxels[1]]);
                    return Some(self.palette[bytes as usize].clone());
                }
            }
        }

        None
    }

    pub fn add_to_palette(&mut self, block_id: BlockId) -> usize {
        if let Some(palette_i) = self.palette.iter().position(|&id| id == block_id) {
            palette_i
        } else {
            if self.palette.len() >= self.index_size.max_palette_size() {
                if let Some(next_palette) = self.index_size.next_larger() {
                    self.resize_to_fit_index(next_palette);
                } else {
                    panic!("Palette is full and cannot be resized further");
                }
            }

            self.palette.push(block_id);
            self.palette.len() - 1
        }
    }

    pub fn set_voxel_by_palette(&mut self, index: usize, palette_index: usize) {
        match self.index_size {
            IndexSize::Nibble => {
                let byte_index = index / 2;
                let is_low_nibble = index % 2 == 0;

                if is_low_nibble {
                    self.voxels[byte_index] = (self.voxels[byte_index] & HIGH_NIBBLE)
                        | (palette_index as u8 & LOW_NIBBLE);
                } else {
                    self.voxels[byte_index] = (self.voxels[byte_index] & LOW_NIBBLE)
                        | ((palette_index as u8 & LOW_NIBBLE) << 4);
                }
            }
            IndexSize::Byte => {
                self.voxels[index] = palette_index as u8;
            }
            IndexSize::Short => {
                let byte_index = index * 2;
                let bytes = (palette_index as u16).to_ne_bytes();
                self.voxels[byte_index] = bytes[0];
                self.voxels[byte_index + 1] = bytes[1];
            }
        }
    }

    pub fn set_voxel(&mut self, index: usize, block_id: BlockId) {
        let palette_index = self.add_to_palette(block_id);

        self.set_voxel_by_palette(index, palette_index);
    }

    pub fn resize_to_fit_index(&mut self, index_size: IndexSize) {
        if self.index_size == index_size {
            return;
        }

        let mut new_voxels = vec![0; index_size.chunk_size_in_bytes()].into_boxed_slice();

        for i in 0..CHUNK_SIZE3 {
            let palette_index = match self.index_size {
                IndexSize::Nibble => {
                    (if self.voxels.len() == 1 {
                        self.voxels[0]
                    } else {
                        let byte = self.voxels[i / 2];
                        if i % 2 == 0 {
                            byte & LOW_NIBBLE
                        } else {
                            (byte >> 4) & LOW_NIBBLE
                        }
                    }) as usize
                }
                IndexSize::Byte => {
                    self.voxels.get(i).or(self.voxels.first()).cloned().unwrap() as usize
                }
                IndexSize::Short => {
                    if self.voxels.len() == 2 {
                        u16::from_ne_bytes([self.voxels[0], self.voxels[1]]) as usize
                    } else {
                        let byte_index = i * 2;
                        u16::from_ne_bytes([self.voxels[byte_index], self.voxels[byte_index + 1]])
                            as usize
                    }
                }
            };

            match index_size {
                IndexSize::Nibble => {
                    let byte_index = i / 2;
                    let is_low_nibble = i % 2 == 0;

                    if is_low_nibble {
                        new_voxels[byte_index] = (new_voxels[byte_index] & HIGH_NIBBLE)
                            | (palette_index as u8 & LOW_NIBBLE);
                    } else {
                        new_voxels[byte_index] = (new_voxels[byte_index] & LOW_NIBBLE)
                            | ((palette_index as u8 & LOW_NIBBLE) << 4);
                    }
                }
                IndexSize::Byte => {
                    new_voxels[i] = palette_index as u8;
                }
                IndexSize::Short => {
                    let byte_index = i * 2;
                    let bytes = (palette_index as u16).to_ne_bytes();
                    new_voxels[byte_index] = bytes[0];
                    new_voxels[byte_index + 1] = bytes[1];
                }
            }
        }

        self.voxels = new_voxels;
        self.index_size = index_size;
    }

    pub fn expand_if_necessary(&mut self) {
        match self.index_size {
            IndexSize::Nibble => {
                if self.voxels.len() == 1 {
                    let block = self.voxels[0] & LOW_NIBBLE;

                    let combined_block = (block << 4) | block;
                    self.voxels =
                        vec![combined_block; self.index_size.chunk_size_in_bytes()].into();
                }
            }
            IndexSize::Byte => {
                if self.voxels.len() == 1 {
                    let block = self.voxels[0];
                    self.voxels = vec![block; self.index_size.chunk_size_in_bytes()].into();
                }
            }
            IndexSize::Short => {
                if self.voxels.len() == 2 {
                    let bytes = [self.voxels[0], self.voxels[1]];
                    let block = u16::from_ne_bytes(bytes);
                    let block_bytes = block.to_ne_bytes();

                    self.voxels =
                        std::iter::repeat_n(block_bytes, self.index_size.chunk_size_in_bytes() / 2)
                            .flatten()
                            .collect();
                }
            }
        }
    }
    pub fn compress_if_possible(&mut self) {
        let mut new_palette = Vec::new();
        let mut index_map = vec![0; self.palette.len()];

        // Find all blocks used in voxels and create a new palette.
        match self.index_size {
            IndexSize::Nibble => {
                for byte in &self.voxels {
                    let low_nibble = byte & LOW_NIBBLE;
                    let high_nibble = (byte >> 4) & LOW_NIBBLE;

                    for &nibble in &[low_nibble, high_nibble] {
                        let idx = nibble as usize;
                        if !new_palette.contains(&self.palette[idx]) {
                            index_map[idx] = new_palette.len();
                            new_palette.push(self.palette[idx]);
                        }
                    }
                }
            }
            IndexSize::Byte => {
                for &byte in &self.voxels {
                    let idx = byte as usize;
                    if !new_palette.contains(&self.palette[idx]) {
                        index_map[idx] = new_palette.len();
                        new_palette.push(self.palette[idx]);
                    }
                }
            }
            IndexSize::Short => {
                for chunk in self.voxels.chunks_exact(2) {
                    let bytes = u16::from_ne_bytes([chunk[0], chunk[1]]);
                    let idx = bytes as usize;
                    if !new_palette.contains(&self.palette[idx]) {
                        index_map[idx] = new_palette.len();
                        new_palette.push(self.palette[idx]);
                    }
                }
            }
        }

        if new_palette.len() == 1 {
            self.palette = vec![new_palette[0]];
            self.voxels = [0].into();
            self.index_size = IndexSize::Nibble;
            return;
        }

        let new_index_size =
            IndexSize::palette_to_index_size(new_palette.len()).expect("Palette too large");
        if new_index_size == self.index_size {
            return; // No resizing needed
        }

        let mut new_voxels = vec![0; new_index_size.chunk_size_in_bytes()].into_boxed_slice();
        match self.index_size {
            IndexSize::Nibble => {
                // Re-map voxel indices to new palette
                // This is a nibble to nibble remap (guaranteed because we can't be smaller than a nibble).
                for (i, byte) in self.voxels.iter().enumerate() {
                    let low_nibble = byte & LOW_NIBBLE;
                    let high_nibble = (byte >> 4) & LOW_NIBBLE;

                    let new_low = index_map[low_nibble as usize] as u8 & LOW_NIBBLE;
                    let new_high = (index_map[high_nibble as usize] as u8 & LOW_NIBBLE) << 4;

                    new_voxels[i] = new_low | new_high;
                }
            }
            IndexSize::Byte => match new_index_size {
                IndexSize::Nibble => {
                    for (i, bytes) in self.voxels.windows(2).enumerate() {
                        let byte = bytes[0];
                        let low_nibble = byte & LOW_NIBBLE;
                        let high_nibble = (byte >> 4) & LOW_NIBBLE;

                        let new_low = index_map[low_nibble as usize] as u8 & LOW_NIBBLE;
                        let new_high = (index_map[high_nibble as usize] as u8 & LOW_NIBBLE) << 4;

                        new_voxels[i] = new_low | new_high;
                    }
                }
                IndexSize::Byte => {
                    for (i, byte) in self.voxels.iter().enumerate() {
                        let idx = *byte as usize;
                        let new_idx = index_map[idx] as u8;
                        new_voxels[i] = new_idx;
                    }
                }
                IndexSize::Short => {
                    // Can't happen, we're compressing. We can only go smaller...
                    unreachable!()
                }
            },
            IndexSize::Short => {
                // This can go to any size...
                match new_index_size {
                    IndexSize::Nibble => {
                        for (i, bytes) in self.voxels.chunks_exact(4).enumerate() {
                            let a = u16::from_ne_bytes([bytes[0], bytes[1]]);
                            let b = u16::from_ne_bytes([bytes[2], bytes[3]]);

                            let new_a = index_map[a as usize] as u8 & LOW_NIBBLE;
                            let new_b = (index_map[b as usize] as u8 & LOW_NIBBLE) << 4;

                            new_voxels[i] = new_a | new_b;
                        }
                    }
                    IndexSize::Byte => {
                        for (i, bytes) in self.voxels.chunks_exact(2).enumerate() {
                            let bytes = u16::from_ne_bytes([bytes[0], bytes[1]]);
                            let idx = bytes as usize;
                            let new_idx = index_map[idx] as u8;
                            new_voxels[i] = new_idx;
                        }
                    }
                    IndexSize::Short => {
                        for (i, bytes) in self.voxels.chunks_exact(2).enumerate() {
                            let bytes = u16::from_ne_bytes([bytes[0], bytes[1]]);
                            let idx = bytes as usize;
                            let new_idx = index_map[idx] as u16;
                            let new_bytes = new_idx.to_ne_bytes();
                            let byte_index = i * 2;
                            new_voxels[byte_index] = new_bytes[0];
                            new_voxels[byte_index + 1] = new_bytes[1];
                        }
                    }
                }
            }
        }

        self.palette = new_palette;
        self.voxels = new_voxels;
        self.index_size = new_index_size;
    }

    pub fn from_block_ids(blocks: &[BlockId]) -> Self {
        let _span = info_span!("ChunkData::from_block_ids").entered();
        assert!(
            blocks.len() == CHUNK_SIZE3 || blocks.len() == 1,
            "Blocks array must be of length CHUNK_SIZE3 or 1"
        );

        let mut palette = Vec::new();

        // Build palette & select index size
        for block in blocks {
            if !palette.contains(block) {
                palette.push(*block);
            }
        }

        if blocks.len() == 1 || palette.len() == 1 {
            return Self {
                palette: vec![blocks[0]],
                voxels: [0].into(),
                index_size: IndexSize::Nibble,
            };
        }

        let index_size = IndexSize::palette_to_index_size(palette.len())
            .expect("Palette size exceeds maximum allowed size");

        let mut voxels = vec![0; index_size.chunk_size_in_bytes()].into_boxed_slice();

        match index_size {
            IndexSize::Nibble => {
                for (i, block) in blocks.chunks_exact(2).enumerate() {
                    let palette_index_a = palette
                        .iter()
                        .position(|&id| id == block[0])
                        .expect("Block ID not found in palette")
                        as u8;
                    let palette_index_b = palette
                        .iter()
                        .position(|&id| id == block[1])
                        .expect("Block ID not found in palette")
                        as u8;

                    voxels[i] = (palette_index_b << 4) | palette_index_a;
                }
            }
            IndexSize::Byte => {
                for (i, blocks) in blocks.iter().enumerate() {
                    let palette_index = palette
                        .iter()
                        .position(|&id| id == *blocks)
                        .expect("Block ID not found in palette");

                    voxels[i] = palette_index as u8;
                }
            }
            IndexSize::Short => {
                for (i, blocks) in blocks.iter().enumerate() {
                    let palette_index = palette
                        .iter()
                        .position(|&id| id == *blocks)
                        .expect("Block ID not found in palette");

                    let byte_index = i * 2;
                    let bytes = (palette_index as u16).to_ne_bytes();
                    voxels[byte_index] = bytes[0];
                    voxels[byte_index + 1] = bytes[1];
                }
            }
        }

        Self {
            palette,
            voxels,
            index_size,
        }
    }
}

#[test]
fn test_chunk_data_from_blocks() {
    let blocks = vec![BlockId(0); CHUNK_SIZE3];
    let chunk_data = ChunkData::from_block_ids(&blocks);
    assert_eq!(
        chunk_data.palette.len(),
        1,
        "Expected chunks with a single block type to have a palette length of 1."
    );
    assert_eq!(
        chunk_data.voxels.len(),
        1,
        "Expected chunks with a single block type to have a voxel length of 1."
    );
    assert_eq!(
        chunk_data.index_size,
        IndexSize::Nibble,
        "Expected chunks with a single block type to use Nibble index size."
    );

    let blocks = std::iter::repeat_n([BlockId(0), BlockId(1)], CHUNK_SIZE3 / 2)
        .flatten()
        .collect::<Vec<_>>();
    let chunk_data = ChunkData::from_block_ids(&blocks);
    assert_eq!(
        chunk_data.palette.len(),
        2,
        "Expected chunks with two block types to have a palette length of 2."
    );
    assert_eq!(
        chunk_data.index_size,
        IndexSize::Nibble,
        "Expected chunks with two block types to use Nibble index size."
    );
    assert_eq!(
        chunk_data.voxels.len(),
        IndexSize::Nibble.chunk_size_in_bytes(),
    );
}

#[test]
fn test_palette_compress() {
    let blocks = std::iter::repeat_n(
        [0u16.to_ne_bytes(), 1u16.to_ne_bytes()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
        CHUNK_SIZE3 / 2,
    )
    .flatten()
    .collect();

    let mut chunk_data = ChunkData {
        palette: (0..64).map(BlockId).collect::<Vec<_>>(),
        voxels: blocks,
        index_size: IndexSize::Short,
    };

    chunk_data.compress_if_possible();

    assert_eq!(
        chunk_data.palette.len(),
        2,
        "Expected palette to be compressed to length 2."
    );
    assert_eq!(
        chunk_data.index_size,
        IndexSize::Nibble,
        "Expected index size to be Nibble after compression."
    );
    assert_eq!(
        chunk_data.voxels.len(),
        IndexSize::Nibble.chunk_size_in_bytes(),
        "Expected voxel data length to match Nibble index size."
    );
}

fn bilinear_interpolation(alpha: f32, beta: f32, x00: f32, x10: f32, x01: f32, x11: f32) -> f32 {
    (1.0 - alpha) * (1.0 - beta) * x00
        + alpha * (1.0 - beta) * x10
        + (1.0 - alpha) * beta * x01
        + alpha * beta * x11
}

fn trilinear_interpolation(
    alpha: f32,
    beta: f32,
    gamma: f32,
    x000: f32,
    x100: f32,
    x010: f32,
    x110: f32,
    x001: f32,
    x101: f32,
    x011: f32,
    x111: f32,
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

    continental_noise.set_frequency(0.0254);
    continental_noise.set_seed(388);
    let continental_noise_downsampler =
        NoiseDownSampler3D::new(2, &continental_noise, IVec3::ZERO, 55.0, None);

    //let n0 = continental_noise_downsampler.get_noise(IVec3::new(0, 0,0));
    //println!("{n0} - {}", continental_noise.get_noise3d(0.0, 0.0, 0.0) * 55.0);

    let n1 = continental_noise_downsampler.get_noise(IVec3::new(0, 1, 0));
    println!(
        "{n1} - {}",
        continental_noise.get_noise3d(0.0, 1.0, 0.0) * 55.0
    );
}

#[derive(Debug, Clone)]
pub struct NoiseDownSampler2D {
    samples: Box<[f32]>,
    upsampling: i32,
    min_point: IVec2,
    edge_length: i32,
}
impl NoiseDownSampler2D {
    pub fn new(
        upsampling: i32,
        noise: &FastNoise,
        chunk_origin: IVec2,
        scale: f32,
        buffer: Option<i16>,
        unitised: bool,
    ) -> Self {
        let buffer = buffer.unwrap_or(0) as i32;

        let min_point: IVec2 = (chunk_origin >> upsampling) - buffer;
        let max_point: IVec2 =
            ((chunk_origin + IVec2::splat(CHUNK_SIZE as i32)) >> upsampling) + 1 + buffer;

        let edge_length = max_point.x - min_point.x;
        let mut samples = vec![0.0; (edge_length * edge_length) as usize].into_boxed_slice();

        for sample_point_z in min_point.y..max_point.y {
            for sample_point_x in min_point.x..max_point.x {
                let sample_point = IVec2::new(sample_point_x, sample_point_z);
                let world_point: IVec2 = sample_point << upsampling;

                let index = sample_point - min_point;
                let index = index.x + index.y * edge_length;

                let noise_value = noise.get_noise(world_point.x as f32, world_point.y as f32);

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
            edge_length,
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
        let sample_point =
            (world_pos - world_sample_point).as_vec2() / (1 << self.upsampling) as f32;

        bilinear_interpolation(
            sample_point.x,
            sample_point.y,
            sample_value_00,
            sample_value_10,
            sample_value_01,
            sample_value_11,
        )
    }
}

#[derive(Debug, Clone)]
pub struct NoiseDownSampler3D {
    samples: Box<[f32]>,
    upsampling: i32,
    min_point: IVec3,
    edge_length: IVec3,
}
impl NoiseDownSampler3D {
    pub fn new(
        upsampling: i32,
        noise: &FastNoise,
        chunk_origin: IVec3,
        scale: f32,
        buffer: Option<IVec3>,
    ) -> Self {
        let min_point: IVec3 = (chunk_origin - buffer.unwrap_or(IVec3::ZERO)) >> upsampling;
        let max_point: IVec3 =
            ((chunk_origin + IVec3::splat(CHUNK_SIZE as i32) + buffer.unwrap_or(IVec3::ZERO))
                >> upsampling)
                + 1;

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

        let index = local_sample_point.x
            + local_sample_point.z * self.edge_length.x
            + local_sample_point.y * self.edge_length.x * self.edge_length.z;
        let layer_offset = self.edge_length.x * self.edge_length.z;

        let sample_value_000 = self.samples[index as usize];
        let sample_value_100 = self.samples[(index + 1) as usize];
        let sample_value_010 = self.samples[(index + self.edge_length.x) as usize];
        let sample_value_110 = self.samples[(index + self.edge_length.x + 1) as usize];

        let sample_value_001 = self.samples[(index + layer_offset) as usize];
        let sample_value_101 = self.samples[(index + 1 + layer_offset) as usize];
        let sample_value_011 = self.samples[(index + self.edge_length.x + layer_offset) as usize];
        let sample_value_111 =
            self.samples[(index + self.edge_length.x + 1 + layer_offset) as usize];

        let world_sample_point = world_sample_point << self.upsampling;
        let sample_point =
            (world_pos - world_sample_point).as_vec3() / (1 << self.upsampling) as f32;

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
            sample_value_111,
        )
    }
}
