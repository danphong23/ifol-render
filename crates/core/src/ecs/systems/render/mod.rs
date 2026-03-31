pub mod state;
pub mod effect_pass;
pub mod composition_pass;
pub mod gizmo_overlay;

pub use gizmo_overlay::*;

use crate::ecs::{World, ContextView};
use crate::frame::{Frame, PassType, RenderPass};
use state::RenderState;

/// Compiles the ECS World's DrawCalls into a renderable Frame.
pub fn render_to_frame(
    world: &World,
    camera_id: &str,
    screen_width: u32,
    screen_height: u32,
    _time_secs: f64,
    custom_cam_x: Option<f32>,
    custom_cam_y: Option<f32>,
    custom_cam_w: Option<f32>,
    custom_cam_h: Option<f32>,
    context: &ContextView,
) -> Frame {
    let mut state = RenderState::new(screen_width, screen_height);
    let storages = &world.storages;

    // ── Pre-discover Main Camera ──
    let cam = world.find_camera(camera_id);
    let cam_top_left_x = cam.map(|c| c.resolved.x - c.resolved.width * 0.5).unwrap_or(0.0);
    let cam_top_left_y = cam.map(|c| c.resolved.y - c.resolved.height * 0.5).unwrap_or(0.0);

    state.root_cam_x = custom_cam_x.unwrap_or(cam_top_left_x);
    state.root_cam_y = custom_cam_y.unwrap_or(cam_top_left_y);
    state.root_cam_w = custom_cam_w
        .unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0))
        .max(1.0);
    state.root_cam_h = custom_cam_h
        .unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0))
        .max(1.0);
        
    state.root_sx = screen_width as f32 / state.root_cam_w;
    state.root_sy = screen_height as f32 / state.root_cam_h;

    state.root_cam_mask = cam
        .and_then(|c| storages.get_component::<crate::ecs::components::CameraComponent>(&c.id))
        .map(|c| c.culling_mask)
        .unwrap_or(crate::ecs::RENDER_MASK_ALL);

    // ── Pre-discover Composition Cameras ──
    let sorted = world.sorted_by_layer();
    for entity in &sorted {
        if !entity.resolved.visible { continue; }
        if !context.active_entities.contains(&entity.id) { continue; }
        if storages.get_component::<crate::ecs::components::Composition>(&entity.id).is_some() {
            let mut cam_ent = None;
            for c_ent in &sorted {
                if !c_ent.resolved.visible { continue; }
                if storages.get_component::<crate::ecs::components::CameraComponent>(&c_ent.id).is_some() {
                    if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&c_ent.id) {
                        if pid.0 == entity.id {
                            cam_ent = Some(c_ent);
                            break;
                        }
                    }
                }
            }
            if let Some(c) = cam_ent {
                let cw = c.resolved.width.max(1.0);
                let ch = c.resolved.height.max(1.0);
                let mask = storages
                    .get_component::<crate::ecs::components::CameraComponent>(&c.id)
                    .map(|cam| cam.culling_mask)
                    .unwrap_or(crate::ecs::RENDER_MASK_DEFAULT);

                // The inner cam's VIEW origin in world space:
                // fixed: visually top-left relative
                let inner_cam_x = c.resolved.x - cw * 0.5;
                let inner_cam_y = c.resolved.y - ch * 0.5;
                state.comp_cameras.insert(entity.id.clone(), (inner_cam_x, inner_cam_y, cw, ch, mask));
            } else {
                log::warn!("Composition '{}' has no direct child CameraComponent, skipping render", entity.id);
            }
        }
    }

    // ── Phase 1: Compile Entities & Effects ──
    effect_pass::build_entity_passes(world, context, &mut state);

    // ── Phase 2: Compile Nested Compositions ──
    composition_pass::compile_composition_buffers(world, context, &mut state);

    // Everything implicitly bubbled up to "main" (the Root)
    let mut flat_entities = state.comp_lists.remove("main").unwrap_or_default();
    
    // Sort main to ensure synthesized composition proxies are perfectly layered
    flat_entities.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.z_index.partial_cmp(&b.z_index).unwrap()));

    // ── Build Main Screen RenderPass ──
    let base_output = if state.camera_effects.is_empty() {
        "main".to_string()
    } else {
        "_camera_src".to_string()
    };

    state.passes.push(RenderPass {
        output: base_output.clone(),
        pass_type: PassType::Entities {
            entities: flat_entities,
            clear_color: [0.0, 0.0, 0.0, 0.0],
        },
        target_width: if state.camera_effects.is_empty() { None } else { Some(screen_width) },
        target_height: if state.camera_effects.is_empty() { None } else { Some(screen_height) },
    });

    // ── Post-processing: Camera Effects ──
    let mut current_cam_key = base_output;
    for (i, effect) in state.camera_effects.iter().enumerate() {
        let out_key = format!("_camera_fx_{}", i);
        state.passes.push(RenderPass {
            output: out_key.clone(),
            pass_type: PassType::Effect {
                shader: effect.shader_id.clone(),
                inputs: vec![current_cam_key],
                params: effect.params.clone(),
            },
            target_width: Some(screen_width),
            target_height: Some(screen_height),
        });
        current_cam_key = out_key;
    }

    state.passes.push(RenderPass {
        output: "final".into(),
        pass_type: PassType::Output { 
            input: if state.camera_effects.is_empty() { "main".into() } else { current_cam_key },
            entities: vec![],
        },
        target_width: Some(screen_width),
        target_height: None,
    });

    Frame {
        passes: state.passes,
        texture_updates: state.texture_updates,
        audio_calls: state.audio_calls,
    }
}
