use crate::ecs::{World, ContextView};
use crate::ecs::systems::render::state::RenderState;
use crate::frame::{FlatEntity, PassType, RenderPass};

/// Phase 1: Group flat entities by TARGET COMPOSITION
pub fn build_entity_passes(
    world: &World,
    context: &ContextView,
    state: &mut RenderState,
    sorted: &Vec<&crate::ecs::Entity>,
) {
    let storages = &world.storages;
    let mut layer_effects_map = std::collections::HashMap::new();

    for entity in sorted {
        if !entity.resolved.visible { continue; }
        if !context.active_entities.contains(&entity.id) { continue; }

        // When scoped into a comp, the comp entity itself is INVISIBLE.
        // Only its children render (they already map to "main" via scope matching).
        if context.scope_id == Some(entity.id.as_str()) {
            // Process texture requests for the comp entity
            for req in &entity.draw.texture_requests {
                match req {
                    crate::ecs::components::draw::TextureRequest::LoadImage { key, asset_url } => {
                        state.texture_updates.push(crate::frame::TextureUpdate::LoadImage { key: key.clone(), path: asset_url.clone() });
                    }
                    crate::ecs::components::draw::TextureRequest::LoadFont { key, asset_url } => {
                        state.texture_updates.push(crate::frame::TextureUpdate::LoadFont { key: key.clone(), path: asset_url.clone() });
                    }
                    crate::ecs::components::draw::TextureRequest::DecodeVideoFrame { key, asset_url, timestamp_secs } => {
                        state.texture_updates.push(crate::frame::TextureUpdate::DecodeVideoFrame { key: key.clone(), path: asset_url.clone(), timestamp_secs: *timestamp_secs, width: None, height: None });
                    }
                    crate::ecs::components::draw::TextureRequest::RasterizeText { key, content, font_size, color, font_key, max_width, line_height, alignment } => {
                        state.texture_updates.push(crate::frame::TextureUpdate::RasterizeText { key: key.clone(), content: content.clone(), font_size: *font_size, color: *color, font_key: font_key.clone(), max_width: *max_width, line_height: *line_height, alignment: *alignment });
                    }
                }
            }
            state.audio_calls.extend(entity.draw.audio_calls.iter().cloned());
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

        if target_comp != "main" && !state.comp_cameras.contains_key(&target_comp) {
            continue; // Skip rendering components inside a camera-less composition!
        }

        if storages.get_component::<crate::ecs::components::Composition>(&entity.id).is_some() {
            if state.comp_cameras.contains_key(&entity.id) {
                state.comp_entities.push(entity.id.clone());
            }
        }

        let (local_cam_x, local_cam_y, local_cam_w, local_cam_h, local_mask) = if target_comp == "main" {
            (state.root_cam_x, state.root_cam_y, state.root_cam_w, state.root_cam_h, state.root_cam_mask)
        } else {
            let (cx, cy, cw, ch, mask) = state.comp_cameras.get(&target_comp).unwrap();
            (*cx, *cy, *cw, *ch, *mask)
        };

        if (entity.resolved.render_category & local_mask) == 0 {
            continue;
        }

        // ── Process Texture Requests ──
        for req in &entity.draw.texture_requests {
            match req {
                crate::ecs::components::draw::TextureRequest::LoadImage { key, asset_url } => {
                    state.texture_updates.push(crate::frame::TextureUpdate::LoadImage {
                        key: key.clone(),
                        path: asset_url.clone(),
                    });
                }
                crate::ecs::components::draw::TextureRequest::LoadFont { key, asset_url } => {
                    state.texture_updates.push(crate::frame::TextureUpdate::LoadFont {
                        key: key.clone(),
                        path: asset_url.clone(),
                    });
                }
                crate::ecs::components::draw::TextureRequest::DecodeVideoFrame { key, asset_url, timestamp_secs } => {
                    state.texture_updates.push(crate::frame::TextureUpdate::DecodeVideoFrame {
                        key: key.clone(),
                        path: asset_url.clone(),
                        timestamp_secs: *timestamp_secs,
                        width: None,
                        height: None,
                    });
                }
                crate::ecs::components::draw::TextureRequest::RasterizeText { key, content, font_size, color, font_key, max_width, line_height, alignment } => {
                    state.texture_updates.push(crate::frame::TextureUpdate::RasterizeText {
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
        state.audio_calls.extend(entity.draw.audio_calls.iter().cloned());

        let iter = entity.draw.draw_calls.iter();
        let r = &entity.resolved;

        // Partition effects by scope
        let mut padded_effects = Vec::new();
        for effect in &entity.draw.effect_chain {
            match effect.scope {
                crate::schema::v2::ShaderScope::Camera => state.camera_effects.push(effect.clone()),
                crate::schema::v2::ShaderScope::Layer => {
                    layer_effects_map.entry(entity.id.clone()).or_insert_with(Vec::new).push(effect.clone());
                }
                _ => padded_effects.push(effect.clone()),
            }
        }

        let has_effects = !padded_effects.is_empty();
        let pad = if has_effects { entity.draw.effect_padding } else { 0.0 };

        let local_sx = if target_comp == "main" { state.root_sx } else { 1.0 };
        let local_sy = if target_comp == "main" { state.root_sy } else { 1.0 };

        let diag = (r.width * r.width + r.height * r.height).sqrt();
        let ew = (diag * local_sx + pad * 2.0).max(1.0).ceil();
        let eh = (diag * local_sy + pad * 2.0).max(1.0).ceil();

        let pass_cam_x = if has_effects { r.x - ew / (2.0 * local_sx) } else { local_cam_x };
        let pass_cam_y = if has_effects { r.y - eh / (2.0 * local_sy) } else { local_cam_y };

        let mut local_flat_list = Vec::new();

        for call in iter {
            let w = call.width * local_sx;
            let h = call.height * local_sy;

            let center_x = (call.x - pass_cam_x) * local_sx;
            let center_y = (call.y - pass_cam_y) * local_sy;
            let flat_x = center_x - w * 0.5;
            let flat_y = center_y - h * 0.5;

            let iw = call.intrinsic_width;
            let ih = call.intrinsic_height;
            let (uv_offset, uv_scale) = call.fit_mode.calculate_uv(call.width, call.height, iw, ih, call.align_x, call.align_y);

            let blend_id = match call.blend_mode.to_lowercase().as_str() {
                "multiply" => 1, "screen" => 2, "overlay" => 3, "add" => 4,
                "subtract" => 5, "darken" => 6, "lighten" => 7, "soft_light" => 8,
                "hard_light" => 9, "difference" => 10, "mask_in" => 11, "mask_out" => 12,
                _ => 0,
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

            let mut fe = FlatEntity {
                id: 0, content_hash: 0,
                x: flat_x, y: flat_y,
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
            };
            fe.content_hash = fe.calculate_hash();
            local_flat_list.push(fe);
        }

        let mut local_entities_to_push = Vec::new();

        if has_effects {
            let input_base_key = format!("_ent_src_{}", entity.id);
            state.push_pass(RenderPass { pass_hash: 0,
                output: input_base_key.clone(),
                pass_type: PassType::Entities { entities: local_flat_list, clear_color: [0.0, 0.0, 0.0, 0.0] },
                target_width: Some(ew as u32), target_height: Some(eh as u32),
            });

            let mut current_key = input_base_key.clone();
            for (i, effect) in padded_effects.iter().enumerate() {
                let out_key = format!("_ent_fx_{}_{}", entity.id, i);
                state.push_pass(RenderPass { pass_hash: 0,
                    output: out_key.clone(),
                    pass_type: PassType::Effect { shader: effect.shader_id.clone(), inputs: vec![current_key], params: effect.params.clone() },
                    target_width: Some(ew as u32), target_height: Some(eh as u32),
                });
                current_key = out_key;

                if effect.scope == crate::schema::v2::ShaderScope::Masked {
                    let masked_key = format!("_ent_masked_{}_{}", entity.id, i);
                    state.push_pass(RenderPass { pass_hash: 0,
                        output: masked_key.clone(),
                        pass_type: PassType::Effect { shader: "mask_composite".to_string(), inputs: vec![current_key, input_base_key.clone()], params: vec![0.0, 0.0, 0.0, 0.0] },
                        target_width: Some(ew as u32), target_height: Some(eh as u32),
                    });
                    current_key = masked_key;
                }
            }

            let layer = if storages.get_component::<crate::ecs::components::CameraComponent>(&entity.id).is_some() { 9999 } else { entity.resolved.layer };
            let blend_id = r.blend_mode.as_u32();

            // T2.4: If blend mode requires reading the destination (Multiply/Screen/Overlay/etc.),
            // inject a snapshot pass + blend_composite pass for accurate per-pixel blending.
            // Modes 0 (Normal), 11 (mask_in), 12 (mask_out) use hardware blending — skip.
            let composite_tex = if blend_id != 0 && blend_id != 11 && blend_id != 12 {
                let dst_key = format!("_bdst_{}", entity.id);
                let bout_key = format!("_bout_{}", entity.id);
                let acc_key = state.current_acc_key.clone().unwrap_or_else(|| "final_acc".to_string());

                // 1. Snapshot current accumulation → dst for blend math
                state.push_pass(RenderPass { pass_hash: 0,
                    output: dst_key.clone(),
                    pass_type: PassType::Snapshot { source_key: acc_key },
                    target_width: Some(state.screen_width), target_height: Some(state.screen_height),
                });

                // 2. Run blend_composite: src=entity texture, dst=snapshot → blended output
                state.push_pass(RenderPass { pass_hash: 0,
                    output: bout_key.clone(),
                    pass_type: PassType::Effect {
                        shader: "blend_composite".to_string(),
                        inputs: vec![current_key.clone(), dst_key],
                        params: vec![blend_id as f32, r.opacity, 0.0, 0.0],
                    },
                    target_width: Some(state.screen_width), target_height: Some(state.screen_height),
                });

                bout_key
            } else {
                current_key.clone()
            };

            // 3. Add Normal-blend entity compositing the final texture into accumulation.
            // For blend modes: opacity is already baked into blend_composite output — use 1.0.
            // For Normal/Mask modes: use actual opacity.
            let (final_opacity, final_blend) = if blend_id != 0 && blend_id != 11 && blend_id != 12 {
                (1.0, 0u32) // Blend math baked in — composite with Normal at full opacity
            } else {
                (r.opacity, blend_id)
            };

            let mut fe = FlatEntity {
                id: 0, content_hash: 0,
                x: (r.x - local_cam_x) * local_sx - ew * 0.5,
                y: (r.y - local_cam_y) * local_sy - eh * 0.5,
                width: ew, height: eh,
                rotation: 0.0,
                opacity: final_opacity,
                blend_mode: final_blend,
                color: [1.0, 1.0, 1.0, 1.0],
                shader: if final_blend == 11 { "composite_mask_in".to_string() } else if final_blend == 12 { "composite_mask_out".to_string() } else { "composite".to_string() },
                textures: vec![composite_tex], params: vec![],
                layer, z_index: layer as f32,
                fit_mode: 0, uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                intrinsic_width: ew, intrinsic_height: eh,
            };
            fe.content_hash = fe.calculate_hash();
            local_entities_to_push.push(fe);

        } else {
            // No effects — entities go directly into the accumulation list.
            // For non-Normal blend modes without effects, inject 2-pass blend for each draw call
            // that has a non-Normal blend mode. Most entities only have 1 draw call.
            let has_non_normal = local_flat_list.iter().any(|e| e.blend_mode != 0 && e.blend_mode != 11 && e.blend_mode != 12);
            if has_non_normal {
                for fe in local_flat_list {
                    let blend_id = fe.blend_mode;
                    if blend_id == 0 || blend_id == 11 || blend_id == 12 {
                        // Normal or mask — push directly
                        local_entities_to_push.push(fe);
                    } else {
                        // T2.4b: non-Normal blend without effects — inject 2-pass blend
                        let src_key = format!("_bsrc_{}_{}", entity.id, blend_id);
                        let dst_key = format!("_bdst_{}_{}", entity.id, blend_id);
                        let bout_key = format!("_bout_{}_{}", entity.id, blend_id);
                        let acc_key = state.current_acc_key.clone().unwrap_or_else(|| "final_acc".to_string());

                        // Render isolated into src
                        let mut src_fe = fe.clone();
                        src_fe.blend_mode = 0; // Render as Normal into isolated buffer
                        src_fe.opacity = 1.0;
                        // Position in isolated pass (same origin as entity center)
                        let iso_w = fe.width.max(1.0).ceil() as u32;
                        let iso_h = fe.height.max(1.0).ceil() as u32;
                        state.push_pass(RenderPass { pass_hash: 0,
                            output: src_key.clone(),
                            pass_type: PassType::Entities { entities: vec![src_fe], clear_color: [0.0, 0.0, 0.0, 0.0] },
                            target_width: Some(iso_w), target_height: Some(iso_h),
                        });

                        // Snapshot accumulation
                        state.push_pass(RenderPass { pass_hash: 0,
                            output: dst_key.clone(),
                            pass_type: PassType::Snapshot { source_key: acc_key },
                            target_width: Some(state.screen_width), target_height: Some(state.screen_height),
                        });

                        // blend_composite
                        state.push_pass(RenderPass { pass_hash: 0,
                            output: bout_key.clone(),
                            pass_type: PassType::Effect {
                                shader: "blend_composite".to_string(),
                                inputs: vec![src_key, dst_key],
                                params: vec![blend_id as f32, fe.opacity, 0.0, 0.0],
                            },
                            target_width: Some(state.screen_width), target_height: Some(state.screen_height),
                        });

                        let mut fe_composite = FlatEntity {
                            id: 0, content_hash: 0,
                            x: 0.0, y: 0.0,
                            width: state.screen_width as f32, height: state.screen_height as f32,
                            rotation: 0.0, opacity: 1.0, blend_mode: 0,
                            color: [1.0, 1.0, 1.0, 1.0],
                            shader: "composite".to_string(),
                            textures: vec![bout_key], params: vec![],
                            layer: fe.layer, z_index: fe.z_index,
                            fit_mode: 0, uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                            intrinsic_width: 0.0, intrinsic_height: 0.0,
                        };
                        fe_composite.content_hash = fe_composite.calculate_hash();
                        local_entities_to_push.push(fe_composite);
                    }
                }
            } else {
                local_entities_to_push.extend(local_flat_list);
            }
        }


        if !local_entities_to_push.is_empty() {
            state.comp_lists.entry(target_comp.clone()).or_default().extend(local_entities_to_push);
        }

        // ── Handle Adjustment Layer Effects ──
        if let Some(layer_fx) = layer_effects_map.get(&entity.id) {
            let fw = if target_comp == "main" { state.screen_width } else { state.comp_cameras.get(&target_comp).unwrap().2 as u32 };
            let fh = if target_comp == "main" { state.screen_height } else { state.comp_cameras.get(&target_comp).unwrap().3 as u32 };

            let taken_entities = std::mem::take(state.comp_lists.entry(target_comp.clone()).or_default());
            if !taken_entities.is_empty() {
                let src_key = format!("_layer_src_{}", entity.id);
                state.push_pass(RenderPass { pass_hash: 0,
                    output: src_key.clone(),
                    pass_type: PassType::Entities { entities: taken_entities, clear_color: [0.0, 0.0, 0.0, 0.0] },
                    target_width: Some(fw), target_height: Some(fh),
                });

                let mut current_key = src_key;
                for (i, effect) in layer_fx.iter().enumerate() {
                    let out_key = format!("_layer_fx_{}_{}", entity.id, i);
                    state.push_pass(RenderPass { pass_hash: 0,
                        output: out_key.clone(),
                        pass_type: PassType::Effect { shader: effect.shader_id.clone(), inputs: vec![current_key], params: effect.params.clone() },
                        target_width: Some(fw), target_height: Some(fh),
                    });
                    current_key = out_key;
                }

                let mut fe = FlatEntity {
                    id: 0, content_hash: 0, x: 0.0, y: 0.0,
                    width: fw as f32, height: fh as f32,
                    rotation: 0.0, opacity: 1.0, blend_mode: 0,
                    color: [1.0, 1.0, 1.0, 1.0],
                    shader: "composite".to_string(),
                    textures: vec![current_key], params: vec![],
                    layer: entity.resolved.layer, z_index: entity.resolved.layer as f32,
                    fit_mode: 0, uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                    intrinsic_width: 0.0, intrinsic_height: 0.0,
                };
                fe.content_hash = fe.calculate_hash();
                state.comp_lists.entry(target_comp.clone()).or_default().push(fe);
            }
        }
    }
}
