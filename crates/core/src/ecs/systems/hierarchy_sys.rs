use crate::ecs::World;
use crate::time::TimeState;

/// Propagates parent transforms and opacity to children.
/// Uses additive position (not matrix multiplication) — correct for 2D compositing.
/// Must run AFTER rect_system (because it uses the final scaled state).
pub fn hierarchy_system(world: &mut World, _time: &TimeState) {
    let storages = &world.storages;
    // Store: (x, y, rotation, opacity, scale_x, scale_y, layer, volume)
    let mut resolved_transforms: std::collections::HashMap<String, (f32, f32, f32, f32, f32, f32, i32, f32)> = std::collections::HashMap::with_capacity(world.entities.len());

    for entity in &mut world.entities {
        if !entity.resolved.visible {
            continue;
        }
        
        // Initial layer fallback
        let entity_layer = storages.get_component::<crate::ecs::components::meta::Layer>(&entity.id).map(|l| l.0).unwrap_or(0);
        entity.resolved.layer = entity_layer;

        if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&entity.id).map(|id| &id.0) {
            // Read from the dynamically accumulated resolved state (topological order required)
            if let Some(&(px, py, p_rot, p_opacity, p_sx, p_sy, p_layer, p_volume)) = resolved_transforms.get(pid) {
                // Apply parent scale to local offset first
                let dx = entity.resolved.x * p_sx;
                let dy = entity.resolved.y * p_sy;
                
                // Rotate child's offset around parent (p_rot is already in radians)
                let cos_r = p_rot.cos();
                let sin_r = p_rot.sin();
                
                // Additive position offset
                entity.resolved.x = px + dx * cos_r - dy * sin_r;
                entity.resolved.y = py + dx * sin_r + dy * cos_r;
                
                // Additive rotation
                entity.resolved.rotation += p_rot;
                
                // Multiplicative scale
                let old_sx = entity.resolved.scale_x;
                let old_sy = entity.resolved.scale_y;
                entity.resolved.scale_x *= p_sx;
                entity.resolved.scale_y *= p_sy;
                
                // Recompute width/height to include parent scale
                // rect_sys already computed: width = base_w * old_sx
                // We need: width = base_w * (old_sx * p_sx) = (width / old_sx) * new_sx
                if old_sx.abs() > 0.001 {
                    entity.resolved.width = (entity.resolved.width / old_sx) * entity.resolved.scale_x;
                }
                if old_sy.abs() > 0.001 {
                    entity.resolved.height = (entity.resolved.height / old_sy) * entity.resolved.scale_y;
                }
                
                // Multiplicative opacity and volume
                entity.resolved.opacity *= p_opacity;
                entity.resolved.volume *= p_volume;
                
                // Additive layer
                entity.resolved.layer += p_layer;
            } else {
                log::warn!("Parent ID '{}' not found or not processed before child '{}'. Requires topological order.", pid, entity.id);
            }
        }

        // Accumulate this entity's FINAL resolved state so its children can use it
        resolved_transforms.insert(
            entity.id.clone(),
            (
                entity.resolved.x,
                entity.resolved.y,
                entity.resolved.rotation,
                entity.resolved.opacity,
                entity.resolved.scale_x,
                entity.resolved.scale_y,
                entity.resolved.layer,
                entity.resolved.volume,
            ),
        );
    }
}
