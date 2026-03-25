//! ECS (Entity-Component-System) architecture.
//!
//! Pure ECS: entities are blank containers, components define identity.
//! Adding CameraComponent → entity becomes a camera.
//! Adding VideoSource + Transform → entity becomes a video layer.

pub mod components;
pub mod pipeline;
pub mod systems;

use crate::scene::{AssetDef, FloatTrack, Lifespan, MaterialV2, StringTrack, TransformTrack};
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
    pub components: Components,

    /// Runtime-resolved data (not serialized).
    #[serde(skip)]
    pub resolved: ResolvedState,
}

/// All possible components an entity can have.
/// Presence of a component defines behavior — entities are not typed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Components {
    // ── Sources (what to display/play — mutually exclusive for visual) ──
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

    // ── Camera (presence = this entity is a camera) ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<components::CameraComponent>,

    // ── Spatial (world units) ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<TransformTrack>,

    // ── Display Rect (width/height + fit_mode) ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rect: Option<components::Rect>,

    // ── Composition (nested timeline group) ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composition: Option<components::Composition>,

    // ── Time ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifespan: Option<Lifespan>,
    /// Z-order layer index (higher = on top).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<i32>,

    // ── Rendering ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<FloatTrack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blend_mode: Option<StringTrack>,
    // Legacy compat: accept old fit_mode field but prefer rect.fit_mode
    #[serde(default, skip_serializing)]
    pub fit_mode: Option<StringTrack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<MaterialV2>,

    // ── Playback (legacy compat — prefer Composition for speed/trim) ──
    #[serde(default, skip_serializing)]
    pub playback_time: Option<FloatTrack>,
    #[serde(default, skip_serializing)]
    pub speed: Option<FloatTrack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<FloatTrack>,

    // ── Relations ──
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask_id: Option<String>,

    // ── Extensible (custom shader uniforms) ──
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub float_uniforms: HashMap<String, FloatTrack>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub string_uniforms: HashMap<String, StringTrack>,
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct World {
    pub entities: Vec<Entity>,
    /// Asset registry: asset_id → definition (URL/path).
    #[serde(default)]
    pub assets: HashMap<String, AssetDef>,
    /// Entity lookup by ID.
    #[serde(skip)]
    id_index: HashMap<EntityId, usize>,
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

    /// Get the URL for an asset by its ID.
    pub fn resolve_asset_url(&self, asset_id: &str) -> Option<&str> {
        self.assets.get(asset_id).map(|a| match a {
            AssetDef::Video { url } => url.as_str(),
            AssetDef::Image { url } => url.as_str(),
            AssetDef::Font { url } => url.as_str(),
            AssetDef::Audio { url } => url.as_str(),
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

    /// Find the first active camera entity.
    pub fn find_camera(&self, camera_id: &str) -> Option<&Entity> {
        if !camera_id.is_empty() {
            return self.get(camera_id).filter(|e| e.components.camera.is_some());
        }
        // Fallback: first visible camera
        self.entities.iter()
            .find(|e| e.resolved.visible && e.components.camera.is_some())
    }

    /// Build ECS World from a SceneV2 definition.
    pub fn load_scene(&mut self, scene: &crate::scene::SceneV2) {
        // Load asset registry
        self.assets = scene.assets.clone();

        // Load entities
        for ent_def in &scene.entities {
            let mut comps = Components::default();

            // Copy all components directly from the scene definition
            comps.video_source = ent_def.video_source.clone();
            comps.image_source = ent_def.image_source.clone();
            comps.text_source = ent_def.text_source.clone();
            comps.color_source = ent_def.color_source.clone();
            comps.audio_source = ent_def.audio_source.clone();
            comps.camera = ent_def.camera.clone();
            comps.transform = ent_def.transform.clone();
            comps.rect = ent_def.rect.clone();
            comps.composition = ent_def.composition.clone();
            comps.lifespan = ent_def.lifespan;
            comps.layer = ent_def.layer;
            comps.opacity = ent_def.opacity.clone();
            comps.blend_mode = ent_def.blend_mode.clone();
            comps.fit_mode = ent_def.fit_mode.clone();
            comps.materials = ent_def.materials.clone().unwrap_or_default();
            comps.playback_time = ent_def.playback_time.clone();
            comps.speed = ent_def.speed.clone();
            comps.volume = ent_def.volume.clone();
            comps.parent_id = ent_def.parent_id.clone();
            comps.mask_id = ent_def.mask_id.clone();
            comps.float_uniforms = ent_def.float_uniforms.clone().unwrap_or_default();
            comps.string_uniforms = ent_def.string_uniforms.clone().unwrap_or_default();

            self.add_entity(Entity {
                id: ent_def.id.clone(),
                components: comps,
                resolved: ResolvedState::default(),
            });
        }

        self.rebuild_index();
    }
}
