use crate::ecs::{World, ContextView};
use crate::ecs::systems::render::state::RenderState;
use crate::frame::{FlatEntity, PassType, RenderPass};

/// Phase 2: Compile Composition Buffers (Deepest First)
pub fn compile_composition_buffers(
    world: &World,
    context: &ContextView,
    state: &mut RenderState,
) {
    let get_depth = |ent_id: &str| -> usize {
        let mut depth = 0;
        let mut cur = ent_id.to_string();
        for _ in 0..32 {
            if let Some(e) = world.get(&cur) {
                if let Some(pid) = world.storages.get_component::<crate::ecs::components::meta::ParentId>(&e.id) {
                    depth += 1;
                    cur = pid.0.clone();
                } else { break; }
            } else { break; }
        }
        depth
    };
    
    // Sort deeply nested compositions to render first
    state.comp_entities.sort_by_key(|id| std::cmp::Reverse(get_depth(id)));

    let comp_entities = state.comp_entities.clone();
    for comp_id in comp_entities {
        let comp_ent = world.get(&comp_id).unwrap();
        let (cw, ch) = {
            let (_, _, cw, ch, _) = state.comp_cameras.get(&comp_id).unwrap();
            (*cw, *ch)
        };
        
        let target_comp = {
            let mut found = "main".to_string();
            let mut cur = if let Some(pid) = world.storages.get_component::<crate::ecs::components::meta::ParentId>(&comp_ent.id) { pid.0.clone() } else { String::new() };
            for _ in 0..32 {
                if cur.is_empty() { break; }
                if let Some(e) = world.get(&cur) {
                    if world.storages.get_component::<crate::ecs::components::Composition>(&e.id).is_some() {
                        if context.scope_id == Some(cur.as_str()) { found = "main".to_string(); } else { found = cur.clone(); }
                        break;
                    }
                    if let Some(pid) = world.storages.get_component::<crate::ecs::components::meta::ParentId>(&e.id) { cur = pid.0.clone(); } else { break; }
                } else { break; }
            }
            found
        };

        let comp_tex_key = format!("_comp_out_{}", comp_id);
        let mut list = state.comp_lists.remove(&comp_id).unwrap_or_default();
        list.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.z_index.partial_cmp(&b.z_index).unwrap()));

        state.push_pass(RenderPass { pass_hash: 0,
            output: comp_tex_key.clone(),
            pass_type: PassType::Entities { entities: list, clear_color: [0.0, 0.0, 0.0, 0.0] },
            target_width: Some(cw as u32), target_height: Some(ch as u32),
        });

        let (parent_cx, parent_cy, p_sx, p_sy) = if target_comp == "main" {
            (state.root_cam_x, state.root_cam_y, state.root_sx, state.root_sy)
        } else {
            // Use the parent comp's inner camera origin (world-top-left of the inner buffer).
            let (pcx, pcy, _, _, _) = state.comp_cameras.get(&target_comp).unwrap();
            (*pcx, *pcy, 1.0_f32, 1.0_f32)
        };

        let original_ew = comp_ent.resolved.width * p_sx;
        let original_eh = comp_ent.resolved.height * p_sy;
        // Push the original comp normal output
        let (uv_offset, uv_scale) = comp_ent.resolved.fit_mode.calculate_uv(
            original_ew, original_eh, cw, ch, 0.5, 0.5
        );
        let center_x = (comp_ent.resolved.x - parent_cx) * p_sx;
        let center_y = (comp_ent.resolved.y - parent_cy) * p_sy;

        let mut flat = FlatEntity {
            id: 0, content_hash: 0,
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
            intrinsic_width: cw, intrinsic_height: ch,
        };
        flat.content_hash = flat.calculate_hash();
        state.comp_lists.entry(target_comp.clone()).or_default().push(flat);

        // Push overlay gizmo pass if selected
        if context.selected_ids.contains(&comp_ent.id) && context.select_mode == "content" {
            let offset = 6.0;
            let pad_w = offset * 2.0;
            let pad_h = offset * 2.0;
            let target_cx = cw + pad_w;
            let target_cy = ch + pad_h;

            let out_key = format!("_comp_fx_{}", comp_id);
            state.push_pass(RenderPass { pass_hash: 0,
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

            let mut gizmo_flat = FlatEntity {
                id: 0, content_hash: 0,
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
            gizmo_flat.content_hash = gizmo_flat.calculate_hash();
            state.comp_lists.entry(target_comp).or_default().push(gizmo_flat);
        }
    }
}
