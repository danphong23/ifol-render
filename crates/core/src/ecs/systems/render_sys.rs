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
    scope_entity_id: Option<&str>,
) -> Frame {
    let mut passes = Vec::new();
    let mut texture_updates = Vec::new();
    let mut audio_calls = Vec::new();
    let storages = &world.storages;

    // ── Camera projection: world units → pixels ──
    let cam = world.find_camera(camera_id);
    let cam_x = custom_cam_x.unwrap_or_else(|| cam.map(|c| c.resolved.x).unwrap_or(0.0));
    let cam_y = custom_cam_y.unwrap_or_else(|| cam.map(|c| c.resolved.y).unwrap_or(0.0));
    let cam_w = custom_cam_w
        .unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0))
        .max(1.0);
    let cam_h = custom_cam_h
        .unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0))
        .max(1.0);
    let sx = screen_width as f32 / cam_w;
    let sy = screen_height as f32 / cam_h;

    // ── Helper: check if entity is a descendant of (or IS) the scope entity ──
    let is_in_scope = |entity_id: &str| -> bool {
        if scope_entity_id.is_none() {
            return true;
        }
        let scope_id = scope_entity_id.unwrap();
        // The scope entity itself is always in scope
        if entity_id == scope_id {
            return true;
        }
        // Walk parent chain to see if any ancestor is the scope entity
        let mut current_id = entity_id.to_string();
        for _ in 0..32 {
            // max depth guard
            if let Some(e) = world.entities.iter().find(|e| e.id == current_id) {
                if let Some(pid) = storages
                    .get_component::<crate::ecs::components::meta::ParentId>(&e.id)
                    .map(|id| &id.0)
                {
                    if pid == scope_id {
                        return true;
                    }
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

    let mut camera_effects: Vec<crate::ecs::components::draw::EffectPassDef> = Vec::new();
    let mut layer_effects_map = std::collections::HashMap::new();

    // ── Phase 1: Group flat entities by TARGET COMPOSITION ──
    let mut comp_lists: std::collections::HashMap<String, Vec<crate::frame::FlatEntity>> = std::collections::HashMap::new();
    let mut comp_entities = Vec::new();
    
    let sorted = world.sorted_by_layer();
    let storages = &world.storages;

    for entity in &sorted {
        if !entity.resolved.visible {
            continue;
        }
        // Skip entities outside render scope
        if !is_in_scope(&entity.id) {
            continue;
        }

        if storages.get_component::<crate::ecs::components::Composition>(&entity.id).is_some() {
            comp_entities.push(entity.id.clone());
        }

        let target_comp = {
            let mut cur = entity.id.clone();
            let mut found = "main".to_string();
            // Start traversal upward from parent
            if let Some(e) = world.get(&cur) {
                if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&e.id) {
                    cur = pid.0.clone();
                } else { cur = "".to_string(); }
            } else { cur = "".to_string(); }
            
            for _ in 0..32 {
                if cur.is_empty() { break; }
                if let Some(e) = world.get(&cur) {
                    if storages.get_component::<crate::ecs::components::Composition>(&e.id).is_some() {
                        // When scoped: the scope entity IS the "root" — treat it as "main".
                        // Its direct children must render to the main output, not an orphaned buffer.
                        if scope_entity_id == Some(cur.as_str()) {
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
                crate::ecs::components::draw::TextureRequest::DecodeVideoFrame {
                    key,
                    asset_url,
                    timestamp_secs,
                } => {
                    texture_updates.push(crate::frame::TextureUpdate::DecodeVideoFrame {
                        key: key.clone(),
                        path: asset_url.clone(),
                        timestamp_secs: *timestamp_secs,
                        width: None,
                        height: None,
                    });
                }
                crate::ecs::components::draw::TextureRequest::RasterizeText {
                    key,
                    content,
                    font_size,
                    color,
                    font_key,
                    max_width,
                    line_height,
                    alignment,
                } => {
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
                    layer_effects_map
                        .entry(entity.id.clone())
                        .or_insert_with(Vec::new)
                        .push(effect.clone());
                }
                _ => padded_effects.push(effect.clone()),
            }
        }

        let has_effects = !padded_effects.is_empty();
        let pad = if has_effects {
            entity.draw.effect_padding
        } else {
            0.0
        };

        let diag = (r.width * r.width + r.height * r.height).sqrt();
        let ew = (diag * sx + pad * 2.0).max(1.0).ceil();
        let eh = (diag * sy + pad * 2.0).max(1.0).ceil();

        let (local_cam_x, local_cam_y) = if has_effects {
            (r.x - ew / (2.0 * sx), r.y - eh / (2.0 * sy))
        } else {
            (cam_x, cam_y)
        };

        let mut local_flat_list = Vec::new();

        for call in iter {
            // ── World units → pixel projection ──
            let w = call.width * sx;
            let h = call.height * sy;

            let cos_r = call.rotation.cos();
            let sin_r = call.rotation.sin();
            let dx = (0.5 - call.anchor_x) * w;
            let dy = (0.5 - call.anchor_y) * h;

            // Map world coordinates considering camera translation relative to top-left.
            let center_x = (call.x - local_cam_x) * sx + dx * cos_r - dy * sin_r;
            let center_y = (call.y - local_cam_y) * sy + dx * sin_r + dy * cos_r;
            let flat_x = center_x - w * 0.5;
            let flat_y = center_y - h * 0.5;

            // ── Fit mode UV parameters ──
            let iw = call.intrinsic_width;
            let ih = call.intrinsic_height;
            let (uv_offset, uv_scale) = call.fit_mode.calculate_uv(
                call.width,
                call.height,
                iw,
                ih,
                call.align_x,
                call.align_y,
            );

            let blend_id = match call.blend_mode.to_lowercase().as_str() {
                "multiply" => 1,
                "screen" => 2,
                "overlay" => 3,
                "soft_light" => 4,
                "add" => 5,
                "difference" => 6,
                "mask_in" => 11,
                "mask_out" => 12,
                _ => 0,
            };

            let mut textures = Vec::new();
            if let Some(t) = &call.texture_key {
                textures.push(t.clone());
            }

            let shader = match call.kind {
                crate::ecs::components::draw::DrawKind::SolidRect => {
                    if blend_id == 11 { "shapes_mask_in" } else if blend_id == 12 { "shapes_mask_out" } else { "shapes" }
                }
                crate::ecs::components::draw::DrawKind::SolidEllipse => {
                    if blend_id == 11 { "shapes_mask_in" } else if blend_id == 12 { "shapes_mask_out" } else { "shapes" }
                }
                crate::ecs::components::draw::DrawKind::Texture => {
                    if blend_id == 11 { "composite_mask_in" } else if blend_id == 12 { "composite_mask_out" } else { "composite" }
                }
                crate::ecs::components::draw::DrawKind::Text => {
                    if blend_id == 11 { "composite_mask_in" } else if blend_id == 12 { "composite_mask_out" } else { "composite" }
                }
                crate::ecs::components::draw::DrawKind::Outline => "outline",
                crate::ecs::components::draw::DrawKind::Gizmo => "gizmo",
                crate::ecs::components::draw::DrawKind::CameraFrame => "composite",
                crate::ecs::components::draw::DrawKind::DashedRect => "dashed_rect",
            };

            let layer = if storages
                .get_component::<crate::ecs::components::CameraComponent>(&entity.id)
                .is_some()
            {
                9999
            } else {
                entity.resolved.layer
            };

            // Force center of the local texture target if this goes to offscreen pass!
            let (target_x, target_y) = if has_effects {
                (ew * 0.5 - w * 0.5, eh * 0.5 - h * 0.5)
            } else {
                (flat_x, flat_y)
            };

            local_flat_list.push(FlatEntity {
                id: 0,
                x: target_x,
                y: target_y,
                width: w,
                height: h,
                rotation: call.rotation,
                opacity: if has_effects { 1.0 } else { call.opacity }, // Apply opacity at the end if effects are used
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

        let flat_entities = comp_lists.entry(target_comp.clone()).or_default();

        if has_effects {
            let input_base_key = format!("_ent_src_{}", entity.id);
            passes.push(RenderPass {
                output: input_base_key.clone(),
                pass_type: PassType::Entities {
                    entities: local_flat_list,
                    clear_color: [0.0, 0.0, 0.0, 0.0],
                },
                target_width: Some(ew as u32),
                target_height: Some(eh as u32),
            });

            let mut current_key = input_base_key.clone();
            for (i, effect) in padded_effects.iter().enumerate() {
                let out_key = format!("_ent_fx_{}_{}", entity.id, i);
                passes.push(RenderPass {
                    output: out_key.clone(),
                    pass_type: PassType::Effect {
                        shader: effect.shader_id.clone(),
                        inputs: vec![current_key],
                        params: effect.params.clone(),
                    },
                    target_width: Some(ew as u32),
                    target_height: Some(eh as u32),
                });
                current_key = out_key;

                if effect.scope == crate::schema::v2::ShaderScope::Masked {
                    let masked_key = format!("_ent_masked_{}_{}", entity.id, i);
                    passes.push(RenderPass {
                        output: masked_key.clone(),
                        pass_type: PassType::Effect {
                            shader: "mask_composite".to_string(),
                            inputs: vec![current_key, input_base_key.clone()],
                            params: vec![0.0, 0.0, 0.0, 0.0],
                        },
                        target_width: Some(ew as u32),
                        target_height: Some(eh as u32),
                    });
                    current_key = masked_key;
                }
            }

            let layer = if storages
                .get_component::<crate::ecs::components::CameraComponent>(&entity.id)
                .is_some()
            {
                9999
            } else {
                entity.resolved.layer
            };

            flat_entities.push(FlatEntity {
                id: 0,
                x: (r.x - cam_x) * sx - ew * 0.5,
                y: (r.y - cam_y) * sy - eh * 0.5,
                width: ew,
                height: eh,
                rotation: 0.0, // Pre-rotated inside offscreen
                opacity: r.opacity,
                blend_mode: r.blend_mode.as_u32(),
                color: [1.0, 1.0, 1.0, 1.0],
                shader: if r.blend_mode.as_u32() == 11 { "composite_mask_in".to_string() } else if r.blend_mode.as_u32() == 12 { "composite_mask_out".to_string() } else { "composite".to_string() },
                textures: vec![current_key],
                params: vec![],
                layer,
                z_index: layer as f32,
                fit_mode: 0,
                uv_offset: [0.0, 0.0],
                uv_scale: [1.0, 1.0],
                intrinsic_width: ew,
                intrinsic_height: eh,
            });
        } else {
            flat_entities.extend(local_flat_list);
        }

        // ── Handle Adjustment Layer Effects ──
        if let Some(layer_fx) = layer_effects_map.get(&entity.id) {
            let flat_entities = comp_lists.entry(target_comp.clone()).or_default();
            if !flat_entities.is_empty() {
                // 1. Render all accumulated entities underneath this layer
                let src_key = format!("_layer_src_{}", entity.id);
                passes.push(RenderPass {
                    output: src_key.clone(),
                    pass_type: PassType::Entities {
                        entities: std::mem::take(flat_entities), // FLUSH and clear
                        clear_color: [0.0, 0.0, 0.0, 0.0],
                    },
                    target_width: Some(screen_width),
                    target_height: Some(screen_height),
                });

                // 2. Apply effects to the accumulated frame
                let mut current_key = src_key;
                for (i, effect) in layer_fx.iter().enumerate() {
                    let out_key = format!("_layer_fx_{}_{}", entity.id, i);
                    passes.push(RenderPass {
                        output: out_key.clone(),
                        pass_type: PassType::Effect {
                            shader: effect.shader_id.clone(),
                            inputs: vec![current_key],
                            params: effect.params.clone(),
                        },
                        target_width: Some(screen_width),
                        target_height: Some(screen_height),
                    });
                    current_key = out_key;
                }

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
                    textures: vec![current_key],
                    params: vec![],
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
        
        let target_comp = {
            let mut found = "main".to_string();
            let mut cur = if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&comp_ent.id) {
                pid.0.clone()
            } else {
                String::new()
            };
            
            for _ in 0..32 {
                if cur.is_empty() { break; }
                if let Some(e) = world.get(&cur) {
                    if storages.get_component::<crate::ecs::components::Composition>(&e.id).is_some() {
                        // When scoped: the scope entity IS the "root" — treat it as "main".
                        // Its nested comp proxies must bubble up to main, not an orphaned buffer.
                        if scope_entity_id == Some(cur.as_str()) {
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

        let comp_tex_key = format!("_comp_out_{}", comp_id);
        let mut list = comp_lists.remove(&comp_id).unwrap_or_default();
        
        // Re-sort to guarantee synthesized proxies respect true Z-index
        list.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.z_index.partial_cmp(&b.z_index).unwrap()));

        // Render all child entities inside this Composition to an isolated Transparent Buffer
        passes.push(RenderPass {
            output: comp_tex_key.clone(),
            pass_type: PassType::Entities {
                entities: list,
                clear_color: [0.0, 0.0, 0.0, 0.0],
            },
            target_width: Some(screen_width),
            target_height: Some(screen_height),
        });

        // Push this finished composition to ITS parent's layer stack
        let flat = FlatEntity {
            id: 0,
            x: 0.0, y: 0.0,
            width: screen_width as f32,
            height: screen_height as f32,
            rotation: 0.0,
            opacity: 1.0, // Fixed to 1.0 to prevent double-application of opacity since children already inherited it
            blend_mode: comp_ent.resolved.blend_mode.as_u32(),
            shader: if comp_ent.resolved.blend_mode.as_u32() == 11 { "composite_mask_in".to_string() } else if comp_ent.resolved.blend_mode.as_u32() == 12 { "composite_mask_out".to_string() } else { "composite".to_string() },
            color: [1.0, 1.0, 1.0, 1.0],
            textures: vec![comp_tex_key],
            params: vec![],
            layer: comp_ent.resolved.layer,
            z_index: comp_ent.resolved.layer as f32,
            fit_mode: 0,
            uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
            intrinsic_width: 0.0, intrinsic_height: 0.0,
        };
        comp_lists.entry(target_comp).or_default().push(flat);
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
