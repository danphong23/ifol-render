use crate::ecs::World;
use crate::time::TimeState;

/// Timeline system — resolves visibility and local_time from lifespan.
///
/// Uses `scope_time` (set by composition_sys) to determine if entity is active.
/// - scope_time = global_time for root entities
/// - scope_time = parent.content_time for children of Composition entities
///
/// Must run AFTER composition_sys.
pub fn timeline_system(world: &mut World, time: &TimeState) {
    for entity in &mut world.entities {
        let scope_time = entity.resolved.scope_time;
        
        if let Some(ls) = &entity.components.lifespan {
            entity.resolved.visible = ls.contains(scope_time);

            if entity.resolved.visible {
                let local_time = scope_time - ls.start;
                let duration = ls.end - ls.start;
                entity.resolved.time.local_time = local_time;
                entity.resolved.time.normalized_time = if duration > 0.0 {
                    local_time / duration
                } else {
                    0.0
                };
                entity.resolved.time.global_time = time.global_time;
                entity.resolved.time.delta_time = time.delta_time;
                entity.resolved.time.frame_index = time.frame_index;
            }
        } else {
            // No lifespan = always visible
            entity.resolved.visible = true;
            entity.resolved.time.global_time = time.global_time;
            entity.resolved.time.local_time = scope_time;
            entity.resolved.time.delta_time = time.delta_time;
            entity.resolved.time.frame_index = time.frame_index;
        }

        // If entity HAS Composition, compute content_time for its children
        if entity.resolved.visible {
            if let Some(comp) = &entity.components.composition {
                let local_time = entity.resolved.time.local_time;
                let raw_content = local_time * (comp.speed as f64) + comp.trim_start;
                
                let duration = match &comp.duration {
                    crate::ecs::components::composition::DurationMode::Manual(d) => *d,
                    crate::ecs::components::composition::DurationMode::Auto => {
                        // Auto-detect from trim_end or default
                        comp.trim_end.unwrap_or(10.0)
                    }
                };
                
                entity.resolved.content_time = crate::ecs::systems::composition_sys::apply_loop_mode(
                    raw_content, duration, &comp.loop_mode
                );
                entity.resolved.speed = comp.speed;
            }
            
            // Playback time for media: use content_time if in composition, else local_time
            if entity.components.video_source.is_some() || entity.components.audio_source.is_some() {
                entity.resolved.playback_time = entity.resolved.time.local_time;
            }
        }
    }
}
