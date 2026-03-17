//! ECS (Entity-Component-System) architecture.
//!
//! The ECS owns all entities, components, and systems.
//! External consumers (like Workflow Builder) create entities via the Scene API.

pub mod components;
pub mod pipeline;
pub mod systems;

use crate::time::EntityTime;
use crate::types::Mat4;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for an entity.
pub type EntityId = String;

/// An entity in the scene — a container for components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub components: Components,

    /// Runtime-resolved data (not serialized).
    #[serde(skip)]
    pub resolved: ResolvedState,
}

/// All possible components an entity can have.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Components {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_source: Option<components::VideoSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_source: Option<components::ImageSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_source: Option<components::TextSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_source: Option<components::ColorSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<components::Timeline>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<components::Transform>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<components::ColorAdjust>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<components::Animation>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects: Option<Vec<components::Effect>>,
}

/// Runtime-resolved state (computed by systems each frame).
#[derive(Debug, Clone, Default)]
pub struct ResolvedState {
    /// Whether this entity is visible at the current time.
    pub visible: bool,
    /// Final computed transform matrix.
    pub world_matrix: Mat4,
    /// Final computed opacity (after animation).
    pub opacity: f32,
    /// Entity-local time information.
    pub time: EntityTime,
    /// Layer order for sorting.
    pub layer: i32,
}

/// The world: collection of all entities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct World {
    pub entities: Vec<Entity>,

    /// Entity lookup by ID.
    #[serde(skip)]
    id_index: HashMap<EntityId, usize>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entity to the world.
    pub fn add_entity(&mut self, entity: Entity) {
        let idx = self.entities.len();
        self.id_index.insert(entity.id.clone(), idx);
        self.entities.push(entity);
    }

    /// Get an entity by ID.
    pub fn get(&self, id: &str) -> Option<&Entity> {
        self.id_index
            .get(id)
            .and_then(|&idx| self.entities.get(idx))
    }

    /// Get a mutable entity by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Entity> {
        self.id_index
            .get(id)
            .copied()
            .and_then(|idx| self.entities.get_mut(idx))
    }

    /// Rebuild the ID index (call after deserialization).
    pub fn rebuild_index(&mut self) {
        self.id_index.clear();
        for (idx, entity) in self.entities.iter().enumerate() {
            self.id_index.insert(entity.id.clone(), idx);
        }
    }

    /// Get entities sorted by layer order.
    pub fn sorted_by_layer(&self) -> Vec<&Entity> {
        let mut sorted: Vec<&Entity> = self
            .entities
            .iter()
            .filter(|e| e.resolved.visible)
            .collect();
        sorted.sort_by_key(|e| e.resolved.layer);
        sorted
    }
}
