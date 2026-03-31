use crate::ecs::World;
use crate::time::TimeState;

/// Evaluates entity transformations hierarchically (parent -> child order).
/// Resolves Local space offsets to World space absolute coordinates.
pub fn hierarchy_system(world: &mut World, _time: &TimeState, scope_entity_id: Option<&str>) {
    let storages = &world.storages;
    // Store: (x, y, rotation, opacity, scale_x, scale_y, layer, volume, visible)
    let mut resolved_transforms: std::collections::HashMap<
        String,
        (f32, f32, f32, f32, f32, f32, i32, f32, bool),
    > = std::collections::HashMap::with_capacity(world.entities.len());

    for entity in &mut world.entities {
        // Initial layer fallback
        let entity_layer = storages
            .get_component::<crate::ecs::components::meta::Layer>(&entity.id)
            .map(|l| l.0)
            .unwrap_or(0);
        entity.resolved.layer = entity_layer;

        let mut curr_anchor_x = entity.resolved.x;
        let mut curr_anchor_y = entity.resolved.y;

        if let Some(pid) = storages
            .get_component::<crate::ecs::components::meta::ParentId>(&entity.id)
            .map(|id| &id.0)
        {
            let is_scope = scope_entity_id.map(|sid| sid == entity.id.as_str()).unwrap_or(false);
            
            // Read from the dynamically accumulated resolved state (topological order required)
            // Skip parent propagation entirely if THIS entity is the isolated scope tab!
            if !is_scope {
            if let Some(&(px, py, p_rot, p_opacity, p_sx, p_sy, p_layer, p_volume, p_visible)) =
                resolved_transforms.get(pid)
            {
                // ── Comp World Isolation ──
                let parent_is_comp = storages
                    .get_component::<crate::ecs::components::Composition>(pid)
                    .is_some();

                // Propagate visibility
                if !p_visible {
                    entity.resolved.visible = false;
                }

                if entity.resolved.visible {
                    if parent_is_comp {
                        // ══ COMP WORLD BOUNDARY ══
                        entity.resolved.opacity *= p_opacity;
                        entity.resolved.volume *= p_volume;
                        entity.resolved.layer += p_layer;
                    } else {
                        // ── Normal hierarchy accumulation ──
                        let dx = entity.resolved.x * p_sx;
                        let dy = entity.resolved.y * p_sy;

                        let cos_r = p_rot.cos();
                        let sin_r = p_rot.sin();

                        // Additive position offset
                        curr_anchor_x = px + dx * cos_r - dy * sin_r;
                        curr_anchor_y = py + dx * sin_r + dy * cos_r;

                        // Additive rotation
                        entity.resolved.rotation += p_rot;

                        // Multiplicative scale
                        let old_sx = entity.resolved.scale_x;
                        let old_sy = entity.resolved.scale_y;
                        entity.resolved.scale_x *= p_sx;
                        entity.resolved.scale_y *= p_sy;

                        if old_sx.abs() > 0.001 {
                            entity.resolved.width = (entity.resolved.width / old_sx) * entity.resolved.scale_x;
                        }
                        if old_sy.abs() > 0.001 {
                            entity.resolved.height = (entity.resolved.height / old_sy) * entity.resolved.scale_y;
                        }

                        entity.resolved.opacity *= p_opacity;
                        entity.resolved.volume *= p_volume;
                        entity.resolved.layer += p_layer;
                    }
                }
            } else {
                log::warn!("Parent ID '{}' not found", pid);
            }
            }
        }

        // Bake Visual Center
        // Any child using THIS entity as a parent MUST pivot around `curr_anchor_x, curr_anchor_y`.
        // BUT the entity's visual rendering should be precisely centered at visual center.
        let final_cos_r = entity.resolved.rotation.cos();
        let final_sin_r = entity.resolved.rotation.sin();
        let anchor_dx = (0.5 - entity.resolved.anchor_x) * entity.resolved.width;
        let anchor_dy = (0.5 - entity.resolved.anchor_y) * entity.resolved.height;

        let visual_center_x = curr_anchor_x + anchor_dx * final_cos_r - anchor_dy * final_sin_r;
        let visual_center_y = curr_anchor_y + anchor_dx * final_sin_r + anchor_dy * final_cos_r;

        entity.resolved.x = visual_center_x;
        entity.resolved.y = visual_center_y;

        // Accumulate this entity's FINAL resolved state so its children can use it
        // Note: the children pivot around the ANCHOR, not the Visual Center!
        resolved_transforms.insert(
            entity.id.clone(),
            (
                curr_anchor_x,
                curr_anchor_y,
                entity.resolved.rotation,
                entity.resolved.opacity,
                entity.resolved.scale_x,
                entity.resolved.scale_y,
                entity.resolved.layer,
                entity.resolved.volume,
                entity.resolved.visible,
            ),
        );
    }
}
