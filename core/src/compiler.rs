//! V2 ECS to Frame Compiler
//!
//! Reads resolved float values from the ECS World and produces
//! flat `Frame` draw commands for the GPU.
//!
//! Pure ECS: detects entity role by component presence.
//!   - has camera → skip (used for viewport projection)
//!   - has video_source/image_source/text_source/color_source → renderable
//!   - has materials + no source → adjustment layer
//!
//! All coordinates are world units — projected to pixels via camera.

use crate::frame::{FlatEntity, Frame, RenderPass, PassType};
use ifol_render_ecs::ecs::World;

/// Compiles the V2 ECS World into a renderable V1 Frame.
pub fn compile_world_to_frame(
    world: &World,
    camera_id: &str,
    screen_width: u32,
    screen_height: u32,
    _time_secs: f64,
    custom_cam_x: Option<f32>,
    custom_cam_y: Option<f32>,
    custom_cam_w: Option<f32>,
    custom_cam_h: Option<f32>,
    selected_entity_ids: &[&str],
) -> Frame {
    let storages = &world.storages;
    let mut passes = Vec::new();
    let mut texture_updates = Vec::new();

    // ── Camera projection: world units → pixels ──
    let cam = world.find_camera(camera_id);
    let cam_x = custom_cam_x.unwrap_or_else(|| cam.map(|c| c.resolved.x).unwrap_or(0.0));
    let cam_y = custom_cam_y.unwrap_or_else(|| cam.map(|c| c.resolved.y).unwrap_or(0.0));
    let cam_w = custom_cam_w.unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0)).max(1.0);
    let cam_h = custom_cam_h.unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0)).max(1.0);
    let sx = screen_width as f32 / cam_w;
    let sy = screen_height as f32 / cam_h;

    // Build parent→children map
    let mut top_level: Vec<&ifol_render_ecs::ecs::Entity> = Vec::new();
    let mut children_map: std::collections::HashMap<String, Vec<&ifol_render_ecs::ecs::Entity>> = std::collections::HashMap::new();
    let sorted = world.sorted_by_layer();
    for entity in &sorted {
        // Skip cameras
        if storages.get_component::<ifol_render_ecs::ecs::components::CameraComponent>(&entity.id).is_some() { continue; }
        if let Some(pid) = storages.get_component::<ifol_render_ecs::ecs::components::meta::ParentId>(&entity.id).map(|id| &id.0) {
            children_map.entry(pid.clone()).or_default().push(*entity);
        } else {
            top_level.push(*entity);
        }
    }

    // ═══════════════════════════════════════════
    // Helper: Check if entity is an adjustment layer (has materials, no source)
    // ═══════════════════════════════════════════
    fn is_adjustment(entity: &ifol_render_ecs::ecs::Entity, storages: &ifol_render_ecs::ecs::typemap::TypeMap) -> bool {
        storages.get_component::<ifol_render_ecs::ecs::components::meta::Materials>(&entity.id).map_or(false, |m| !m.0.is_empty())
            && storages.get_component::<ifol_render_ecs::ecs::components::VideoSource>(&entity.id).is_none()
            && storages.get_component::<ifol_render_ecs::ecs::components::ImageSource>(&entity.id).is_none()
            && storages.get_component::<ifol_render_ecs::ecs::components::TextSource>(&entity.id).is_none()
            && storages.get_component::<ifol_render_ecs::ecs::components::ColorSource>(&entity.id).is_none()
            && storages.get_component::<ifol_render_ecs::ecs::components::CameraComponent>(&entity.id).is_none()
    }

    // ═══════════════════════════════════════════
    // Helper: Build material effect chain passes
    // ═══════════════════════════════════════════
    fn apply_material_chain(
        passes: &mut Vec<RenderPass>,
        input_rt: &str,
        materials: &[ifol_render_ecs::scene::MaterialV2],
        entity_id: &str,
        local_time: f64,
        tw: Option<u32>,
        th: Option<u32>,
    ) -> String {
        let mut cur = input_rt.to_string();
        for (i, mat) in materials.iter().enumerate() {
            let next = format!("{}_mat{}", entity_id, i);
            let mut params = Vec::new();
            let mut keys: Vec<&String> = mat.float_uniforms.keys().collect();
            keys.sort();
            for k in keys { params.push(mat.float_uniforms[k].evaluate(local_time, 0.0) as f32); }
            passes.push(RenderPass {
                output: next.clone(),
                pass_type: PassType::Effect { shader: mat.shader_id.clone(), inputs: vec![cur], params },
                target_width: tw, target_height: th,
            });
            cur = next;
        }
        cur
    }

    // ═══════════════════════════════════════════
    // Process entities recursively
    // ═══════════════════════════════════════════
    fn process_node<'a>(
        entity: &'a ifol_render_ecs::ecs::Entity,
        children_map: &std::collections::HashMap<String, Vec<&'a ifol_render_ecs::ecs::Entity>>,
        passes: &mut Vec<RenderPass>,
        texture_updates: &mut Vec<crate::frame::TextureUpdate>,
        world: &ifol_render_ecs::ecs::World,
        sx: f32, sy: f32,
        cam_x: f32, cam_y: f32,
        screen_width: u32, screen_height: u32,
        collected: &mut Vec<FlatEntity>,
    ) {
        let storages = &world.storages;
        if !entity.resolved.visible { return; }
        if is_adjustment(entity, storages) { return; }

        let local_time = entity.resolved.time.local_time;
        let r = &entity.resolved;

        // ── World units → pixel projection ──
        let w = r.width * sx;
        let h = r.height * sy;
        let rot_rad = r.rotation.to_radians();
        let cos_r = rot_rad.cos();
        let sin_r = rot_rad.sin();
        let dx = (0.5 - r.anchor_x) * w;
        let dy = (0.5 - r.anchor_y) * h;
        // Center of the camera should map to the center of the screen
        let center_x = (r.x - cam_x) * sx + (screen_width as f32) * 0.5 + dx * cos_r - dy * sin_r;
        let center_y = (r.y - cam_y) * sy + (screen_height as f32) * 0.5 + dx * sin_r + dy * cos_r;
        let flat_x = center_x - w * 0.5;
        let flat_y = center_y - h * 0.5;

        // ── Fit mode UV parameters ──
        let iw = r.intrinsic_width;
        let ih = r.intrinsic_height;
        let ax = storages.get_component::<ifol_render_ecs::ecs::components::Rect>(&entity.id).map(|rc| rc.align_x).unwrap_or(0.5);
        let ay = storages.get_component::<ifol_render_ecs::ecs::components::Rect>(&entity.id).map(|rc| rc.align_y).unwrap_or(0.5);
        let (uv_offset, uv_scale) = r.fit_mode.calculate_uv(r.width, r.height, iw, ih, ax, ay);

        let mut flat = FlatEntity {
            id: 0,
            x: flat_x, y: flat_y, width: w, height: h,
            rotation: rot_rad,
            opacity: r.opacity,
            blend_mode: r.blend_mode.as_u32(),
            color: [1.0, 1.0, 1.0, 1.0],
            shader: "composite".into(),
            textures: vec![], params: vec![],
            layer: r.layer,
            z_index: r.layer as f32,
            fit_mode: r.fit_mode.as_u32(),
            uv_offset,
            uv_scale,
            intrinsic_width: iw,
            intrinsic_height: ih,
        };

        // ── Content sources (detected by component presence) ──
        let mut has_content = false;
        if let Some(cs) = storages.get_component::<ifol_render_ecs::ecs::components::ColorSource>(&entity.id) {
            flat.color = [
                storages.get_component::<ifol_render_ecs::ecs::components::meta::FloatUniforms>(&entity.id).map(|m| m.0.clone()).unwrap_or_default().get("color_r").map(|t: &ifol_render_ecs::scene::FloatTrack| t.evaluate(local_time, cs.color.r)).unwrap_or(cs.color.r) as f32,
                storages.get_component::<ifol_render_ecs::ecs::components::meta::FloatUniforms>(&entity.id).map(|m| m.0.clone()).unwrap_or_default().get("color_g").map(|t: &ifol_render_ecs::scene::FloatTrack| t.evaluate(local_time, cs.color.g)).unwrap_or(cs.color.g) as f32,
                storages.get_component::<ifol_render_ecs::ecs::components::meta::FloatUniforms>(&entity.id).map(|m| m.0.clone()).unwrap_or_default().get("color_b").map(|t: &ifol_render_ecs::scene::FloatTrack| t.evaluate(local_time, cs.color.b)).unwrap_or(cs.color.b) as f32,
                storages.get_component::<ifol_render_ecs::ecs::components::meta::FloatUniforms>(&entity.id).map(|m| m.0.clone()).unwrap_or_default().get("color_a").map(|t: &ifol_render_ecs::scene::FloatTrack| t.evaluate(local_time, cs.color.a)).unwrap_or(cs.color.a) as f32,
            ];
            has_content = true;
        } else if let Some(video) = storages.get_component::<ifol_render_ecs::ecs::components::VideoSource>(&entity.id) {
            // Resolve asset_id → URL from world registry
            let url = world.resolve_asset_url(&video.asset_id)
                .unwrap_or(&video.asset_id).to_string();
            flat.textures.push(url.clone());
            has_content = true;
            texture_updates.push(crate::frame::TextureUpdate::DecodeVideoFrame {
                key: url.clone(), path: url,
                timestamp_secs: entity.resolved.playback_time,
                width: None, height: None,
            });
        } else if let Some(image) = storages.get_component::<ifol_render_ecs::ecs::components::ImageSource>(&entity.id) {
            let url = world.resolve_asset_url(&image.asset_id)
                .unwrap_or(&image.asset_id).to_string();
            flat.textures.push(url);
            has_content = true;
        } else if let Some(text) = storages.get_component::<ifol_render_ecs::ecs::components::TextSource>(&entity.id) {
            flat.textures.push(text.content.clone());
            flat.color = text.color.into();
            has_content = true;
        }

        // ── Level 4: Group Materials ──
        let has_children = children_map.get(&entity.id).map(|k| !k.is_empty()).unwrap_or(false);
        let has_materials = storages.get_component::<ifol_render_ecs::ecs::components::meta::Materials>(&entity.id).map_or(false, |m| !m.0.is_empty());

        if has_children && has_materials {
            let group_rt = format!("{}_group", entity.id);
            let mut group_ents = Vec::new();
            if has_content {
                let mut local_flat = flat.clone();
                local_flat.x = 0.0; local_flat.y = 0.0; local_flat.rotation = 0.0;
                group_ents.push(local_flat);
            }
            if let Some(kids) = children_map.get(&entity.id) {
                for kid in kids {
                    process_node(kid, children_map, passes, texture_updates, world, sx, sy, cam_x, cam_y, screen_width, screen_height, &mut group_ents);
                }
            }
            if !group_ents.is_empty() {
                let tw = Some(w.ceil() as u32);
                let th = Some(h.ceil() as u32);
                passes.push(RenderPass {
                    output: group_rt.clone(),
                    pass_type: PassType::Entities { entities: group_ents, clear_color: [0.0,0.0,0.0,0.0] },
                    target_width: tw, target_height: th,
                });
                let final_rt = apply_material_chain(passes, &group_rt, &storages.get_component::<ifol_render_ecs::ecs::components::meta::Materials>(&entity.id).map(|m| m.0.clone()).unwrap_or_default(), &entity.id, local_time, tw, th);
                let mut out = flat.clone();
                out.shader = "composite".into();
                out.textures = vec![final_rt];
                out.color = [1.0,1.0,1.0,1.0];
                collected.push(out);
            }
            return;
        }

        // ── Level 1: Entity Materials (no children) ──
        if has_materials && has_content {
            let tw = Some(w.ceil() as u32);
            let th = Some(h.ceil() as u32);
            let base = format!("{}_base", entity.id);
            let mut leaf = flat.clone();
            leaf.x = 0.0; leaf.y = 0.0; leaf.rotation = 0.0;
            passes.push(RenderPass {
                output: base.clone(),
                pass_type: PassType::Entities { entities: vec![leaf], clear_color: [0.0,0.0,0.0,0.0] },
                target_width: tw, target_height: th,
            });
            let final_rt = apply_material_chain(passes, &base, &storages.get_component::<ifol_render_ecs::ecs::components::meta::Materials>(&entity.id).map(|m| m.0.clone()).unwrap_or_default(), &entity.id, local_time, tw, th);
            let mut out = flat.clone();
            out.shader = "composite".into();
            out.textures = vec![final_rt];
            out.color = [1.0,1.0,1.0,1.0];
            collected.push(out);
        } else if has_content {
            collected.push(flat);
        }

        // Children in world space
        if let Some(kids) = children_map.get(&entity.id) {
            for kid in kids {
                process_node(kid, children_map, passes, texture_updates, world, sx, sy, cam_x, cam_y, screen_width, screen_height, collected);
            }
        }
    }

    // ═══════════════════════════════════════════
    // Main compilation: handle adjustment layers at top level
    // ═══════════════════════════════════════════
    let default_flat = || FlatEntity {
        id: 0, x: 0.0, y: 0.0,
        width: screen_width as f32, height: screen_height as f32,
        rotation: 0.0, opacity: 1.0, blend_mode: 0,
        color: [1.0,1.0,1.0,1.0],
        shader: "composite".into(),
        textures: vec![], params: vec![],
        layer: 0, z_index: -1.0, fit_mode: 0,
        uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
        intrinsic_width: 0.0, intrinsic_height: 0.0,
    };

    let mut current_batch: Vec<FlatEntity> = Vec::new();
    let mut adj_counter = 0u32;
    let mut floor_rt: Option<String> = None;

    for entity in &top_level {
        if is_adjustment(entity, storages) && storages.get_component::<ifol_render_ecs::ecs::components::meta::Materials>(&entity.id).map_or(false, |m| !m.0.is_empty()) {
            if !current_batch.is_empty() || floor_rt.is_some() {
                let batch_rt = format!("_adj_batch_{}", adj_counter);
                if let Some(ref floor) = floor_rt {
                    let mut bg = default_flat();
                    bg.textures = vec![floor.clone()];
                    current_batch.insert(0, bg);
                }
                passes.push(RenderPass {
                    output: batch_rt.clone(),
                    pass_type: PassType::Entities { entities: current_batch, clear_color: [0.0,0.0,0.0,1.0] },
                    target_width: None, target_height: None,
                });
                let local_time = entity.resolved.time.local_time;
                let result_rt = apply_material_chain(&mut passes, &batch_rt, &storages.get_component::<ifol_render_ecs::ecs::components::meta::Materials>(&entity.id).map(|m| m.0.clone()).unwrap_or_default(), &entity.id, local_time, None, None);
                floor_rt = Some(result_rt);
                current_batch = Vec::new();
                adj_counter += 1;
            }
        } else {
            process_node(entity, &children_map, &mut passes, &mut texture_updates, world, sx, sy, cam_x, cam_y, screen_width, screen_height, &mut current_batch);
        }
    }

    // Final main composite pass
    if let Some(ref floor) = floor_rt {
        let mut bg = default_flat();
        bg.textures = vec![floor.clone()];
        current_batch.insert(0, bg);
    }

    let main_rt = "main".to_string();
    passes.push(RenderPass {
        output: main_rt.clone(),
        pass_type: PassType::Entities { entities: current_batch, clear_color: [0.0,0.0,0.0,1.0] },
        target_width: None, target_height: None,
    });

    // ── Camera Post-Effects ──
    let mut output_input = main_rt;
    if let Some(cam_ent) = cam {
        if let Some(cam_comp) = storages.get_component::<ifol_render_ecs::ecs::components::CameraComponent>(&cam_ent.id) {
            if !cam_comp.post_effects.is_empty() {
                let local_time = cam_ent.resolved.time.local_time;
                output_input = apply_material_chain(&mut passes, &output_input, &cam_comp.post_effects, "cam_post", local_time, None, None);
            }
        }
    }

    passes.push(RenderPass {
        output: "".into(),
        pass_type: PassType::Output { input: output_input },
        target_width: None, target_height: None,
    });

    // ── Selection Overlay (Multi-Select) ──
    // Renders outlines for ALL selected entities in one pass:
    //   1. Render all selected entities as solid white silhouettes → _sel_silhouette
    //   2. Apply selection_outline effect → _sel_outline (outer cyan edge only)
    //   3. Composite outline on top of main output
    if !selected_entity_ids.is_empty() {
        let mut silhouettes = Vec::new();
        for sel_id in selected_entity_ids {
            if let Some(sel_ent) = sorted.iter().find(|e| e.id == *sel_id) {
                if !sel_ent.resolved.visible { continue; }
                let r = &sel_ent.resolved;
                let w = r.width * sx;
                let h = r.height * sy;
                let rot_rad = r.rotation.to_radians();
                let cos_r = rot_rad.cos();
                let sin_r = rot_rad.sin();
                let dx = (0.5 - r.anchor_x) * w;
                let dy = (0.5 - r.anchor_y) * h;
                let center_x = (r.x - cam_x) * sx + (screen_width as f32) * 0.5 + dx * cos_r - dy * sin_r;
                let center_y = (r.y - cam_y) * sy + (screen_height as f32) * 0.5 + dx * sin_r + dy * cos_r;
                let flat_x = center_x - w * 0.5;
                let flat_y = center_y - h * 0.5;

                silhouettes.push(FlatEntity {
                    id: 999999 - silhouettes.len() as u64,
                    x: flat_x, y: flat_y, width: w, height: h,
                    rotation: rot_rad,
                    opacity: 1.0, blend_mode: 0,
                    color: [1.0, 1.0, 1.0, 1.0],
                    shader: "composite".into(),
                    textures: vec![], params: vec![],
                    layer: silhouettes.len() as i32, z_index: silhouettes.len() as f32,
                    fit_mode: 0,
                    uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                    intrinsic_width: 0.0, intrinsic_height: 0.0,
                });
            }
        }

        if !silhouettes.is_empty() {
            // Step 1: All silhouettes in one pass
            passes.push(RenderPass {
                output: "_sel_silhouette".into(),
                pass_type: PassType::Entities {
                    entities: silhouettes,
                    clear_color: [0.0, 0.0, 0.0, 0.0],
                },
                target_width: None, target_height: None,
            });

            // Step 2: Edge detection
            passes.push(RenderPass {
                output: "_sel_outline".into(),
                pass_type: PassType::Effect {
                    shader: "selection_outline".into(),
                    inputs: vec!["_sel_silhouette".into()],
                    params: vec![3.0, 0.0, 0.0, 0.0],
                },
                target_width: None, target_height: None,
            });

            // Step 3: Composite onto output
            let last_idx = passes.len() - 3;
            if let Some(out_pass) = passes.get(last_idx) {
                if let PassType::Output { input: ref inp } = out_pass.pass_type {
                    let prev_input = inp.clone();
                    passes.remove(last_idx);

                    let bg = FlatEntity {
                        id: 999998,
                        x: 0.0, y: 0.0,
                        width: screen_width as f32, height: screen_height as f32,
                        rotation: 0.0, opacity: 1.0, blend_mode: 0,
                        color: [1.0, 1.0, 1.0, 1.0],
                        shader: "composite".into(),
                        textures: vec![prev_input],
                        params: vec![], layer: 0, z_index: -1.0,
                        fit_mode: 0,
                        uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                        intrinsic_width: 0.0, intrinsic_height: 0.0,
                    };
                    let overlay = FlatEntity {
                        id: 999997,
                        x: 0.0, y: 0.0,
                        width: screen_width as f32, height: screen_height as f32,
                        rotation: 0.0, opacity: 1.0, blend_mode: 0,
                        color: [1.0, 1.0, 1.0, 1.0],
                        shader: "composite".into(),
                        textures: vec!["_sel_outline".into()],
                        params: vec![], layer: 1, z_index: 999999.0,
                        fit_mode: 0,
                        uv_offset: [0.0, 0.0], uv_scale: [1.0, 1.0],
                        intrinsic_width: 0.0, intrinsic_height: 0.0,
                    };
                    passes.push(RenderPass {
                        output: "_with_sel".into(),
                        pass_type: PassType::Entities {
                            entities: vec![bg, overlay],
                            clear_color: [0.0, 0.0, 0.0, 1.0],
                        },
                        target_width: None, target_height: None,
                    });
                    passes.push(RenderPass {
                        output: "".into(),
                        pass_type: PassType::Output { input: "_with_sel".into() },
                        target_width: None, target_height: None,
                    });
                }
            }
        }
    }

    Frame { passes, texture_updates }
}
