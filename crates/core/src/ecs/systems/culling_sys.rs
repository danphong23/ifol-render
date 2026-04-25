use crate::ecs::World;
use crate::time::TimeState;

/// Evaluates visibility bounds intersecting the active camera to trim off-screen renders.
///
/// Computes an axis-aligned bounding box (AABB) for each visible entity and tests
/// intersection against every camera viewport. Entities that fall completely outside
/// ALL camera viewports are marked invisible, preventing draw call generation and
/// GPU work downstream.
pub fn culling_system(world: &mut World, _time: &TimeState) {
    let storages = &world.storages;

    // Collect all camera viewports (AABB: left, top, right, bottom)
    let mut camera_bounds: Vec<(f32, f32, f32, f32)> = Vec::new();
    for entity in world.entities.iter() {
        if !entity.resolved.visible {
            continue;
        }
        if storages
            .get_component::<crate::ecs::components::CameraComponent>(&entity.id)
            .is_some()
        {
            let half_w = entity.resolved.width * 0.5;
            let half_h = entity.resolved.height * 0.5;
            camera_bounds.push((
                entity.resolved.x - half_w,
                entity.resolved.y - half_h,
                entity.resolved.x + half_w,
                entity.resolved.y + half_h,
            ));
        }
    }

    // If no cameras found, skip culling (legacy single-camera fallback)
    if camera_bounds.is_empty() {
        return;
    }

    for entity in &mut world.entities {
        if !entity.resolved.visible {
            continue;
        }

        // Skip cameras themselves — they must always remain visible
        if storages
            .get_component::<crate::ecs::components::CameraComponent>(&entity.id)
            .is_some()
        {
            continue;
        }

        // Skip compositions — they define viewports, not visual elements
        if storages
            .get_component::<crate::ecs::components::Composition>(&entity.id)
            .is_some()
        {
            continue;
        }

        // Compute entity AABB (accounting for rotation by using the diagonal as conservative bound)
        let r = &entity.resolved;
        let half_w = r.width * 0.5;
        let half_h = r.height * 0.5;

        let (aabb_half_w, aabb_half_h) = if r.rotation.abs() > 1e-6 {
            // Conservative AABB: use the enclosing circle radius for rotated entities
            let diag = (half_w * half_w + half_h * half_h).sqrt();
            (diag, diag)
        } else {
            (half_w, half_h)
        };

        let ent_left = r.x - aabb_half_w;
        let ent_top = r.y - aabb_half_h;
        let ent_right = r.x + aabb_half_w;
        let ent_bottom = r.y + aabb_half_h;

        // Add padding to prevent popping at edges (accounts for effects like blur/glow)
        let padding = 64.0;

        // Check if entity intersects ANY camera viewport
        let in_any_camera = camera_bounds.iter().any(|&(cl, ct, cr, cb)| {
            ent_right + padding >= cl
                && ent_left - padding <= cr
                && ent_bottom + padding >= ct
                && ent_top - padding <= cb
        });

        if !in_any_camera {
            entity.resolved.visible = false;
        }
    }
}
