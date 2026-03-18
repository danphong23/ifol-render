//! ECS systems that process entities each frame.

use crate::ecs::World;
use crate::time::TimeState;
use crate::types::Mat4;

/// Resolve which entities are visible at the current time.
///
/// Handles:
/// - Timeline start/end range
/// - Entity `visible` flag
/// - Track `muted` / `solo` logic
/// - Z-index resolution
pub fn timeline_system(world: &mut World, time: &TimeState) {
    // Solo logic: if ANY entity has solo=true, only solo entities are visible
    let has_solo = world
        .entities
        .iter()
        .any(|e| e.components.timeline.as_ref().is_some_and(|tl| tl.solo));

    for entity in &mut world.entities {
        // Check entity-level visibility flag
        if !entity.components.visible {
            entity.resolved.visible = false;
            continue;
        }

        if let Some(tl) = &entity.components.timeline {
            let end = tl.start_time + tl.duration;
            let in_range = time.global_time >= tl.start_time && time.global_time < end;

            // Apply mute/solo logic
            let track_visible = if has_solo {
                tl.solo // Only solo tracks visible
            } else {
                !tl.muted // Non-muted tracks visible
            };

            entity.resolved.visible = in_range && track_visible;
            entity.resolved.layer = tl.layer;

            // Resolve z_index from transform
            if let Some(tf) = &entity.components.transform {
                entity.resolved.z_index = tf.z_index;
            }

            if entity.resolved.visible {
                let local_time = time.global_time - tl.start_time;
                entity.resolved.time.local_time = local_time;
                entity.resolved.time.normalized_time = if tl.duration > 0.0 {
                    local_time / tl.duration
                } else {
                    0.0
                };
                entity.resolved.time.global_time = time.global_time;
                entity.resolved.time.delta_time = time.delta_time;
                entity.resolved.time.frame_index = time.frame_index;
            }
        } else {
            // No timeline = always visible
            entity.resolved.visible = true;
        }
    }
}

/// Resolve animation keyframes at the current time.
pub fn animation_system(world: &mut World, _time: &TimeState) {
    for entity in &mut world.entities {
        if !entity.resolved.visible {
            continue;
        }
        let local_time = entity.resolved.time.local_time;

        if let Some(anim) = &entity.components.animation {
            // Animate opacity
            if let Some(val) = anim.evaluate("opacity", local_time) {
                entity.resolved.opacity = val as f32;
            } else {
                entity.resolved.opacity = entity.components.opacity.unwrap_or(1.0);
            }

            // Animate transform properties
            if let Some(ref tf) = entity.components.transform {
                let mut tf = tf.clone();
                if let Some(x) = anim.evaluate("transform.position.x", local_time) {
                    tf.position.x = x as f32;
                }
                if let Some(y) = anim.evaluate("transform.position.y", local_time) {
                    tf.position.y = y as f32;
                }
                if let Some(sx) = anim.evaluate("transform.scale.x", local_time) {
                    tf.scale.x = sx as f32;
                }
                if let Some(sy) = anim.evaluate("transform.scale.y", local_time) {
                    tf.scale.y = sy as f32;
                }
                if let Some(rot) = anim.evaluate("transform.rotation", local_time) {
                    tf.rotation = rot as f32;
                }
                entity.components.transform = Some(tf);
            }
        } else {
            entity.resolved.opacity = entity.components.opacity.unwrap_or(1.0);
        }
    }
}

/// Compute final world transform matrices.
/// Resolves parent-child hierarchy: child_world = parent_world * child_local
pub fn transform_system(world: &mut World, _time: &TimeState) {
    // Pass 1: Compute local transform matrices for all visible entities
    for entity in &mut world.entities {
        if !entity.resolved.visible {
            continue;
        }
        if let Some(tf) = &entity.components.transform {
            entity.resolved.world_matrix =
                Mat4::from_2d(tf.position, tf.scale, tf.rotation, tf.anchor);
        } else {
            entity.resolved.world_matrix = Mat4::identity();
        }
    }

    // Pass 2: Resolve parent-child hierarchy
    // Clone IDs + parent refs to avoid borrow issues
    let hierarchy: Vec<(String, Option<String>)> = world
        .entities
        .iter()
        .map(|e| (e.id.clone(), e.components.parent.clone()))
        .collect();

    for (id, parent_id) in &hierarchy {
        if let Some(parent) = parent_id {
            // Find parent's world matrix
            let parent_matrix = world
                .entities
                .iter()
                .find(|e| e.id == *parent)
                .map(|e| e.resolved.world_matrix)
                .unwrap_or(Mat4::identity());

            // Apply: child_world = parent_world * child_local
            if let Some(entity) = world.entities.iter_mut().find(|e| e.id == *id) {
                entity.resolved.world_matrix = parent_matrix.mul(&entity.resolved.world_matrix);
            }
        }
    }
}
