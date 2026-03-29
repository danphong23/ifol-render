use crate::ecs::World;
use crate::ecs::components::meta::Materials;
use crate::ecs::components::draw::EffectPassDef;
use std::collections::HashMap;

/// Material System (Phase 2)
///
/// Processes the `Materials` component to generate off-screen effect passes.
/// This runs after `source_sys` so we can append to the `DrawComponent.effect_chain`.
pub fn material_system(world: &mut World) {
    let storages = &world.storages;
    for entity in &mut world.entities {
        if !entity.resolved.visible {
            continue;
        }

        let local_time = entity.resolved.time.local_time;

        if let Some(materials) = storages.get_component::<Materials>(&entity.id) {
            let mut total_padding = 0.0;
            let mut effect_chain = Vec::new();

            for mat in &materials.0 {
                // 1. Evaluate uniform params at the current local time
                let mut params_map = HashMap::new();
                for (name, track) in &mat.float_uniforms {
                    let val = track.evaluate(local_time, 0.0) as f32;
                    params_map.insert(name.clone(), val);
                }

                // 2. Estimate padding based on ShaderScope using MAX tracks value to prevent texture size thrashing
                let padding = match mat.scope {
                    crate::schema::v2::ShaderScope::Clipped => 0.0,
                    crate::schema::v2::ShaderScope::Padded => estimate_padding(&mat.shader_id, &mat.float_uniforms),
                    crate::schema::v2::ShaderScope::Layer => estimate_padding(&mat.shader_id, &mat.float_uniforms),
                    crate::schema::v2::ShaderScope::Camera => 0.0,
                    crate::schema::v2::ShaderScope::Masked => 0.0, // Masked doesn't need to overflow bounds visually
                };
                total_padding += padding;

                // 3. Map params consistently to flat f32 vectors for GPU using ALPHABETICAL SORT
                // This allows any arbitrary shader to work flawlessly by naming parameters like u0_x, u1_y
                let mut keys: Vec<&String> = params_map.keys().collect();
                keys.sort();
                let param_vec: Vec<f32> = keys.iter().map(|k| params_map[*k]).collect();

                effect_chain.push(EffectPassDef {
                    shader_id: mat.shader_id.clone(),
                    scope: mat.scope.clone(),
                    params: param_vec,
                    padding,
                    pass_count: 1, // Our built-in radial WGSL shaders are 1-pass
                });
            }

            entity.draw.effect_chain = effect_chain;
            entity.draw.effect_padding = total_padding;
        } else {
            // Ensure we clear from previous frames
            entity.draw.effect_chain.clear();
            entity.draw.effect_padding = 0.0;
        }
    }
}

/// Estimate the REQUIRED PEAK padding in world units for an effect so it doesn't clip AND so it doesn't thrash exact-match textures.
fn estimate_padding(shader_id: &str, float_uniforms: &HashMap<String, crate::schema::tracks::FloatTrack>) -> f32 {
    let get_max_param = |keys: &[&str]| {
        keys.iter()
            .find_map(|&k| float_uniforms.get(k))
            .map(|track| {
                if track.keyframes.is_empty() {
                    0.0
                } else {
                    track.keyframes.iter().map(|k| k.value.abs()).fold(0.0_f32, |m, v| m.max(v))
                }
            })
            .unwrap_or(0.0)
    };
    
    match shader_id {
        "blur" => {
            get_max_param(&["u2_radius", "radius"]) * 2.0
        }
        "glow" => {
            get_max_param(&["u4_size", "size"]) * 2.0
        }
        "drop_shadow" => {
            let radius = get_max_param(&["u6_blur", "blur", "radius"]);
            let offset_x = get_max_param(&["u4_offset_x", "offset_x"]);
            let offset_y = get_max_param(&["u5_offset_y", "offset_y"]);
            (radius + offset_x.abs().max(offset_y.abs())) * 2.0
        }
        _ => 0.0,
    }
}
