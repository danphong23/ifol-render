use serde::{Deserialize, Serialize};
use crate::color::Color4;

/// Video file source — references an asset by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSource {
    /// Asset registry key (maps to SceneV2.assets).
    pub asset_id: String,
    #[serde(default)]
    pub trim_start: f64,
    #[serde(default)]
    pub trim_end: Option<f64>,
    /// Native video width (pixels). Frontend fills after probe.
    #[serde(default)]
    pub intrinsic_width: f32,
    /// Native video height (pixels).
    #[serde(default)]
    pub intrinsic_height: f32,
    /// Total source duration (seconds).
    #[serde(default)]
    pub duration: f64,
    /// Source fps.
    #[serde(default = "default_fps")]
    pub fps: f64,
}

fn default_fps() -> f64 { 30.0 }

/// Image file source — references an asset by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSource {
    pub asset_id: String,
    #[serde(default)]
    pub intrinsic_width: f32,
    #[serde(default)]
    pub intrinsic_height: f32,
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

fn default_font() -> String { "Inter".into() }
fn default_font_size() -> f32 { 48.0 }

/// Solid color fill source.
/// Display size comes from entity-level `Rect` component, not here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorSource {
    pub color: Color4,
}

/// Audio source — references an asset by ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSource {
    pub asset_id: String,
    #[serde(default)]
    pub trim_start: f64,
    #[serde(default)]
    pub trim_end: Option<f64>,
    #[serde(default)]
    pub duration: f64,
}
