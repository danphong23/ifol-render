//! Effect components — per-entity effect stack.

use serde::{Deserialize, Serialize};

/// A single effect applied to an entity.
///
/// Effect type name maps to the render tool's `EffectRegistry`.
/// Params are passed through to the effect shader as uniforms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effect {
    /// Effect type name (maps to shader registry).
    #[serde(rename = "type")]
    pub effect_type: String,
    /// Effect parameters.
    #[serde(default)]
    pub params: std::collections::HashMap<String, serde_json::Value>,
}

/// Ordered list of effects applied to an entity.
///
/// Effects are processed in order (first to last).
/// In the JSON scene format:
/// ```json
/// "effects": [
///   { "type": "blur", "params": { "radius": 5.0 } },
///   { "type": "color_grade", "params": { "brightness": 0.1, "contrast": 1.2 } }
/// ]
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EffectStack {
    #[serde(default)]
    pub effects: Vec<Effect>,
}

impl EffectStack {
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }
}
