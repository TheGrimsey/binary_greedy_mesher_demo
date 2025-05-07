use std::sync::Arc;

use bevy::ecs::system::Resource;

pub mod model;

/// Exclusive range of faces in the model.
/// This represents the indexes in the model's face buffer.
#[derive(Debug, Clone)]
pub struct QuadRange {
    /// The start index of the face range.
    pub start: u32,
    /// The exclusive end index of the face range.
    pub end: u32,

    // Per face AO data.
    pub ao_direction: Box<[u8]>,
}

/// An IndexedModel which only references the faces in the model quad buffer.
#[derive(Debug, Clone)]
pub struct IndexedModel {
    /// Always visible faces.
    pub always_visible_faces: QuadRange,

    /// Faces that are visible when the model is not occluded per direction.
    pub occluded_faces: [QuadRange; 6],
}

/// The model registry.
#[derive(Default)]
pub struct IndexedModelRegistry {
    pub models: Vec<IndexedModel>,
}

#[derive(Resource)]
pub struct IndexedModelRegistryResource(pub Arc<IndexedModelRegistry>);