use ifol_render_ecs::ecs::{World, ContextView};
use ifol_render_ecs::ecs::components::CameraComponent;
use ifol_render_ecs::frame::FlatEntity;

/// Editor Gizmo System
///
/// Responsible for rendering editor-only visual aids like Camera bounding boxes
/// and full-screen selection outlines. This appends directly to the generated Frame.
pub fn editor_gizmo_system(
    world: &World,
    selected_entity_ids: &[&str],
    select_mode: &str,
    cam_x: f32,
    cam_y: f32,
    sx: f32,
    sy: f32,
    _screen_width: u32,
    _screen_height: u32,
    context: &ContextView,
    _gizmo_base_layer: i32,
) -> Vec<FlatEntity> {
    let mut gizmos = Vec::new();
    let storages = &world.storages;
    let sorted = world.sorted_by_layer();

    let is_in_comp = |ent_id: &str| -> bool {
        let mut cur = ent_id.to_string();
        for _ in 0..32 {
            if let Some(pid) = storages.get_component::<ifol_render_ecs::ecs::components::meta::ParentId>(&cur) {
                if storages.get_component::<ifol_render_ecs::ecs::components::Composition>(&pid.0).is_some() {
                    // If we're SCOPED INTO this composition, its children are
                    // effectively root-level — NOT "in comp" for gizmo purposes
                    if context.scope_id == Some(pid.0.as_str()) {
                        return false;
                    }
                    return true;
                }
                cur = pid.0.clone();
            } else { break; }
        }
        false
    };

    // 1. Camera Gizmo Passes (Dashed rect & triangle)
    for entity in &sorted {
        if !entity.resolved.visible { continue; }
        if !context.active_entities.contains(&entity.id) { continue; }
        if storages.get_component::<CameraComponent>(&entity.id).is_none() { continue; }
        if is_in_comp(&entity.id) { continue; }
        // Do not draw gizmos for ephemeral editor cameras
        if entity.id.starts_with("__editor") || entity.id == "__gizmo_cam__" { continue; }

        let is_selected = selected_entity_ids.contains(&entity.id.as_str());
        let cam_color = if is_selected { [0.0, 0.85, 1.0, 1.0] } else { [1.0, 0.0, 1.0, 0.6] };
        let cam_px_thickness = if is_selected { 10.0 } else { 6.0 };

        let max_dim = entity.resolved.width.max(entity.resolved.height).max(1.0);
        let norm_border = cam_px_thickness / max_dim;
        let norm_dash = 24.0 / max_dim;
        let norm_gap = 16.0 / max_dim;

        let draw_w = entity.resolved.width * sx;
        let draw_h = entity.resolved.height * sy;
        
        let cos_r = entity.resolved.rotation.cos();
        let sin_r = entity.resolved.rotation.sin();

        let center_x = (entity.resolved.x - cam_x) * sx;
        let center_y = (entity.resolved.y - cam_y) * sy;
        
        let tri_size = entity.resolved.width * 0.05;
        let local_tri_x = 0.0;
        let local_tri_y = -entity.resolved.height * 0.5 - tri_size * 0.6;
        
        let tri_x_world = entity.resolved.x + local_tri_x * cos_r - local_tri_y * sin_r;
        let tri_y_world = entity.resolved.y + local_tri_x * sin_r + local_tri_y * cos_r;
        
        let tri_w = tri_size * sx;
        let tri_center_x = (tri_x_world - cam_x) * sx;
        let tri_center_y = (tri_y_world - cam_y) * sy;

        gizmos.push(FlatEntity { content_hash: 0,
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
            layer: 99999 + entity.resolved.layer, 
            z_index: 99999.0 + entity.resolved.layer as f32,
            fit_mode: 0,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
            intrinsic_width: draw_w,
            intrinsic_height: draw_h,
        });

        gizmos.push(FlatEntity { content_hash: 0,
            id: 0,
            x: tri_center_x - tri_w * 0.5,
            y: tri_center_y - tri_w * 0.5,
            width: tri_w,
            height: tri_w,
            rotation: entity.resolved.rotation + std::f32::consts::PI,
            opacity: 1.0,
            blend_mode: 0,
            color: [0.3, 0.9, 0.4, 0.8],
            shader: "shapes".to_string(),
            textures: vec![],
            params: vec![5.0, 0.0, 0.0, 0.0], // 5.0 = triangle
            layer: 99999 + entity.resolved.layer + 1,
            z_index: 99999.0 + entity.resolved.layer as f32 + 1.0,
            fit_mode: 0,
            uv_offset: [0.0, 0.0],
            uv_scale: [1.0, 1.0],
            intrinsic_width: tri_w,
            intrinsic_height: tri_w,
        });
    }

    // 2. Selection Masking Pass (for non-camera selected objects)
    for entity in &sorted {
        if !entity.resolved.visible { continue; }
        if !context.active_entities.contains(&entity.id) { continue; }
        if !selected_entity_ids.contains(&entity.id.as_str()) { continue; }
        if storages.get_component::<CameraComponent>(&entity.id).is_some() { continue; }
        if is_in_comp(&entity.id) { continue; }

        let bounds_w = entity.resolved.width * sx;
        let bounds_h = entity.resolved.height * sy;
        let bounds_rot = entity.resolved.rotation;
        
        let bounds_cx = (entity.resolved.x - cam_x) * sx;
        let bounds_cy = (entity.resolved.y - cam_y) * sy;

        let is_content = select_mode == "content";
        let is_select = select_mode == "select";

        let thicc = if is_select { 8.0 } else { 4.0 };
        let box_col = if is_content { [0.0, 0.898, 1.0, 0.9] } else if is_select { [1.0, 1.0, 1.0, 1.0] } else { [0.5, 0.5, 0.5, 0.8] };
        
        let max_dim = bounds_w.max(bounds_h).max(1.0);

        let mut is_circle = false;
        if let Some(call) = entity.draw.draw_calls.first() {
            if call.kind == ifol_render_ecs::ecs::components::draw::DrawKind::SolidEllipse {
                is_circle = true;
            }
        }

        let pad = thicc * 0.5; // Padding to prevent stroke clipping
        let draw_w = bounds_w.max(1.0) + pad * 2.0;
        let draw_h = bounds_h.max(1.0) + pad * 2.0;

        if is_content && is_circle {
            gizmos.push(FlatEntity { content_hash: 0,
                id: 0,
                x: bounds_cx - bounds_w * 0.5 - pad,
                y: bounds_cy - bounds_h * 0.5 - pad,
                width: draw_w,
                height: draw_h,
                rotation: bounds_rot,
                opacity: 1.0,
                blend_mode: 0,
                color: box_col,
                shader: "shapes".to_string(),
                textures: vec![],
                params: vec![
                    3.0,                  // 3 = ellipse
                    0.0,                  // param1
                    thicc / max_dim,      // param2 = hollow border width
                    pad / draw_w.max(draw_h)  // param3 = normalize UV inset
                ],
                layer: 1000000, 
                z_index: 1000000.0,
                fit_mode: 0, uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                intrinsic_width: bounds_w.max(1.0), intrinsic_height: bounds_h.max(1.0),
            });
        } else {
            gizmos.push(FlatEntity { content_hash: 0,
                id: 0,
                x: bounds_cx - bounds_w * 0.5 - pad,
                y: bounds_cy - bounds_h * 0.5 - pad,
                width: draw_w,
                height: draw_h,
                rotation: bounds_rot,
                opacity: 1.0,
                blend_mode: 0,
                color: box_col,
                shader: "dashed_rect".to_string(),
                textures: vec![],
                params: vec![
                    if is_content { 0.0 } else { 100.0 }, // no dashes for content outline
                    0.0,
                    thicc / max_dim,
                    0.0
                ],
                layer: 1000000, 
                z_index: 1000000.0,
                fit_mode: 0, uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                intrinsic_width: bounds_w.max(1.0), intrinsic_height: bounds_h.max(1.0),
            });
        }
    }

    gizmos
}
