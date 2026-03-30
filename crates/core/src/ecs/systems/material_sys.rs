use crate::ecs::World;
use crate::ecs::components::draw::EffectPassDef;
use crate::ecs::components::meta::Materials;
use crate::scene::MaterialV2;
use std::collections::HashMap;

/// Evaluate a single MaterialV2 into an EffectPassDef with flattened params.
/// Shared helper for both entity materials and camera post_effects.
fn evaluate_material(
    mat: &MaterialV2,
    local_time: f64,
    force_scope: Option<crate::schema::v2::ShaderScope>,
) -> (EffectPassDef, f32) {
    // 1. Evaluate float uniform params at the current local time
    let mut params_map = HashMap::new();
    for (name, track) in &mat.float_uniforms {
        let val = track.evaluate(local_time, 0.0) as f32;
        params_map.insert(name.clone(), val);
    }

    // 1b. Evaluate vec4 uniforms → flatten to individual float params
    // Use numeric suffixes _0,_1,_2,_3 so alphabetical sort preserves component order.
    for (name, track) in &mat.vec4_uniforms {
        let val = track.evaluate(local_time, [0.0, 0.0, 0.0, 1.0]);
        params_map.insert(format!("{}_0", name), val[0]);
        params_map.insert(format!("{}_1", name), val[1]);
        params_map.insert(format!("{}_2", name), val[2]);
        params_map.insert(format!("{}_3", name), val[3]);
    }

    // 2. Estimate padding
    let scope = force_scope.unwrap_or_else(|| mat.scope.clone());
    let padding = match scope {
        crate::schema::v2::ShaderScope::Clipped => 0.0,
        crate::schema::v2::ShaderScope::Padded => {
            estimate_padding(&mat.shader_id, &mat.float_uniforms)
        }
        crate::schema::v2::ShaderScope::Layer => {
            estimate_padding(&mat.shader_id, &mat.float_uniforms)
        }
        crate::schema::v2::ShaderScope::Camera => 0.0,
        crate::schema::v2::ShaderScope::Masked => 0.0,
    };

    // 3. Alphabetical sort → flat f32 vector
    let mut keys: Vec<&String> = params_map.keys().collect();
    keys.sort();
    let mut param_vec: Vec<f32> = keys.iter().map(|k| params_map[*k]).collect();

    // 4. Pad to a multiple of 4 floats (16-byte alignment)
    while param_vec.len() % 4 != 0 {
        param_vec.push(0.0);
    }

    let effect = EffectPassDef {
        shader_id: mat.shader_id.clone(),
        scope,
        params: param_vec,
        padding,
        pass_count: 1,
    };
    (effect, padding)
}

/// Material System (Phase 2)
///
/// Processes the `Materials` component and `CameraComponent.post_effects`
/// to generate off-screen effect passes.
/// This runs after `source_sys` so we can append to the `DrawComponent.effect_chain`.
pub fn material_system(world: &mut World) {
    let storages = &world.storages;
    for entity in &mut world.entities {
        if !entity.resolved.visible {
            continue;
        }

        let local_time = entity.resolved.time.local_time;
        let mut total_padding = 0.0;
        let mut effect_chain = Vec::new();

        // Process entity materials (padded/layer/camera/clipped/masked)
        if let Some(materials) = storages.get_component::<Materials>(&entity.id) {
            for mat in &materials.0 {
                let (effect, padding) = evaluate_material(mat, local_time, None);
                total_padding += padding;
                effect_chain.push(effect);
            }
        }

        // Process camera post_effects (always Camera scope)
        if let Some(cam) = storages.get_component::<crate::ecs::components::CameraComponent>(&entity.id) {
            for mat in &cam.post_effects {
                let (effect, _) = evaluate_material(
                    mat,
                    local_time,
                    Some(crate::schema::v2::ShaderScope::Camera),
                );
                effect_chain.push(effect);
            }
        }

        entity.draw.effect_chain = effect_chain;
        entity.draw.effect_padding = total_padding;
    }
}


/// Estimate the REQUIRED PEAK padding in world units for an effect so it doesn't clip AND so it doesn't thrash exact-match textures.
fn estimate_padding(
    shader_id: &str,
    float_uniforms: &HashMap<String, crate::schema::tracks::FloatTrack>,
) -> f32 {
    let get_max_param = |keys: &[&str]| {
        keys.iter()
            .find_map(|&k| float_uniforms.get(k))
            .map(|track| {
                if track.keyframes.is_empty() {
                    0.0
                } else {
                    track
                        .keyframes
                        .iter()
                        .map(|k| k.value.abs())
                        .fold(0.0_f32, |m, v| m.max(v))
                }
            })
            .unwrap_or(0.0)
    };

    match shader_id {
        "blur" => get_max_param(&["u2_radius", "radius", "u0_radius"]) * 2.0,
        "glow" => get_max_param(&["u4_size", "size", "u0_radius", "u0_size"]) * 2.0,
        "drop_shadow" => {
            let radius = get_max_param(&["u6_blur", "blur", "radius", "u2_blur"]);
            let offset_x = get_max_param(&["u4_offset_x", "offset_x", "u0_dx"]);
            let offset_y = get_max_param(&["u5_offset_y", "offset_y", "u1_dy"]);
            (radius + offset_x.abs().max(offset_y.abs())) * 2.0
        }
        _ => 0.0,
    }
}
