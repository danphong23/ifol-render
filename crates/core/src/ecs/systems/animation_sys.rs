use crate::ecs::components::animation::AnimTarget;
use crate::ecs::World;

/// Animation System (V4)
///
/// Responsible for moving static component values into the dynamic `resolved` state,
/// and then evaluating any keyframe animations to override those defaults.
pub fn animation_system(world: &mut World) {
    let storages = &world.storages;
    for entity in &mut world.entities {
        if !entity.resolved.visible {
            continue;
        }

        let local_time = entity.resolved.time.local_time;

        // 1. Copy default static values from components → resolved
        if let Some(t) = storages.get_component::<crate::ecs::components::Transform>(&entity.id) {
            entity.resolved.x = t.x;
            entity.resolved.y = t.y;
            entity.resolved.rotation = t.rotation;
            entity.resolved.anchor_x = t.anchor_x;
            entity.resolved.anchor_y = t.anchor_y;
            entity.resolved.scale_x = t.scale_x;
            entity.resolved.scale_y = t.scale_y;
        } else {
            entity.resolved.x = 0.0;
            entity.resolved.y = 0.0;
            entity.resolved.rotation = 0.0;
            entity.resolved.anchor_x = 0.0;
            entity.resolved.anchor_y = 0.0;
            entity.resolved.scale_x = 1.0;
            entity.resolved.scale_y = 1.0;
        }

        if let Some(r) = storages.get_component::<crate::ecs::components::Rect>(&entity.id) {
            entity.resolved.width = r.width;
            entity.resolved.height = r.height;
            entity.resolved.fit_mode = r.fit_mode;
        } else {
            entity.resolved.width = 0.0;
            entity.resolved.height = 0.0;
            entity.resolved.fit_mode = Default::default();
        }

        if let Some(v) = storages.get_component::<crate::ecs::components::Visual>(&entity.id) {
            entity.resolved.opacity = v.opacity;
            entity.resolved.volume = v.volume;
            // blend_mode parse removed for brevity, will map properly later
            entity.resolved.blend_mode = Default::default();
        } else {
            entity.resolved.opacity = 1.0;
            entity.resolved.volume = 1.0;
            entity.resolved.blend_mode = Default::default();
        }

        // Color fallback (e.g. from ColorSource or ShapeSource)
        if let Some(c) = storages.get_component::<crate::ecs::components::ColorSource>(&entity.id) {
            let col = &c.color;
            entity.resolved.color = [col.r, col.g, col.b, col.a];
        } else if let Some(s) = storages.get_component::<crate::ecs::components::ShapeSource>(&entity.id) {
            entity.resolved.color = s.fill_color;
        } else {
            entity.resolved.color = [1.0, 1.0, 1.0, 1.0];
        }

        // 2. Evaluate AnimationComponent (if present) to override resolved values
        if let Some(anim) = storages.get_component::<crate::ecs::components::AnimationComponent>(&entity.id) {
            // Float tracks -> f32 targets
            for track in &anim.float_tracks {
                if track.track.keyframes.is_empty() {
                    continue;
                }
                let val = track.track.evaluate(local_time, 0.0) as f32;
                match track.target {
                    AnimTarget::TransformX => entity.resolved.x = val,
                    AnimTarget::TransformY => entity.resolved.y = val,
                    AnimTarget::TransformRotation => entity.resolved.rotation = val,
                    AnimTarget::TransformAnchorX => entity.resolved.anchor_x = val,
                    AnimTarget::TransformAnchorY => entity.resolved.anchor_y = val,
                    AnimTarget::TransformScaleX => entity.resolved.scale_x = val,
                    AnimTarget::TransformScaleY => entity.resolved.scale_y = val,
                    
                    AnimTarget::RectWidth => entity.resolved.width = val,
                    AnimTarget::RectHeight => entity.resolved.height = val,
                    
                    AnimTarget::Opacity => entity.resolved.opacity = val,
                    AnimTarget::Volume => entity.resolved.volume = val,
                    AnimTarget::PlaybackTime => entity.resolved.playback_time = val as f64,
                    
                    AnimTarget::ColorR => entity.resolved.color[0] = val,
                    AnimTarget::ColorG => entity.resolved.color[1] = val,
                    AnimTarget::ColorB => entity.resolved.color[2] = val,
                    AnimTarget::ColorA => entity.resolved.color[3] = val,
                    
                    AnimTarget::FloatUniform(_) => {
                        // Handled dynamically if needed, or matched internally
                    }
                    _ => {}
                }
            }

            // String tracks -> enum/string targets
            for track in &anim.string_tracks {
                if track.track.keyframes.is_empty() {
                    continue;
                }
                let val = track.track.evaluate(local_time, "");
                match track.target {
                    AnimTarget::BlendMode => {
                        // TODO: Map string back to BlendMode enum
                    }
                    AnimTarget::StringUniform(_) => {
                        // Uniform handling
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::animation::{AnimTarget, FloatAnimTrack};
    use crate::ecs::components::{AnimationComponent, Transform};
    use crate::ecs::Entity;
    use crate::scene::{FloatTrack, Interpolation, Keyframe};

    #[test]
    fn test_animation_system_evaluates_float_track() {
        let mut world = World::new();
        
        // Entity with base transform x=10, but animated to go from 100 to 200 over 1 second (linear)
        world.add_entity(Entity {
            id: "anim_test".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });

        // Static fallback
        world.add_component("anim_test", Transform {
            x: 10.0,
            y: 0.0,
            rotation: 0.0,
            anchor_x: 0.0,
            anchor_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        });

        // Animation override
        let mut track = FloatTrack::default();
        track.keyframes.push(Keyframe {
            time: 0.0,
            value: 100.0,
            interpolation: Interpolation::Linear,
        });
        track.keyframes.push(Keyframe {
            time: 1.0,
            value: 200.0,
            interpolation: Interpolation::Hold,
        });

        world.add_component("anim_test", AnimationComponent {
            float_tracks: vec![FloatAnimTrack {
                target: AnimTarget::TransformX,
                track,
            }],
            string_tracks: vec![],
        });

        // Force visibility
        world.entities[0].resolved.visible = true;

        // Test at t=0.5
        world.entities[0].resolved.time.local_time = 0.5;
        animation_system(&mut world);
        
        // Assert value is interpolated (150.0) and not the static base (10.0)
        assert_eq!(world.entities[0].resolved.x, 150.0);
    }
}
