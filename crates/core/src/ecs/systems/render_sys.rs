use crate::ecs::World;
use crate::frame::{FlatEntity, Frame, RenderPass, PassType};

/// Compiles the ECS World's DrawCalls into a renderable Frame.
/// This Replaces the V2 `compiler.rs` payload, running pure data translations.
pub fn render_to_frame(
    world: &World,
    camera_id: &str,
    is_editor_mode: bool,
    screen_width: u32,
    screen_height: u32,
    _time_secs: f64,
    custom_cam_x: Option<f32>,
    custom_cam_y: Option<f32>,
    custom_cam_w: Option<f32>,
    custom_cam_h: Option<f32>,
    _selected_entity_ids: &[&str],
    scope_entity_id: Option<&str>,
    select_mode: &str,
) -> Frame {
    let mut passes = Vec::new();
    let mut texture_updates = Vec::new();
    let storages = &world.storages;

    // ── Camera projection: world units → pixels ──
    let cam = world.find_camera(camera_id);
    let cam_x = custom_cam_x.unwrap_or_else(|| cam.map(|c| c.resolved.x).unwrap_or(0.0));
    let cam_y = custom_cam_y.unwrap_or_else(|| cam.map(|c| c.resolved.y).unwrap_or(0.0));
    let cam_w = custom_cam_w.unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0)).max(1.0);
    let cam_h = custom_cam_h.unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0)).max(1.0);
    let sx = screen_width as f32 / cam_w;
    let sy = screen_height as f32 / cam_h;

    // ── Helper: check if entity is a descendant of scope ──
    let is_in_scope = |entity_id: &str| -> bool {
        if scope_entity_id.is_none() { return true; }
        let scope_id = scope_entity_id.unwrap();
        // Walk parent chain to see if any ancestor is the scope entity
        let mut current_id = entity_id.to_string();
        for _ in 0..32 { // max depth guard
            if let Some(e) = world.entities.iter().find(|e| e.id == current_id) {
                if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&e.id).map(|id| &id.0) {
                    if pid == scope_id { return true; }
                    current_id = pid.to_string();
                } else {
                    return false; // reached root without finding scope
                }
            } else {
                return false;
            }
        }
        false
    };

    // Collect and sort flat entities directly from the pre-generated DrawCalls
    let mut flat_entities = Vec::new();

    let sorted = world.sorted_by_layer();
    for entity in &sorted {
        if !entity.resolved.visible { continue; }
        // Skip entities outside render scope
        if !is_in_scope(&entity.id) { continue; }
        // Skip cameras unless in editor mode
        if storages.get_component::<crate::ecs::components::CameraComponent>(&entity.id).is_some() && !is_editor_mode {
            continue;
        }

        // ── Camera gizmo: dashed border + orientation triangle ──
        let mut gizmo_draws: Vec<crate::ecs::components::draw::DrawCall> = Vec::new();
        if storages.get_component::<crate::ecs::components::CameraComponent>(&entity.id).is_some() && is_editor_mode {
            let is_selected = _selected_entity_ids.contains(&entity.id.as_str());
            let cam_color = if is_selected { [0.0, 0.85, 1.0, 1.0] } else { [1.0, 0.0, 1.0, 0.6] };
            let cam_px_thickness = if is_selected { 6.0 } else { 4.0 };

            // Dashed border around camera viewport
            let mut dash_call = crate::ecs::components::draw::DrawCall::default();
            dash_call.kind = crate::ecs::components::draw::DrawKind::DashedRect;
            dash_call.x = entity.resolved.x;
            dash_call.y = entity.resolved.y;
            dash_call.width = entity.resolved.width;
            dash_call.height = entity.resolved.height;
            dash_call.anchor_x = entity.resolved.anchor_x;
            dash_call.anchor_y = entity.resolved.anchor_y;
            dash_call.rotation = entity.resolved.rotation;
            dash_call.color = cam_color;
            dash_call.opacity = 1.0;
            
            // params: [dash_length, gap_length, border_width, _pad]
            // Use true pixel-relative sizing for normalized SDF
            let max_dim = entity.resolved.width.max(entity.resolved.height).max(1.0);
            let norm_border = cam_px_thickness / max_dim;
            let norm_dash = 24.0 / max_dim;
            let norm_gap = 16.0 / max_dim;
            
            dash_call.params = vec![norm_dash, norm_gap, norm_border, 0.0];
            gizmo_draws.push(dash_call);

            // Unity-style camera gizmo: triangle ABOVE the frame, pointing DOWN
            let tri_size = entity.resolved.width * 0.05;
            
            // Triangle center in local anchor space — ABOVE the viewport's top edge
            let orig_min_x = -entity.resolved.anchor_x * entity.resolved.width;
            let orig_min_y = -entity.resolved.anchor_y * entity.resolved.height;
            let local_tri_x = orig_min_x + entity.resolved.width * 0.5;
            let local_tri_y = orig_min_y - tri_size * 0.6; // Above top edge, outside viewport
            
            let cos_r = entity.resolved.rotation.cos();
            let sin_r = entity.resolved.rotation.sin();

            let mut tri_call = crate::ecs::components::draw::DrawCall::default();
            tri_call.kind = crate::ecs::components::draw::DrawKind::SolidRect; // Uses shapes shader
            tri_call.x = entity.resolved.x + local_tri_x * cos_r - local_tri_y * sin_r;
            tri_call.y = entity.resolved.y + local_tri_x * sin_r + local_tri_y * cos_r;
            tri_call.width = tri_size;
            tri_call.height = tri_size;
            tri_call.anchor_x = 0.5;
            tri_call.anchor_y = 0.5;
            // Rotate 180° (π radians) so triangle points DOWN toward the camera
            tri_call.rotation = entity.resolved.rotation + std::f32::consts::PI;
            tri_call.color = cam_color;
            tri_call.opacity = 1.0;
            // params: [shape_type=5 (triangle), corner_radius, border_width=0 (filled), _pad]
            tri_call.params = vec![5.0, 0.0, 0.0, 0.0];
            gizmo_draws.push(tri_call);
        }

        // NOTE: Selection overlays are NOT rendered inline anymore.
        // They are collected separately and rendered in a top-most editor pass.

        // ── Process Texture Requests ──
        for req in &entity.draw.texture_requests {
            match req {
                crate::ecs::components::draw::TextureRequest::LoadImage { key, asset_url } => {
                    texture_updates.push(crate::frame::TextureUpdate::LoadImage {
                        key: key.clone(),
                        path: asset_url.clone(),
                    });
                }
                crate::ecs::components::draw::TextureRequest::DecodeVideoFrame { key, asset_url, timestamp_secs } => {
                    texture_updates.push(crate::frame::TextureUpdate::DecodeVideoFrame {
                        key: key.clone(),
                        path: asset_url.clone(),
                        timestamp_secs: *timestamp_secs,
                        width: None,
                        height: None,
                    });
                }
                crate::ecs::components::draw::TextureRequest::RasterizeText { key, content, font_size, color, font_key, max_width, line_height, alignment } => {
                    texture_updates.push(crate::frame::TextureUpdate::RasterizeText {
                        key: key.clone(),
                        content: content.clone(),
                        font_size: *font_size,
                        color: *color,
                        font_key: font_key.clone(),
                        max_width: *max_width,
                        line_height: *line_height,
                        alignment: *alignment,
                    });
                }
            }
        }

        let iter = entity.draw.draw_calls.iter().chain(gizmo_draws.iter());

        for call in iter {
            // ── World units → pixel projection ──
            let w = call.width * sx;
            let h = call.height * sy;
            
            let cos_r = call.rotation.cos();
            let sin_r = call.rotation.sin();
            let dx = (0.5 - call.anchor_x) * w;
            let dy = (0.5 - call.anchor_y) * h;
            
            // Map world coordinates considering camera translation relative to top-left.
            let center_x = (call.x - cam_x) * sx + dx * cos_r - dy * sin_r;
            let center_y = (call.y - cam_y) * sy + dx * sin_r + dy * cos_r;
            let flat_x = center_x - w * 0.5;
            let flat_y = center_y - h * 0.5;

            // ── Fit mode UV parameters ──
            let iw = call.intrinsic_width;
            let ih = call.intrinsic_height;
            let (uv_offset, uv_scale) = call.fit_mode.calculate_uv(call.width, call.height, iw, ih, call.align_x, call.align_y);

            // Map Blend Mode string back to an ID, defaulting to Normal (0) if unmapped
            let blend_id = match call.blend_mode.to_lowercase().as_str() {
                "multiply" => 1,
                "screen" => 2,
                "overlay" => 3,
                "soft_light" => 4,
                "add" => 5,
                "difference" => 6,
                _ => 0,
            };

            let mut textures = Vec::new();
            if let Some(t) = &call.texture_key {
                textures.push(t.clone());
            }

            let shader = match call.kind {
                crate::ecs::components::draw::DrawKind::SolidRect => "shapes",
                crate::ecs::components::draw::DrawKind::SolidEllipse => "shapes",
                crate::ecs::components::draw::DrawKind::Texture => "composite",
                crate::ecs::components::draw::DrawKind::Text => "composite",
                crate::ecs::components::draw::DrawKind::Outline => "outline",
                crate::ecs::components::draw::DrawKind::Gizmo => "gizmo",
                crate::ecs::components::draw::DrawKind::CameraFrame => "composite",
                crate::ecs::components::draw::DrawKind::DashedRect => "dashed_rect",
            };

            let layer = if storages.get_component::<crate::ecs::components::CameraComponent>(&entity.id).is_some() { 9999 } else { entity.resolved.layer };
            
            flat_entities.push(FlatEntity {
                id: 0,
                x: flat_x,
                y: flat_y,
                width: w,
                height: h,
                rotation: call.rotation,
                opacity: call.opacity,
                blend_mode: blend_id,
                color: call.color,
                shader: shader.to_string(),
                textures,
                params: call.params.clone(),
                layer,
                z_index: layer as f32,
                fit_mode: match call.fit_mode {
                    crate::ecs::components::FitMode::Contain => 1,
                    crate::ecs::components::FitMode::Cover => 2,
                    _ => 0, // Stretch
                },
                uv_offset,
                uv_scale,
                intrinsic_width: iw,
                intrinsic_height: ih,
            });
        }
    }

    // ── Build selection outline via off-screen post-process ──
    // Instead of a simple SolidRect border, we render the selected entity to an
    // off-screen mask, apply the selection_outline effect shader, and composite
    // the result as a fullscreen overlay on top of all entities.
    let mut sel_mask_entities = Vec::new();
    if is_editor_mode {
        for entity in &sorted {
            if !entity.resolved.visible { continue; }
            if !_selected_entity_ids.contains(&entity.id.as_str()) { continue; }

            let is_camera = storages.get_component::<crate::ecs::components::CameraComponent>(&entity.id).is_some();
            if is_camera { continue; }

            if select_mode == "content" {
                // Generate silhouette mask based on exact pixel boundaries (Content Mode)
                for call in entity.draw.draw_calls.iter() {
                    let w = call.width * sx;
                    let h = call.height * sy;

                    let cos_r = call.rotation.cos();
                    let sin_r = call.rotation.sin();
                    let dx = (0.5 - call.anchor_x) * w;
                    let dy = (0.5 - call.anchor_y) * h;
                    let center_x = (call.x - cam_x) * sx + dx * cos_r - dy * sin_r;
                    let center_y = (call.y - cam_y) * sy + dx * sin_r + dy * cos_r;
                    let flat_x = center_x - w * 0.5;
                    let flat_y = center_y - h * 0.5;
                    let iw = call.intrinsic_width;
                    let ih = call.intrinsic_height;
                    let (uv_offset, uv_scale) = call.fit_mode.calculate_uv(call.width, call.height, iw, ih, call.align_x, call.align_y);

                    let mut textures = Vec::new();
                    if let Some(t) = &call.texture_key {
                        textures.push(t.to_string());
                    }

                    let shader = match call.kind {
                        crate::ecs::components::draw::DrawKind::SolidRect => "shapes",
                        crate::ecs::components::draw::DrawKind::SolidEllipse => "shapes",
                        crate::ecs::components::draw::DrawKind::Texture => "composite",
                        crate::ecs::components::draw::DrawKind::Text => "composite",
                        _ => "composite",
                    };

                    sel_mask_entities.push(FlatEntity {
                        id: 0,
                        x: flat_x,
                        y: flat_y,
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
                        fit_mode: match call.fit_mode {
                            crate::ecs::components::FitMode::Contain => 1,
                            crate::ecs::components::FitMode::Cover => 2,
                            _ => 0,
                        },
                        uv_offset,
                        uv_scale,
                        intrinsic_width: iw,
                        intrinsic_height: ih,
                    });
                }
            } else if select_mode == "rect" {
                // Render a solid rectangle mask for the full layer bounds, which the outline shader will trace and glow
                let r = &entity.resolved;
                let cb_w = r.width;
                let cb_h = r.height;

                let cos_r = r.rotation.cos();
                let sin_r = r.rotation.sin();

                // Local center of the bounding box relative to the anchor
                let cx = cb_w * 0.5 - r.width * r.anchor_x;
                let cy = cb_h * 0.5 - r.height * r.anchor_y;

                let cb_x_rot = cx * cos_r - cy * sin_r;
                let cb_y_rot = cx * sin_r + cy * cos_r;

                let sel_w = cb_w * sx;
                let sel_h = cb_h * sy;
                let sel_x = (r.x - cam_x) * sx + cb_x_rot * sx - sel_w * 0.5;
                let sel_y = (r.y - cam_y) * sy + cb_y_rot * sy - sel_h * 0.5;

                sel_mask_entities.push(FlatEntity {
                    id: 0,
                    x: sel_x,
                    y: sel_y,
                    width: sel_w,
                    height: sel_h,
                    rotation: r.rotation,
                    opacity: 1.0,
                    blend_mode: 0,
                    color: [1.0, 1.0, 1.0, 1.0], // Solid white mask
                    shader: "shapes".to_string(), // Use shapes shader to draw solid rect
                    textures: vec![],
                    params: vec![0.0, 0.0, 0.0, 0.0], // params[0] = 0.0 (SolidRect)
                    layer: entity.resolved.layer,
                    z_index: entity.resolved.layer as f32,
                    fit_mode: 0,
                    uv_offset: [0.0, 0.0],
                    uv_scale: [1.0, 1.0],
                    intrinsic_width: 0.0,
                    intrinsic_height: 0.0,
                });
            }
        }
    }

    // ── Emit render passes ──
    let has_selection_outline = !sel_mask_entities.is_empty();

    // Pass 1: Selection mask (render selected entity to off-screen texture)
    if has_selection_outline {
        passes.push(RenderPass {
            output: "_sel_mask".into(),
            pass_type: PassType::Entities {
                entities: sel_mask_entities,
                clear_color: [0.0, 0.0, 0.0, 0.0], // Transparent background
            },
            target_width: None,
            target_height: None,
        });

        // Pass 2: Selection outline effect (edge detection on mask)
        passes.push(RenderPass {
            output: "_sel_outline".into(),
            pass_type: PassType::Effect {
                shader: "selection_outline".into(),
                inputs: vec!["_sel_mask".into()],
                params: vec![3.0, 0.0, 0.0, 0.0], // thickness=3px
            },
            target_width: None,
            target_height: None,
        });
    }

    // Pass 3: Main entities pass (includes outline composite if selection active)
    if !flat_entities.is_empty() || has_selection_outline {
        // Add selection outline as a fullscreen composite entity at top layer
        if has_selection_outline {
            flat_entities.push(FlatEntity {
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

        passes.push(RenderPass {
            output: "main".into(),
            pass_type: PassType::Entities {
                entities: flat_entities,
                clear_color: [0.0, 0.0, 0.0, 0.0]
            },
            target_width: None,
            target_height: None,
        });

        passes.push(RenderPass {
            output: "final".into(),
            pass_type: PassType::Output { input: "main".into() },
            target_width: None,
            target_height: None,
        });
    }

    Frame {
        passes,
        texture_updates,
    }
}
