//! Core datatypes used throughout the engine.

use serde::{Deserialize, Serialize};

/// 2D vector.
#[derive(
    Debug, Clone, Copy, Default, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable,
)]
#[repr(C)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

/// 3D vector.
#[derive(
    Debug, Clone, Copy, Default, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable,
)]
#[repr(C)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// 4D vector.
#[derive(
    Debug, Clone, Copy, Default, Serialize, Deserialize, bytemuck::Pod, bytemuck::Zeroable,
)]
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
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
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
            scale.x * cos,
            scale.x * sin,
            0.0,
            0.0,
            -scale.y * sin,
            scale.y * cos,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            tx,
            ty,
            0.0,
            1.0,
        ])
    }

    /// Multiply two 4x4 matrices (column-major): self * rhs
    pub fn mul(&self, rhs: &Mat4) -> Mat4 {
        let a = &self.0;
        let b = &rhs.0;
        let mut out = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0f32;
                for k in 0..4 {
                    sum += a[k * 4 + row] * b[col * 4 + k];
                }
                out[col * 4 + row] = sum;
            }
        }
        Mat4(out)
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
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier_ease(t, *x1, *y1, *x2, *y2),
        }
    }
}

/// Solve cubic bezier easing using Newton-Raphson iteration.
/// Control points: (0,0), (x1,y1), (x2,y2), (1,1)
fn cubic_bezier_ease(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    // First find the parameter `s` such that bezier_x(s) = t
    let mut s = t; // initial guess
    for _ in 0..8 {
        let x = bezier_sample(s, x1, x2) - t;
        let dx = bezier_derivative(s, x1, x2);
        if dx.abs() < 1e-7 {
            break;
        }
        s -= x / dx;
        s = s.clamp(0.0, 1.0);
    }
    // Then evaluate bezier_y(s)
    bezier_sample(s, y1, y2)
}

#[inline]
fn bezier_sample(t: f32, p1: f32, p2: f32) -> f32 {
    // B(t) = 3(1-t)²t·p1 + 3(1-t)t²·p2 + t³
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    3.0 * mt2 * t * p1 + 3.0 * mt * t2 * p2 + t3
}

#[inline]
fn bezier_derivative(t: f32, p1: f32, p2: f32) -> f32 {
    // B'(t) = 3(1-t)²·p1 + 6(1-t)t·(p2-p1) + 3t²·(1-p2)
    let mt = 1.0 - t;
    3.0 * mt * mt * p1 + 6.0 * mt * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
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

// ══════════════════════════════════════
// Media types (platform-agnostic data)
// ══════════════════════════════════════

/// Video metadata. Data-only struct used by all platforms.
/// On native, populated by `video::probe()` via ffprobe.
/// On WASM, populated by `MediaBackend::get_video_info()` via JS.
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_secs: f64,
    pub codec: String,
}

/// System information regarding hardware rendering and OS capabilities.
/// Data-only struct used by all platforms.
/// On native, populated by `sysinfo::SysInfo::probe()`.
/// On WASM, can be constructed with defaults.
#[derive(Debug, Clone)]
pub struct SysInfo {
    pub os: String,
    pub vendor_name: String,
    pub has_nvidia: bool,
    pub has_intel: bool,
    pub has_amd: bool,
    pub has_mac_hw: bool,
    pub ffmpeg_hw_encoders: Vec<String>,
}

impl Default for SysInfo {
    fn default() -> Self {
        Self {
            os: "unknown".into(),
            vendor_name: "unknown".into(),
            has_nvidia: false,
            has_intel: false,
            has_amd: false,
            has_mac_hw: false,
            ffmpeg_hw_encoders: Vec::new(),
        }
    }
}

