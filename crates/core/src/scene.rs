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
    #[serde(default)]
    pub output_color_space: ColorSpace,
    /// Whether HDR (Rgba16Float) rendering is enabled for the pipeline
    #[serde(default)]
    pub hdr_enabled: bool,
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

pub use crate::schema::tracks::*;
pub use crate::schema::v2::*;

// ══════════════════════════════════════
// Unit Tests
// ══════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper: build a FloatTrack quickly ──
    fn track(kfs: &[(f64, f32)]) -> FloatTrack {
        FloatTrack {
            keyframes: kfs.iter().map(|&(t, v)| Keyframe {
                time: t, value: v, interpolation: Interpolation::Linear,
            }).collect(),
        }
    }

    fn track_with_interp(kfs: &[(f64, f32, Interpolation)]) -> FloatTrack {
        FloatTrack {
            keyframes: kfs.iter().map(|&(t, v, ref ip)| Keyframe {
                time: t, value: v, interpolation: ip.clone(),
            }).collect(),
        }
    }

    // ══════════════════════════════════════
    // FloatTrack::evaluate
    // ══════════════════════════════════════

    #[test]
    fn float_track_empty_returns_default() {
        let t = FloatTrack { keyframes: vec![] };
        assert_eq!(t.evaluate(0.0, 42.0), 42.0);
        assert_eq!(t.evaluate(5.0, -1.0), -1.0);
    }

    #[test]
    fn float_track_single_keyframe_constant() {
        let t = track(&[(0.0, 100.0)]);
        assert_eq!(t.evaluate(-1.0, 0.0), 100.0); // before
        assert_eq!(t.evaluate(0.0, 0.0), 100.0);  // at
        assert_eq!(t.evaluate(5.0, 0.0), 100.0);  // after
        assert_eq!(t.evaluate(999.0, 0.0), 100.0); // way after
    }

    #[test]
    fn float_track_before_first_returns_first_value() {
        let t = track(&[(2.0, 50.0), (5.0, 100.0)]);
        assert_eq!(t.evaluate(0.0, 999.0), 50.0);
        assert_eq!(t.evaluate(1.99, 999.0), 50.0);
        assert_eq!(t.evaluate(-10.0, 999.0), 50.0);
    }

    #[test]
    fn float_track_after_last_returns_last_value() {
        let t = track(&[(0.0, 10.0), (2.0, 50.0)]);
        assert_eq!(t.evaluate(2.0, 999.0), 50.0);
        assert_eq!(t.evaluate(3.0, 999.0), 50.0);
        assert_eq!(t.evaluate(100.0, 999.0), 50.0);
    }

    #[test]
    fn float_track_linear_interpolation() {
        let t = track(&[(0.0, 0.0), (4.0, 100.0)]);
        assert!((t.evaluate(0.0, 0.0) - 0.0).abs() < 0.01);
        assert!((t.evaluate(1.0, 0.0) - 25.0).abs() < 0.01);
        assert!((t.evaluate(2.0, 0.0) - 50.0).abs() < 0.01);
        assert!((t.evaluate(3.0, 0.0) - 75.0).abs() < 0.01);
        assert!((t.evaluate(4.0, 0.0) - 100.0).abs() < 0.01);
    }

    #[test]
    fn float_track_linear_three_keyframes() {
        // 0→100 over [0,2], then 100→0 over [2,4]
        let t = track(&[(0.0, 0.0), (2.0, 100.0), (4.0, 0.0)]);
        assert!((t.evaluate(1.0, 0.0) - 50.0).abs() < 0.01);
        assert!((t.evaluate(2.0, 0.0) - 100.0).abs() < 0.01);
        assert!((t.evaluate(3.0, 0.0) - 50.0).abs() < 0.01);
    }

    #[test]
    fn float_track_linear_negative_values() {
        let t = track(&[(0.0, -100.0), (2.0, 100.0)]);
        assert!((t.evaluate(1.0, 0.0) - 0.0).abs() < 0.01);     // midpoint
        assert!((t.evaluate(0.5, 0.0) - (-50.0)).abs() < 0.01);  // quarter
    }

    #[test]
    fn float_track_hold_interpolation() {
        let t = track_with_interp(&[
            (0.0, 10.0, Interpolation::Hold),
            (2.0, 50.0, Interpolation::Hold),
            (4.0, 100.0, Interpolation::Linear),
        ]);
        // Hold: keeps previous value until next keyframe
        assert_eq!(t.evaluate(0.0, 0.0), 10.0);
        assert_eq!(t.evaluate(0.5, 0.0), 10.0);
        assert_eq!(t.evaluate(1.99, 0.0), 10.0);
        // At t=2.0 we're now in the [2,4] segment with Hold
        assert_eq!(t.evaluate(2.0, 0.0), 50.0);
        assert_eq!(t.evaluate(3.0, 0.0), 50.0);
        assert_eq!(t.evaluate(3.99, 0.0), 50.0);
    }

    #[test]
    fn float_track_cubic_bezier_ease_in_out() {
        let t = track_with_interp(&[
            (0.0, 0.0, Interpolation::CubicBezier { x1: 0.42, y1: 0.0, x2: 0.58, y2: 1.0 }),
            (2.0, 100.0, Interpolation::Linear),
        ]);
        let mid = t.evaluate(1.0, 0.0);
        // Ease-in-out: at midpoint the value should still be ~50 (symmetric easing)
        assert!((mid - 50.0).abs() < 5.0, "cubic_bezier midpoint was {} (expected ~50)", mid);
        // At quarter time, ease-in should be slower → less than 25
        let quarter = t.evaluate(0.5, 0.0);
        assert!(quarter < 20.0, "ease-in at quarter should be < 20, was {}", quarter);
        // At three-quarter time, ease-out slows → more than 75
        let three_q = t.evaluate(1.5, 0.0);
        assert!(three_q > 80.0, "ease-out at 3/4 should be > 80, was {}", three_q);
    }

    #[test]
    fn float_track_cubic_bezier_linear_equivalent() {
        // CubicBezier(0.0, 0.0, 1.0, 1.0) should behave like linear
        let t = track_with_interp(&[
            (0.0, 0.0, Interpolation::CubicBezier { x1: 0.0, y1: 0.0, x2: 1.0, y2: 1.0 }),
            (2.0, 100.0, Interpolation::Linear),
        ]);
        assert!((t.evaluate(0.5, 0.0) - 25.0).abs() < 2.0);
        assert!((t.evaluate(1.0, 0.0) - 50.0).abs() < 2.0);
        assert!((t.evaluate(1.5, 0.0) - 75.0).abs() < 2.0);
    }

    #[test]
    fn float_track_bezier_ae_tangents() {
        let t = track_with_interp(&[
            (0.0, 0.0, Interpolation::Bezier { out_x: 0.33, out_y: 0.0, in_x: 0.33, in_y: 0.0 }),
            (2.0, 100.0, Interpolation::Bezier { out_x: 0.33, out_y: 0.0, in_x: 0.33, in_y: 0.0 }),
        ]);
        let mid = t.evaluate(1.0, 0.0);
        // With default tangents (0.33, 0), should be roughly S-curve
        assert!(mid > 30.0 && mid < 70.0, "bezier midpoint was {} (expected ~50)", mid);
    }

    // ══════════════════════════════════════
    // StringTrack::evaluate
    // ══════════════════════════════════════

    #[test]
    fn string_track_empty_returns_default() {
        let t = StringTrack { keyframes: vec![] };
        assert_eq!(t.evaluate(0.0, "normal"), "normal");
        assert_eq!(t.evaluate(99.0, "fallback"), "fallback");
    }

    #[test]
    fn string_track_step_interpolation() {
        let t = StringTrack { keyframes: vec![
            StringKeyframe { time: 0.0, value: "normal".into() },
            StringKeyframe { time: 2.0, value: "multiply".into() },
            StringKeyframe { time: 5.0, value: "screen".into() },
        ]};
        assert_eq!(t.evaluate(-1.0, "default"), "default"); // before first
        assert_eq!(t.evaluate(0.0, "default"), "normal");   // at first
        assert_eq!(t.evaluate(1.0, "default"), "normal");   // between 1st-2nd → holds 1st
        assert_eq!(t.evaluate(2.0, "default"), "multiply"); // at second
        assert_eq!(t.evaluate(3.5, "default"), "multiply"); // between 2nd-3rd
        assert_eq!(t.evaluate(5.0, "default"), "screen");   // at third
        assert_eq!(t.evaluate(99.0, "default"), "screen");  // after last
    }

    // ══════════════════════════════════════
    // Lifespan::contains
    // ══════════════════════════════════════

    #[test]
    fn lifespan_contains() {
        let ls = Lifespan { start: 2.0, end: 5.0 };
        assert!(!ls.contains(1.99));  // before
        assert!(ls.contains(2.0));    // at start (inclusive)
        assert!(ls.contains(3.5));    // middle
        assert!(!ls.contains(5.0));   // at end (exclusive)
        assert!(!ls.contains(5.01));  // after
    }
}

