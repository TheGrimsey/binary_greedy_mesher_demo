use bevy::{app::{App, Plugin}, ecs::event::Event, math::IVec3};

pub struct ChunkEventsPlugin;
impl Plugin for ChunkEventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ChunkGenerated>()
            .add_event::<ChunkUnloaded>()
            .add_event::<ChunkModified>();
    }
}

/// Fired when a chunk is first generated.
#[derive(Event)]
pub struct ChunkGenerated(pub IVec3);

/// Fired when a chunk is removed.
#[derive(Event)]
pub struct ChunkUnloaded(pub IVec3);

/// Fired when a chunk is modified
#[derive(Event)]
pub struct ChunkModified(pub IVec3);