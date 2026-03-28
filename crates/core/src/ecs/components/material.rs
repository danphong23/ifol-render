use serde::{Deserialize, Serialize};

/// Color adjustments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorAdjust {
    #[serde(default)]
    pub brightness: f32,
    #[serde(default = "one")]
    pub contrast: f32,
    #[serde(default = "one")]
    pub saturation: f32,
    #[serde(default)]
    pub hue: f32,
    /// Color temperature in Kelvin (6500 = neutral).
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn one() -> f32 {
    1.0
}
fn default_temperature() -> f32 {
    6500.0
}

impl Default for ColorAdjust {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            hue: 0.0,
            temperature: 6500.0,
        }
    }
}

/// An effect applied to an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effect {
    /// Effect type name (maps to shader registry).
    #[serde(rename = "type")]
    pub effect_type: String,
    /// Effect parameters.
    #[serde(default)]
    pub params: std::collections::HashMap<String, serde_json::Value>,
}

/// Runtime ECS Material Component used by RenderGraph.
/// Derived from Scene definition combined with effect registry defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaterialComponent {
    pub shader_id: String,
    /// Flat list of evaluated uniforms ready for WGPU
    #[serde(default)]
    pub uniforms: Vec<f32>,
    /// How much extra padding (in pixels) this effect requires
    #[serde(default)]
    pub padding: f32,
    /// Explicit texture input keys/names
    #[serde(default)]
    pub inputs: Vec<String>,
}
