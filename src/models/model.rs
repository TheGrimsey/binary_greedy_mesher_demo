use bevy::{ecs::system::Resource, math::{Vec2, Vec3}, render::render_resource::ShaderType, utils::HashMap};

pub const AO_CORNERS: [[i32; 3]; 8] = [
    [0, 0, 0], // 0
    [1, 0, 0], // 1
    [0, 1, 0], // 2
    [1, 1, 0], // 3
    [0, 0, 1], // 4
    [1, 0, 1], // 5
    [0, 1, 1], // 6
    [1, 1, 1], // 7
];

const REMAP_CORNERS: [[u8; 4]; 6] = [
    [0, 3, 1, 2], // +X
    [0, 1, 3, 2], // -X
    [0, 3, 1, 2], // +Y
    [0, 3, 1, 2], // -Y
    [0, 3, 1, 2], // +Z
    [0, 3, 1, 2], // -Z
];

#[derive(ShaderType, Clone, Debug)]
pub struct ModelQuad {
    pub positions: [Vec3; 4],
    pub uv: [Vec2; 4],
    pub normal: Vec3,

    // 3 bits for which face the quad is on (0-5). 
    // Nearest corner (of the 4 face-corners) to each vertex, used for AO.
    // 2 bits per vertex, 4 vertices.
    // 11 bits total, packed into a u32.
    pub ao: u32,
}
impl ModelQuad {
    pub fn with_calculated_normal(mut self) -> Self {
        let v0 = self.positions[1] - self.positions[0];
        let v1 = self.positions[2] - self.positions[0];

        self.normal = v0.cross(v1).normalize();

        self
    }

    pub fn with_ao_corner(mut self) -> Self {
        let face_normal: u32 = closest_face_direction(self.normal);

        self.ao = face_normal & 0b111; // 3 bits for the face normal

        let (a, b) = FACE_AXES[face_normal as usize];

        for i in 0..4 {
            let pos = self.positions[i];

            let bit_a = if pos[a] >= 0.5 { 1 } else { 0 };
            let bit_b = if pos[b] >= 0.5 { 1 } else { 0 };

            let corner_index = (bit_b << 1) | bit_a; // 2 bits: bit_b = y, bit_a = x

            let remapped_index = REMAP_CORNERS[face_normal as usize][corner_index as usize];

            self.ao |= (remapped_index as u32) << (3 + i * 2); // Offset by 3 bits for face
        }
        

        self
    }
}

const FACE_NORMALS: [Vec3; 6] = [
    Vec3::X,   // +X → 0
    Vec3::NEG_X,  // -X → 1
    Vec3::Y,   // +Y → 2
    Vec3::NEG_Y,  // -Y → 3
    Vec3::Z,   // +Z → 4
    Vec3::NEG_Z,  // -Z → 5
];

const FACE_AXES: [(usize, usize); 6] = [
    (1, 2), // +X → YZ
    (1, 2), // -X → YZ
    (0, 2), // +Y → XZ
    (0, 2), // -Y → XZ
    (0, 1), // +Z → XY
    (0, 1), // -Z → XY
];

fn closest_face_direction(normal: Vec3) -> u32 {
    let mut best_dot = f32::MIN;
    let mut best_index = 0;

    for (i, &face_normal) in FACE_NORMALS.iter().enumerate() {
        let dot = normal.dot(face_normal);
        if dot > best_dot {
            best_dot = dot;
            best_index = i;
        }
    }

    best_index as u32 // This will be 0–5
}


#[test]
fn test_ao_corners() {
    use bevy::math::Vec3Swizzles;

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

    println!("{:b}", model.ao);
    assert_eq!(model.ao & 0b111, 1); // Face normal is -X, so first 3 bits are 001
    
    println!("0: {} ({:b})", model.positions[0].yz(), model.ao >> 3 & 0b11);
    println!("1: {} ({:b})", model.positions[1].yz(), model.ao >> 5 & 0b11);
    println!("2: {} ({:b})", model.positions[2].yz(), model.ao >> 7 & 0b11);
    println!("3: {} ({:b})", model.positions[3].yz(), model.ao >> 9 & 0b11);

    assert_eq!(model.ao >> 3 & 0b11, 2);
    assert_eq!(model.ao >> 5 & 0b11, 3);
    assert_eq!(model.ao >> 7 & 0b11, 1);
    assert_eq!(model.ao >> 9 & 0b11, 0);

    /*
    * (0,0) == 0
    * (0,1) == 2
    * (1,0) == 1
    * (1,1) == 3
     */

    
    let model_y = ModelQuad {
        positions: [
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(1.0, 1.0, 0.0),
        ],
        uv: [
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 1.0),
        ],
        normal: Vec3::Y,
        ao: 0,
    }.with_ao_corner();

    
    println!("{:b}", model_y.ao);
    assert_eq!(model_y.ao & 0b111, 2); // Face normal is +Y, so first 3 bits are 010
    
    println!("0: {} ({:b})", model_y.positions[0].xz(), model_y.ao >> 3 & 0b11);
    println!("1: {} ({:b})", model_y.positions[1].xz(), model_y.ao >> 5 & 0b11);
    println!("2: {} ({:b})", model_y.positions[2].xz(), model_y.ao >> 7 & 0b11);
    println!("3: {} ({:b})", model_y.positions[3].xz(), model_y.ao >> 9 & 0b11);

    assert_eq!(model_y.ao >> 3 & 0b11, 2);
    assert_eq!(model_y.ao >> 5 & 0b11, 3);
    assert_eq!(model_y.ao >> 7 & 0b11, 1);
    assert_eq!(model_y.ao >> 9 & 0b11, 0);

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
#[derive(Debug, Clone)]
pub struct TexturedBlockModel {
    pub model: ModelId,
    pub texture_ids: VoxelTexturingType,
}

#[derive(Debug, Clone)]
pub enum VoxelTexturingType {
    SingleTexture(u32),
    MultiTexture {
        // Texture IDs for each direction + unculled quads.
        all_faces: Box<[Box<[u32]>; 7]>,
    },
}

#[derive(Resource, Default, Debug)]
pub struct ModelRegistry {
    pub models: Vec<BlockModel>,  
}