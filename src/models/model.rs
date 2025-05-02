use bevy::{ecs::system::Resource, math::{Vec2, Vec3}, render::render_resource::ShaderType, utils::HashMap};

#[derive(ShaderType, Clone)]
pub struct ModelQuad {
    pub positions: [Vec3; 4],
    pub uv: [Vec2; 4],
    pub normal: Vec3,

    // AO corner (of the 8 corners) for each vertex.
    // 3 bits per vertex, 4 vertices.
    // 12 bits total, packed into a u32.
    pub ao: u32,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Direction {
    PosX, NegX, PosY, NegY, PosZ, NegZ,
}
pub const DIRECTIONS : [Direction; 6] = [
    Direction::PosX,
    Direction::NegX,
    Direction::PosY,
    Direction::NegY,
    Direction::PosZ,
    Direction::NegZ,
];

pub struct BlockModel {
    pub unculled_quads: Vec<ModelQuad>,

    pub quads: HashMap<Direction, Vec<ModelQuad>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelId(pub u32);

// Model attached to a block.
struct BlockTextureModel {
    model: ModelId,
    texture_ids: Box<[u32]>,
}

#[derive(Resource, Default)]
pub struct ModelRegistry {
    pub models: Vec<BlockModel>,  
}