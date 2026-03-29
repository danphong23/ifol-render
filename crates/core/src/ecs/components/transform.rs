use serde::{Deserialize, Serialize};

/// Transform component.
///
/// Defines the spatial properties of the entity relative to its parent.
/// These are static fallback values; if an AnimationComponent is present,
/// its keyframes will override these during the `animation_sys` phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transform {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    /// Rotation in degrees (clockwise).
    #[serde(default)]
    pub rotation: f32,
    /// Anchor X relative to the entity's size (0.0 = left, 0.5 = center, 1.0 = right).
    #[serde(default)]
    pub anchor_x: f32,
    /// Anchor Y relative to the entity's size (0.0 = top, 0.5 = center, 1.0 = bottom).
    #[serde(default)]
    pub anchor_y: f32,
    #[serde(default = "default_scale")]
    pub scale_x: f32,
    #[serde(default = "default_scale")]
    pub scale_y: f32,
}

fn default_scale() -> f32 {
    1.0
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            anchor_x: 0.0,
            anchor_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }
    }
}
