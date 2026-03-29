use crate::ecs::World;
use crate::ecs::components::CameraComponent;
use crate::frame::{Frame, FlatEntity, RenderPass};

/// Editor Gizmo System
///
/// Responsible for rendering editor-only visual aids like Camera bounding boxes
/// and full-screen selection outlines. This appends directly to the generated Frame.
pub fn editor_gizmo_system(
    world: &World,
    frame: &mut Frame,
    selected_entity_ids: &[&str],
    select_mode: &str,
    cam_x: f32,
    cam_y: f32,
    sx: f32,
    sy: f32,
    screen_width: u32,
    screen_height: u32,
) {
    let storages = &world.storages;
    let sorted = world.sorted_by_layer();

    // 1. Camera Gizmo Passes (Dashed rect & triangle)
    // We append them directly into the primary screen pass of the frame.
    // The main pass is typically the first or last pass drawn to "screen".
    if let Some(main_pass) = frame.passes.iter_mut().find(|p| {
        p.output == "main" || p.output == "final"
    }) {
        for entity in &sorted {
            if !entity.resolved.visible { continue; }
            if storages.get_component::<CameraComponent>(&entity.id).is_none() { continue; }

            let is_selected = selected_entity_ids.contains(&entity.id.as_str());
            let cam_color = if is_selected { [0.0, 0.85, 1.0, 1.0] } else { [1.0, 0.0, 1.0, 0.6] };
            let cam_px_thickness = if is_selected { 6.0 } else { 4.0 };

            let max_dim = entity.resolved.width.max(entity.resolved.height).max(1.0);
            let norm_border = cam_px_thickness / max_dim;
            let norm_dash = 24.0 / max_dim;
            let norm_gap = 16.0 / max_dim;

            // Dashed border (FlatEntity representation)
            let draw_w = entity.resolved.width * sx;
            let draw_h = entity.resolved.height * sy;
            
            // Transform point
            let cos_r = entity.resolved.rotation.cos();
            let sin_r = entity.resolved.rotation.sin();
            let dx = (0.5 - entity.resolved.anchor_x) * draw_w;
            let dy = (0.5 - entity.resolved.anchor_y) * draw_h;
            let center_x = (entity.resolved.x - cam_x) * sx + dx * cos_r - dy * sin_r;
            let center_y = (entity.resolved.y - cam_y) * sy + dx * sin_r + dy * cos_r;
            
            let tri_size = entity.resolved.width * 0.05;
            let orig_min_x = -entity.resolved.anchor_x * entity.resolved.width;
            let orig_min_y = -entity.resolved.anchor_y * entity.resolved.height;
            let local_tri_x = orig_min_x + entity.resolved.width * 0.5;
            let local_tri_y = orig_min_y - tri_size * 0.6;
            
            let tri_x_world = entity.resolved.x + local_tri_x * cos_r - local_tri_y * sin_r;
            let tri_y_world = entity.resolved.y + local_tri_x * sin_r + local_tri_y * cos_r;
            
            let tri_w = tri_size * sx;
            let tri_h = tri_size * sy;
            let tri_cx = (tri_x_world - cam_x) * sx;
            let tri_cy = (tri_y_world - cam_y) * sy;

            if let crate::frame::PassType::Entities { ref mut entities, .. } = main_pass.pass_type {
                entities.push(FlatEntity {
                    id: 0,
                    x: center_x - draw_w * 0.5,
                    y: center_y - draw_h * 0.5,
                    width: draw_w,
                    height: draw_h,
                    rotation: entity.resolved.rotation,
                    opacity: 1.0,
                    blend_mode: 0,
                    color: cam_color,
                    shader: "dashed_rect".to_string(),
                    textures: vec![],
                    params: vec![norm_dash, norm_gap, norm_border, 0.0],
                    layer: 9999,
                    z_index: 9999.0,
                    fit_mode: 0,
                    uv_offset: [0.0, 0.0],
                    uv_scale: [1.0, 1.0],
                    intrinsic_width: 1.0,
                    intrinsic_height: 1.0,
                });

                entities.push(FlatEntity {
                    id: 0,
                    x: tri_cx - tri_w * 0.5,
                    y: tri_cy - tri_h * 0.5,
                    width: tri_w,
                    height: tri_h,
                    rotation: entity.resolved.rotation + std::f32::consts::PI,
                    opacity: 1.0,
                    blend_mode: 0,
                    color: cam_color,
                    shader: "shapes".to_string(),
                    textures: vec![],
                    params: vec![5.0, 0.0, 0.0, 0.0],
                    layer: 9999,
                    z_index: 9999.0,
                    fit_mode: 0,
                    uv_offset: [0.0, 0.0],
                    uv_scale: [1.0, 1.0],
                    intrinsic_width: 1.0,
                    intrinsic_height: 1.0,
                });
            }
        }
        
        // Sort main pass to keep gizmos on top
        if let crate::frame::PassType::Entities { ref mut entities, .. } = main_pass.pass_type {
            entities.sort_by(|a, b| a.z_index.partial_cmp(&b.z_index).unwrap());
        }
    }

    // 2. Selection Outline Pass (Mask + Glow)
    if selected_entity_ids.is_empty() { return; }

    let mut sel_mask_entities = Vec::new();
    for entity in &sorted {
        if !entity.resolved.visible { continue; }
        if !selected_entity_ids.contains(&entity.id.as_str()) { continue; }
        if storages.get_component::<CameraComponent>(&entity.id).is_some() { continue; }

        let r = &entity.resolved;
        if select_mode == "content" {
            for call in entity.draw.draw_calls.iter() {
                let w = call.width * sx;
                let h = call.height * sy;
                let cos_r = call.rotation.cos();
                let sin_r = call.rotation.sin();
                let dx = (0.5 - call.anchor_x) * w;
                let dy = (0.5 - call.anchor_y) * h;
                let center_x = (call.x - cam_x) * sx + dx * cos_r - dy * sin_r;
                let center_y = (call.y - cam_y) * sy + dx * sin_r + dy * cos_r;
                
                let iw = call.intrinsic_width;
                let ih = call.intrinsic_height;
                let (uv_offset, uv_scale) = call.fit_mode.calculate_uv(call.width, call.height, iw, ih, call.align_x, call.align_y);
                
                let mut textures = Vec::new();
                if let Some(t) = &call.texture_key { textures.push(t.clone()); }
                
                let shader = match call.kind {
                    crate::ecs::components::draw::DrawKind::SolidRect => "shapes",
                    crate::ecs::components::draw::DrawKind::SolidEllipse => "shapes",
                    _ => "composite",
                };

                sel_mask_entities.push(FlatEntity {
                    id: 0,
                    x: center_x - w * 0.5,
                    y: center_y - h * 0.5,
                    width: w,
                    height: h,
                    rotation: call.rotation,
                    opacity: 1.0,
                    blend_mode: 0,
                    color: [1.0, 1.0, 1.0, 1.0],
                    shader: shader.to_string(),
                    textures,
                    params: call.params.clone(),
                    layer: entity.resolved.layer,
                    z_index: entity.resolved.layer as f32,
                    fit_mode: match call.fit_mode { crate::ecs::components::FitMode::Contain => 1, crate::ecs::components::FitMode::Cover => 2, _ => 0 },
                    uv_offset,
                    uv_scale,
                    intrinsic_width: iw,
                    intrinsic_height: ih,
                });
            }
        } else if select_mode == "rect" {
            let w = r.width * sx;
            let h = r.height * sy;
            let cos_r = r.rotation.cos();
            let sin_r = r.rotation.sin();
            let dx = (0.5 - r.anchor_x) * w;
            let dy = (0.5 - r.anchor_y) * h;
            let cx = (r.x - cam_x) * sx + dx * cos_r - dy * sin_r;
            let cy = (r.y - cam_y) * sy + dx * sin_r + dy * cos_r;

            sel_mask_entities.push(FlatEntity {
                id: 0,
                x: cx - w * 0.5,
                y: cy - h * 0.5,
                width: w,
                height: h,
                rotation: r.rotation,
                opacity: 1.0,
                blend_mode: 0,
                color: [1.0, 1.0, 1.0, 1.0],
                shader: "shapes".to_string(),
                textures: vec![],
                params: vec![1.0, 0.0, 0.0, 0.0], // 1.0 = solid rect
                layer: r.layer,
                z_index: r.layer as f32,
                fit_mode: 0,
                uv_offset: [0.0, 0.0],
                uv_scale: [1.0, 1.0],
                intrinsic_width: 1.0,
                intrinsic_height: 1.0,
            });
        }
    }

    if !sel_mask_entities.is_empty() {
        // Wait, texture updates for rendertarget shouldn't recreate it every frame unless size changed.
        // It's fine for now. We skip it, since the engine handles non-existent render targets lazily.
        
        frame.passes.push(RenderPass {
            output: "_sel_mask".to_string(),
            pass_type: crate::frame::PassType::Entities {
                entities: sel_mask_entities,
                clear_color: [0.0, 0.0, 0.0, 0.0],
            },
            target_width: Some(screen_width),
            target_height: Some(screen_height),
        });

        frame.passes.push(RenderPass {
            output: "_sel_outline".to_string(),
            pass_type: crate::frame::PassType::Effect {
                shader: "selection_outline".into(),
                inputs: vec!["_sel_mask".into()],
                params: vec![3.0, 0.0, 0.0, 0.0], // thickness=3px
            },
            target_width: Some(screen_width),
            target_height: Some(screen_height),
        });

        // Ensure "final" exists or composite it
        if let Some(main_pass) = frame.passes.iter_mut().find(|p| p.output == "main" || p.output == "final") {
            if let crate::frame::PassType::Entities { ref mut entities, .. } = main_pass.pass_type {
                entities.push(FlatEntity {
                    id: 0,
                    x: 0.0,
                    y: 0.0,
                    width: screen_width as f32,
                    height: screen_height as f32,
                    rotation: 0.0,
                    opacity: 1.0,
                    blend_mode: 0,
                    color: [1.0, 1.0, 1.0, 1.0],
                    shader: "composite".to_string(),
                    textures: vec!["_sel_outline".to_string()],
                    params: vec![],
                    layer: 10001,
                    z_index: 10001.0,
                    fit_mode: 0,
                    uv_offset: [0.0, 0.0],
                    uv_scale: [1.0, 1.0],
                    intrinsic_width: 0.0,
                    intrinsic_height: 0.0,
                });
            }
        }
    }
}
