use std::{num::NonZero, sync::Arc};

use bevy::{
    asset::{RenderAssetUsages, load_internal_asset, weak_handle},
    ecs::system::{SystemParamItem, lifetimeless::SRes},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    platform::collections::HashMap,
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_asset::RenderAssets,
        render_resource::{
            AsBindGroup, AsBindGroupError, BindGroupEntries, BindGroupLayout,
            BindGroupLayoutEntries, BindGroupLayoutEntry, BindingResources, BufferInitDescriptor,
            BufferUsages, PolygonMode, PreparedBindGroup, RenderPipelineDescriptor,
            SamplerBindingType, ShaderRef, ShaderStages, ShaderType, SpecializedMeshPipelineError,
            TextureSampleType, UnpreparedBindGroup,
            binding_types::{sampler, storage_buffer_read_only_sized, texture_2d, uniform_buffer},
            encase::UniformBuffer,
        },
        renderer::RenderDevice,
        storage::{GpuShaderStorageBuffer, ShaderStorageBuffer},
        texture::{FallbackImage, GpuImage},
    },
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use indexmap::IndexSet;

use crate::{
    chunk_mesh::{ATTRIBUTE_VOXEL, ChunkMesh},
    chunks_refs::ChunksRefs,
    constants::ADJACENT_CHUNK_DIRECTIONS,
    events::ChunkModified,
    models::{
        IndexedModel, IndexedModelRegistry, IndexedModelRegistryResource, QuadRange,
        model::{DIRECTIONS, ModelRegistry},
    },
    scanner::{
        ChunkGainedScannerRelevance, ChunkLostScannerRelevance, ChunkPos,
        GlobalScannerDesiredChunks, MeshScanner, Scanner,
    },
    voxel::{BlockRegistryResource, FLAG_SOLID, FLAG_TRANSPARENT},
    voxel_engine::{MeshingMethod, VoxelEngine, join_data},
};

pub const CHUNK_SHADER_HANDLE: Handle<Shader> =
    weak_handle!("f4cc2d00-78bd-4d79-a803-3f5147cb6606");
pub const CHUNK_PREPASS_HANDLE: Handle<Shader> =
    weak_handle!("97a77bda-9a7c-4a3b-8800-15a5f5198777");

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

        app.init_resource::<MeshingPipeline>()
            .init_resource::<ChunkMeshEntities>();

        app.add_systems(PostStartup, initialize_global_material_buffers);

        load_internal_asset!(app, CHUNK_SHADER_HANDLE, "chunk.wgsl", Shader::from_wgsl);

        load_internal_asset!(
            app,
            CHUNK_PREPASS_HANDLE,
            "chunk_prepass.wgsl",
            Shader::from_wgsl
        );

        app.add_systems(
            PostUpdate,
            (join_mesh, unload_mesh, start_mesh_tasks.after(join_data)).chain(),
        );
    }
}

/// All the textures used by blocks in the world.
/// Must be initialized before any chunks are loaded.
#[derive(Resource, Default)]
pub struct TextureBuffer(pub Arc<[Handle<Image>]>);

#[derive(Resource)]
pub struct SharedMaterialBuffers {
    pub model_buffer: Handle<ShaderStorageBuffer>,
}

fn initialize_global_material_buffers(
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut commands: Commands,
    mut model_registry: ResMut<ModelRegistry>,
) {
    // TODO: Create IndexedModelRegistryResource and add it to the shader storage buffer.
    // 1. Iterate over each model in the registry.
    // 2. Add every quad to a temporary Vec<>, save the start and end indexes of each quad range.

    let mut indexed_models = vec![];

    let mut model_quads = vec![];
    for model in model_registry.models.iter_mut() {
        let start_index = model_quads.len() as u32;
        model_quads.extend(model.unculled_quads.iter().cloned());
        let end_index = model_quads.len() as u32;

        let unculled_ao_directions: Box<[u8]> = model
            .unculled_quads
            .iter()
            .map(|quad| (quad.ao & 0b111) as u8)
            .collect();

        let mut indexed_model = IndexedModel {
            always_required_face_directions: unculled_ao_directions
                .iter()
                .fold(0, |acc, &dir| acc | (1 << dir)),
            always_visible_faces: QuadRange {
                start: start_index,
                end: end_index,
                ao_direction: unculled_ao_directions,
            },
            occluded_faces: std::array::from_fn(|_i| QuadRange {
                start: 0,
                end: 0,
                ao_direction: Box::default(),
            }),
        };

        for (quad_range, direction) in indexed_model.occluded_faces.iter_mut().zip(DIRECTIONS) {
            if let Some(quads) = model.quads.get(&direction) {
                quad_range.start = model_quads.len() as u32;
                model_quads.extend(quads.iter().cloned());
                quad_range.end = model_quads.len() as u32;

                quad_range.ao_direction =
                    quads.iter().map(|quad| (quad.ao & 0b111) as u8).collect();
            }
        }

        indexed_models.push(indexed_model);
    }

    commands.insert_resource(IndexedModelRegistryResource(Arc::new(
        IndexedModelRegistry {
            models: indexed_models,
        },
    )));

    let mut model_buffer = ShaderStorageBuffer::from(model_quads);
    model_buffer.asset_usage = RenderAssetUsages::RENDER_WORLD;

    let model_buffer = buffers.add(model_buffer);

    commands.insert_resource(SharedMaterialBuffers { model_buffer });
}

#[derive(Component)]
pub enum ChunkEntityType {
    Opaque,
    Transparent,
}

const MAX_TEXTURE_COUNT: usize = 128; // There's no true texture arrays :( WebGPU!!!! Very annoying :(
// Mac only supports 128.

#[derive(Reflect, ShaderType, Debug, Clone, Copy)]
pub struct MaterialProperties {
    reflectance: Vec3,
    perceptual_roughness: f32,
    metallic: f32,
}

#[derive(Asset, Reflect, Debug, Clone)]
pub struct ChunkMaterial {
    pub properties: MaterialProperties,

    pub model_buffer: Handle<ShaderStorageBuffer>,

    pub face_buffer: Handle<ShaderStorageBuffer>,

    pub textures: Arc<[Handle<Image>]>,

    pub alpha_mode: AlphaMode,
}

impl Material for ChunkMaterial {
    fn vertex_shader() -> ShaderRef {
        CHUNK_SHADER_HANDLE.into()
    }
    fn fragment_shader() -> ShaderRef {
        CHUNK_SHADER_HANDLE.into()
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
        let vertex_layout = layout
            .0
            .get_layout(&[ATTRIBUTE_VOXEL.at_shader_location(0)])?;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }

    fn prepass_vertex_shader() -> ShaderRef {
        CHUNK_PREPASS_HANDLE.into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        CHUNK_PREPASS_HANDLE.into()
    }
}
impl AsBindGroup for ChunkMaterial {
    type Data = ();

    type Param = (
        SRes<RenderAssets<GpuShaderStorageBuffer>>,
        SRes<RenderAssets<GpuImage>>,
        SRes<FallbackImage>,
    );

    fn as_bind_group(
        &self,
        layout: &BindGroupLayout,
        render_device: &RenderDevice,
        (storage_buffers, image_assets, fallback_image): &mut SystemParamItem<'_, '_, Self::Param>,
    ) -> Result<PreparedBindGroup<Self::Data>, AsBindGroupError> {
        // retrieve the render resources from handles

        let mut properties_buffer = UniformBuffer::new(Vec::new());
        properties_buffer.write(&self.properties).unwrap();

        let properties_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: None,
            contents: properties_buffer.as_ref(),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let Some(model_buffer) = storage_buffers.get(&self.model_buffer) else {
            return Err(AsBindGroupError::RetryNextUpdate);
        };

        let model_buffer = model_buffer.buffer.as_entire_buffer_binding();

        let Some(face_buffer) = storage_buffers.get(&self.face_buffer) else {
            return Err(AsBindGroupError::RetryNextUpdate);
        };
        let face_buffer = face_buffer.buffer.as_entire_buffer_binding();

        let mut images = vec![];
        for handle in self.textures.iter().take(MAX_TEXTURE_COUNT) {
            match image_assets.get(handle) {
                Some(image) => images.push(image),
                None => return Err(AsBindGroupError::RetryNextUpdate),
            }
        }

        let fallback_image = &fallback_image.d2;

        let mut textures = std::iter::repeat_n(&fallback_image.texture_view, MAX_TEXTURE_COUNT)
            .map(|texture| &**texture)
            .collect::<Vec<_>>();

        let mut fallback_sampler = &fallback_image.sampler;

        // Use sampler settings of the first image if available.
        if let Some(first_image) = images.first() {
            fallback_sampler = &first_image.sampler;
        }

        // fill in up to the first `MAX_TEXTURE_COUNT` textures and samplers to the arrays
        for (id, image) in images.into_iter().enumerate() {
            textures[id] = &*image.texture_view;
            fallback_sampler = &image.sampler;
        }

        let bind_group = render_device.create_bind_group(
            "chunk_material_bind_group",
            layout,
            &BindGroupEntries::sequential((
                properties_buffer.as_entire_buffer_binding(),
                model_buffer,
                face_buffer,
                &textures[..],
                fallback_sampler,
            )),
        );

        Ok(PreparedBindGroup {
            bindings: BindingResources(vec![]),
            bind_group,
            data: (),
        })
    }

    fn unprepared_bind_group(
        &self,
        _layout: &BindGroupLayout,
        _render_device: &RenderDevice,
        _param: &mut SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup<Self::Data>, AsBindGroupError> {
        Err(AsBindGroupError::CreateBindGroupDirectly)
    }

    fn bind_group_layout_entries(_: &RenderDevice, _: bool) -> Vec<BindGroupLayoutEntry>
    where
        Self: Sized,
    {
        BindGroupLayoutEntries::with_indices(
            // The layout entries will only be visible in the fragment stage
            ShaderStages::VERTEX_FRAGMENT,
            (
                (
                    0,
                    // Properties buffer
                    uniform_buffer::<MaterialProperties>(false),
                ),
                (
                    1,
                    // Model buffer
                    storage_buffer_read_only_sized(false, None),
                ),
                (
                    2,
                    // Face buffer
                    storage_buffer_read_only_sized(false, None),
                ),
                // Voxel texture array
                (
                    3,
                    texture_2d(TextureSampleType::Float { filterable: true })
                        .count(NonZero::<u32>::new(MAX_TEXTURE_COUNT as u32).unwrap()),
                ),
                // Sampler
                //
                // @group(2) @binding(1) var nearest_sampler: sampler;
                //
                // Note: as with textures, multiple samplers can also be bound
                // onto one binding slot:
                //
                // ```
                // sampler(SamplerBindingType::Filtering)
                //     .count(NonZero::<u32>::new(MAX_TEXTURE_COUNT as u32).unwrap()),
                // ```
                //
                // One may need to pay attention to the limit of sampler binding
                // amount on some platforms.
                (4, sampler(SamplerBindingType::Filtering)),
            ),
        )
        .to_vec()
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

    #[storage(1, read_only)]
    pub block_colors: Handle<ShaderStorageBuffer>,

    #[storage(2, read_only)]
    pub block_emissive: Handle<ShaderStorageBuffer>,
}

impl Material for ChunkMaterialWireframe {
    fn vertex_shader() -> ShaderRef {
        CHUNK_SHADER_HANDLE.into()
    }
    fn fragment_shader() -> ShaderRef {
        CHUNK_SHADER_HANDLE.into()
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
        let vertex_layout = layout
            .0
            .get_layout(&[ATTRIBUTE_VOXEL.at_shader_location(0)])?;
        descriptor.primitive.polygon_mode = PolygonMode::Line;
        descriptor.vertex.buffers = vec![vertex_layout];
        Ok(())
    }

    fn prepass_vertex_shader() -> ShaderRef {
        CHUNK_PREPASS_HANDLE.into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        CHUNK_PREPASS_HANDLE.into()
    }
}

pub const MAX_MESH_TASKS: usize = 32;

#[derive(Resource, Default)]
pub struct MeshingPipeline {
    pub load_mesh_queue: IndexSet<IVec3>,
    pub unload_mesh_queue: Vec<IVec3>,
    pub mesh_tasks: Vec<(IVec3, Option<Task<MeshTask>>)>,

    pub vertex_diagnostic: HashMap<IVec3, i32>,
}

#[derive(Resource, Default)]
pub struct ChunkMeshEntities(pub HashMap<IVec3, Entity>);

pub struct MeshTask {
    opaque: Option<ChunkMesh>,
    transparent: Option<ChunkMesh>,
}

/// begin mesh building tasks for chunks in range
pub fn start_mesh_tasks(
    mut mesh_pipeline: ResMut<MeshingPipeline>,
    voxel_engine: Res<VoxelEngine>,
    scanners: Query<&ChunkPos, With<Scanner<MeshScanner>>>,
    block_registry: Res<BlockRegistryResource>,
    model_registry: Res<IndexedModelRegistryResource>,
    mut chunk_gained_mesh_relevance: EventReader<ChunkGainedScannerRelevance<MeshScanner>>,
    mut chunk_modified: EventReader<ChunkModified>,
    global_mesh_scanner_chunks: Res<GlobalScannerDesiredChunks<MeshScanner>>,
) {
    let task_pool = AsyncComputeTaskPool::get();

    let VoxelEngine {
        world_data,
        lod,
        meshing_method,
        ..
    } = voxel_engine.as_ref();

    // Order by FURTHEST distance to any scanner.
    // Closest chunks are at the end.
    // We do this so we can pop from the end of the list.
    if !chunk_gained_mesh_relevance.is_empty() || !chunk_modified.is_empty() {
        mesh_pipeline
            .load_mesh_queue
            .extend(chunk_gained_mesh_relevance.read().map(|e| e.chunk));

        mesh_pipeline.load_mesh_queue.extend(
            chunk_modified
                .read()
                .map(|e| e.0)
                .filter(|chunk| global_mesh_scanner_chunks.chunks.contains(chunk)),
        );

        // TODO: With many chunks in queue, this is SLOW.
        let _span = info_span!("Sorting meshing queue by distance to scanners").entered();
        mesh_pipeline.load_mesh_queue.sort_by_cached_key(|pos| {
            let mut closest_distance = i32::MAX;

            for scan_pos in scanners.iter() {
                let distance = pos.distance_squared(scan_pos.0);
                if distance < closest_distance {
                    closest_distance = distance;
                }
            }

            -closest_distance
        });
    }

    let mut i = mesh_pipeline.load_mesh_queue.len();
    while i > 0 && mesh_pipeline.mesh_tasks.len() < MAX_MESH_TASKS {
        i -= 1;

        let world_pos = mesh_pipeline.load_mesh_queue[i];

        // We can only generate a mesh if all neighbors are available.
        let all_neighbors_available = ADJACENT_CHUNK_DIRECTIONS
            .iter()
            .all(|&dir| world_data.contains_key(&(world_pos + dir)));

        if !all_neighbors_available {
            continue;
        }
        mesh_pipeline.load_mesh_queue.swap_remove(&world_pos);

        let Some(chunks_refs) = ChunksRefs::try_new(world_data, world_pos) else {
            continue;
        };

        let llod = *lod;
        let block_registry = block_registry.0.clone();
        let model_registry = model_registry.0.clone();

        let task = match meshing_method {
            MeshingMethod::BinaryGreedyMeshing => task_pool.spawn(async move {
                MeshTask {
                    opaque: crate::face_model_mesher::build_chunk_mesh(
                        &chunks_refs,
                        llod,
                        &block_registry,
                        &model_registry,
                        FLAG_SOLID,
                        true,
                    ),
                    transparent: crate::face_model_mesher::build_chunk_mesh(
                        &chunks_refs,
                        llod,
                        &block_registry,
                        &model_registry,
                        FLAG_TRANSPARENT,
                        true,
                    ),
                }
            }),
        };

        mesh_pipeline.mesh_tasks.push((world_pos, Some(task)));
    }
}

/// destroy enqueued, chunk mesh entities
pub fn unload_mesh(
    mut commands: Commands,
    mut mesh_pipeline: ResMut<MeshingPipeline>,
    mut chunk_mesh_entities: ResMut<ChunkMeshEntities>,
    mut chunk_lost_mesh_relevance: EventReader<ChunkLostScannerRelevance<MeshScanner>>,
) {
    let MeshingPipeline {
        unload_mesh_queue,
        load_mesh_queue,
        vertex_diagnostic,
        ..
    } = mesh_pipeline.as_mut();

    unload_mesh_queue.extend(chunk_lost_mesh_relevance.read().map(|e| e.chunk));

    for chunk_pos in unload_mesh_queue.drain(..) {
        let Some(chunk_id) = chunk_mesh_entities.0.remove(&chunk_pos) else {
            continue;
        };

        vertex_diagnostic.remove(&chunk_pos);

        if let Ok(mut entity_commands) = commands.get_entity(chunk_id) {
            entity_commands.despawn();
        }

        load_mesh_queue.swap_remove(&chunk_pos);
    }
}

/// join the multithreaded chunk mesh tasks, and construct a finalized chunk entity
pub fn join_mesh(
    mut shader_storage_buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut mesh_pipeline: ResMut<MeshingPipeline>,
    mut chunk_mesh_entities: ResMut<ChunkMeshEntities>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ChunkMaterial>>,
    shared_material_buffers: Res<SharedMaterialBuffers>,
    texture_buffer: Res<TextureBuffer>,
) {
    let MeshingPipeline {
        mesh_tasks,
        vertex_diagnostic,
        ..
    } = mesh_pipeline.as_mut();

    let properties = MaterialProperties {
        reflectance: Vec3::splat(0.5),
        perceptual_roughness: 1.0,
        metallic: 0.01,
    };

    for (world_pos, task_option) in mesh_tasks.iter_mut() {
        let Some(mut task) = task_option.take() else {
            // should never happend, because we drop None values later
            warn!("someone modified task?");
            continue;
        };
        let Some(mut chunk_mesh_task) = block_on(poll_once(&mut task)) else {
            // failed polling, keep task alive
            *task_option = Some(task);
            continue;
        };

        // Despawn the old chunk entity if it exists.
        // Checking before we check the mesh because we may not get a mesh.
        if let Some(mut entity_commands) = chunk_mesh_entities
            .0
            .remove(world_pos)
            .and_then(|entity| commands.get_entity(entity).ok())
        {
            entity_commands.despawn();
        }

        let mut total_vertex_count = 0;
        if chunk_mesh_task.opaque.is_some() || chunk_mesh_task.transparent.is_some() {
            // spawn chunk entity
            let mut chunk_entity = commands.spawn((
                Transform::from_translation(world_pos.as_vec3() * Vec3::splat(32.0)),
                Visibility::Inherited,
                Name::new(format!("Chunk: {:?}", world_pos)),
            ));
            chunk_mesh_entities.0.insert(*world_pos, chunk_entity.id());

            if let Some(mesh) = chunk_mesh_task.opaque.take() {
                total_vertex_count += mesh.faces.len() * 4;

                let aabb = mesh.calculate_aabb();
                let (bevy_mesh, faces) = mesh.to_bevy_mesh();
                let mesh_handle = meshes.add(bevy_mesh);

                let mut face_buffer = ShaderStorageBuffer::from(faces);
                face_buffer.asset_usage = RenderAssetUsages::RENDER_WORLD;

                let face_buffer = shader_storage_buffers.add(face_buffer);

                chunk_entity.with_child((
                    aabb,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(materials.add(ChunkMaterial {
                        properties,
                        model_buffer: shared_material_buffers.model_buffer.clone(),
                        face_buffer,
                        alpha_mode: AlphaMode::Opaque,
                        textures: texture_buffer.0.clone(),
                    })),
                    ChunkEntityType::Opaque,
                    Name::new("Opaque"),
                ));
            }

            if let Some(mesh) = chunk_mesh_task.transparent.take() {
                total_vertex_count += mesh.faces.len() * 4;

                let aabb = mesh.calculate_aabb();

                let (bevy_mesh, faces) = mesh.to_bevy_mesh();
                let mesh_handle = meshes.add(bevy_mesh);

                let mut face_buffer = ShaderStorageBuffer::from(faces);
                face_buffer.asset_usage = RenderAssetUsages::RENDER_WORLD;

                let face_buffer = shader_storage_buffers.add(face_buffer);

                chunk_entity.with_child((
                    aabb,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(materials.add(ChunkMaterial {
                        properties,
                        model_buffer: shared_material_buffers.model_buffer.clone(),
                        face_buffer,
                        alpha_mode: AlphaMode::Premultiplied,
                        textures: texture_buffer.0.clone(),
                    })),
                    ChunkEntityType::Transparent,
                    Name::new("Transparent"),
                ));
            }
        }

        vertex_diagnostic.insert(*world_pos, total_vertex_count as i32);
    }

    mesh_pipeline.mesh_tasks.retain(|(_p, op)| op.is_some());
}
