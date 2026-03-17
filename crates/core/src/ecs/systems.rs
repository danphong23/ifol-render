//! ECS systems that process entities each frame.

use crate::ecs::World;
use crate::time::TimeState;
use crate::types::Mat4;

/// Resolve which entities are visible at the current time.
pub fn timeline_system(world: &mut World, time: &TimeState) {
    for entity in &mut world.entities {
        if let Some(tl) = &entity.components.timeline {
            let end = tl.start_time + tl.duration;
            entity.resolved.visible = time.global_time >= tl.start_time && time.global_time < end;
            entity.resolved.layer = tl.layer;

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
            if let Some(ref mut tf) = entity.components.transform.clone() {
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
                // Store back (we'll use this for transform_system)
                entity.components.transform = Some(tf.clone());
            }
        } else {
            entity.resolved.opacity = entity.components.opacity.unwrap_or(1.0);
        }
    }
}

/// Compute final world transform matrices.
pub fn transform_system(world: &mut World, _time: &TimeState) {
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
}
