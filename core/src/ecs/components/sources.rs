//! Source components — what visual/audio content an entity represents.

use crate::color::Color4;
use crate::types::Vec2;
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
    /// Original pixel size (auto-detected when loaded).
    /// Used to compute unit size: unit_size = pixel_size / PPU.
    #[serde(default)]
    pub pixel_size: Option<[u32; 2]>,
}

fn default_playback_rate() -> f64 {
    1.0
}

/// Image file source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSource {
    pub path: String,
    /// Original pixel size (auto-detected when loaded).
    /// Used to compute unit size: unit_size = pixel_size / PPU.
    #[serde(default)]
    pub pixel_size: Option<[u32; 2]>,
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
    /// Rasterized text pixel size (auto-computed after rasterization).
    #[serde(skip)]
    pub pixel_size: Option<[u32; 2]>,
}

fn default_font() -> String {
    "NotoSans".into()
}
fn default_font_size() -> f32 {
    48.0
}

/// Solid color fill source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorSource {
    pub color: Color4,
    /// Size in units. If None, fills the entire canvas.
    #[serde(default)]
    pub size: Option<Vec2>,
}

/// Audio file source.
///
/// Audio entities are not rendered visually but appear on the timeline.
/// They are mixed into the export via FFmpeg.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSource {
    pub path: String,
    /// Trim start (seconds from beginning of the audio file).
    #[serde(default)]
    pub trim_start: f64,
    /// Trim end (None = play to end).
    #[serde(default)]
    pub trim_end: Option<f64>,
    /// Volume (0.0 = silent, 1.0 = full, >1.0 = amplify).
    #[serde(default = "default_volume")]
    pub volume: f64,
    /// Fade in duration (seconds).
    #[serde(default)]
    pub fade_in: f64,
    /// Fade out duration (seconds).
    #[serde(default)]
    pub fade_out: f64,
    /// Whether to loop the audio.
    #[serde(default)]
    pub loop_audio: bool,
}

fn default_volume() -> f64 {
    1.0
}
