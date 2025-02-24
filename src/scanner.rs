use std::marker::PhantomData;

use bevy::{prelude::*, utils::HashSet};

use crate::
    utils::world_to_chunk
;

pub const MAX_DATA_TASKS: usize = 9;
pub const MAX_MESH_TASKS: usize = 3;

pub const MAX_SCANS: usize = 26000;

pub struct ChunkTrackerPlugin;

impl Plugin for ChunkTrackerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            update_chunk_pos.run_if(any_with_component::<TrackChunkPos>),
        );

        app.register_type::<ChunkPos>();
    }
}

#[derive(Default)]
pub struct ScannerPlugin<T: Send + Sync + Default + 'static> {
    phantom_data: PhantomData<T>
}

impl<T: Send + Sync + Default + 'static> Plugin for ScannerPlugin<T> {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlobalScannerDesiredChunks<T>>();

        app.add_systems(
            PreUpdate,
            scan::<T>.after(update_chunk_pos).run_if(any_with_component::<Scanner<T>>.or(any_component_removed::<Scanner<T>>)),
        );

        app.add_event::<ChunkGainedScannerRelevance<T>>()
            .add_event::<ChunkLostScannerRelevance<T>>();
    }
}

#[derive(Component, Default)]
#[require(ChunkPos, GlobalTransform)]
pub struct TrackChunkPos;

#[derive(Component, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct ChunkPos(pub IVec3);

/// Iterates over chunks in a box around the center, within the given radius.
fn iter_chunks_around(center: IVec3, horizontal_radius: i32, vertical_radius: i32) -> impl Iterator<Item = IVec3> {
    let r = horizontal_radius + 1;
    let v_r = vertical_radius + 1;
    (-r..r).flat_map(move |x| {
        (-v_r..v_r).flat_map(move |y| {
            (-r..r).map(move |z| {
                IVec3::new(x, y, z) + center
            })
        })
    })
}

fn update_chunk_pos(
    mut query: Query<(&GlobalTransform, &mut ChunkPos), Changed<GlobalTransform>>,
) {
    for (g_transform, mut chunk_pos) in query.iter_mut() {
        chunk_pos.set_if_neq(ChunkPos(world_to_chunk(g_transform.translation())));
    }
}

#[derive(Component)]
#[require(TrackChunkPos)]
pub struct Scanner<T: Send + Sync + 'static> {
    horizontal_radius: u8,
    vertical_radius: u8,

    /// Chunks this scanner wants to load.
    /// Checked by the global collector.
    desired_chunks: HashSet<IVec3>,

    phantom_data: PhantomData<T>
}
impl<T: Send + Sync + 'static> Scanner::<T> {
    pub fn new(horizontal_radius: u8, vertical_radius: Option<u8>) -> Self {
        Self {
            horizontal_radius,
            vertical_radius: vertical_radius.unwrap_or(horizontal_radius),
            desired_chunks: HashSet::with_capacity(horizontal_radius as usize * vertical_radius.unwrap_or(horizontal_radius) as usize * horizontal_radius as usize),
            phantom_data: PhantomData
        }
    }
}

#[derive(Resource, Default)]
pub struct GlobalScannerDesiredChunks<T: Send + Sync + 'static> {
    pub chunks: HashSet<IVec3>,
    phantom_data: PhantomData<T>
}

#[derive(Default)]
pub struct MeshScanner;
#[derive(Default)]
pub struct DataScanner;

#[derive(Event)]
pub struct ChunkGainedScannerRelevance<T: Send + Sync + Default + 'static> {
    pub chunk: IVec3,
    phantom_data: PhantomData<T>
}

#[derive(Event)]
pub struct ChunkLostScannerRelevance<T: Send + Sync + Default + 'static> {
    pub chunk: IVec3,
    phantom_data: PhantomData<T>
}

pub fn scan<T: Send + Sync + Default + 'static>(
    any_changed_query: Query<(), (With<Scanner<T>>, Changed<ChunkPos>)>,
    scanners: Query<(&Scanner<T>, &ChunkPos)>,
    mut global_desired_chunks: ResMut<GlobalScannerDesiredChunks<T>>,
    mut current_desired_chunks: Local<HashSet<IVec3>>,
    mut gained_relevance_events: EventWriter<ChunkGainedScannerRelevance<T>>,
    mut lost_relevance_events: EventWriter<ChunkLostScannerRelevance<T>>,
    mut removed_scanners: RemovedComponents<Scanner<T>>,
) {
    if any_changed_query.is_empty() && removed_scanners.read().next().is_none() {
        return;
    }

    // Update the global collector.
    {
        let _span = info_span!("Filling globally desired chunks.").entered();
        current_desired_chunks.clear();
        for (scanner, chunk_pos) in scanners.iter() {
            current_desired_chunks.extend(iter_chunks_around(chunk_pos.0, scanner.horizontal_radius as i32, scanner.vertical_radius as i32));
        }
    }

    {
        let _span = info_span!("Finding newly desired chunks.").entered();
        let newly_desired_chunks = current_desired_chunks.difference(&global_desired_chunks.chunks);
        gained_relevance_events.send_batch(newly_desired_chunks.into_iter().map(|&chunk| ChunkGainedScannerRelevance { chunk, phantom_data: PhantomData }));
    }

    {
        let _span = info_span!("Finding no longer desired chunks.").entered();
        let no_longer_desired_chunks = global_desired_chunks.chunks.difference(&current_desired_chunks);
        lost_relevance_events.send_batch(no_longer_desired_chunks.into_iter().map(|&chunk| ChunkLostScannerRelevance { chunk, phantom_data: PhantomData }));
    }

    // Swap the lists because it's faster than copying.
    std::mem::swap(&mut global_desired_chunks.chunks, &mut current_desired_chunks);
}
