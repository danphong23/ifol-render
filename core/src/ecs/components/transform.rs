//! Spatial transform component.

use crate::types::Vec2;
use serde::{Deserialize, Serialize};

/// Spatial transform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    #[serde(default = "default_position")]
    pub position: Vec2,
    #[serde(default = "default_scale")]
    pub scale: Vec2,
    /// Rotation in radians.
    #[serde(default)]
    pub rotation: f32,
    #[serde(default = "default_anchor")]
    pub anchor: Vec2,
    /// Depth ordering within the same layer (higher = closer to camera).
    #[serde(default)]
    pub z_index: f32,
}

fn default_position() -> Vec2 {
    Vec2 { x: 0.0, y: 0.0 }
}
fn default_scale() -> Vec2 {
    Vec2 { x: 1.0, y: 1.0 }
}
fn default_anchor() -> Vec2 {
    Vec2 { x: 0.5, y: 0.5 }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: default_position(),
            scale: default_scale(),
            rotation: 0.0,
            anchor: default_anchor(),
            z_index: 0.0,
        }
    }
}
