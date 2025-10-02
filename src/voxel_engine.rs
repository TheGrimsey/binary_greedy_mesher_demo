use std::sync::Arc;

use bevy::{
    math::U8Vec3,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use indexmap::IndexSet;

use crate::{
    chunk::{ChunkData, ChunkGenerator},
    constants::CHUNK_SIZE,
    events::{ChunkEventsPlugin, ChunkGenerated, ChunkModified, ChunkUnloaded},
    lod::Lod,
    scanner::{
        ChunkGainedScannerRelevance, ChunkLostScannerRelevance, ChunkPos, ChunkTrackerPlugin,
        DataScanner, MeshScanner, Scanner, ScannerPlugin, scan,
    },
    utils::vec3_to_index_in_chunk,
    voxel::BlockId,
};

pub struct VoxelEnginePlugin;

pub const MAX_DATA_TASKS: usize = 64;

impl Plugin for VoxelEnginePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelEngine>();

        app.add_plugins((
            ChunkEventsPlugin,
            ChunkTrackerPlugin,
            ScannerPlugin::<DataScanner>::default(),
            ScannerPlugin::<MeshScanner>::default(),
        ));

        app.add_systems(Update, start_modifications);
        app.add_systems(
            Update,
            (
                join_data,
                (unload_data, start_data_tasks)
                    .chain()
                    .after(scan::<DataScanner>),
            )
                .chain(),
        );
    }
}

/// holds all voxel world data
#[derive(Resource)]
pub struct VoxelEngine {
    pub world_data: HashMap<IVec3, Arc<ChunkData>>,
    // Using index map to only load a chunk once & still be able to sort.
    pub load_data_queue: IndexSet<IVec3>,
    pub unload_data_queue: Vec<IVec3>,
    pub data_tasks: HashMap<IVec3, Option<Task<ChunkData>>>,
    pub lod: Lod,
    pub chunk_modifications: HashMap<IVec3, Vec<ChunkModification>>,
}

pub struct ChunkModification(pub U8Vec3, pub BlockId);

impl Default for VoxelEngine {
    fn default() -> Self {
        VoxelEngine {
            world_data: HashMap::new(),
            load_data_queue: IndexSet::new(),
            unload_data_queue: Vec::new(),
            data_tasks: HashMap::new(),
            lod: Lod::L32,
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

    let tasks_left = MAX_DATA_TASKS
        .saturating_sub(data_tasks.len())
        .min(load_data_queue.len());
    for world_pos in load_data_queue.drain(0..tasks_left) {
        let k = world_pos;
        let generate = chunk_generator.generate.clone();
        let task = task_pool.spawn(async move { generate(k) });
        data_tasks.insert(world_pos, Some(task));
    }
}

/// destroy enqueued, chunk data
pub fn unload_data(
    mut voxel_engine: ResMut<VoxelEngine>,
    mut events: EventWriter<ChunkUnloaded>,
    mut chunk_lost_data_relevance: EventReader<ChunkLostScannerRelevance<DataScanner>>,
) {
    let VoxelEngine {
        unload_data_queue,
        world_data,
        load_data_queue,
        ..
    } = voxel_engine.as_mut();

    unload_data_queue.extend(chunk_lost_data_relevance.read().map(|e| e.chunk));

    events.write_batch(unload_data_queue.iter().copied().map(ChunkUnloaded));

    for chunk_pos in unload_data_queue.drain(..) {
        load_data_queue.swap_remove(&chunk_pos);
        world_data.remove(&chunk_pos);
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
        ..
    } = voxel_engine.as_mut();
    for (chunk_pos, mods) in chunk_modifications.drain() {
        let Some(chunk_data) = world_data.get_mut(&chunk_pos) else {
            continue;
        };

        let new_chunk_data = Arc::make_mut(chunk_data);
        for ChunkModification(local_pos, block_type) in mods.into_iter() {
            let i = vec3_to_index_in_chunk(local_pos.as_uvec3());
            new_chunk_data.expand_if_necessary();
            new_chunk_data.set_voxel(i, block_type);

            // Add pos chunks to the modified list.
            if local_pos.x == 0 {
                updated_and_adjecant_chunks_set.insert(chunk_pos - IVec3::new(1, 0, 0));
            } else if local_pos.x == CHUNK_SIZE as u8 - 1 {
                updated_and_adjecant_chunks_set.insert(chunk_pos + IVec3::new(1, 0, 0));
            }

            if local_pos.y == 0 {
                updated_and_adjecant_chunks_set.insert(chunk_pos - IVec3::new(0, 1, 0));
            } else if local_pos.y == CHUNK_SIZE as u8 - 1 {
                updated_and_adjecant_chunks_set.insert(chunk_pos + IVec3::new(0, 1, 0));
            }

            if local_pos.z == 0 {
                updated_and_adjecant_chunks_set.insert(chunk_pos - IVec3::new(0, 0, 1));
            } else if local_pos.z == CHUNK_SIZE as u8 - 1 {
                updated_and_adjecant_chunks_set.insert(chunk_pos + IVec3::new(0, 0, 1));
            }
        }
        updated_and_adjecant_chunks_set.insert(chunk_pos);
    }

    events.write_batch(updated_and_adjecant_chunks_set.drain().map(ChunkModified));
}

/// join the chunkdata threads
pub fn join_data(mut voxel_engine: ResMut<VoxelEngine>, mut events: EventWriter<ChunkGenerated>) {
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
        let Some(chunk_data) = block_on(poll_once(&mut task)) else {
            *task_option = Some(task);
            continue;
        };

        world_data.insert(*world_pos, Arc::new(chunk_data));
        events.write(ChunkGenerated(*world_pos));
    }
    data_tasks.retain(|_k, op| op.is_some());
}
