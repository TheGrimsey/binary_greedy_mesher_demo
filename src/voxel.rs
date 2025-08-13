use bevy::{ecs::resource::Resource, platform::collections::HashMap};

use crate::models::model::{ModelId, TexturedBlockModel, VoxelTexturingType};
use std::sync::Arc;

/// The on disk identifier for a block.
/// Consistent between adding & removing block types.
#[derive(Default, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockStringIdentifier(pub Box<str>);

/// The in memory identifier for a block.
/// Not consistent between adding & removing block types.
///
/// These ids do not have gaps.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(pub u16);

/// All faces of this voxel are solid & aligned with the grid.
/// We don't need to add cullable faces adjacent to this voxel.
pub const FLAG_SOLID: u8 = 1 << 0;
/// This voxel's faces are fully opaque & should be added to the opaque mesh.
pub const FLAG_OPAQUE: u8 = 1 << 1;
/// At least one of this voxel's faces are transparent & should be added to the transparent mesh.
pub const FLAG_TRANSPARENT: u8 = 1 << 2;

#[derive(Default, Debug, Clone)]
pub struct BlockRegistry {
    pub block_string_identifier_to_id: HashMap<BlockStringIdentifier, BlockId>,

    /// Maps block id to block string identifier.
    pub block_id_to_string_identifier: Vec<BlockStringIdentifier>,
    /// Maps block id to block flags.
    pub block_flags: Vec<u8>,

    pub block_model: Vec<TexturedBlockModel>,
}
impl BlockRegistry {
    #[inline]
    pub fn is_solid(&self, block_id: BlockId) -> bool {
        self.block_flags[block_id.0 as usize] & FLAG_SOLID != 0
    }
    #[inline]
    pub fn has_flag(&self, block_id: BlockId, flag: u8) -> bool {
        self.block_flags[block_id.0 as usize] & flag != 0
    }

    pub fn add_block(&mut self, identifier: BlockStringIdentifier, block: Block) -> BlockId {
        let flags = match block.visibility {
            BlockVisibilty::Solid => FLAG_SOLID,
            BlockVisibilty::Transparent => FLAG_TRANSPARENT,
            BlockVisibilty::Invisible => 0,
        } | block.flags;

        let block_id = BlockId(self.block_id_to_string_identifier.len() as u16);

        self.block_id_to_string_identifier.push(identifier.clone());
        self.block_flags.push(flags);
        self.block_model.push(block.model);

        self.block_string_identifier_to_id
            .insert(identifier, block_id);

        block_id
    }
}

#[derive(Debug, Resource)]
pub struct BlockRegistryResource(pub Arc<BlockRegistry>);

#[derive(Default, Copy, Clone, Debug)]
pub struct BlockData {
    pub block_type: BlockId,
}

pub enum BlockVisibilty {
    Solid,
    Transparent,
    Invisible,
}

pub struct Block {
    pub visibility: BlockVisibilty,
    pub model: TexturedBlockModel,
    /// The flags for this block.
    ///
    /// First 2 bits are reserved for solid and transparent.
    pub flags: u8,
}
impl Default for Block {
    fn default() -> Self {
        Self {
            visibility: BlockVisibilty::Solid,
            model: TexturedBlockModel {
                model: ModelId(0),
                texture_ids: VoxelTexturingType::SingleTexture(0),
            },
            flags: 0,
        }
    }
}
