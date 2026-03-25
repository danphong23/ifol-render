use crate::ecs::World;
use crate::time::TimeState;

/// Speed system — legacy compatibility for standalone speed tracks.
///
/// For new entities, speed should be in Composition component.
/// This system handles legacy `speed` component on entities.
pub fn speed_system(world: &mut World, _time: &TimeState) {
    for entity in &mut world.entities {
        if !entity.resolved.visible { continue; }

        let local_time = entity.resolved.time.local_time;
        
        // Legacy speed track (if Composition hasn't already set it)
        if entity.components.composition.is_none() {
            if let Some(track) = &entity.components.speed {
                let speed = track.evaluate(local_time, 1.0) as f64;
                entity.resolved.speed = speed as f32;
                
                // Legacy playback time adjustment
                let mut trim_start = 0.0;
                if let Some(vs) = &entity.components.video_source {
                    trim_start = vs.trim_start;
                }
                entity.resolved.playback_time = (local_time * speed) + trim_start;
            }
        }
    }
}
