pub mod state;
pub mod effect_pass;
pub mod composition_pass;

use crate::ecs::{World, ContextView};
use crate::ecs::components::camera::CameraRenderMode;
use crate::ecs::systems::render::state::SceneCameraInfo;
use crate::frame::{Frame, PassType, RenderPass};
use state::RenderState;

/// Compiles the ECS World's DrawCalls into a renderable Frame.
pub fn render_to_frame(
    world: &World,
    camera_id: &str,
    screen_width: u32,
    screen_height: u32,
    _time_secs: f64,
    context: &ContextView,
) -> Frame {
    let mut state = RenderState::new(screen_width, screen_height);
    let storages = &world.storages;

    // ── Pre-discover Main Camera ──
    let cam = world.find_camera(camera_id);
    let cam_top_left_x = cam.map(|c| c.resolved.x - c.resolved.width * 0.5).unwrap_or(0.0);
    let cam_top_left_y = cam.map(|c| c.resolved.y - c.resolved.height * 0.5).unwrap_or(0.0);

    state.root_cam_x = cam_top_left_x;
    state.root_cam_y = cam_top_left_y;
    state.root_cam_w = cam.map(|c| c.resolved.width).unwrap_or(1280.0).max(1.0);
    state.root_cam_h = cam.map(|c| c.resolved.height).unwrap_or(720.0).max(1.0);

    state.root_sx = screen_width as f32 / state.root_cam_w;
    state.root_sy = screen_height as f32 / state.root_cam_h;

    let root_cam_component = cam
        .and_then(|c| storages.get_component::<crate::ecs::components::CameraComponent>(&c.id));

    state.root_cam_mask = root_cam_component
        .map(|c| c.culling_mask)
        .unwrap_or(crate::ecs::RENDER_MASK_ALL);

    // ── Pre-discover Composition Cameras ──
    // Compute sorted entity list ONCE — reused by camera discovery and effect_pass.
    let sorted = world.sorted_by_layer();
    for entity in &sorted {
        if !entity.resolved.visible {
            if storages.get_component::<crate::ecs::components::Composition>(&entity.id).is_some() {
                log::debug!("[COMP] {} INVISIBLE at scope_time={:.3}", entity.id, entity.resolved.scope_time);
            }
            continue;
        }
        if !context.active_entities.contains(&entity.id) { continue; }
        if storages.get_component::<crate::ecs::components::Composition>(&entity.id).is_some() {
            log::debug!("[COMP] {} VISIBLE at scope_time={:.3}, content_time={:.3}", entity.id, entity.resolved.scope_time, entity.resolved.content_time);
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

                let inner_cam_x = c.resolved.x - cw * 0.5;
                let inner_cam_y = c.resolved.y - ch * 0.5;
                state.comp_cameras.insert(entity.id.clone(), (inner_cam_x, inner_cam_y, cw, ch, mask));
            } else {
                // Fallback virtual camera matching the composition's own bounds!
                // This prevents the composition from disappearing if the user forgets to add a camera.
                let cw = entity.resolved.width.max(1.0);
                let ch = entity.resolved.height.max(1.0);
                let inner_cam_x = entity.resolved.x - cw * 0.5;
                let inner_cam_y = entity.resolved.y - ch * 0.5;
                state.comp_cameras.insert(entity.id.clone(), (inner_cam_x, inner_cam_y, cw, ch, crate::ecs::RENDER_MASK_DEFAULT));
            }
        }
    }

    // ── Phase 5: Discover all root-level scene cameras ──
    // A "root-level" camera is one that is NOT a child of a Composition entity
    // and is not scoped into one. These drive multi-camera compositing.
    {
        let mut root_cameras: Vec<SceneCameraInfo> = Vec::new();

        for entity in &sorted {
            if !entity.resolved.visible { continue; }
            if !context.active_entities.contains(&entity.id) { continue; }
            let cam_comp = match storages.get_component::<crate::ecs::components::CameraComponent>(&entity.id) {
                Some(c) => c,
                None => continue,
            };

            // Skip cameras that are children of a Composition (those are nested cams)
            let is_root_cam = {
                let mut is_root = true;
                let mut cur = entity.id.clone();
                for _ in 0..32 {
                    if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&cur) {
                        if storages.get_component::<crate::ecs::components::Composition>(&pid.0).is_some() {
                            is_root = false;
                            break;
                        }
                        cur = pid.0.clone();
                    } else { break; }
                }
                is_root
            };
            if !is_root_cam { continue; }

            // Evaluate post_effects for this camera via material_sys
            let eval_post_fx = |mat_list: &Vec<crate::scene::MaterialV2>, scope_time: f64|
                -> Vec<crate::ecs::components::draw::EffectPassDef> {
                mat_list.iter().map(|mat| {
                    let (effect, _) = crate::ecs::systems::material_sys::evaluate_material_pub(
                        mat,
                        scope_time,
                        Some(crate::schema::v2::ShaderScope::Camera),
                    );
                    effect
                }).collect()
            };

            match cam_comp.render_mode {
                CameraRenderMode::Cameras => {
                    // Master compositor camera: defines an ORDERED list of sub-cameras.
                    // We delay processing until after all Layers cameras have been collected,
                    // so we can look them up by entity_id. Mark with a sentinel for now.
                    // (Handled in the second pass below)
                }
                CameraRenderMode::Layers => {
                    let post_fx = eval_post_fx(&cam_comp.post_effects, entity.resolved.scope_time);
                    root_cameras.push(SceneCameraInfo {
                        entity_id: entity.id.clone(),
                        render_order: cam_comp.render_order,
                        target_layers: cam_comp.target_layers.clone(),
                        culling_mask: cam_comp.culling_mask,
                        cam_x: entity.resolved.x - entity.resolved.width * 0.5,
                        cam_y: entity.resolved.y - entity.resolved.height * 0.5,
                        cam_w: entity.resolved.width.max(1.0),
                        cam_h: entity.resolved.height.max(1.0),
                        render_scale: cam_comp.render_scale,
                        post_effects: post_fx,
                    });
                }
            }
        }

        // Second pass: resolve CameraRenderMode::Cameras master cameras.
        // A master camera's target_cameras list OVERRIDES the auto-discovered order.
        // If any master camera exists, use its ordered sub-camera list instead.
        let master_cam = sorted.iter().find(|e| {
            if !e.resolved.visible { return false; }
            if !context.active_entities.contains(&e.id) { return false; }
            storages.get_component::<crate::ecs::components::CameraComponent>(&e.id)
                .map(|c| c.render_mode == CameraRenderMode::Cameras)
                .unwrap_or(false)
        });

        if let Some(master) = master_cam {
            if let Some(master_comp) = storages.get_component::<crate::ecs::components::CameraComponent>(&master.id) {
                if !master_comp.target_cameras.is_empty() {
                    // Re-order root_cameras to match master's target_cameras order
                    let ordered: Vec<SceneCameraInfo> = master_comp.target_cameras.iter()
                        .filter_map(|target_id| {
                            root_cameras.iter().find(|c| &c.entity_id == target_id).cloned()
                        })
                        .collect();

                    if !ordered.is_empty() {
                        // Use the master camera's ordered list
                        root_cameras = ordered;
                        log::debug!(
                            "Multi-cam: master '{}' compositing {} sub-cameras in order: {:?}",
                            master.id,
                            root_cameras.len(),
                            root_cameras.iter().map(|c| &c.entity_id).collect::<Vec<_>>()
                        );
                    }
                }
            }
        } else {
            // No master camera: sort all Layers cameras by render_order (auto composite)
            root_cameras.sort_by_key(|c| c.render_order);
        }

        state.scene_cameras = root_cameras;
    }

    // ── Phase 1: Compile Entities & Effects ──
    effect_pass::build_entity_passes(world, context, &mut state, &sorted);

    // ── Phase 2: Compile Nested Compositions ──
    composition_pass::compile_composition_buffers(world, context, &mut state);

    // Everything implicitly bubbled up to "main" (the Root)
    let flat_entities_all = state.comp_lists.remove("main").unwrap_or_default();

    // ── Phase 5: Multi-Camera Compositing ──
    // If multiple root cameras discovered, partition entities by each camera's target_layers
    // and composite them in order. Otherwise fall back to legacy single-pass.
    let has_multi_cameras = state.scene_cameras.len() > 1;

    if has_multi_cameras {
        build_multi_camera_passes(&mut state, flat_entities_all, screen_width, screen_height);
    } else {
        // Legacy single-camera path (backward compatible)
        build_single_camera_pass(&mut state, flat_entities_all, screen_width, screen_height);
    }

    log::debug!("[RENDER] Frame compiled: {} passes, {} tex_updates, {} audio_calls, comp_cameras: {:?}",
        state.passes.len(), state.texture_updates.len(), state.audio_calls.len(),
        state.comp_cameras.keys().collect::<Vec<_>>());

    Frame {
        passes: state.passes,
        texture_updates: state.texture_updates,
        audio_calls: state.audio_calls,
    }
}

/// Shared helper: emit entity passes with 2-pass blend mode support.
///
/// Batches Normal/Mask entities together, and when a Photoshop-style blend mode
/// entity is encountered, flushes the batch, snapshots the current target,
/// renders the entity in isolation, and composites back via `blend_composite`.
///
/// `output_key`: the render target to accumulate into.
/// `target_width`/`target_height`: dimensions for all passes.
/// `camera_id`: identifier used for unique snapshot/isolation key names.
fn emit_entities_with_blend(
    state: &mut RenderState,
    entities: Vec<crate::frame::FlatEntity>,
    output_key: &str,
    target_width: Option<u32>,
    target_height: Option<u32>,
    camera_id: &str,
) {
    state.current_acc_key = Some(output_key.to_string());

    let mut current_batch = Vec::new();
    let mut is_first_batch = true;

    for fe in entities {
        let blend_id = fe.blend_mode;
        // 0 = Normal, 11 = MaskIn, 12 = MaskOut — hardware blending, no 2-pass needed
        if blend_id == 0 || blend_id == 11 || blend_id == 12 {
            current_batch.push(fe);
        } else {
            // Photoshop blend mode: 2-pass blending required
            // 1. Flush current batch into the target
            if !current_batch.is_empty() || is_first_batch {
                let clear_color = if is_first_batch { Some([0.0, 0.0, 0.0, 0.0]) } else { None };
                state.push_pass(RenderPass { pass_hash: 0,
                    output: output_key.to_string(),
                    pass_type: PassType::Entities {
                        entities: std::mem::take(&mut current_batch),
                        clear_color,
                    },
                    target_width,
                    target_height,
                });
                is_first_batch = false;
            }

            // 2. Snapshot the current target as the blend destination
            let dst_key = format!("_bdst_{}_{}", camera_id, fe.id);
            state.push_pass(RenderPass { pass_hash: 0,
                output: dst_key.clone(),
                pass_type: PassType::Snapshot { source_key: output_key.to_string() },
                target_width, target_height,
            });

            // 3. Render the entity isolated into a clean buffer
            let iso_key = format!("_bsrc_{}_{}", camera_id, fe.id);
            let mut iso_fe = fe.clone();
            iso_fe.blend_mode = 0; // Normal blend in isolated buffer
            iso_fe.opacity = 1.0;
            state.push_pass(RenderPass { pass_hash: 0,
                output: iso_key.clone(),
                pass_type: PassType::Entities {
                    entities: vec![iso_fe],
                    clear_color: Some([0.0, 0.0, 0.0, 0.0]),
                },
                target_width, target_height,
            });

            // 4. Composite blend result back to the target
            state.push_pass(RenderPass { pass_hash: 0,
                output: output_key.to_string(),
                pass_type: PassType::Effect {
                    shader: "blend_composite".to_string(),
                    inputs: vec![iso_key, dst_key],
                    params: vec![blend_id as f32, fe.opacity, 0.0, 0.0],
                },
                target_width, target_height,
            });
        }
    }

    // Flush any remaining normal entities
    if !current_batch.is_empty() || is_first_batch {
        let clear_color = if is_first_batch { Some([0.0, 0.0, 0.0, 0.0]) } else { None };
        state.push_pass(RenderPass { pass_hash: 0,
            output: output_key.to_string(),
            pass_type: PassType::Entities {
                entities: current_batch,
                clear_color,
            },
            target_width,
            target_height,
        });
    }
}

/// Legacy single-camera pass (Phase 4.1 behavior, backward compatible).
fn build_single_camera_pass(
    state: &mut RenderState,
    mut flat_entities: Vec<crate::frame::FlatEntity>,
    screen_width: u32,
    screen_height: u32,
) {
    flat_entities.sort_by(|a, b| a.layer.cmp(&b.layer)
        .then(a.z_index.partial_cmp(&b.z_index).unwrap_or(std::cmp::Ordering::Equal)));

    let base_output = if state.camera_effects.is_empty() {
        "main".to_string()
    } else {
        "_camera_src".to_string()
    };

    let tw = if state.camera_effects.is_empty() { None } else { Some(screen_width) };
    let th = if state.camera_effects.is_empty() { None } else { Some(screen_height) };

    // Use shared blend-aware batching helper
    emit_entities_with_blend(state, flat_entities, &base_output, tw, th, "main");

    // Post-processing: Camera Effects
    let mut current_cam_key = base_output;
    let camera_effects = state.camera_effects.clone();
    for (i, effect) in camera_effects.iter().enumerate() {
        let out_key = format!("_camera_fx_{}", i);
        state.push_pass(RenderPass { pass_hash: 0,
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

    state.push_pass(RenderPass { pass_hash: 0,
        output: "final".into(),
        pass_type: PassType::Output {
            input: if state.camera_effects.is_empty() { "main".into() } else { current_cam_key },
            entities: vec![],
        },
        target_width: Some(screen_width),
        target_height: None,
    });
}

/// Phase 5: Multi-Camera compositing pass.
/// Each camera renders its own subset of entities (filtered by target_layers),
/// applies its own post_effects, then all cameras are composited in render_order.
fn build_multi_camera_passes(
    state: &mut RenderState,
    flat_entities_all: Vec<crate::frame::FlatEntity>,
    screen_width: u32,
    screen_height: u32,
) {
    // Sort all entities once
    let mut sorted_entities = flat_entities_all;
    sorted_entities.sort_by(|a, b| a.layer.cmp(&b.layer)
        .then(a.z_index.partial_cmp(&b.z_index).unwrap_or(std::cmp::Ordering::Equal)));

    let mut camera_output_keys: Vec<String> = Vec::new();

    // Clone scene_cameras to avoid borrow conflict
    let cameras = state.scene_cameras.clone();

    for cam_info in &cameras {
        let cam_key = format!("_cam_{}", cam_info.entity_id);
        
        let scaled_width = (screen_width as f32 * cam_info.render_scale).max(1.0) as u32;
        let scaled_height = (screen_height as f32 * cam_info.render_scale).max(1.0) as u32;

        // Filter entities by this camera's target_layers
        let mut cam_entities: Vec<_> = sorted_entities.iter().filter(|e| {
            match &cam_info.target_layers {
                None => true,
                Some(layers) => layers.contains(&e.layer),
            }
        }).cloned().collect();

        // Render entities for this camera
        let src_key = if cam_info.post_effects.is_empty() {
            cam_key.clone()
        } else {
            format!("{}_src", cam_key)
        };

        // Scale FlatEntity spatial properties if render_quality is applied
        // This ensures the entities mathematically project to the same clip-space 
        // bounds when drawn into the smaller scaled_width/height render target.
        if (cam_info.render_scale - 1.0).abs() > 0.001 {
            for fe in &mut cam_entities {
                fe.x *= cam_info.render_scale;
                fe.y *= cam_info.render_scale;
                fe.width *= cam_info.render_scale;
                fe.height *= cam_info.render_scale;
                fe.intrinsic_width *= cam_info.render_scale;
                fe.intrinsic_height *= cam_info.render_scale;
                // DO NOT scale fe.params! Shaders like shapes.wgsl and dashed_rect.wgsl 
                // use normalized coordinates (0.0 to 1.0) relative to UV space.
                // Their physical pixel size scales automatically with fe.width and fe.height.
                
                // CRITICAL: We modified the spatial properties, so we MUST recalculate 
                // the content hash! Otherwise RenderGraph caching will fail to detect 
                // changes when panning/zooming at lower render qualities.
                fe.content_hash = fe.calculate_hash();
            }
        }

        // Use shared blend-aware batching helper
        emit_entities_with_blend(
            state,
            cam_entities,
            &src_key,
            Some(scaled_width),
            Some(scaled_height),
            &cam_info.entity_id,
        );

        // Apply this camera's post_effects chain
        let mut current_key = src_key;
        let mut scaled_effects = cam_info.post_effects.clone();
        if (cam_info.render_scale - 1.0).abs() > 0.001 {
            for effect in &mut scaled_effects {
                match effect.shader_id.as_str() {
                    "blur" => {
                        // params: [dir_x, dir_y, radius, texel_size]
                        if effect.params.len() > 2 {
                            effect.params[2] *= cam_info.render_scale;
                        }
                    }
                    "drop_shadow" => {
                        // params: [r, g, b, a, offset_x, offset_y, blur, pad]
                        if effect.params.len() > 6 {
                            effect.params[4] *= cam_info.render_scale;
                            effect.params[5] *= cam_info.render_scale;
                            effect.params[6] *= cam_info.render_scale;
                        }
                    }
                    "glow" => {
                        // params: [r, g, b, a, size, intensity, pad, pad]
                        if effect.params.len() > 4 {
                            effect.params[4] *= cam_info.render_scale;
                        }
                    }
                    "selection_outline" => {
                        // params: [thickness, pad, pad, pad]
                        if effect.params.len() > 0 {
                            effect.params[0] *= cam_info.render_scale;
                        }
                    }
                    _ => {} // Other effects like color_grade, vignette use unitless values
                }
            }
        }

        for (i, effect) in scaled_effects.iter().enumerate() {
            let fx_key = format!("{}_fx_{}", cam_key, i);
            state.push_pass(RenderPass { pass_hash: 0,
                output: fx_key.clone(),
                pass_type: PassType::Effect {
                    shader: effect.shader_id.clone(),
                    inputs: vec![current_key],
                    params: effect.params.clone(),
                },
                target_width: Some(scaled_width),
                target_height: Some(scaled_height),
            });
            current_key = fx_key;
        }

        camera_output_keys.push(current_key);
    }

    // Composite all camera outputs in order using the Output pass.
    // The first camera is the "background" (input to Output), subsequent cameras
    // are composited as overlay entities using the composite shader.
    let base_input = camera_output_keys.first().cloned().unwrap_or_else(|| "main".to_string());

    let overlay_entities: Vec<crate::frame::FlatEntity> = camera_output_keys
        .iter()
        .skip(1) // First camera is base input, rest are overlays
        .enumerate()
        .map(|(i, tex_key)| {
            let mut fe = crate::frame::FlatEntity {
                id: 0, content_hash: 0,
                x: 0.0,
                y: 0.0,
                width: screen_width as f32,
                height: screen_height as f32,
                rotation: 0.0,
                opacity: 1.0,
                blend_mode: 0,
                color: [1.0, 1.0, 1.0, 1.0],
                shader: "composite".to_string(),
                textures: vec![tex_key.clone()],
                params: vec![],
                layer: i as i32,
                z_index: i as f32,
                fit_mode: 0,
                uv_offset: [0.0, 0.0],
                uv_scale: [1.0, 1.0],
                intrinsic_width: screen_width as f32,
                intrinsic_height: screen_height as f32,
            };
            fe.content_hash = fe.calculate_hash();
            fe
        })
        .collect();

    state.push_pass(RenderPass { pass_hash: 0,
        output: "final".into(),
        pass_type: PassType::Output {
            input: base_input,
            entities: overlay_entities,
        },
        target_width: Some(screen_width),
        target_height: None,
    });
}
