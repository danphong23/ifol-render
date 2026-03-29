use serde::{Deserialize, Serialize};
use super::FitMode;

/// Display rectangle component.
///
/// Defines the entity's display size (in world units) and how
/// source content fits within it.
/// This value acts as a default fallback; it can be overridden
/// by an AnimationComponent if the `RectWidth` or `RectHeight` targets exist.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    /// Display width in world units.
    #[serde(default)]
    pub width: f32,
    /// Display height in world units.
    #[serde(default)]
    pub height: f32,
    /// How source content fits within this rect.
    #[serde(default)]
    pub fit_mode: FitMode,
    /// Horizontal alignment (0.0 = left, 0.5 = center, 1.0 = right).
    #[serde(default = "default_align")]
    pub align_x: f32,
    /// Vertical alignment (0.0 = top, 0.5 = center, 1.0 = bottom).
    #[serde(default = "default_align")]
    pub align_y: f32,
}

fn default_align() -> f32 {
    0.5
}
