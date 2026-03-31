use crate::ecs::World;
use crate::frame::{FlatEntity, PassType, RenderPass, TextureUpdate, AudioCall};
use crate::ecs::components::draw::EffectPassDef;
use std::collections::HashMap;

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
    
    /// Top-level camera effects to be applied to the final output
    pub camera_effects: Vec<EffectPassDef>,

    pub screen_width: u32,
    pub screen_height: u32,
    
    pub root_cam_x: f32,
    pub root_cam_y: f32,
    pub root_cam_w: f32,
    pub root_cam_h: f32,
    pub root_cam_mask: u32,
    pub root_sx: f32,
    pub root_sy: f32,
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
            screen_width,
            screen_height,
            root_cam_x: 0.0,
            root_cam_y: 0.0,
            root_cam_w: 1.0,
            root_cam_h: 1.0,
            root_cam_mask: crate::ecs::RENDER_MASK_DEFAULT,
            root_sx: 1.0,
            root_sy: 1.0,
        }
    }
}
