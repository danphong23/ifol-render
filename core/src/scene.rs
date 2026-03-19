//! Scene API — the entry point for consumers.
//!
//! Consumers provide a `SceneDescription` (JSON). The engine parses it into
//! an internal `World` with entities and components.

use crate::color::ColorSpace;
use crate::ecs::{Entity, World};
use serde::{Deserialize, Serialize};

/// Top-level scene description — the API contract between consumers and the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDescription {
    /// Schema version.
    #[serde(default = "default_version")]
    pub version: String,

    /// Global render settings.
    pub settings: RenderSettings,

    /// All entities in the scene.
    pub entities: Vec<Entity>,

    /// Custom shaders (optional).
    #[serde(default)]
    pub shaders: Vec<ShaderDef>,
}

fn default_version() -> String {
    "1.0".into()
}

/// Global render settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_fps")]
    pub fps: f64,
    #[serde(default)]
    pub duration: f64,
    /// Working color space (default: LinearSrgb).
    #[serde(default)]
    pub color_space: ColorSpace,
    /// Output color space (default: Srgb).
    #[serde(default)]
    pub output_color_space: ColorSpace,
    /// Pixels Per Unit — conversion factor for unit-based coordinates.
    /// Default 100: 1 unit = 100 pixels. An 1920x1080 image → 19.2 x 10.8 units.
    /// When user changes PPU or resolution, entities keep their unit size.
    #[serde(default = "default_ppu")]
    pub ppu: f32,
    /// Preview resolution scale (0.25 = quarter, 0.5 = half, 1.0 = full).
    /// Only affects preview rendering, export always uses full resolution.
    #[serde(default = "default_preview_scale")]
    pub preview_scale: f32,
    /// Scene background color (default: transparent black).
    #[serde(default)]
    pub background_color: crate::color::Color4,
}

fn default_ppu() -> f32 {
    100.0
}
fn default_preview_scale() -> f32 {
    1.0
}

fn default_fps() -> f64 {
    30.0
}

/// A custom shader definition provided by the consumer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderDef {
    /// Unique shader ID.
    pub id: String,
    /// WGSL source code.
    pub code: String,
    /// Uniform declarations.
    #[serde(default)]
    pub uniforms: std::collections::HashMap<String, UniformDef>,
}

/// Shader uniform definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UniformDef {
    /// Data type: "f32", "vec2", "vec3", "vec4", "mat4".
    #[serde(rename = "type")]
    pub data_type: String,
    /// Default value.
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    /// Auto-binding (e.g., "frameTime", "globalTime", "normalizedTime").
    #[serde(default)]
    pub binding: Option<String>,
}

impl SceneDescription {
    /// Parse from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Create a SceneDescription from a World and RenderSettings.
    pub fn from_world(world: &World, settings: &RenderSettings) -> Self {
        Self {
            version: "1.0".into(),
            settings: settings.clone(),
            entities: world.entities.clone(),
            shaders: vec![],
        }
    }

    /// Convert to a World for ECS processing.
    pub fn into_world(self) -> World {
        let mut world = World::new();
        for entity in self.entities {
            world.add_entity(entity);
        }
        world.rebuild_index();
        world
    }
}
