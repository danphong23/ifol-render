use serde::{Deserialize, Serialize};
use crate::scene::MaterialV2;

/// Camera component — makes an entity act as a virtual camera.
///
/// When present, the entity's `transform.width/height` defines the
/// visible world region, and `transform.x/y` defines the viewport position.
/// The compiler uses the active camera to project world→pixels.
///
/// ## Properties
/// - `resolution_width/height`: Output pixel resolution (default 1280×720).
///   This is the native render size. Frontend can override via `engine.resize()`.
/// - `bg_color`: Background fill color RGBA in linear space.
/// - `fov`: Field of view angle (degrees). Reserved for 3D perspective projection.
/// - `near/far`: Near/far clip planes. Reserved for 3D.
/// - `post_effects`: Frame-level post-processing effects chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraComponent {
    /// Output resolution width in pixels (default 1280)
    #[serde(default = "default_res_w")]
    pub resolution_width: u32,
    /// Output resolution height in pixels (default 720)
    #[serde(default = "default_res_h")]
    pub resolution_height: u32,
    /// Background color RGBA in linear color space. Default: opaque black.
    #[serde(default = "default_bg_color")]
    pub bg_color: [f32; 4],
    /// Field of view in degrees (reserved for future 3D projection)
    #[serde(default)]
    pub fov: f32,
    /// Near clip plane (reserved for future 3D projection)
    #[serde(default = "default_near")]
    pub near: f32,
    /// Far clip plane (reserved for future 3D projection)
    #[serde(default = "default_far")]
    pub far: f32,
    /// Frame-level post-processing effects (applied after all entities are composited).
    #[serde(default)]
    pub post_effects: Vec<MaterialV2>,
}

impl Default for CameraComponent {
    fn default() -> Self {
        Self {
            resolution_width: 1280,
            resolution_height: 720,
            bg_color: [0.0, 0.0, 0.0, 1.0],
            fov: 0.0,
            near: 0.1,
            far: 1000.0,
            post_effects: Vec::new(),
        }
    }
}

fn default_res_w() -> u32 { 1280 }
fn default_res_h() -> u32 { 720 }
fn default_bg_color() -> [f32; 4] { [0.0, 0.0, 0.0, 1.0] }
fn default_near() -> f32 { 0.1 }
fn default_far() -> f32 { 1000.0 }
