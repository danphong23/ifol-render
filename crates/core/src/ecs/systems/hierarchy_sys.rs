use crate::ecs::World;
use crate::time::TimeState;

/// Propagates parent transforms and opacity to children.
/// Uses additive position (not matrix multiplication) — correct for 2D compositing.
/// Must run AFTER transform_system.
pub fn hierarchy_system(world: &mut World, _time: &TimeState) {
    // Collect parent data first to avoid borrow issues
    let parent_data: std::collections::HashMap<String, (f32, f32, f32, f32)> = world
        .entities
        .iter()
        .filter(|e| e.resolved.visible)
        .map(|e| (e.id.clone(), (e.resolved.x, e.resolved.y, e.resolved.rotation, e.resolved.opacity)))
        .collect();

    for entity in &mut world.entities {
        if !entity.resolved.visible {
            continue;
        }
        if let Some(pid) = &entity.components.parent_id {
            if let Some(&(px, py, p_rot, p_opacity)) = parent_data.get(pid) {
                // Additive position: child position is offset from parent
                // If parent has rotation, rotate child's offset around parent
                let rot_rad = p_rot.to_radians();
                let cos_r = rot_rad.cos();
                let sin_r = rot_rad.sin();
                let dx = entity.resolved.x;
                let dy = entity.resolved.y;
                entity.resolved.x = px + dx * cos_r - dy * sin_r;
                entity.resolved.y = py + dx * sin_r + dy * cos_r;
                // Additive rotation
                entity.resolved.rotation += p_rot;
                // Multiplicative opacity
                entity.resolved.opacity *= p_opacity;
            }
        }
    }
}
