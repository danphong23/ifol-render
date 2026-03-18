//! Visual appearance components — blend mode, color adjustments.

use serde::{Deserialize, Serialize};

/// Blend mode for compositing layers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    SoftLight,
    Add,
    Difference,
}

impl BlendMode {
    /// All available blend modes for UI dropdowns.
    pub const ALL: &'static [BlendMode] = &[
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::SoftLight,
        BlendMode::Add,
        BlendMode::Difference,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Multiply => "Multiply",
            BlendMode::Screen => "Screen",
            BlendMode::Overlay => "Overlay",
            BlendMode::SoftLight => "Soft Light",
            BlendMode::Add => "Add",
            BlendMode::Difference => "Difference",
        }
    }
}

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
