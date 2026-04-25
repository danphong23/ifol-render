use crate::scene::MaterialV2;
use serde::{Deserialize, Serialize};

/// How this camera decides which entities to include in its render pass.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum CameraRenderMode {
    /// Render entities whose `layer` value is within `target_layers` (if Some),
    /// or all layers (if None). This is the default mode.
    #[default]
    #[serde(rename = "layers")]
    Layers,
    /// Composite the output textures of a list of sub-cameras in order.
    /// `target_cameras` holds their entity IDs.
    #[serde(rename = "cameras")]
    Cameras,
}

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
/// - `render_mode`: `Layers` (default) or `Cameras` composite mode.
/// - `target_layers`: When `render_mode = Layers`, only render entities whose
///   layer is in this inclusive range `[min, max]`. `None` = render all layers.
/// - `render_order`: Compositing order. Lower = rendered first (background).
///   Content camera = 0, Editor/Gizmo camera = 1 (on top, no post-fx).
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
    /// Bitmask for RenderCategories this camera is allowed to render.
    #[serde(default = "default_culling_mask")]
    pub culling_mask: crate::ecs::RenderCategory,

    /// Resolution scale factor (default 1.0).
    /// Used to reduce internal rendering resolution for performance (e.g. 50% quality viewport).
    #[serde(default = "default_render_scale")]
    pub render_scale: f32,

    // ── Phase 5: Multi-Camera Architecture ─────────────────────────────────────

    /// How this camera selects content: by layer range or by compositing sub-cameras.
    #[serde(default)]
    pub render_mode: CameraRenderMode,

    /// When `render_mode = Layers`: only render entities whose layer is in this list.
    /// `None` means render ALL layers (backward-compatible default).
    /// Example: `[0, 1, 2]` renders layer 0–2 only.
    /// Editor/Gizmo camera should use `[999]` to isolate overlay content.
    #[serde(default)]
    pub target_layers: Option<Vec<i32>>,

    /// When `render_mode = Cameras`: list of camera entity IDs to composite (in order).
    #[serde(default)]
    pub target_cameras: Vec<String>,

    /// Composite order. Lower values are rendered first (background).
    /// Content camera: 0. Editor overlay camera: 1 (renders on top, no post_effects).
    /// Default is 0.
    #[serde(default)]
    pub render_order: i32,
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
            culling_mask: crate::ecs::RENDER_MASK_DEFAULT,
            render_scale: 1.0,
            render_mode: CameraRenderMode::Layers,
            target_layers: None,
            target_cameras: Vec::new(),
            render_order: 0,
        }
    }
}

impl CameraComponent {
    /// Returns true if this camera should render the given layer index.
    /// If `target_layers` is None, all layers pass (backward-compatible).
    pub fn accepts_layer(&self, layer: i32) -> bool {
        match &self.target_layers {
            None => true,
            Some(layers) => layers.contains(&layer),
        }
    }
}

fn default_culling_mask() -> crate::ecs::RenderCategory {
    crate::ecs::RENDER_MASK_DEFAULT
}

fn default_render_scale() -> f32 {
    1.0
}

fn default_res_w() -> u32 {
    1280
}
fn default_res_h() -> u32 {
    720
}
fn default_bg_color() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}
fn default_near() -> f32 {
    0.1
}
fn default_far() -> f32 {
    1000.0
}
