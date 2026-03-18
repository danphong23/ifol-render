//! Source components — what visual content an entity represents.

use crate::color::Color4;
use serde::{Deserialize, Serialize};

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
