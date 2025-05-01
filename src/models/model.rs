use bevy::{ecs::system::Resource, math::{Vec2, Vec3}, render::render_resource::ShaderType};

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

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Direction {
    PosX, NegX, PosY, NegY, PosZ, NegZ,
}

pub struct ModelQuadWithCull {
    pub quad: ModelQuad,
    pub cull_face: Direction,
}

struct BlockModel {
    pub unculled_quads: Vec<ModelQuad>,

    pub quads: Vec<ModelQuadWithCull>,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelId(pub u32);

// Model attached to a block.
struct BlockTextureModel {
    model: ModelId,
    texture_ids: Box<[u32]>,
}

#[derive(Resource)]
pub struct ModelRegistry {
    pub models: Vec<BlockModel>,  
}