use std::sync::Arc;

use bevy::{ecs::system::Resource, utils::HashMap};

use crate::models::model::{ModelId, TexturedBlockModel, VoxelTexturingType};

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

bitflags::bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct BlockFlags: u8 {
        /// This is a solid block which appears in the mesh.
        const SOLID = 1 << 0;
        /// The is a transparent block which should appear in the transparent mesh.
        const TRANSPARENT = 1 << 1;
        /// The block has collision and should affect the collision mesh.
        const COLLISION = 1 << 2;
    }
}

#[derive(Default, Debug)]
pub struct BlockRegistry {
    pub block_string_identifier_to_id: HashMap<BlockStringIdentifier, BlockId>,

    /// Maps block id to block string identifier.
    pub block_id_to_string_identifier: Vec<BlockStringIdentifier>,
    /// Maps block id to block flags.
    pub block_flags: Vec<BlockFlags>,

    pub block_model: Vec<TexturedBlockModel>,
}
impl BlockRegistry {
    #[inline]
    pub fn is_solid(&self, block_id: BlockId) -> bool {
        self.block_flags[block_id.0 as usize].contains(BlockFlags::SOLID)
    }
    #[inline]
    pub fn has_flag(&self, block_id: BlockId, flag: BlockFlags) -> bool {
        self.block_flags[block_id.0 as usize].contains(flag)
    }

    pub fn add_block(
        &mut self,
        identifier: BlockStringIdentifier,
        block: Block,
    ) -> BlockId{
        let mut flags = match block.visibility {
            BlockVisibilty::Solid => BlockFlags::SOLID,
            BlockVisibilty::Transparent => BlockFlags::TRANSPARENT,
            BlockVisibilty::Invisible => BlockFlags::empty(),
        };
        if block.collision {
            flags |= BlockFlags::COLLISION;
        }

        let block_id = BlockId(self.block_id_to_string_identifier.len() as u16);
        
        self.block_id_to_string_identifier.push(identifier.clone());
        self.block_flags.push(flags);
        self.block_model.push(block.model);

        self.block_string_identifier_to_id.insert(identifier, block_id);

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
    Invisible
}

pub struct Block {
    pub visibility: BlockVisibilty,
    pub collision: bool,
    pub model: TexturedBlockModel,
}
impl Default for Block {
    fn default() -> Self {
        Self {
            visibility: BlockVisibilty::Solid,
            collision: true,
            model: TexturedBlockModel {
                model: ModelId(0),
                texture_ids: VoxelTexturingType::SingleTexture(0),
            },
        }
    }
}
