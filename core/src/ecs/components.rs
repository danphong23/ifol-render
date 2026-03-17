//! All component types available to entities.

use crate::color::Color4;
use crate::types::{Keyframe, Vec2};
use serde::{Deserialize, Serialize};

// ══════════════════════════════════════
// Source Components
// ══════════════════════════════════════

/// Video file source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSource {
    pub path: String,
    #[serde(default)]
    pub trim_start: f64,
    #[serde(default)]
    pub trim_end: Option<f64>,
    #[serde(default = "default_playback_rate")]
    pub playback_rate: f64,
}

fn default_playback_rate() -> f64 {
    1.0
}

/// Image file source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    pub path: String,
}

/// Text source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSource {
    pub content: String,
    #[serde(default = "default_font")]
    pub font: String,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "Color4::white")]
    pub color: Color4,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
}

fn default_font() -> String {
    "Inter".into()
}
fn default_font_size() -> f32 {
    48.0
}

/// Solid color fill source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorSource {
    pub color: Color4,
}

// ══════════════════════════════════════
// Layout & Timing Components
// ══════════════════════════════════════

/// Timeline placement of an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    /// Start time on the global timeline (seconds).
    pub start_time: f64,
    /// Duration (seconds).
    pub duration: f64,
    /// Layer/track index (higher = on top).
    #[serde(default)]
    pub layer: i32,
}

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
        }
    }
}

// ══════════════════════════════════════
// Visual Components
// ══════════════════════════════════════

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

/// Animation component — collection of keyframes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Animation {
    pub keyframes: Vec<Keyframe>,
}

impl Animation {
    /// Evaluate a property at the given time (relative to entity start).
    pub fn evaluate(&self, property: &str, time: f64) -> Option<f64> {
        let relevant: Vec<&Keyframe> = self
            .keyframes
            .iter()
            .filter(|k| k.property == property)
            .collect();

        if relevant.is_empty() {
            return None;
        }

        // Before first keyframe
        if time <= relevant[0].time {
            return Some(relevant[0].value);
        }

        // After last keyframe
        if time >= relevant.last().unwrap().time {
            return Some(relevant.last().unwrap().value);
        }

        // Interpolate between keyframes
        for window in relevant.windows(2) {
            let (a, b) = (window[0], window[1]);
            if time >= a.time && time < b.time {
                let t = ((time - a.time) / (b.time - a.time)) as f32;
                let eased = a.easing.evaluate(t);
                return Some(a.value + (b.value - a.value) * eased as f64);
            }
        }

        None
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
