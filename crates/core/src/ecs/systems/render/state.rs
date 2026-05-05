use crate::frame::{FlatEntity, RenderPass, TextureUpdate, AudioCall};
use crate::ecs::components::draw::EffectPassDef;
use std::collections::HashMap;

/// Describes a top-level camera found in the scene, used for multi-camera compositing.
#[derive(Debug, Clone)]
pub struct SceneCameraInfo {
    pub entity_id: String,
    pub render_order: i32,
    /// Layer filter: None = all layers, Some(vec) = only those layers
    pub target_layers: Option<Vec<i32>>,
    pub culling_mask: u32,
    /// Camera world-space view bounds (top-left origin)
    pub cam_x: f32,
    pub cam_y: f32,
    pub cam_w: f32,
    pub cam_h: f32,
    /// Quality scaling factor (1.0 = native resolution)
    pub render_scale: f32,
    /// Post-processing effects on camera output
    pub post_effects: Vec<EffectPassDef>,
}

/// State threaded through the rendering passes
pub struct RenderState {
    pub passes: Vec<RenderPass>,
    pub texture_updates: Vec<TextureUpdate>,
    pub audio_calls: Vec<AudioCall>,

    /// Grouped entities per target composition. Key "main" is the root screen.
    pub comp_lists: HashMap<String, Vec<FlatEntity>>,
    /// Compositions that have rendering content
    pub comp_entities: Vec<String>,

    /// Camera inner buffer boundaries: (inner_cam_x, inner_cam_y, cw, ch, mask)
    pub comp_cameras: HashMap<String, (f32, f32, f32, f32, u32)>,

    /// Top-level camera effects to be applied to the final output (legacy single-camera)
    pub camera_effects: Vec<EffectPassDef>,

    /// Phase 5: All discovered root-level cameras (sorted by render_order in mod.rs)
    pub scene_cameras: Vec<SceneCameraInfo>,

    pub screen_width: u32,
    pub screen_height: u32,

    pub root_cam_x: f32,
    pub root_cam_y: f32,
    pub root_cam_w: f32,
    pub root_cam_h: f32,
    pub root_cam_mask: u32,
    pub root_sx: f32,
    pub root_sy: f32,

    /// T2.4: Key of the current accumulation texture being built.
    /// When a non-Normal blend entity is encountered, this is snapshotted as the dst.
    /// Updated by mod.rs as the frame compiler emits composition passes.
    pub current_acc_key: Option<String>,
}

impl RenderState {
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            passes: Vec::new(),
            texture_updates: Vec::new(),
            audio_calls: Vec::new(),
            comp_lists: HashMap::new(),
            comp_entities: Vec::new(),
            comp_cameras: HashMap::new(),
            camera_effects: Vec::new(),
            scene_cameras: Vec::new(),
            screen_width,
            screen_height,
            root_cam_x: 0.0,
            root_cam_y: 0.0,
            root_cam_w: 1.0,
            root_cam_h: 1.0,
            root_cam_mask: crate::ecs::RENDER_MASK_DEFAULT,
            root_sx: 1.0,
            root_sy: 1.0,
            current_acc_key: None,
        }
    }

    /// Automatically computes pass_hash before pushing the pass to the frame.
    /// Used by the RenderGraph to detect and skip unchanged GPU work.
    pub fn push_pass(&mut self, mut pass: RenderPass) {
        pass.pass_hash = pass.calculate_hash();
        self.passes.push(pass);
    }
}
