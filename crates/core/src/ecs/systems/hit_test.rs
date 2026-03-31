//! Entity hit-testing system.
//!
//! Provides `pick_entity_at()` — given a screen-space point, returns the
//! topmost entity under the cursor by reverse-traversing z-order and
//! testing against each entity's Oriented Bounding Box (OBB).
//!
//! This module belongs to **Core** — it knows nothing about mouse events
//! or DOM. The App layer calls this API with pre-computed screen coords.

use crate::ecs::World;

/// Pick the topmost entity at a screen-space coordinate.
///
/// # Arguments
/// - `world`  — the ECS world (must have resolved state from a pipeline run)
/// - `screen_x`, `screen_y` — click position in pixels (canvas space, origin top-left)
/// - `cam_x`, `cam_y` — camera world-space position (top-left)
/// - `scale_x`, `scale_y` — pixels-per-world-unit (screen_width / cam_viewport_width)
/// - `is_editor_mode` — when true, cameras are pickable (they render as gizmos)
///
/// # Returns
/// The entity ID of the topmost hit, or `None` if nothing was hit.
pub struct HitResult {
    pub entity_id: String,
    pub u: f32, // Normalized 0..1 across the hit bounds
    pub v: f32, // Normalized 0..1
}

pub fn pick_entity_at(
    world: &World,
    screen_x: f32,
    screen_y: f32,
    cam_x: f32,
    cam_y: f32,
    scale_x: f32,
    scale_y: f32,
    is_editor_mode: bool,
    scope_id: Option<&str>,
) -> Vec<HitResult> {
    let storages = &world.storages;

    // Helper: check if an entity is a child (direct or nested) of a Composition
    let is_inside_comp = |ent_id: &str| -> bool {
        let mut cur = ent_id.to_string();
        for _ in 0..32 {
            if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&cur) {
                if storages.get_component::<crate::ecs::components::Composition>(&pid.0).is_some() {
                    // If we're scoped into this comp, its children are "root-level"
                    if scope_id == Some(pid.0.as_str()) {
                        return false;
                    }
                    return true;
                }
                cur = pid.0.clone();
            } else { break; }
        }
        false
    };

    // Helper: check if entity is within the active scope
    let is_in_scope = |ent_id: &str| -> bool {
        match scope_id {
            None => !is_inside_comp(ent_id),  // Root: only top-level entities
            Some(sid) => {
                // Must be a descendant of the scope entity
                let mut cur = ent_id.to_string();
                for _ in 0..32 {
                    if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&cur) {
                        if pid.0 == sid { return true; }
                        cur = pid.0.clone();
                    } else { return false; }
                }
                false
            }
        }
    };

    // Collect visible entities sorted by layer (top-first for picking)
    let mut sorted: Vec<(&crate::ecs::Entity, i32)> = world
        .entities
        .iter()
        .filter(|e| e.resolved.visible)
        .filter(|e| is_in_scope(&e.id))
        .filter(|e| {
            // Skip cameras in render mode (they are not visible content)
            // In editor mode, cameras are pickable (rendered as dashed gizmos)
            if storages
                .get_component::<crate::ecs::components::CameraComponent>(&e.id)
                .is_some()
            {
                return is_editor_mode;
            }
            true
        })
        .map(|e| {
            // Cameras render at layer 9999 in editor mode — match that for picking
            let pick_layer = if storages
                .get_component::<crate::ecs::components::CameraComponent>(&e.id)
                .is_some()
            {
                9999
            } else {
                e.resolved.layer
            };
            (e, pick_layer)
        })
        .collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1)); // Reverse: top first

    let mut hits = Vec::new();

    for (entity, _layer) in &sorted {
        let r = &entity.resolved;
        let is_camera = storages
            .get_component::<crate::ecs::components::CameraComponent>(&entity.id)
            .is_some();

        let (min_x, max_x, min_y, max_y) = if is_camera {
            // Unity-style: triangle ABOVE the camera frame, pointing DOWN
            let tri_size = r.width * 0.05;
            let orig_min_x = -r.anchor_x * r.width;
            let orig_min_y = -r.anchor_y * r.height;
            let tri_center_x = orig_min_x + r.width * 0.5;
            let tri_center_y = orig_min_y - tri_size * 0.6; // Above top edge
            let hw = tri_size * 0.5;
            let hh = tri_size * 0.5;
            (
                tri_center_x - hw,
                tri_center_x + hw,
                tri_center_y - hh,
                tri_center_y + hh,
            )
        } else {
            // Fetch align properties from Rect component (default to 0.5 center if missing)
            let align_x = storages
                .get_component::<crate::ecs::components::Rect>(&entity.id)
                .as_ref()
                .map(|rc| rc.align_x)
                .unwrap_or(0.5);
            let align_y = storages
                .get_component::<crate::ecs::components::Rect>(&entity.id)
                .as_ref()
                .map(|rc| rc.align_y)
                .unwrap_or(0.5);

            // Always use content-adjusted boundaries (Content Mode)
            let (rb_ox, rb_oy, rb_w, rb_h) = r.fit_mode.calculate_rendered_bounds(
                r.width,
                r.height,
                r.intrinsic_width,
                r.intrinsic_height,
                align_x,
                align_y,
            );

            // Bounds in local space where (0,0) is the VISUAL center!
            let orig_min_x = -0.5 * r.width;
            let orig_min_y = -0.5 * r.height;

            let min_x = orig_min_x + rb_ox;
            let max_x = min_x + rb_w;
            let min_y = orig_min_y + rb_oy;
            let max_y = min_y + rb_h;
            (min_x, max_x, min_y, max_y)
        };

        // Project actual anchor to screen space (this is the center of rotation!)
        let anchor_sx = (r.x - cam_x) * scale_x;
        let anchor_sy = (r.y - cam_y) * scale_y;

        // Vector from anchor to click point
        let dx = screen_x - anchor_sx;
        let dy = screen_y - anchor_sy;

        // Rotate click point into local anchor space (inverse rotation, then scale inverse)
        let (mut local_x, mut local_y) = if r.rotation.abs() < 1e-6 {
            (dx, dy)
        } else {
            let cos_r = r.rotation.cos();
            let sin_r = r.rotation.sin();
            (dx * cos_r + dy * sin_r, -dx * sin_r + dy * cos_r)
        };

        // Convert screen-scale local vector back to world-scale local vector
        local_x /= scale_x;
        local_y /= scale_y;

        // Exact bounds test on the rendered content
        if local_x >= min_x && local_x <= max_x && local_y >= min_y && local_y <= max_y {
            let w = (max_x - min_x).max(0.0001);
            let h = (max_y - min_y).max(0.0001);
            let u = (local_x - min_x) / w;
            let v = (local_y - min_y) / h;

            hits.push(HitResult {
                entity_id: entity.id.clone(),
                u,
                v,
            });
        }
    }

    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::*;
    use crate::ecs::{Entity, World};
    use crate::scene::Lifespan;

    fn build_test_world() -> World {
        let mut w = World::new();

        // Entity at (100, 100) with 200x100 rect
        w.add_entity(Entity {
            id: "box_a".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });
        w.add_component(
            "box_a",
            Lifespan {
                start: 0.0,
                end: 10.0,
            },
        );
        w.add_component(
            "box_a",
            Rect {
                width: 200.0,
                height: 100.0,
                ..Default::default()
            },
        );
        w.add_component(
            "box_a",
            Transform {
                x: 100.0,
                y: 100.0,
                ..Default::default()
            },
        );
        w.add_component("box_a", meta::Layer(0));

        // Entity at (150, 120) with 100x100 rect — overlapping, higher layer
        w.add_entity(Entity {
            id: "box_b".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });
        w.add_component(
            "box_b",
            Lifespan {
                start: 0.0,
                end: 10.0,
            },
        );
        w.add_component(
            "box_b",
            Rect {
                width: 100.0,
                height: 100.0,
                ..Default::default()
            },
        );
        w.add_component(
            "box_b",
            Transform {
                x: 150.0,
                y: 120.0,
                ..Default::default()
            },
        );
        w.add_component("box_b", meta::Layer(1));

        // Camera (should not be pickable)
        w.add_entity(Entity {
            id: "cam".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });
        w.add_component("cam", CameraComponent::default());
        w.add_component(
            "cam",
            Lifespan {
                start: 0.0,
                end: 10.0,
            },
        );
        w.add_component(
            "cam",
            Rect {
                width: 1280.0,
                height: 720.0,
                ..Default::default()
            },
        );
        w.add_component("cam", Transform::default());

        // Run pipeline to resolve positions
        let time = crate::time::TimeState {
            global_time: 1.0,
            fps: 60.0,
            ..Default::default()
        };
        crate::ecs::pipeline::run(&mut w, &time, None, None);

        w
    }

    #[test]
    fn test_pick_hits_topmost_entity() {
        let world = build_test_world();
        let hits = pick_entity_at(&world, 200.0, 170.0, 0.0, 0.0, 1.0, 1.0, false, None);
        assert_eq!(
            hits.first().map(|h| h.entity_id.clone()),
            Some("box_b".to_string())
        );
    }

    #[test]
    fn test_pick_hits_lower_entity_outside_overlap() {
        let world = build_test_world();
        let hits = pick_entity_at(&world, 110.0, 110.0, 0.0, 0.0, 1.0, 1.0, false, None);
        assert_eq!(
            hits.first().map(|h| h.entity_id.clone()),
            Some("box_a".to_string())
        );
    }

    #[test]
    fn test_pick_misses_empty_space() {
        let world = build_test_world();
        let hits = pick_entity_at(&world, 50.0, 50.0, 0.0, 0.0, 1.0, 1.0, false, None);
        assert!(hits.is_empty());
    }

    #[test]
    fn test_pick_skips_cameras_in_render_mode() {
        let world = build_test_world();
        let hits = pick_entity_at(&world, 640.0, 360.0, 0.0, 0.0, 1.0, 1.0, false, None);
        assert!(hits.is_empty());
    }

    #[test]
    fn test_pick_cameras_in_editor_mode() {
        let world = build_test_world();
        let hits = pick_entity_at(&world, 640.0, -38.0, 0.0, 0.0, 1.0, 1.0, true, None);
        assert_eq!(
            hits.first().map(|h| h.entity_id.clone()),
            Some("cam".to_string())
        );
    }

    #[test]
    fn test_pick_camera_body_not_pickable() {
        let world = build_test_world();
        let hits = pick_entity_at(&world, 640.0, 360.0, 0.0, 0.0, 1.0, 1.0, true, None);
        assert!(hits.is_empty());
    }

    #[test]
    fn test_pick_with_camera_offset() {
        let world = build_test_world();
        let hits = pick_entity_at(&world, 10.0, 10.0, 100.0, 100.0, 1.0, 1.0, false, None);
        assert_eq!(
            hits.first().map(|h| h.entity_id.clone()),
            Some("box_a".to_string())
        );
    }
}
