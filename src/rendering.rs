use bevy::{
    asset::load_internal_asset, pbr::{MaterialPipeline, MaterialPipelineKey}, prelude::*, render::{
        mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef},
        render_resource::{
            AsBindGroup, PolygonMode, RenderPipelineDescriptor, ShaderRef,
            SpecializedMeshPipelineError, VertexFormat,
        }, storage::ShaderStorageBuffer,
    }, tasks::{block_on, poll_once, AsyncComputeTaskPool, Task}, utils::HashMap
};
use indexmap::IndexSet;

use crate::{chunk_mesh::{ChunkMesh, ATTRIBUTE_VOXEL}, chunks_refs::ChunksRefs, constants::ADJACENT_CHUNK_DIRECTIONS, events::ChunkModified, scanner::{ChunkGainedScannerRelevance, ChunkLostScannerRelevance, ChunkPos, GlobalScannerDesiredChunks, MeshScanner, Scanner}, voxel::{BlockFlags, BlockRegistryResource}, voxel_engine::{join_data, MeshingMethod, VoxelEngine}};


pub const CHUNK_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(138165523578389129966343978676199385893);
pub const CHUNK_PREPASS_HANDLE: Handle<Shader> = Handle::weak_from_u128(38749848998489157831713083983198931828);

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

        app.init_resource::<MeshingPipeline>().init_resource::<ChunkMeshEntities>();

        app.add_systems(Startup, initialize_global_chunk_materials);
        app.add_systems(Update, apply_chunk_material);

        load_internal_asset!(
            app,
            CHUNK_SHADER_HANDLE,
            "chunk.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            CHUNK_PREPASS_HANDLE,
            "chunk_prepass.wgsl",
            Shader::from_wgsl
        );

        app.add_systems(PostUpdate, (
            join_mesh,
            unload_mesh,
            start_mesh_tasks.after(join_data),
        ).chain());
    }
}

fn initialize_global_chunk_materials(
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut chunk_materials_wireframe: ResMut<Assets<ChunkMaterialWireframe>>,
    mut chunk_materials: ResMut<Assets<ChunkMaterial>>,
    mut commands: Commands,
    block_registry: Res<BlockRegistryResource>,
) {
    let colors = block_registry.0.block_color.iter().map(|color| color.to_linear().to_f32_array()).collect::<Vec<_>>();
    let colors = buffers.add(ShaderStorageBuffer::from(colors));
    
    let emissive = block_registry.0.block_emissive.iter().map(|color| color.to_linear().to_f32_array()).collect::<Vec<_>>();
    let emissive = buffers.add(ShaderStorageBuffer::from(emissive));

    // TODO: Add transparent material.
    
    commands.insert_resource(GlobalChunkMaterial {
        opaque: chunk_materials.add(ChunkMaterial {
            reflectance: 0.5,
            perceptual_roughness: 1.0,
            metallic: 0.01,
            block_colors: colors.clone(),
            block_emissive: emissive.clone(),
            alpha_mode: AlphaMode::Opaque
        }),
        transparent: chunk_materials.add(ChunkMaterial {
            reflectance: 0.5,
            perceptual_roughness: 1.0,
            metallic: 0.01,
            block_colors: colors.clone(),
            block_emissive: emissive.clone(),
            alpha_mode: AlphaMode::Premultiplied
        }),   
    });

    
    commands.insert_resource(GlobalChunkWireframeMaterial(chunk_materials_wireframe.add(
        ChunkMaterialWireframe {
            reflectance: 0.5,
            perceptual_roughness: 1.0,
            metallic: 0.01,
            block_colors: colors.clone(),
            block_emissive: emissive.clone(),
        },
    )));
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
    
    #[storage(2,read_only)]
    pub block_emissive: Handle<ShaderStorageBuffer>,

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
        let vertex_layout = layout.0.get_layout(&[ATTRIBUTE_VOXEL.at_shader_location(0)])?;
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
// copy of chunk material pipeline but with wireframe
#[derive(Asset, Reflect, AsBindGroup, Debug, Clone)]
pub struct ChunkMaterialWireframe {
    #[uniform(0)]
    pub reflectance: f32,
    #[uniform(0)]
    pub perceptual_roughness: f32,
    #[uniform(0)]
    pub metallic: f32,
    
    #[storage(1,read_only)]
    pub block_colors: Handle<ShaderStorageBuffer>,
    
    #[storage(2,read_only)]
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
        let vertex_layout = layout.0.get_layout(&[ATTRIBUTE_VOXEL.at_shader_location(0)])?;
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
    mut chunk_gained_mesh_relevance: EventReader<ChunkGainedScannerRelevance<MeshScanner>>,
    mut chunk_modified: EventReader<ChunkModified>,
    global_mesh_scanner_chunks: Res<GlobalScannerDesiredChunks<MeshScanner>>
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
        mesh_pipeline.load_mesh_queue.extend(chunk_gained_mesh_relevance.read().map(|e| e.chunk));

        mesh_pipeline.load_mesh_queue.extend(chunk_modified.read().map(|e| e.0).filter(|chunk| global_mesh_scanner_chunks.chunks.contains(chunk)));

        // TODO: With many chunks in queue, this is SLOW.
        let _span = info_span!("Sorting meshing queue by distance to scanners").entered();
        mesh_pipeline.load_mesh_queue.sort_by_cached_key(|pos| {
            let mut closest_distance = i32::MAX;
            // TODO: This could use bevy_spatial for better performance.
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
        let all_neighbors_available = ADJACENT_CHUNK_DIRECTIONS.iter().all(|&dir| {
            world_data.contains_key(&(world_pos + dir))
        });

        if !all_neighbors_available {
            continue;
        }
        mesh_pipeline.load_mesh_queue.swap_remove(&world_pos);

        let Some(chunks_refs) = ChunksRefs::try_new(world_data, world_pos) else {
            continue;
        };
        
        let llod = *lod;
        let block_registry = block_registry.0.clone();
        
        let task = match meshing_method {
            MeshingMethod::BinaryGreedyMeshing => task_pool.spawn(async move {
                MeshTask {
                    opaque: crate::greedy_mesher_optimized::build_chunk_mesh(&chunks_refs, llod, block_registry.clone(), BlockFlags::SOLID, true, false),
                    transparent: crate::greedy_mesher_optimized::build_chunk_mesh(&chunks_refs, llod, block_registry, BlockFlags::TRANSPARENT, true, false)
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
    mut chunk_lost_mesh_relevance: EventReader<ChunkLostScannerRelevance<MeshScanner>>
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
        
        if let Some(entity_commands) = commands.get_entity(chunk_id) {
            entity_commands.despawn_recursive();
        }

        load_mesh_queue.swap_remove(&chunk_pos);
    }
}

/// join the multithreaded chunk mesh tasks, and construct a finalized chunk entity
pub fn join_mesh(
    mut mesh_pipeline: ResMut<MeshingPipeline>,
    mut chunk_mesh_entities: ResMut<ChunkMeshEntities>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    global_chunk_material: Res<GlobalChunkMaterial>,
) {
    let MeshingPipeline {
        mesh_tasks,
        vertex_diagnostic,
        ..
    } = mesh_pipeline.as_mut();

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
        if let Some(entity) = chunk_mesh_entities.0.remove(world_pos) {
            commands.entity(entity).despawn_recursive();
        }

        let mut total_vertex_count = 0;
        if chunk_mesh_task.opaque.is_some() || chunk_mesh_task.transparent.is_some() {
            // spawn chunk entity
            let mut chunk_entity = commands
                .spawn((
                    Transform::from_translation(world_pos.as_vec3() * Vec3::splat(32.0)),
                    Visibility::Inherited,
                    Name::new(format!("Chunk: {:?}", world_pos)),
                ));
            chunk_mesh_entities.0.insert(*world_pos, chunk_entity.id());

            if let Some(mesh) = chunk_mesh_task.opaque.take() {
                total_vertex_count += mesh.vertices.len();

                let aabb = mesh.calculate_aabb();
                let bevy_mesh = mesh.to_bevy_mesh();
                let mesh_handle = meshes.add(bevy_mesh);
                
                chunk_entity.with_child((
                    aabb,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(global_chunk_material.opaque.clone()),
                    ChunkEntityType::Opaque,
                    Name::new("Opaque")
                ));
            }

            if let Some(mesh) = chunk_mesh_task.transparent.take() {
                total_vertex_count += mesh.vertices.len();

                let aabb = mesh.calculate_aabb();
                let bevy_mesh = mesh.to_bevy_mesh();
                let mesh_handle = meshes.add(bevy_mesh);
                
                chunk_entity.with_child((
                    aabb,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(global_chunk_material.transparent.clone()),
                    ChunkEntityType::Transparent,
                    Name::new("Transparent")
                ));
            }
        }

        vertex_diagnostic.insert(*world_pos, total_vertex_count as i32);
    }

    mesh_pipeline.mesh_tasks.retain(|(_p, op)| op.is_some());
}