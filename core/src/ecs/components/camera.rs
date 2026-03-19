//! Camera component for viewport control.

use crate::types::{Mat4, Vec2};
use serde::{Deserialize, Serialize};

/// Camera component for viewport pan/zoom.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Camera {
    /// Camera center position (world coordinates in units).
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

impl Camera {
    /// Compute the camera's VIEW matrix (inverse of camera transform).
    ///
    /// This matrix is applied to all entity world matrices before clip conversion.
    /// Effect: zoom scales entities, position offsets them (pan).
    pub fn to_view_matrix(&self) -> Mat4 {
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();
        let z = self.zoom;

        // Inverse of camera transform:
        // 1. Translate by -position
        // 2. Rotate by -rotation
        // 3. Scale by zoom
        Mat4([
            z * cos,
            z * sin,
            0.0,
            0.0,
            -z * sin,
            z * cos,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            -self.position.x * z * cos + self.position.y * z * sin,
            -self.position.x * z * sin - self.position.y * z * cos,
            0.0,
            1.0,
        ])
    }
}
