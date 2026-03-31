use crate::ecs::World;
use crate::frame::{FlatEntity, Frame, PassType, RenderPass};

/// Compiles the ECS World's DrawCalls into a renderable Frame.
/// This Replaces the V2 `compiler.rs` payload, running pure data translations.
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
    context: &crate::ecs::ContextView,
) -> Frame {
    let mut passes = Vec::new();
    let mut texture_updates = Vec::new();
    let mut audio_calls = Vec::new();
    let storages = &world.storages;

    // ── Camera projection: world units → pixels ──
    let cam = world.find_camera(camera_id);
    let cam_top_left_x = cam.map(|c| c.resolved.x - c.resolved.width * 0.5).unwrap_or(0.0);
    let cam_top_left_y = cam.map(|c| c.resolved.y - c.resolved.height * 0.5).unwrap_or(0.0);

    let cam_x = custom_cam_x.unwrap_or(cam_top_left_x);
    let cam_y = custom_cam_y.unwrap_or(cam_top_left_y);
    let cam_w = custom_cam_w
        .unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0))
        .max(1.0);
    let cam_h = custom_cam_h
        .unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0))
        .max(1.0);
    let sx = screen_width as f32 / cam_w;
    let sy = screen_height as f32 / cam_h;


    let cam_mask = cam
        .and_then(|c| storages.get_component::<crate::ecs::components::CameraComponent>(&c.id))
        .map(|c| c.culling_mask)
        .unwrap_or(crate::ecs::RENDER_MASK_ALL);

    // ── Pre-discover Composition Cameras ──
    let sorted = world.sorted_by_layer();
    let storages = &world.storages;
    
    let mut comp_cameras = std::collections::HashMap::new();
    for entity in &sorted {
        if !entity.resolved.visible { continue; }
        if !context.active_entities.contains(&entity.id) { continue; }
        if storages.get_component::<crate::ecs::components::Composition>(&entity.id).is_some() {
            let mut cam_ent = None;
            for c_ent in &sorted {
                if !c_ent.resolved.visible { continue; }
                if storages.get_component::<crate::ecs::components::CameraComponent>(&c_ent.id).is_some() {
                    // Check if c_ent is a DIRECT child of entity (not deeper nested)
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
                // camera world pos - comp entity's anchor * cam resolution
                // This maps so that entities at the comp's center project to the center of the inner buffer.
                let inner_cam_x = c.resolved.x - c.resolved.anchor_x * cw;
                let inner_cam_y = c.resolved.y - c.resolved.anchor_y * ch;
                comp_cameras.insert(entity.id.clone(), (inner_cam_x, inner_cam_y, cw, ch, mask));
            } else {
                log::warn!("Composition '{}' has no direct child CameraComponent, skipping render", entity.id);
            }
        }
    }

    let mut camera_effects: Vec<crate::ecs::components::draw::EffectPassDef> = Vec::new();
    let mut layer_effects_map = std::collections::HashMap::new();

    // ── Phase 1: Group flat entities by TARGET COMPOSITION ──
    let mut comp_lists: std::collections::HashMap<String, Vec<crate::frame::FlatEntity>> = std::collections::HashMap::new();
    let mut comp_entities = Vec::new();

    for entity in &sorted {
        if !entity.resolved.visible { continue; }
        if !context.active_entities.contains(&entity.id) { continue; }

        // When scoped into a comp, the comp entity itself is INVISIBLE.
        // Only its children render (they already map to "main" via scope matching).
        if context.scope_id == Some(entity.id.as_str()) {
            // Still process texture requests and audio for the comp entity
            for req in &entity.draw.texture_requests {
                match req {
                    crate::ecs::components::draw::TextureRequest::LoadImage { key, asset_url } => {
                        texture_updates.push(crate::frame::TextureUpdate::LoadImage { key: key.clone(), path: asset_url.clone() });
                    }
                    crate::ecs::components::draw::TextureRequest::LoadFont { key, asset_url } => {
                        texture_updates.push(crate::frame::TextureUpdate::LoadFont { key: key.clone(), path: asset_url.clone() });
                    }
                    crate::ecs::components::draw::TextureRequest::DecodeVideoFrame { key, asset_url, timestamp_secs } => {
                        texture_updates.push(crate::frame::TextureUpdate::DecodeVideoFrame { key: key.clone(), path: asset_url.clone(), timestamp_secs: *timestamp_secs, width: None, height: None });
                    }
                    crate::ecs::components::draw::TextureRequest::RasterizeText { key, content, font_size, color, font_key, max_width, line_height, alignment } => {
                        texture_updates.push(crate::frame::TextureUpdate::RasterizeText { key: key.clone(), content: content.clone(), font_size: *font_size, color: *color, font_key: font_key.clone(), max_width: *max_width, line_height: *line_height, alignment: *alignment });
                    }
                }
            }
            audio_calls.extend(entity.draw.audio_calls.iter().cloned());
            continue; // Skip all visual rendering for the scoped comp entity
        }

        let target_comp = {
            let mut cur = entity.id.clone();
            let mut found = "main".to_string();
            if let Some(e) = world.get(&cur) {
                if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&e.id) {
                    cur = pid.0.clone();
                } else { cur = "".to_string(); }
            } else { cur = "".to_string(); }
            
            for _ in 0..32 {
                if cur.is_empty() { break; }
                if let Some(e) = world.get(&cur) {
                    if storages.get_component::<crate::ecs::components::Composition>(&e.id).is_some() {
                        if context.scope_id == Some(cur.as_str()) {
                            found = "main".to_string();
                        } else {
                            found = cur.clone();
                        }
                        break;
                    }
                    if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&e.id) {
                        cur = pid.0.clone();
                    } else { break; }
                } else { break; }
            }
            found
        };

        if target_comp != "main" && !comp_cameras.contains_key(&target_comp) {
            continue; // Skip rendering components inside a camera-less composition!
        }

        if storages.get_component::<crate::ecs::components::Composition>(&entity.id).is_some() {
            if comp_cameras.contains_key(&entity.id) {
                comp_entities.push(entity.id.clone());
            }
        }

        let (mut local_cam_x, mut local_cam_y, _local_cam_w, _local_cam_h, local_mask) = if target_comp == "main" {
            (cam_x, cam_y, cam_w, cam_h, cam_mask)
        } else {
            let (cx, cy, cw, ch, mask) = comp_cameras.get(&target_comp).unwrap();
            (*cx, *cy, *cw, *ch, *mask)
        };

        if (entity.resolved.render_category & local_mask) == 0 {
            continue;
        }

        // ── Process Texture Requests ──
        for req in &entity.draw.texture_requests {
            match req {
                crate::ecs::components::draw::TextureRequest::LoadImage { key, asset_url } => {
                    texture_updates.push(crate::frame::TextureUpdate::LoadImage {
                        key: key.clone(),
                        path: asset_url.clone(),
                    });
                }
                crate::ecs::components::draw::TextureRequest::LoadFont { key, asset_url } => {
                    texture_updates.push(crate::frame::TextureUpdate::LoadFont {
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

        // ── Process Audio Calls ──
        audio_calls.extend(entity.draw.audio_calls.iter().cloned());

        let iter = entity.draw.draw_calls.iter();
        let r = &entity.resolved;

        // Partition effects by scope
        let mut padded_effects = Vec::new();
        for effect in &entity.draw.effect_chain {
            match effect.scope {
                crate::schema::v2::ShaderScope::Camera => camera_effects.push(effect.clone()),
                crate::schema::v2::ShaderScope::Layer => {
                    layer_effects_map.entry(entity.id.clone()).or_insert_with(Vec::new).push(effect.clone());
                }
                _ => padded_effects.push(effect.clone()),
            }
        }

        let is_selected_content = context.select_mode == "content" && context.selected_ids.contains(&entity.id);
        let has_effects = !padded_effects.is_empty() || is_selected_content;
        let mut pad = if has_effects { entity.draw.effect_padding } else { 0.0 };

        if is_selected_content {
            pad = pad.max(6.0); // Ensure buffer has room for the outline glow
        }

        let local_sx = if target_comp == "main" { sx } else { 1.0 };
        let local_sy = if target_comp == "main" { sy } else { 1.0 };

        let diag = (r.width * r.width + r.height * r.height).sqrt();
        let ew = (diag * local_sx + pad * 2.0).max(1.0).ceil();
        let eh = (diag * local_sy + pad * 2.0).max(1.0).ceil();

        let pass_cam_x = if has_effects { r.x - ew / (2.0 * local_sx) } else { local_cam_x };
        let pass_cam_y = if has_effects { r.y - eh / (2.0 * local_sy) } else { local_cam_y };

        let mut local_flat_list = Vec::new();

        for call in iter {
            let w = call.width * local_sx;
            let h = call.height * local_sy;

            let cos_r = call.rotation.cos();
            let sin_r = call.rotation.sin();

            let center_x = (call.x - pass_cam_x) * local_sx;
            let center_y = (call.y - pass_cam_y) * local_sy;
            let flat_x = center_x - w * 0.5;
            let flat_y = center_y - h * 0.5;

            let iw = call.intrinsic_width;
            let ih = call.intrinsic_height;
            let (uv_offset, uv_scale) = call.fit_mode.calculate_uv(call.width, call.height, iw, ih, call.align_x, call.align_y);

            let blend_id = match call.blend_mode.to_lowercase().as_str() {
                "multiply" => 1, "screen" => 2, "overlay" => 3, "soft_light" => 4, "add" => 5, "difference" => 6, "mask_in" => 11, "mask_out" => 12, _ => 0,
            };

            let mut textures = Vec::new();
            if let Some(t) = &call.texture_key { textures.push(t.clone()); }

            let shader = match call.kind {
                crate::ecs::components::draw::DrawKind::SolidRect => if blend_id == 11 { "shapes_mask_in" } else if blend_id == 12 { "shapes_mask_out" } else { "shapes" },
                crate::ecs::components::draw::DrawKind::SolidEllipse => if blend_id == 11 { "shapes_mask_in" } else if blend_id == 12 { "shapes_mask_out" } else { "shapes" },
                crate::ecs::components::draw::DrawKind::Texture => if blend_id == 11 { "composite_mask_in" } else if blend_id == 12 { "composite_mask_out" } else { "composite" },
                crate::ecs::components::draw::DrawKind::Text => if blend_id == 11 { "composite_mask_in" } else if blend_id == 12 { "composite_mask_out" } else { "composite" },
                crate::ecs::components::draw::DrawKind::Outline => "outline",
                crate::ecs::components::draw::DrawKind::Gizmo => "gizmo",
                crate::ecs::components::draw::DrawKind::CameraFrame => "composite",
                crate::ecs::components::draw::DrawKind::DashedRect => "dashed_rect",
            };

            let layer = if storages.get_component::<crate::ecs::components::CameraComponent>(&entity.id).is_some() { 9999 } else { entity.resolved.layer };

            let target_x = flat_x;
            let target_y = flat_y;

            local_flat_list.push(FlatEntity {
                id: 0,
                x: target_x, y: target_y,
                width: w, height: h,
                rotation: call.rotation,
                opacity: if has_effects { 1.0 } else { call.opacity },
                blend_mode: blend_id,
                color: call.color,
                shader: shader.to_string(),
                textures, params: call.params.clone(),
                layer, z_index: layer as f32,
                fit_mode: match call.fit_mode { crate::ecs::components::FitMode::Contain => 1, crate::ecs::components::FitMode::Cover => 2, _ => 0 },
                uv_offset, uv_scale,
                intrinsic_width: iw, intrinsic_height: ih,
            });
        }

        let flat_entities = comp_lists.entry(target_comp.clone()).or_default();

        if has_effects {
            let input_base_key = format!("_ent_src_{}", entity.id);
            passes.push(RenderPass {
                output: input_base_key.clone(),
                pass_type: PassType::Entities { entities: local_flat_list, clear_color: [0.0, 0.0, 0.0, 0.0] },
                target_width: Some(ew as u32), target_height: Some(eh as u32),
            });

            let mut current_key = input_base_key.clone();
            for (i, effect) in padded_effects.iter().enumerate() {
                let out_key = format!("_ent_fx_{}_{}", entity.id, i);
                passes.push(RenderPass {
                    output: out_key.clone(),
                    pass_type: PassType::Effect { shader: effect.shader_id.clone(), inputs: vec![current_key], params: effect.params.clone() },
                    target_width: Some(ew as u32), target_height: Some(eh as u32),
                });
                current_key = out_key;

                if effect.scope == crate::schema::v2::ShaderScope::Masked {
                    let masked_key = format!("_ent_masked_{}_{}", entity.id, i);
                    passes.push(RenderPass {
                        output: masked_key.clone(),
                        pass_type: PassType::Effect { shader: "mask_composite".to_string(), inputs: vec![current_key, input_base_key.clone()], params: vec![0.0, 0.0, 0.0, 0.0] },
                        target_width: Some(ew as u32), target_height: Some(eh as u32),
                    });
                    current_key = masked_key;
                }
            }

            let layer = if storages.get_component::<crate::ecs::components::CameraComponent>(&entity.id).is_some() { 9999 } else { entity.resolved.layer };

            flat_entities.push(FlatEntity {
                id: 0,
                x: (r.x - local_cam_x) * local_sx - ew * 0.5,
                y: (r.y - local_cam_y) * local_sy - eh * 0.5,
                width: ew, height: eh,
                rotation: 0.0,
                opacity: r.opacity,
                blend_mode: r.blend_mode.as_u32(),
                color: [1.0, 1.0, 1.0, 1.0],
                shader: if r.blend_mode.as_u32() == 11 { "composite_mask_in".to_string() } else if r.blend_mode.as_u32() == 12 { "composite_mask_out".to_string() } else { "composite".to_string() },
                textures: vec![current_key.clone()], params: vec![],
                layer, z_index: layer as f32,
                fit_mode: 0, uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                intrinsic_width: ew, intrinsic_height: eh,
            });

            if is_selected_content {
                let sel_key = format!("_ent_sel_{}", entity.id);
                passes.push(RenderPass {
                    output: sel_key.clone(),
                    pass_type: PassType::Effect {
                        shader: "selection_outline".to_string(),
                        inputs: vec![current_key.clone()],
                        params: vec![4.0, 0.0, 0.0, 0.0]
                    },
                    target_width: Some(ew as u32),
                    target_height: Some(eh as u32),
                });

                flat_entities.push(FlatEntity {
                    id: 0,
                    x: (r.x - local_cam_x) * local_sx - ew * 0.5,
                    y: (r.y - local_cam_y) * local_sy - eh * 0.5,
                    width: ew, height: eh,
                    rotation: 0.0,
                    opacity: 1.0,
                    blend_mode: 0,
                    color: [1.0, 1.0, 1.0, 1.0],
                    shader: "composite".to_string(),
                    textures: vec![sel_key], params: vec![],
                    layer: 999999, z_index: 999999.0,
                    fit_mode: 0, uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                    intrinsic_width: ew, intrinsic_height: eh,
                });
            }

        } else {
            flat_entities.extend(local_flat_list);
        }

        // ── Handle Adjustment Layer Effects ──
        if let Some(layer_fx) = layer_effects_map.get(&entity.id) {
            let fw = if target_comp == "main" { screen_width } else { comp_cameras.get(&target_comp).unwrap().2 as u32 };
            let fh = if target_comp == "main" { screen_height } else { comp_cameras.get(&target_comp).unwrap().3 as u32 };

            let flat_entities = comp_lists.entry(target_comp.clone()).or_default();
            if !flat_entities.is_empty() {
                let src_key = format!("_layer_src_{}", entity.id);
                passes.push(RenderPass {
                    output: src_key.clone(),
                    pass_type: PassType::Entities { entities: std::mem::take(flat_entities), clear_color: [0.0, 0.0, 0.0, 0.0] },
                    target_width: Some(fw), target_height: Some(fh),
                });

                let mut current_key = src_key;
                for (i, effect) in layer_fx.iter().enumerate() {
                    let out_key = format!("_layer_fx_{}_{}", entity.id, i);
                    passes.push(RenderPass {
                        output: out_key.clone(),
                        pass_type: PassType::Effect { shader: effect.shader_id.clone(), inputs: vec![current_key], params: effect.params.clone() },
                        target_width: Some(fw), target_height: Some(fh),
                    });
                    current_key = out_key;
                }

                flat_entities.push(FlatEntity {
                    id: 0, x: 0.0, y: 0.0,
                    width: fw as f32, height: fh as f32,
                    rotation: 0.0, opacity: 1.0, blend_mode: 0,
                    color: [1.0, 1.0, 1.0, 1.0],
                    shader: "composite".to_string(),
                    textures: vec![current_key], params: vec![],
                    layer: entity.resolved.layer, z_index: entity.resolved.layer as f32,
                    fit_mode: 0, uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                    intrinsic_width: 0.0, intrinsic_height: 0.0,
                });
            }
        }
    }

    // ── Phase 2: Compile Composition Buffers (Deepest First) ──
    let get_depth = |ent_id: &str| -> usize {
        let mut depth = 0;
        let mut cur = ent_id.to_string();
        for _ in 0..32 {
            if let Some(e) = world.get(&cur) {
                if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&e.id) {
                    depth += 1;
                    cur = pid.0.clone();
                } else { break; }
            } else { break; }
        }
        depth
    };
    
    comp_entities.sort_by_key(|id| std::cmp::Reverse(get_depth(id)));

    for comp_id in comp_entities {
        let comp_ent = world.get(&comp_id).unwrap();
        let (_, _, cw, ch, _) = comp_cameras.get(&comp_id).unwrap();
        
        let target_comp = {
            let mut found = "main".to_string();
            let mut cur = if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&comp_ent.id) { pid.0.clone() } else { String::new() };
            for _ in 0..32 {
                if cur.is_empty() { break; }
                if let Some(e) = world.get(&cur) {
                    if storages.get_component::<crate::ecs::components::Composition>(&e.id).is_some() {
                        if context.scope_id == Some(cur.as_str()) { found = "main".to_string(); } else { found = cur.clone(); }
                        break;
                    }
                    if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&e.id) { cur = pid.0.clone(); } else { break; }
                } else { break; }
            }
            found
        };

        let comp_tex_key = format!("_comp_out_{}", comp_id);
        let mut list = comp_lists.remove(&comp_id).unwrap_or_default();
        list.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.z_index.partial_cmp(&b.z_index).unwrap()));

        passes.push(RenderPass {
            output: comp_tex_key.clone(),
            pass_type: PassType::Entities { entities: list, clear_color: [0.0, 0.0, 0.0, 0.0] },
            target_width: Some(*cw as u32), target_height: Some(*ch as u32),
        });

        let (parent_cx, parent_cy, p_sx, p_sy) = if target_comp == "main" {
            (cam_x, cam_y, sx, sy)
        } else {
            // Use the parent comp's inner camera origin (world-top-left of the inner buffer).
            // pixel_x = (world_x - parent_inner_cam_x) * 1.0  (1:1 since inner buffer is in world units)
            let (pcx, pcy, _, _, _) = comp_cameras.get(&target_comp).unwrap();
            (*pcx, *pcy, 1.0_f32, 1.0_f32)
        };


        let original_ew = comp_ent.resolved.width * p_sx;
        let original_eh = comp_ent.resolved.height * p_sy;
        // Push the original comp normal output
        let (uv_offset, uv_scale) = comp_ent.resolved.fit_mode.calculate_uv(
            original_ew, original_eh, *cw, *ch, 0.5, 0.5
        );
        let center_x = (comp_ent.resolved.x - parent_cx) * p_sx;
        let center_y = (comp_ent.resolved.y - parent_cy) * p_sy;

        let flat = FlatEntity {
            id: 0,
            x: center_x - original_ew * 0.5, y: center_y - original_eh * 0.5,
            width: original_ew, height: original_eh,
            rotation: comp_ent.resolved.rotation,
            opacity: 1.0,
            blend_mode: comp_ent.resolved.blend_mode.as_u32(),
            shader: if comp_ent.resolved.blend_mode.as_u32() == 11 { "composite_mask_in".to_string() } else if comp_ent.resolved.blend_mode.as_u32() == 12 { "composite_mask_out".to_string() } else { "composite".to_string() },
            color: [1.0, 1.0, 1.0, 1.0],
            textures: vec![comp_tex_key.clone()], params: vec![],
            layer: comp_ent.resolved.layer, z_index: comp_ent.resolved.layer as f32,
            fit_mode: 0, uv_offset, uv_scale,
            intrinsic_width: *cw, intrinsic_height: *ch,
        };
        comp_lists.entry(target_comp.clone()).or_default().push(flat);

        // Push overlay gizmo pass if selected
        if context.selected_ids.contains(&comp_ent.id) && context.select_mode == "content" {
            let offset = 6.0;
            let pad_w = offset * 2.0;
            let pad_h = offset * 2.0;
            let target_cx = *cw + pad_w;
            let target_cy = *ch + pad_h;

            let out_key = format!("_comp_fx_{}", comp_id);
            passes.push(RenderPass {
                output: out_key.clone(),
                pass_type: PassType::Effect {
                    shader: "selection_outline".to_string(),
                    inputs: vec![comp_tex_key.clone()],
                    params: vec![4.0, 0.0, 0.0, 0.0]
                },
                target_width: Some(target_cx as u32),
                target_height: Some(target_cy as u32),
            });
            
            let ew = original_ew + pad_w * p_sx;
            let eh = original_eh + pad_h * p_sy;

            let gizmo_flat = FlatEntity {
                id: 0,
                x: center_x - ew * 0.5, y: center_y - eh * 0.5,
                width: ew, height: eh,
                rotation: comp_ent.resolved.rotation,
                opacity: 1.0,
                blend_mode: 0,
                shader: "composite".to_string(),
                color: [1.0, 1.0, 1.0, 1.0],
                textures: vec![out_key], params: vec![],
                layer: 999999, z_index: 999999.0,
                fit_mode: 0, uv_offset, uv_scale,
                intrinsic_width: target_cx, intrinsic_height: target_cy,
            };
            comp_lists.entry(target_comp).or_default().push(gizmo_flat);
        }
    }
    
    // Everything implicitly bubbled up to "main" (the Root)
    let mut flat_entities = comp_lists.remove("main").unwrap_or_default();
    
    // Sort main to ensure synthesized composition proxies are perfectly layered
    flat_entities.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.z_index.partial_cmp(&b.z_index).unwrap()));

    // ── Build Main Screen RenderPass ──

    // If no camera effects, we output directly to "main"
    let base_output = if camera_effects.is_empty() {
        "main".to_string()
    } else {
        "_camera_src".to_string()
    };

    passes.push(RenderPass {
        output: base_output.clone(),
        pass_type: PassType::Entities {
            entities: flat_entities,
            clear_color: [0.0, 0.0, 0.0, 0.0],
        },
        target_width: if camera_effects.is_empty() {
            None
        } else {
            Some(screen_width)
        },
        target_height: if camera_effects.is_empty() {
            None
        } else {
            Some(screen_height)
        },
    });

    // Loop over camera effects
    let mut current_cam_key = base_output;
    for (i, effect) in camera_effects.iter().enumerate() {
        let out_key = format!("_camera_fx_{}", i);

        passes.push(RenderPass {
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

    passes.push(RenderPass {
        output: "final".into(),
        pass_type: PassType::Output { 
            input: if camera_effects.is_empty() { "main".into() } else { current_cam_key },
            entities: vec![],
        },
        target_width: Some(screen_width),
        target_height: None,
    });

    Frame {
        passes,
        texture_updates,
        audio_calls,
    }
}
