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

// ══════════════════════════════════════
// V2 Protocol Formats
// ══════════════════════════════════════

/// The full definition of an ifol-render V2 project/scene.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneV2 {
    /// A registry of all assets used in the scene.
    #[serde(default)]
    pub assets: std::collections::HashMap<String, AssetDef>,
    /// Flat list of entities — components define identity (pure ECS).
    #[serde(default)]
    pub entities: Vec<EntityV2>,
}

/// Asset types supported by the V2 engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AssetDef {
    Video { url: String },
    Image { url: String },
    Font { url: String },
    Audio { url: String },
}

/// Material/Shader attached to an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialV2 {
    pub shader_id: String,
    #[serde(default)]
    pub float_uniforms: std::collections::HashMap<String, FloatTrack>,
    #[serde(default)]
    pub string_uniforms: std::collections::HashMap<String, StringTrack>,
}

/// A single entity in the scene (pure ECS — components define identity).
/// Adding `camera` makes it a camera. Adding `video_source` + `transform` makes it a video layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityV2 {
    pub id: String,

    /// All components attached to this entity, captured dynamically from JSON properties.
    #[serde(flatten)]
    pub components: std::collections::HashMap<String, serde_json::Value>,
}

/// Defines the absolute start and end time (in seconds).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Lifespan {
    pub start: f64,
    pub end: f64,
}

impl Default for Lifespan {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: f64::MAX,
        }
    }
}

impl Lifespan {
    pub fn contains(&self, time: f64) -> bool {
        time >= self.start && time < self.end
    }
}

// ══════════════════════════════════════
// Interpolation System
// ══════════════════════════════════════

/// Interpolation mode between keyframes.
///
/// Core provides only 4 fundamental primitives. Any custom easing
/// (ease-in, ease-out, steps, spring, etc.) should be defined at the
/// app/frontend level by mapping to CubicBezier control points or
/// by generating additional keyframes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Interpolation {
    /// Constant value — holds until next keyframe.
    Hold,
    /// Straight line between keyframes.
    Linear,
    /// CSS-style cubic bezier timing curve.
    /// x1,y1,x2,y2 define the two control points of the timing function.
    /// This is the universal easing primitive — covers all standard curves:
    ///   ease-in:     (0.42, 0.0,  1.0,  1.0)
    ///   ease-out:    (0.0,  0.0,  0.58, 1.0)
    ///   ease-in-out: (0.42, 0.0,  0.58, 1.0)
    CubicBezier {
        #[serde(default = "default_cb_x1")] x1: f32,
        #[serde(default)] y1: f32,
        #[serde(default = "default_cb_x2")] x2: f32,
        #[serde(default = "default_cb_y2")] y2: f32,
    },
    /// AE-style outgoing tangent handle (paired with next keyframe's in tangent).
    Bezier {
        /// Outgoing tangent time offset (0.0–1.0 normalized to segment duration)
        #[serde(default = "default_tangent")] out_x: f32,
        /// Outgoing tangent value offset (normalized to value range)
        #[serde(default)] out_y: f32,
        /// Incoming tangent time offset for this keyframe (from previous segment)
        #[serde(default = "default_tangent")] in_x: f32,
        /// Incoming tangent value offset
        #[serde(default)] in_y: f32,
    },
}

fn default_cb_x1() -> f32 { 0.42 }
fn default_cb_x2() -> f32 { 0.58 }
fn default_cb_y2() -> f32 { 1.0 }
fn default_tangent() -> f32 { 0.33 }

impl Default for Interpolation {
    fn default() -> Self { Interpolation::Linear }
}

/// Evaluate a cubic bezier timing function at normalized time t.
/// Returns the eased progress (0.0–1.0).
fn cubic_bezier_eval(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    // Newton-Raphson to solve for parameter u where bezier_x(u) = t
    let mut u = t; // initial guess
    for _ in 0..8 {
        let bx = 3.0 * (1.0 - u).powi(2) * u * x1
               + 3.0 * (1.0 - u) * u.powi(2) * x2
               + u.powi(3);
        let dx = 3.0 * (1.0 - u).powi(2) * x1
               + 6.0 * (1.0 - u) * u * (x2 - x1)
               + 3.0 * u.powi(2) * (1.0 - x2);
        if dx.abs() < 1e-7 { break; }
        u -= (bx - t) / dx;
        u = u.clamp(0.0, 1.0);
    }
    // Evaluate y at found parameter u
    3.0 * (1.0 - u).powi(2) * u * y1
    + 3.0 * (1.0 - u) * u.powi(2) * y2
    + u.powi(3)
}

/// A single keyframe with full interpolation control.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Keyframe {
    pub time: f64,
    pub value: f32,
    /// Interpolation mode to the NEXT keyframe. Default: Linear.
    #[serde(default)]
    pub interpolation: Interpolation,
}

/// Track of keyframes with interpolation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FloatTrack {
    #[serde(default)]
    pub keyframes: Vec<Keyframe>,
}

impl FloatTrack {
    /// Evaluates the property value at the given time using the keyframe interpolation.
    pub fn evaluate(&self, time: f64, default_val: f32) -> f32 {
        if self.keyframes.is_empty() {
            return default_val;
        }
        
        // Before the first keyframe
        if time <= self.keyframes[0].time {
            return self.keyframes[0].value;
        }
        
        // After the last keyframe
        let last = self.keyframes.last().unwrap();
        if time >= last.time {
            return last.value;
        }
        
        // Find bounding keyframes and interpolate
        for window in self.keyframes.windows(2) {
            let a = &window[0];
            let b = &window[1];
            if time >= a.time && time < b.time {
                let t = ((time - a.time) / (b.time - a.time)) as f32;
                let eased_t = match a.interpolation {
                    Interpolation::Hold => 0.0,
                    Interpolation::Linear => t,
                    Interpolation::CubicBezier { x1, y1, x2, y2 } => {
                        cubic_bezier_eval(t, x1, y1, x2, y2)
                    }
                    Interpolation::Bezier { out_x, out_y, .. } => {
                        // Use outgoing tangent of 'a' and incoming tangent of 'b'
                        let (in_x, in_y) = match b.interpolation {
                            Interpolation::Bezier { in_x, in_y, .. } => (in_x, in_y),
                            _ => (0.67, 0.0),
                        };
                        cubic_bezier_eval(t, out_x, out_y, 1.0 - in_x, 1.0 - in_y)
                    }
                };
                return a.value + (b.value - a.value) * eased_t;
            }
        }
        
        default_val
    }
}

/// Animated spatial transform properties.
/// Position, rotation, scale, anchor — NO width/height (size comes from source component).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransformTrack {
    #[serde(default)]
    pub x: FloatTrack,
    #[serde(default)]
    pub y: FloatTrack,
    #[serde(default)]
    pub rotation: FloatTrack,
    /// Anchor X (0.0–1.0 normalized, 0.0 = left edge). Default: 0.0
    #[serde(default)]
    pub anchor_x: FloatTrack,
    /// Anchor Y (0.0–1.0 normalized, 0.0 = top edge). Default: 0.0
    #[serde(default)]
    pub anchor_y: FloatTrack,
    /// Scale X multiplier. Default: 1.0 (no scaling)
    #[serde(default)]
    pub scale_x: FloatTrack,
    /// Scale Y multiplier. Default: 1.0 (no scaling)
    #[serde(default)]
    pub scale_y: FloatTrack,
    // ── Legacy compat: silently accept but ignore old width/height fields ──
    #[serde(default, skip_serializing)]
    pub width: FloatTrack,
    #[serde(default, skip_serializing)]
    pub height: FloatTrack,
}

/// A step-interpolated keyframe for string values (Enums, Texts, Shader IDs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringKeyframe {
    pub time: f64,
    pub value: String,
}

/// Track of text/string keyframes using Step interpolation.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StringTrack {
    #[serde(default)]
    pub keyframes: Vec<StringKeyframe>,
}

impl StringTrack {
    /// Evaluates the string value at a given time using step interpolation (holds previous value).
    pub fn evaluate<'a>(&'a self, time: f64, default_val: &'a str) -> &'a str {
        if self.keyframes.is_empty() {
            return default_val;
        }

        if time < self.keyframes[0].time {
            return default_val;
        }

        // Find the most recent keyframe that is <= current time
        let mut last_val: &str = &self.keyframes[0].value;
        for kf in &self.keyframes {
            if time >= kf.time {
                last_val = &kf.value;
            } else {
                break;
            }
        }
        
        last_val
    }
}

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

