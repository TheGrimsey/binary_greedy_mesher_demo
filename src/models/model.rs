use bevy::{ecs::system::Resource, math::{Vec2, Vec3}, render::render_resource::ShaderType, utils::HashMap};

pub const AO_CORNERS: [[u32; 3]; 8] = [
    [0, 0, 0], // 0
    [1, 0, 0], // 1
    [0, 1, 0], // 2
    [1, 1, 0], // 3
    [0, 0, 1], // 4
    [1, 0, 1], // 5
    [0, 1, 1], // 6
    [1, 1, 1], // 7
];

#[derive(ShaderType, Clone, Debug)]
pub struct ModelQuad {
    pub positions: [Vec3; 4],
    pub uv: [Vec2; 4],
    pub normal: Vec3,

    // Nearest corner (of the 8 corners) to each vertex, used for AO.
    // 3 bits per vertex, 4 vertices.
    // 12 bits total, packed into a u32.
    pub ao: u32,
}
impl ModelQuad {
    pub fn with_ao_corner(mut self) -> Self {
        for i in 0..4 {
            let mut corner: u32 = 0;
            for j in 0..3 {
                if self.positions[i][j] > 0.5 {
                    corner |= 1 << j;
                }
            }
            self.ao |= corner << (i * 3);
        }

        self
    }
}

#[test]
fn test_ao_corners() {
    let model = ModelQuad {
        positions: [
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 1.0, 1.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
        ],
        uv: [
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 1.0),
        ],
        normal: Vec3::NEG_X,
        ao: 0,
    }.with_ao_corner();

    assert_eq!(model.ao & 0b111, 4);
    assert_eq!(model.ao >> 3 & 0b111, 6);
    assert_eq!(model.ao >> 6 & 0b111, 2);
    assert_eq!(model.ao >> 9 & 0b111, 0);
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
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

#[derive(Debug)]
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

#[derive(Resource, Default, Debug)]
pub struct ModelRegistry {
    pub models: Vec<BlockModel>,  
}