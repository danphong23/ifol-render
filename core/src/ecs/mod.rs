//! ECS (Entity-Component-System) architecture.
//!
//! The ECS owns all entities, components, and systems.
//! External consumers (like Workflow Builder) create entities via the Scene API.

pub mod components;
pub mod draw;
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Components {
    // ── Sources ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_source: Option<components::VideoSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_source: Option<components::ImageSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_source: Option<components::TextSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_source: Option<components::ColorSource>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_source: Option<components::AudioSource>,

    // ── Layout & Timing ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<components::Timeline>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<components::Transform>,

    // ── Appearance ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub blend_mode: Option<components::BlendMode>,

    /// Whether this entity is visible in the scene.
    #[serde(default = "default_true")]
    pub visible: bool,

    /// Human-readable display name (falls back to id if None).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    // ── Effects & Animation ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<components::ColorAdjust>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<components::Animation>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects: Option<Vec<components::Effect>>,

    /// Ordered effect stack (blur, color grade, etc.) for post-processing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_stack: Option<components::EffectStack>,

    // ── Camera ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<components::Camera>,

    // ── Hierarchy ──
    /// Parent entity ID for transform hierarchy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<EntityId>,

    /// Child entity IDs (managed automatically by reparent operations).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<EntityId>,
}

fn default_true() -> bool {
    true
}

impl Default for Components {
    fn default() -> Self {
        Self {
            video_source: None,
            image_source: None,
            text_source: None,
            color_source: None,
            audio_source: None,
            timeline: None,
            transform: None,
            opacity: None,
            blend_mode: None,
            visible: true, // entities are visible by default
            name: None,
            color: None,
            animation: None,
            effects: None,
            effect_stack: None,
            camera: None,
            parent: None,
            children: Vec::new(),
        }
    }
}

impl Entity {
    /// Get the display name (name or id).
    pub fn display_name(&self) -> &str {
        self.components.name.as_deref().unwrap_or(self.id.as_str())
    }
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
    /// Z-index for depth sorting within the same layer.
    pub z_index: f32,
    /// Content size in units (computed from pixel_size / PPU).
    /// Used by draw.rs to apply correct size transform.
    pub content_size: Option<[f32; 2]>,
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

    /// Remove an entity by index. Returns the removed entity.
    pub fn remove_entity(&mut self, index: usize) -> Option<Entity> {
        if index >= self.entities.len() {
            return None;
        }
        let entity = self.entities.remove(index);
        // Remove from parent's children list
        if let Some(ref parent_id) = entity.components.parent {
            let parent_id = parent_id.clone();
            if let Some(parent) = self.get_mut(&parent_id) {
                parent.components.children.retain(|c| c != &entity.id);
            }
        }
        // Orphan any children
        let child_ids: Vec<EntityId> = entity.components.children.clone();
        for child_id in child_ids {
            if let Some(child) = self.get_mut(&child_id) {
                child.components.parent = None;
            }
        }
        self.rebuild_index();
        Some(entity)
    }

    /// Remove an entity by ID. Returns the removed entity.
    pub fn remove_entity_by_id(&mut self, id: &str) -> Option<Entity> {
        let idx = self.id_index.get(id).copied()?;
        self.remove_entity(idx)
    }

    /// Move an entity to a new index position.
    pub fn move_entity(&mut self, from: usize, to: usize) {
        if from >= self.entities.len() || to >= self.entities.len() || from == to {
            return;
        }
        let entity = self.entities.remove(from);
        self.entities.insert(to, entity);
        self.rebuild_index();
    }

    /// Reparent an entity under a new parent (or to root if parent_id is None).
    pub fn reparent(&mut self, entity_id: &str, new_parent_id: Option<&str>) {
        // Remove from old parent's children
        let old_parent_id = self
            .get(entity_id)
            .and_then(|e| e.components.parent.clone());
        if let Some(ref old_pid) = old_parent_id {
            let old_pid = old_pid.clone();
            if let Some(old_parent) = self.get_mut(&old_pid) {
                old_parent.components.children.retain(|c| c != entity_id);
            }
        }

        // Set new parent
        let entity_id_owned = entity_id.to_string();
        if let Some(entity) = self.get_mut(entity_id) {
            entity.components.parent = new_parent_id.map(|s| s.to_string());
        }

        // Add to new parent's children
        if let Some(pid) = new_parent_id {
            let pid = pid.to_string();
            if let Some(parent) = self.get_mut(&pid)
                && !parent.components.children.contains(&entity_id_owned)
            {
                parent.components.children.push(entity_id_owned);
            }
        }
    }

    /// Get all root entities (entities with no parent).
    pub fn get_roots(&self) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|e| e.components.parent.is_none())
            .collect()
    }

    /// Get children of an entity by ID.
    pub fn get_children(&self, parent_id: &str) -> Vec<&Entity> {
        self.entities
            .iter()
            .filter(|e| e.components.parent.as_deref() == Some(parent_id))
            .collect()
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

    /// Get entity index by ID.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.id_index.get(id).copied()
    }

    /// Rebuild the ID index (call after deserialization or mutations).
    pub fn rebuild_index(&mut self) {
        self.id_index.clear();
        for (idx, entity) in self.entities.iter().enumerate() {
            self.id_index.insert(entity.id.clone(), idx);
        }
    }

    /// Get entities sorted by layer order and z-index.
    pub fn sorted_by_layer(&self) -> Vec<&Entity> {
        let mut sorted: Vec<&Entity> = self
            .entities
            .iter()
            .filter(|e| e.resolved.visible)
            .collect();
        sorted.sort_by(|a, b| {
            a.resolved.layer.cmp(&b.resolved.layer).then(
                a.resolved
                    .z_index
                    .partial_cmp(&b.resolved.z_index)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        sorted
    }
}
