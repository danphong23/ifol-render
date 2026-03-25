use crate::ecs::World;
use crate::time::TimeState;
use crate::ecs::components::{BlendMode, FitMode};

/// Visual system — resolves visual rendering properties.
///
/// Single responsibility: opacity, blend mode, volume, layer.
/// Does NOT handle: size (rect_sys), position (transform_sys), fit mode (rect_sys),
/// speed/playback (composition_sys + speed_sys).
pub fn visual_system(world: &mut World, _time: &TimeState) {
    for entity in &mut world.entities {
        if !entity.resolved.visible { continue; }

        let local_time = entity.resolved.time.local_time;

        // Opacity
        if let Some(track) = &entity.components.opacity {
            entity.resolved.opacity = track.evaluate(local_time, 1.0) as f32;
        }

        // Volume
        if let Some(track) = &entity.components.volume {
            entity.resolved.volume = track.evaluate(local_time, 1.0) as f32;
        }

        // Blend mode
        if let Some(track) = &entity.components.blend_mode {
            let val = track.evaluate(local_time, "normal");
            entity.resolved.blend_mode = BlendMode::from_str(val);
        }

        // Layer
        entity.resolved.layer = entity.components.layer.unwrap_or(0);
    }
}
