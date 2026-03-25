use serde::{Deserialize, Serialize};
use crate::scene::FloatTrack;
use super::FitMode;

/// Display rectangle component.
///
/// Defines the entity's display size (in world units) and how
/// source content fits within it.
///
/// **Size resolution order:**
///   1. If `Rect.width/height` is set → use that
///   2. Else if entity has video/image source → auto-size to `intrinsic_width/height`
///   3. Else → default 200×200
///
/// `fit_mode` controls how source content (with its intrinsic aspect ratio)
/// maps into this display rect — identical to CSS `object-fit`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    /// Display width in world units (animatable).
    #[serde(default)]
    pub width: FloatTrack,
    /// Display height in world units (animatable).
    #[serde(default)]
    pub height: FloatTrack,
    /// How source content fits within this rect.
    /// - stretch: fill rect, ignore aspect ratio
    /// - contain: fit inside rect, keep aspect (letterbox)
    /// - cover: fill rect, keep aspect (may crop)
    #[serde(default)]
    pub fit_mode: FitMode,
}
