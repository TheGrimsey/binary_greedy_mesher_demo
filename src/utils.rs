use bevy::prelude::*;

#[inline]
pub fn index_to_ivec3(i: i32) -> IVec3 {
    let x = i % 32;
    let y = (i / 32) % 32;
    let z = i / (32 * 32);
    IVec3::new(x, y, z)
}

#[inline]
pub fn index_to_ivec3_bounds(i: i32, bounds: i32) -> IVec3 {
    let x = i % bounds;
    let y = (i / bounds) % bounds;
    let z = i / (bounds * bounds);
    IVec3::new(x, y, z)
}

#[inline]
pub fn index_to_ivec3_bounds_reverse(i: i32, bounds: i32) -> IVec3 {
    let z = i % bounds;
    let y = (i / bounds) % bounds;
    let x = i / (bounds * bounds);
    IVec3::new(x, y, z)
}

#[inline]
pub fn is_on_edge(pos: IVec3) -> bool {
    if pos.x == 0 || pos.x == 32 {
        return true;
    }
    if pos.y == 0 || pos.y == 32 {
        return true;
    }
    if pos.z == 0 || pos.z == 32 {
        return true;
    }
    false
}

/// if lying on the edge of our chunk, return the edging chunk
#[inline]
pub fn get_edging_chunk(pos: IVec3) -> Option<IVec3> {
    let mut chunk_dir = IVec3::ZERO;
    if pos.x == 0 {
        chunk_dir.x = -1;
    } else if pos.x == 31 {
        chunk_dir.x = 1;
    }
    if pos.y == 0 {
        chunk_dir.y = -1;
    } else if pos.y == 31 {
        chunk_dir.y = 1;
    }
    if pos.z == 0 {
        chunk_dir.z = -1;
    } else if pos.z == 31 {
        chunk_dir.z = 1;
    }
    if chunk_dir == IVec3::ZERO {
        None
    } else {
        Some(chunk_dir)
    }
}

/// Vertex format:
/// position: 6 bits each, 18 bits total
/// ao: 3 bits
/// normal: 3 bits (Original comment said 4 but shader only uses 3?)
/// block type: 8 bits (256 block types max :/)
/// total: 32 bits
#[inline]
pub fn make_vertex_u32(
    // position: [i32; 3], /*, normal: i32, color: Color, texture_id: u32*/
    pos: IVec3, /*, normal: i32, color: Color, texture_id: u32*/
    ao: u32,
    normal: u32,
    block_type: u32,
) -> u32 {
    pos.x as u32
        | (pos.y as u32) << 6u32
        | (pos.z as u32) << 12u32
        | ao << 18u32
        | normal << 21u32
        | block_type << 24u32
    // | (normal as u32) << 18u32
    // | (texture_id) << 21u32
}

#[inline]
fn x_positive_bits(bits: u32) -> u32{
    (1 << bits) - 1
}

#[inline]
pub fn get_pos_from_vertex_u32(vertex: u32) -> IVec3 {
    IVec3::new(
        (vertex & x_positive_bits(6)) as i32,
        ((vertex >> 6) & x_positive_bits(6)) as i32,
        ((vertex >> 12) & x_positive_bits(6)) as i32,
    )
}

#[inline]
pub fn world_to_chunk(pos: Vec3) -> IVec3 {
    ((pos - Vec3::splat(16.0)) * (1.0 / 32.0)).as_ivec3()
}

/// generate a vec of indices
/// assumes vertices are made of quads, and counter clockwise ordered
#[inline]
pub fn generate_indices(vertex_count: usize) -> Vec<u32> {
    let indices_count = vertex_count / 4;
    let mut indices = Vec::<u32>::with_capacity(indices_count * 6);
    (0..indices_count).for_each(|vert_index| {
        let vert_index = vert_index as u32 * 4u32;
        indices.push(vert_index);
        indices.push(vert_index + 1);
        indices.push(vert_index + 2);
        indices.push(vert_index);
        indices.push(vert_index + 2);
        indices.push(vert_index + 3);
    });

    indices
}

#[test]
fn index_functions() {
    for z in 0..32 {
        for y in 0..32 {
            for x in 0..32 {
                let pos = IVec3::new(x, y, z);
                let index = vec3_to_index(pos, 32);
                let from_index = index_to_ivec3_bounds(index as i32, 32);
                assert_eq!(pos, from_index);
            }
        }
    }
}

#[inline]
pub fn vec3_to_index(pos: IVec3, bounds: i32) -> usize {
    let x_i = pos.x % bounds;
    let y_i = pos.y * bounds;
    let z_i = pos.z * (bounds * bounds);
    (x_i + y_i + z_i) as usize
}
