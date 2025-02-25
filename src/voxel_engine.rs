use std::sync::Arc;

use bevy::{
    diagnostic::{Diagnostic, DiagnosticPath, Diagnostics, RegisterDiagnostic},
    prelude::*,
    tasks::{block_on, AsyncComputeTaskPool, Task},
    utils::{HashMap, HashSet},
};
use bevy_screen_diagnostics::{Aggregate, ScreenDiagnostics};
use indexmap::IndexSet;

use crate::{
    chunk::{ChunkData, ChunkGenerator}, chunk_mesh::ChunkMesh, chunks_refs::ChunksRefs, constants::{ADJACENT_CHUNK_DIRECTIONS, CHUNK_SIZE3}, events::{ChunkEventsPlugin, ChunkGenerated, ChunkModified, ChunkUnloaded}, lod::Lod, rendering::{ChunkEntityType, GlobalChunkMaterial}, scanner::{ChunkGainedScannerRelevance, ChunkLostScannerRelevance, ChunkPos, ChunkTrackerPlugin, DataScanner, MeshScanner, Scanner, ScannerPlugin}, utils::{get_edging_chunk, vec3_to_index}, voxel::{load_block_registry, BlockFlags, BlockId, BlockRegistryResource}
};
use futures_lite::future;

pub struct VoxelEnginePlugin;

pub const MAX_DATA_TASKS: usize = 64;
pub const MAX_MESH_TASKS: usize = 32;

impl Plugin for VoxelEnginePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelEngine>();

        app.add_plugins((
            ChunkEventsPlugin,
            ChunkTrackerPlugin,
            ScannerPlugin::<DataScanner>::default(),
            ScannerPlugin::<MeshScanner>::default(),
        ));
        

        app.add_systems(PostUpdate, (start_data_tasks, start_mesh_tasks));
        app.add_systems(Update, start_modifications);
        app.add_systems(
            // PostUpdate,
            Update,
            ((join_data, join_mesh), (unload_data, unload_mesh)).chain(),
        );
        app.add_systems(Startup, setup_diagnostics);
        app.register_diagnostic(Diagnostic::new(DIAG_LOAD_MESH_QUEUE));
        app.register_diagnostic(Diagnostic::new(DIAG_UNLOAD_MESH_QUEUE));
        app.register_diagnostic(Diagnostic::new(DIAG_LOAD_DATA_QUEUE));
        app.register_diagnostic(Diagnostic::new(DIAG_UNLOAD_DATA_QUEUE));
        app.register_diagnostic(Diagnostic::new(DIAG_VERTEX_COUNT));
        app.register_diagnostic(Diagnostic::new(DIAG_MESH_TASKS));
        app.register_diagnostic(Diagnostic::new(DIAG_DATA_TASKS));
        app.add_systems(Update, diagnostics_count);

        app.add_systems(PreStartup, load_block_registry);
    }
}

#[derive(Debug, Reflect, Copy, Clone, Eq, PartialEq, Hash)]
pub enum MeshingMethod {
    BinaryGreedyMeshing,
}

/// holds all voxel world data
#[derive(Resource)]
pub struct VoxelEngine {
    pub world_data: HashMap<IVec3, Arc<ChunkData>>,
    pub vertex_diagnostic: HashMap<IVec3, i32>,
    // Using index map to only load a chunk once & still be able to sort.
    pub load_data_queue: IndexSet<IVec3>,
    pub load_mesh_queue: IndexSet<IVec3>,
    pub unload_data_queue: Vec<IVec3>,
    pub unload_mesh_queue: Vec<IVec3>,
    pub data_tasks: HashMap<IVec3, Option<Task<ChunkData>>>,
    pub mesh_tasks: Vec<(IVec3, Option<Task<MeshTask>>)>,
    pub chunk_entities: HashMap<IVec3, Entity>,
    pub lod: Lod,
    pub meshing_method: MeshingMethod,
    pub chunk_modifications: HashMap<IVec3, Vec<ChunkModification>>,
}

pub struct ChunkModification(pub IVec3, pub BlockId);

const DIAG_LOAD_DATA_QUEUE: DiagnosticPath = DiagnosticPath::const_new("load_data_queue");
const DIAG_UNLOAD_DATA_QUEUE: DiagnosticPath = DiagnosticPath::const_new("unload_data_queue");
const DIAG_LOAD_MESH_QUEUE: DiagnosticPath = DiagnosticPath::const_new("load_mesh_queue");
const DIAG_UNLOAD_MESH_QUEUE: DiagnosticPath = DiagnosticPath::const_new("unload_mesh_queue");
const DIAG_VERTEX_COUNT: DiagnosticPath = DiagnosticPath::const_new("vertex_count");
const DIAG_MESH_TASKS: DiagnosticPath = DiagnosticPath::const_new("mesh_tasks");
const DIAG_DATA_TASKS: DiagnosticPath = DiagnosticPath::const_new("data_tasks");

fn setup_diagnostics(mut onscreen: ResMut<ScreenDiagnostics>) {
    onscreen
        .add("load_data_queue".to_string(), DIAG_LOAD_DATA_QUEUE)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>4.0}"));
    onscreen
        .add("unload_data_queue".to_string(), DIAG_UNLOAD_DATA_QUEUE)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>3.0}"));
    onscreen
        .add("load_mesh_queue".to_string(), DIAG_LOAD_MESH_QUEUE)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>4.0}"));
    onscreen
        .add("unload_mesh_queue".to_string(), DIAG_UNLOAD_MESH_QUEUE)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>3.0}"));
    onscreen
        .add("vertex_count".to_string(), DIAG_VERTEX_COUNT)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>7.0}"));
    onscreen
        .add("mesh_tasks".to_string(), DIAG_MESH_TASKS)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>4.0}"));
    onscreen
        .add("data_tasks".to_string(), DIAG_DATA_TASKS)
        .aggregate(Aggregate::Value)
        .format(|v| format!("{v:0>2.0}"));
}

fn diagnostics_count(mut diagnostics: Diagnostics, voxel_engine: Res<VoxelEngine>) {
    diagnostics.add_measurement(&DIAG_LOAD_DATA_QUEUE, || {
        voxel_engine.load_data_queue.len() as f64
    });
    diagnostics.add_measurement(&DIAG_UNLOAD_DATA_QUEUE, || {
        voxel_engine.unload_data_queue.len() as f64
    });
    diagnostics.add_measurement(&DIAG_LOAD_MESH_QUEUE, || {
        voxel_engine.load_mesh_queue.len() as f64
    });
    diagnostics.add_measurement(&DIAG_UNLOAD_MESH_QUEUE, || {
        voxel_engine.unload_mesh_queue.len() as f64
    });
    diagnostics.add_measurement(&DIAG_MESH_TASKS, || voxel_engine.mesh_tasks.len() as f64);
    diagnostics.add_measurement(&DIAG_DATA_TASKS, || voxel_engine.data_tasks.len() as f64);
    diagnostics.add_measurement(&DIAG_VERTEX_COUNT, || {
        voxel_engine
            .vertex_diagnostic
            .iter()
            .map(|(_, v)| v)
            .sum::<i32>() as f64
    });
}

impl VoxelEngine {
    /*pub fn unload_all_meshes(&mut self, scanner: &Scanner, scanner_transform: &GlobalTransform) {
        // stop all any current proccessing
        self.load_mesh_queue.clear();
        self.mesh_tasks.clear();
        let scan_pos =
            ((scanner_transform.translation() - Vec3::splat(16.0)) * (1.0 / 32.0)).as_ivec3();
        for offset in &scanner.mesh_sampling_offsets {
            let wpos = scan_pos + *offset;
            self.load_mesh_queue.insert(wpos);
        }
    }*/
}

impl Default for VoxelEngine {
    fn default() -> Self {
        VoxelEngine {
            world_data: HashMap::new(),
            load_data_queue: IndexSet::new(),
            load_mesh_queue: IndexSet::new(),
            unload_data_queue: Vec::new(),
            unload_mesh_queue: Vec::new(),
            data_tasks: HashMap::new(),
            mesh_tasks: Vec::new(),
            chunk_entities: HashMap::new(),
            lod: Lod::L32,
            meshing_method: MeshingMethod::BinaryGreedyMeshing,
            vertex_diagnostic: HashMap::new(),
            chunk_modifications: HashMap::new(),
        }
    }
}

/// begin data building tasks for chunks in range
pub fn start_data_tasks(
    mut voxel_engine: ResMut<VoxelEngine>,
    scanners: Query<&ChunkPos, With<Scanner<DataScanner>>>,
    mut chunk_gained_data_relevance: EventReader<ChunkGainedScannerRelevance<DataScanner>>,
    chunk_generator: Res<ChunkGenerator>,
) {
    let task_pool = AsyncComputeTaskPool::get();

    let VoxelEngine {
        load_data_queue,
        data_tasks,
        ..
    } = voxel_engine.as_mut();

    
    // Order by closest distance to any scanner.
    if !chunk_gained_data_relevance.is_empty() {
        load_data_queue.extend(chunk_gained_data_relevance.read().map(|e| e.chunk));
        
        // TODO: With many chunks in queue, this is SLOW.
        let _span = info_span!("Sorting data queue by distance to scanners").entered();
        load_data_queue.sort_by_cached_key(|pos| {
            let mut closest_distance = i32::MAX;
            
            for scan_pos in scanners.iter() {
                let distance = pos.distance_squared(scan_pos.0);
                if distance < closest_distance {
                    closest_distance = distance;
                }
            }
    
            closest_distance
        });
    }

    let tasks_left = MAX_DATA_TASKS.saturating_sub(data_tasks.len()).min(load_data_queue.len());
    for world_pos in load_data_queue.drain(0..tasks_left) {
        let k = world_pos;
        let generate = chunk_generator.generate.clone();
        let task = task_pool.spawn(async move {
            generate(k)
        });
        data_tasks.insert(world_pos, Some(task));
    }
}

/// destroy enqueued, chunk data
pub fn unload_data(
    mut voxel_engine: ResMut<VoxelEngine>,
    mut events: EventWriter<ChunkUnloaded>,
    mut chunk_lost_data_relevance: EventReader<ChunkLostScannerRelevance<DataScanner>>
) {
    let VoxelEngine {
        unload_data_queue,
        world_data,
        load_data_queue,
        ..
    } = voxel_engine.as_mut();

    unload_data_queue.extend(chunk_lost_data_relevance.read().map(|e| e.chunk));

    events.send_batch(unload_data_queue.iter().copied().map(ChunkUnloaded));

    for chunk_pos in unload_data_queue.drain(..) {
        load_data_queue.swap_remove(&chunk_pos);
        world_data.remove(&chunk_pos);
    }
}

/// destroy enqueued, chunk mesh entities
pub fn unload_mesh(
    mut commands: Commands,
    mut voxel_engine: ResMut<VoxelEngine>,
    mut chunk_lost_mesh_relevance: EventReader<ChunkLostScannerRelevance<MeshScanner>>
) {
    let VoxelEngine {
        unload_mesh_queue,
        load_mesh_queue,
        chunk_entities,
        vertex_diagnostic,
        ..
    } = voxel_engine.as_mut();

    unload_mesh_queue.extend(chunk_lost_mesh_relevance.read().map(|e| e.chunk));

    for chunk_pos in unload_mesh_queue.drain(..) {
        let Some(chunk_id) = chunk_entities.remove(&chunk_pos) else {
            continue;
        };
        vertex_diagnostic.remove(&chunk_pos);
        if let Some(entity_commands) = commands.get_entity(chunk_id) {
            entity_commands.despawn_recursive();
        }

        load_mesh_queue.swap_remove(&chunk_pos);
    }
}

pub struct MeshTask {
    opaque: Option<ChunkMesh>,
    transparent: Option<ChunkMesh>,
}

/// begin mesh building tasks for chunks in range
pub fn start_mesh_tasks(
    mut voxel_engine: ResMut<VoxelEngine>,
    scanners: Query<&ChunkPos, With<Scanner<MeshScanner>>>,
    block_registry: Res<BlockRegistryResource>,
    mut chunk_gained_mesh_relevance: EventReader<ChunkGainedScannerRelevance<MeshScanner>>,
) {
    let task_pool = AsyncComputeTaskPool::get();

    let VoxelEngine {
        load_mesh_queue,
        mesh_tasks,
        world_data,
        lod,
        meshing_method,
        ..
    } = voxel_engine.as_mut();

    
    // Order by FURTHEST distance to any scanner.
    // Closest chunks are at the end.
    // We do this so we can pop from the end of the list.
    if !chunk_gained_mesh_relevance.is_empty() {
        load_mesh_queue.extend(chunk_gained_mesh_relevance.read().map(|e| e.chunk));
        // TODO: With many chunks in queue, this is SLOW.
        let _span = info_span!("Sorting meshing queue by distance to scanners").entered();
        load_mesh_queue.sort_by_cached_key(|pos| {
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

    // We can't use extract_if yet, so we have to do this manually.
    let tasks_left = (MAX_MESH_TASKS as i32 - mesh_tasks.len() as i32)
        .min(load_mesh_queue.len() as i32)
        .max(0) as usize;
    let mut chunks_to_generate = Vec::with_capacity(tasks_left);

    let mut i = load_mesh_queue.len() - 1;
    while let Some(world_pos) = load_mesh_queue.get_index(i).copied() {
        // We can only generate a mesh if all neighbors are available.
        let all_neighbors_available = ADJACENT_CHUNK_DIRECTIONS.iter().all(|&dir| {
            world_data.contains_key(&(world_pos + dir))
        });

        if all_neighbors_available {
            chunks_to_generate.push(world_pos);
            load_mesh_queue.swap_remove(&world_pos);

            if chunks_to_generate.len() >= tasks_left {
                break;
            }
        }
        i -= 1;
    }

    for world_pos in chunks_to_generate {
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

        mesh_tasks.push((world_pos, Some(task)));
    }
}

// start
pub fn start_modifications(
    mut voxel_engine: ResMut<VoxelEngine>,
    mut events: EventWriter<ChunkModified>,
    mut updated_and_adjecant_chunks_set: Local<HashSet<IVec3>>,
) {
    let VoxelEngine {
        world_data,
        chunk_modifications,
        load_mesh_queue,
        ..
    } = voxel_engine.as_mut();
    for (pos, mods) in chunk_modifications.drain() {
        // say i want to load mesh now :)
        let Some(chunk_data) = world_data.get_mut(&pos) else {
            continue;
        };
        let new_chunk_data = Arc::make_mut(chunk_data);
        for ChunkModification(local_pos, block_type) in mods.into_iter() {
            let i = vec3_to_index(local_pos, 32);
            if new_chunk_data.voxels.len() == 1 {
                let value = new_chunk_data.voxels[0];
                new_chunk_data.voxels.resize(CHUNK_SIZE3, value);
            }
            new_chunk_data.voxels[i].block_type = block_type;
            if let Some(edge_chunk) = get_edging_chunk(local_pos) {
                updated_and_adjecant_chunks_set.insert(pos + edge_chunk);
            }
        }
        updated_and_adjecant_chunks_set.insert(pos);
        
        events.send(ChunkModified(pos));
    }

    load_mesh_queue.extend(updated_and_adjecant_chunks_set.drain());
}

/// join the chunkdata threads
pub fn join_data(
    mut voxel_engine: ResMut<VoxelEngine>,
    mut events: EventWriter<ChunkGenerated>
) {
    let VoxelEngine {
        world_data,
        data_tasks,
        ..
    } = voxel_engine.as_mut();
    for (world_pos, task_option) in data_tasks.iter_mut() {
        let Some(mut task) = task_option.take() else {
            // should never happend, because we drop None values later
            warn!("someone modified task?");
            continue;
        };
        let Some(chunk_data) = block_on(future::poll_once(&mut task)) else {
            *task_option = Some(task);
            continue;
        };

        world_data.insert(*world_pos, Arc::new(chunk_data));
        events.send(ChunkGenerated(*world_pos));
    }
    data_tasks.retain(|_k, op| op.is_some());
}

/// join the multithreaded chunk mesh tasks, and construct a finalized chunk entity
pub fn join_mesh(
    mut voxel_engine: ResMut<VoxelEngine>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    global_chunk_material: Res<GlobalChunkMaterial>,
) {
    let VoxelEngine {
        mesh_tasks,
        chunk_entities,
        vertex_diagnostic,
        ..
    } = voxel_engine.as_mut();
    for (world_pos, task_option) in mesh_tasks.iter_mut() {
        let Some(mut task) = task_option.take() else {
            // should never happend, because we drop None values later
            warn!("someone modified task?");
            continue;
        };
        let Some(mut chunk_mesh_task) = block_on(future::poll_once(&mut task)) else {
            // failed polling, keep task alive
            *task_option = Some(task);
            continue;
        };
        
        // Despawn the old chunk entity if it exists.
        // Checking before we check the mesh because we may not get a mesh.
        if let Some(entity) = chunk_entities.remove(world_pos) {
            commands.entity(entity).despawn_recursive();
        }

        let mut total_vertex_count = 0;
        if chunk_mesh_task.opaque.is_some() || chunk_mesh_task.transparent.is_some() {
            // spawn chunk entity
            let mut chunk_entity = commands
                .spawn((
                    Transform::from_translation(world_pos.as_vec3() * Vec3::splat(32.0)),
                    Name::new(format!("Chunk: {:?}", world_pos)),
                ));
            chunk_entities.insert(*world_pos, chunk_entity.id());

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
    mesh_tasks.retain(|(_p, op)| op.is_some());
}
