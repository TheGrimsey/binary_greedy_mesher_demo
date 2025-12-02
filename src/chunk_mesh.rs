use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    math::IVec3,
    mesh::{Indices, Mesh, MeshVertexAttribute, PrimitiveTopology},
    render::{
        render_resource::{ShaderType, VertexFormat},
        storage::ShaderStorageBuffer,
    },
};

use crate::utils::get_pos_from_vertex_u32;

// A "high" random id should be used for custom attributes to ensure consistent sorting and avoid collisions with other attributes.
// See the MeshVertexAttribute docs for more info.
pub const ATTRIBUTE_VOXEL: MeshVertexAttribute =
    MeshVertexAttribute::new("Voxel", 988540919, VertexFormat::Uint32);

/// gpu ready mesh payload
#[derive(Default)]
pub struct ChunkMesh {
    pub indices: Vec<u32>,
    pub faces: Vec<Face>,
}
impl ChunkMesh {
    pub fn into_bevy_mesh(self) -> (Mesh, ShaderStorageBuffer, Aabb) {
        let aabb = self.calculate_aabb();

        let mut bevy_mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        );

        bevy_mesh.insert_indices(Indices::U32(self.indices));

        // Hack becasue bevy doesn't support having no vertex data :( Will panic trying to do a div by zero otherwise
        bevy_mesh.insert_attribute(
            ATTRIBUTE_VOXEL,
            std::iter::repeat_n(0, self.faces.len() * 4).collect::<Vec<u32>>(),
        );

        let mut faces_buffer = ShaderStorageBuffer::from(self.faces);
        faces_buffer.asset_usage = RenderAssetUsages::RENDER_WORLD;

        (bevy_mesh, faces_buffer, aabb)
    }

    pub fn calculate_aabb(&self) -> Aabb {
        // Calculate the AABB for the chunk (purely for minorly improved culling, might not be necessary)
        let (min, max) = self
            .faces
            .iter()
            .fold((IVec3::MAX, IVec3::MIN), |(min, max), face| {
                let pos = get_pos_from_vertex_u32(face.pos_ao);

                (min.min(pos), max.max(pos))
            });

        Aabb::from_min_max(min.as_vec3(), (max + IVec3::ONE).as_vec3())
    }
}

#[derive(ShaderType, Debug, Clone, Copy)]
pub struct Face {
    /// Voxel position in the chunk (x, y, z) 3 * 5 bits
    /// AO per voxel corner 8 * 2 bits
    pub pos_ao: u32,

    pub model_id: u32,
    pub texture_id: u32,
}
