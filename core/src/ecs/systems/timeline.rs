//! Timeline system — resolve entity visibility at current time.

use crate::ecs::World;
use crate::time::TimeState;

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
