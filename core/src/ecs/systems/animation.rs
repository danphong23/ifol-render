//! Animation system — resolve keyframe interpolation.

use crate::ecs::World;
use crate::time::TimeState;

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
