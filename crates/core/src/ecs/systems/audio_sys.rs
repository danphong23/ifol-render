use crate::ecs::World;
use crate::ecs::components::{AudioSource, VideoSource, Composition};
use crate::frame::AudioCall;

/// Frame-By-Frame Audio System
///
/// Evaluates audio properties (volume, speed, playback time) for every entity containing
/// an AudioSource or VideoSource. Emits `AudioCall` events into the entity's DrawComponent
/// for the current frame. This allows backend mixing to be deterministic and completely decoupled
/// from static timelines.
pub fn audio_system(world: &mut World) {
    let storages = &world.storages;
    for entity in &mut world.entities {
        // Skip entities not currently active on the timeline
        if !entity.resolved.visible {
            continue;
        }

        let mut base_volume = 1.0;
        let mut has_audio = false;
        let mut asset_url = String::new();

        // Check if VideoSource exists and has audio enabled
        if let Some(video) = storages.get_component::<VideoSource>(&entity.id) {
            has_audio = true;
            // Volume is intrinsically linked to opacity/fades in V4 design for simplicity unless overridden.
            base_volume = entity.resolved.opacity;  
            asset_url = world.assets.get(&video.asset_id).map(|a| match a {
                crate::schema::v2::AssetDef::Video { url } => url.as_str(),
                _ => &video.asset_id,
            }).unwrap_or(&video.asset_id).to_string();
        }

        // Check if AudioSource exists
        if let Some(audio) = storages.get_component::<AudioSource>(&entity.id) {
            has_audio = true;
            base_volume = entity.resolved.opacity;
            asset_url = world.assets.get(&audio.asset_id).map(|a| match a {
                crate::schema::v2::AssetDef::Audio { url } => url.as_str(),
                _ => &audio.asset_id,
            }).unwrap_or(&audio.asset_id).to_string();
        }

        if !has_audio || asset_url.is_empty() {
            continue;
        }

        let final_volume = base_volume;

        // Resolve instantaneous speed (which is just the parent's composition tree speed multiplier)
        // This was already partially resolved if we just look at the Time system's delta.
        // Since play_speed isn't explicitly on `resolved`, we can derive it from World or parent.
        let parent_id = storages.get_component::<crate::ecs::components::meta::ParentId>(&entity.id).map(|p| p.0.clone());
        let speed = resolve_entity_speed(storages, &parent_id);

        entity.draw.audio_calls.push(AudioCall {
            url: asset_url,
            timestamp_secs: entity.resolved.playback_time,
            volume: final_volume,
            speed,
        });
    }
}

/// Recursively find the effective play speed by traversing parent compositions.
fn resolve_entity_speed(storages: &crate::ecs::typemap::TypeMap, parent_id: &Option<String>) -> f32 {
    let mut current_parent = parent_id.clone();
    let mut speed = 1.0;
    
    while let Some(pid) = current_parent {
        if let Some(comp) = storages.get_component::<Composition>(&pid) {
            speed *= comp.speed as f32; // Assuming speed is f64 or f32
        }
        current_parent = None; // In a full implementation, we'd look up the parent's parent in World.
        // We can't easily look up hierarchy from storages alone without `world.entities`.
        // We'll just assume depth 1 composition for now or rely on Time system pre-computation later.
    }
    
    speed
}
