use bevy::{
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::{
        mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef},
        render_resource::{
            AsBindGroup, PolygonMode, RenderPipelineDescriptor, ShaderRef,
            SpecializedMeshPipelineError, VertexFormat,
        }, storage::ShaderStorageBuffer,
    },
};

use crate::voxel::BlockRegistryResource;

#[derive(Resource)]
pub enum ChunkMaterialWireframeMode {
    On,
    Off,
}

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ChunkMaterial>::default());
        app.add_plugins(MaterialPlugin::<ChunkMaterialWireframe>::default());
        app.insert_resource(ChunkMaterialWireframeMode::Off);

        app.add_systems(Startup, initialize_global_chunk_materials);
        app.add_systems(Update, apply_chunk_material);
    }
}

fn initialize_global_chunk_materials(
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
    mut commands: Commands,
    block_registry: Res<BlockRegistryResource>,
) {
    let colors = block_registry.0.block_color.iter().map(|color| color.to_srgba().to_f32_array()).collect::<Vec<_>>();

    let colors = buffers.add(ShaderStorageBuffer::from(colors));

    // TODO: Add transparent material.
    
    commands.insert_resource(GlobalChunkMaterial {
        opaque: chunk_materials.add(ChunkMaterial {
            reflectance: 0.5,
            perceptual_roughness: 1.0,
            metallic: 0.01,
            block_colors: colors.clone(),
            alpha_mode: AlphaMode::Opaque
        }), 
        transparent: chunk_materials.add(ChunkMaterial {
            reflectance: 0.5,
            perceptual_roughness: 1.0,
            metallic: 0.01,
            block_colors: colors.clone(),
            alpha_mode: AlphaMode::Premultiplied
        }),   
    });
}

fn apply_chunk_material(
    no_wireframe: Query<Entity, With<MeshMaterial3d<ChunkMaterial>>>,
    wireframe: Query<(Entity, &ChunkEntityType), With<MeshMaterial3d<ChunkMaterialWireframe>>>,
    input: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<ChunkMaterialWireframeMode>,
    mut commands: Commands,
    chunk_mat: Res<GlobalChunkMaterial>,
    chunk_mat_wireframe: Res<GlobalChunkWireframeMaterial>,
) {
    if !input.just_pressed(KeyCode::KeyT) {
        return;
    }
    use ChunkMaterialWireframeMode as F;
    *mode = match *mode {
        F::On => F::Off,
        F::Off => F::On,
    };
    match *mode {
        F::On => {
            for entity in no_wireframe.iter() {
                commands
                    .entity(entity)
                    .insert(MeshMaterial3d(chunk_mat_wireframe.0.clone()))
                    .remove::<MeshMaterial3d<ChunkMaterial>>();
            }
        }
        F::Off => {
            for (entity, chunk_type) in wireframe.iter() {
                commands
                    .entity(entity)
                    .insert(MeshMaterial3d(match chunk_type {
                        ChunkEntityType::Opaque => chunk_mat.opaque.clone(),
                        ChunkEntityType::Transparent => chunk_mat.transparent.clone(),
                    }))
                    .remove::<MeshMaterial3d<ChunkMaterialWireframe>>();
            }
        }
    }
}

#[derive(Resource, Reflect)]
pub struct GlobalChunkMaterial {
    pub opaque: Handle<ChunkMaterial>,
    pub transparent: Handle<ChunkMaterial>,
}
#[derive(Resource, Reflect)]
pub struct GlobalChunkWireframeMaterial(pub Handle<ChunkMaterialWireframe>);

// A "high" random id should be used for custom attributes to ensure consistent sorting and avoid collisions with other attributes.
// See the MeshVertexAttribute docs for more info.
pub const ATTRIBUTE_VOXEL: MeshVertexAttribute =
    MeshVertexAttribute::new("Voxel", 988540919, VertexFormat::Uint32);

#[derive(Component)]
pub enum ChunkEntityType {
    Opaque,
    Transparent,
}

// This is the struct that will be passed to your shader
#[derive(Asset, Reflect, AsBindGroup, Debug, Clone)]
pub struct ChunkMaterial {
    #[uniform(0)]
    pub reflectance: f32,
    #[uniform(0)]
    pub perceptual_roughness: f32,
    #[uniform(0)]
    pub metallic: f32,

    #[storage(1,read_only)]
    pub block_colors: Handle<ShaderStorageBuffer>,

    pub alpha_mode: AlphaMode,
}

impl Material for ChunkMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/chunk.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/chunk.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[ATTRIBUTE_VOXEL.at_shader_location(0)])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/chunk_prepass.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/chunk_prepass.wgsl".into()
    }
}
// copy of chunk material pipeline but with wireframe
#[derive(Asset, Reflect, AsBindGroup, Debug, Clone)]
pub struct ChunkMaterialWireframe {
    #[uniform(0)]
    pub reflectance: f32,
    #[uniform(0)]
    pub perceptual_roughness: f32,
    #[uniform(0)]
    pub metallic: f32,
}

impl Material for ChunkMaterialWireframe {
    fn vertex_shader() -> ShaderRef {
        "shaders/chunk.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/chunk.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn specialize(
        _pipeline: &MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex_layout = layout.0.get_layout(&[ATTRIBUTE_VOXEL.at_shader_location(0)])?;
        descriptor.primitive.polygon_mode = PolygonMode::Line;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }

    fn prepass_vertex_shader() -> ShaderRef {
        "shaders/chunk_prepass.wgsl".into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "shaders/chunk_prepass.wgsl".into()
    }
}
