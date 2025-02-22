use bevy::{asset::RenderAssetUsages, math::IVec3, render::{mesh::{Indices, Mesh, PrimitiveTopology}, primitives::Aabb}};

use crate::{rendering::ATTRIBUTE_VOXEL, utils::get_pos_from_vertex_u32};

/// gpu ready mesh payload
#[derive(Default)]
pub struct ChunkMesh {
    pub indices: Vec<u32>,
    pub vertices: Vec<u32>,
}
impl ChunkMesh {
    pub fn to_bevy_mesh(self) -> Mesh {
        let mut bevy_mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );
        
        bevy_mesh.insert_attribute(ATTRIBUTE_VOXEL, self.vertices);
        bevy_mesh.insert_indices(Indices::U32(self.indices));

        bevy_mesh
    }

    pub fn calculate_aabb(&self) -> Aabb {
        // Calculate the AABB for the chunk (purely for minorly improved culling, might not be necessary)
        let (min, max) = self.vertices.iter().fold((IVec3::MAX, IVec3::MIN), |(min, max), v| {
            let pos = get_pos_from_vertex_u32(*v);

            (min.min(pos), max.max(pos))
        });

        Aabb::from_min_max(min.as_vec3(), max.as_vec3())
    }
}
