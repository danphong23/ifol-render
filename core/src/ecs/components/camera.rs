//! Camera component for viewport control.

use crate::types::Vec2;
use serde::{Deserialize, Serialize};

/// Camera component for viewport pan/zoom.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Camera {
    /// Camera center position (world coordinates).
    #[serde(default)]
    pub position: Vec2,
    /// Zoom level (1.0 = 100%).
    #[serde(default = "one")]
    pub zoom: f32,
    /// Rotation in radians.
    #[serde(default)]
    pub rotation: f32,
}

fn one() -> f32 {
    1.0
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec2 { x: 0.0, y: 0.0 },
            zoom: 1.0,
            rotation: 0.0,
        }
    }
}
