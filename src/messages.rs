use bevy::{
    app::{App, Plugin},
    ecs::message::Message,
    math::IVec3,
};

pub struct ChunkEventsPlugin;
impl Plugin for ChunkEventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ChunkGenerated>()
            .add_message::<ChunkUnloaded>()
            .add_message::<ChunkModified>();
    }
}

/// Fired when a chunk is first generated.
#[derive(Message)]
pub struct ChunkGenerated(pub IVec3);

/// Fired when a chunk is removed.
#[derive(Message)]
pub struct ChunkUnloaded(pub IVec3);

/// Fired when a chunk is modified
#[derive(Message)]
pub struct ChunkModified(pub IVec3);
