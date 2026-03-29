//! ECS (Entity-Component-System) architecture.
//!
//! Pure ECS: entities are blank containers, components define identity.
//! Adding CameraComponent → entity becomes a camera.
//! Adding VideoSource + Transform → entity becomes a video layer.

pub mod components;
pub mod pipeline;
pub mod systems;
pub mod typemap;
pub mod registry;
#[cfg(test)]
mod tests;

use crate::schema::v2::AssetDef;
use crate::time::EntityTime;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for an entity.
pub type EntityId = String;

/// An entity — a blank container for components.
/// Components define what it is and how it behaves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,

    /// Runtime-resolved data (not serialized).
    #[serde(skip)]
    pub resolved: ResolvedState,

    /// Runtime-only draw instructions (not serialized).
    #[serde(skip)]
    pub draw: components::DrawComponent,
}

/// Runtime-resolved state (computed by systems each frame).
#[derive(Debug, Clone)]
pub struct ResolvedState {
    /// Whether this entity is visible at the current time.
    pub visible: bool,
    // ── Resolved Transform (world units) ──
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
    pub anchor_x: f32,
    pub anchor_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    // ── Resolved Rect (display size = base × scale) ──
    pub width: f32,
    pub height: f32,
    pub intrinsic_width: f32,
    pub intrinsic_height: f32,
    pub aspect_ratio: f32,
    pub display_aspect: f32,
    pub fit_mode: components::FitMode,
    // ── Visual ──
    pub opacity: f32,
    pub volume: f32,
    pub blend_mode: components::BlendMode,
    pub color: [f32; 4],
    pub layer: i32,
    // ── Time ──
    pub time: EntityTime,
    /// The timeline time scope this entity belongs to.
    /// Root entities: scope_time = global_time.
    /// Children of Composition: scope_time = parent's content_time.
    pub scope_time: f64,
    /// Internal content time (for Composition entities only).
    /// content_time = local_time × speed + trim_start, with loop applied.
    pub content_time: f64,
    /// Time to seek media to (video/audio).
    pub playback_time: f64,
    pub speed: f32,
    /// Maximum internal duration for Composition entities (from children).
    pub max_duration: f64,
}

impl Default for ResolvedState {
    fn default() -> Self {
        Self {
            visible: false,
            x: 0.0, y: 0.0,
            rotation: 0.0,
            anchor_x: 0.0, anchor_y: 0.0,
            scale_x: 1.0, scale_y: 1.0,
            width: 100.0, height: 100.0,
            intrinsic_width: 0.0, intrinsic_height: 0.0,
            aspect_ratio: 1.0, display_aspect: 1.0,
            fit_mode: components::FitMode::Stretch,
            opacity: 1.0,
            volume: 1.0,
            blend_mode: components::BlendMode::Normal,
            color: [1.0, 1.0, 1.0, 1.0],
            layer: 0,
            time: EntityTime::default(),
            scope_time: 0.0,
            content_time: 0.0,
            playback_time: 0.0,
            speed: 1.0,
            max_duration: 0.0,
        }
    }
}

/// The world: collection of all entities + asset registry.
#[derive(Default, Serialize, Deserialize)]
pub struct World {
    pub entities: Vec<Entity>,
    /// Asset registry: asset_id → definition (URL/path).
    #[serde(default)]
    pub assets: HashMap<String, AssetDef>,
    /// Entity lookup by ID.
    #[serde(skip)]
    id_index: HashMap<EntityId, usize>,
    
    #[serde(skip)]
    pub storages: typemap::TypeMap,
    #[serde(skip)]
    pub registry: registry::ComponentRegistry,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_entity(&mut self, entity: Entity) {
        let idx = self.entities.len();
        self.id_index.insert(entity.id.clone(), idx);
        self.entities.push(entity);
    }

    pub fn get(&self, id: &str) -> Option<&Entity> {
        self.id_index
            .get(id)
            .and_then(|&idx| self.entities.get(idx))
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Entity> {
        self.id_index
            .get(id)
            .copied()
            .and_then(|idx| self.entities.get_mut(idx))
    }

    pub fn rebuild_index(&mut self) {
        self.id_index.clear();
        for (idx, entity) in self.entities.iter().enumerate() {
            self.id_index.insert(entity.id.clone(), idx);
        }
    }

    pub fn resolve_asset_url(&self, asset_id: &str) -> Option<&str> {
        self.assets.get(asset_id).map(|a| match a {
            AssetDef::Video { url } => url.as_str(),
            AssetDef::Image { url } => url.as_str(),
            AssetDef::Font { url } => url.as_str(),
            AssetDef::Audio { url } => url.as_str(),
            AssetDef::Shader { url } => url.as_str(),
        })
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

    pub fn find_camera(&self, camera_id: &str) -> Option<&Entity> {
        let storages = &self.storages;
        if !camera_id.is_empty() {
            return self.get(camera_id).filter(|e| storages.get_component::<crate::ecs::components::CameraComponent>(&e.id).is_some());
        }
        // Fallback: first visible camera
        self.entities.iter()
            .find(|e| e.resolved.visible && storages.get_component::<crate::ecs::components::CameraComponent>(&e.id).is_some())
    }

    pub fn add_component<T: 'static>(&mut self, entity_id: &str, component: T) {
        if self.storages.get::<HashMap<EntityId, T>>().is_none() {
            self.storages.insert(HashMap::<EntityId, T>::new());
        }
        let map = self.storages.get_mut::<HashMap<EntityId, T>>().unwrap();
        map.insert(entity_id.to_string(), component);
    }

    pub fn get_component<T: 'static>(&self, entity_id: &str) -> Option<&T> {
        self.storages.get::<HashMap<EntityId, T>>()
            .and_then(|map| map.get(entity_id))
    }
    
    pub fn get_component_mut<T: 'static>(&mut self, entity_id: &str) -> Option<&mut T> {
        self.storages.get_mut::<HashMap<EntityId, T>>()
            .and_then(|map| map.get_mut(entity_id))
    }

    /// Build ECS World from a SceneV2 definition.
    pub fn load_scene(&mut self, scene: &crate::scene::SceneV2) {
        // Load asset registry
        self.assets = scene.assets.clone();

        self.entities.clear();
        self.storages = typemap::TypeMap::new();

        // Load entities
        for ent_def in &scene.entities {
            let entity_id = ent_def.id.clone();
            
            self.add_entity(Entity {
                id: entity_id.clone(),
                resolved: ResolvedState::default(),
                draw: crate::ecs::components::DrawComponent::default(),
            });

            // Dynamically inject components using the registry
            for (key, value) in &ent_def.components {
                let loader_opt = self.registry.loaders.get(key).copied();
                if let Some(loader) = loader_opt {
                    if let Err(e) = loader(self, &entity_id, value) {
                        eprintln!("Failed to load component '{}' for entity '{}': {}", key, entity_id, e);
                    }
                } else {
                    println!("Warning: Unknown component '{}' for entity '{}'", key, entity_id);
                }
            }
        }

        self.rebuild_index();

        // ── Topological Sort (Parents before Children) ──
        let mut visited = std::collections::HashSet::new();
        let mut roots = Vec::new();
        let mut children_map: HashMap<String, Vec<String>> = HashMap::new();

        // Build relationships
        for ent in &self.entities {
            let mut is_root = true;
            if let Some(pid) = self.storages.get_component::<crate::ecs::components::meta::ParentId>(&ent.id) {
                if self.id_index.contains_key(&pid.0) {
                    is_root = false;
                    children_map.entry(pid.0.clone()).or_default().push(ent.id.clone());
                }
            }
            if is_root {
                roots.push(ent.id.clone());
            }
        }

        // Recursive flattening
        fn visit(
            id: &str, 
            cmap: &HashMap<String, Vec<String>>, 
            visited: &mut std::collections::HashSet<String>, 
            sorted_ids: &mut Vec<String>
        ) {
            if visited.insert(id.to_string()) {
                sorted_ids.push(id.to_string());
                if let Some(children) = cmap.get(id) {
                    for child in children {
                        visit(child, cmap, visited, sorted_ids);
                    }
                }
            }
        }

        let mut sorted_ids = Vec::with_capacity(self.entities.len());
        for root in roots {
            visit(&root, &children_map, &mut visited, &mut sorted_ids);
        }
        
        // Add any disconnected cycles
        for ent in &self.entities {
            if !visited.contains(&ent.id) {
                visit(&ent.id.clone(), &children_map, &mut visited, &mut sorted_ids);
            }
        }

        // Apply new order
        let mut id_to_entity: HashMap<String, Entity> = self.entities.drain(..).map(|e| (e.id.clone(), e)).collect();
        for id in sorted_ids {
            if let Some(ent) = id_to_entity.remove(&id) {
                self.entities.push(ent);
            }
        }
        self.rebuild_index();
    }
}
