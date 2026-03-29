use serde::{Deserialize, Serialize};

/// Visual component.
///
/// Controls transparency, audio volume, and blend mode.
/// These are static fallback values, which can be animated via `AnimationComponent`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Visual {
    /// Opacity from 0.0 (transparent) to 1.0 (opaque).
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// Audio volume scale (1.0 = normal, 0.0 = muted).
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// CSS-like blend mode.
    #[serde(default = "default_blend_mode")]
    pub blend_mode: String,
}

fn default_opacity() -> f32 {
    1.0
}

fn default_volume() -> f32 {
    1.0
}

fn default_blend_mode() -> String {
    "normal".to_string()
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            volume: 1.0,
            blend_mode: "normal".to_string(),
        }
    }
}
