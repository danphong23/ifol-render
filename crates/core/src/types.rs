//! Core datatypes used throughout the engine.

use serde::{Deserialize, Serialize};

/// 2D vector.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

/// 3D vector.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// 4D vector.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

/// 4x4 transformation matrix (column-major).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Mat4(pub [f32; 16]);

impl Default for Mat4 {
    fn default() -> Self {
        Self::identity()
    }
}

impl Mat4 {
    pub fn identity() -> Self {
        Self([
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ])
    }

    /// Create a 2D transform matrix from position, scale, rotation, and anchor.
    pub fn from_2d(position: Vec2, scale: Vec2, rotation: f32, anchor: Vec2) -> Self {
        let cos = rotation.cos();
        let sin = rotation.sin();

        // Translate to anchor → scale → rotate → translate to position
        let tx = position.x - anchor.x * scale.x * cos + anchor.y * scale.y * sin;
        let ty = position.y - anchor.x * scale.x * sin - anchor.y * scale.y * cos;

        Self([
            scale.x * cos,  scale.x * sin, 0.0, 0.0,
            -scale.y * sin, scale.y * cos, 0.0, 0.0,
            0.0,            0.0,           1.0, 0.0,
            tx,             ty,            0.0, 1.0,
        ])
    }
}

/// Time range in seconds.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}

impl TimeRange {
    pub fn new(start: f64, end: f64) -> Self {
        Self { start, end }
    }

    pub fn duration(&self) -> f64 {
        self.end - self.start
    }

    pub fn contains(&self, time: f64) -> bool {
        time >= self.start && time < self.end
    }
}

/// Easing function type for animations.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    /// Evaluate the easing function at time t (0.0..1.0).
    pub fn evaluate(&self, t: f32) -> f32 {
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::CubicBezier(x1, y1, x2, y2) => {
                // Simplified cubic bezier (TODO: proper Newton-Raphson)
                let _ = (x1, y1, x2, y2);
                t
            }
        }
    }
}

/// A single keyframe in an animation curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    /// Time relative to entity start (seconds).
    pub time: f64,
    /// Target property name (e.g., "opacity", "transform.position.x").
    pub property: String,
    /// Value at this keyframe.
    pub value: f64,
    /// Easing to next keyframe.
    #[serde(default)]
    pub easing: Easing,
}
