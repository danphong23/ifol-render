use crate::ecs::components::composition::LoopMode;
use crate::ecs::World;
use crate::time::TimeState;
use std::collections::HashMap;

/// Unified Time System (V4)
///
/// Responsible for time resolution and visibility across the entire hierarchy.
/// Replaces old composition_sys, timeline_sys, and speed_sys.
///
/// Resolution steps:
/// 1. Establish initial `scope_time` (global_time for roots).
/// 2. Iteratively cascade `content_time` down the hierarchy until stable.
/// 3. Determine final `visible` state and `local_time` for animation.
pub fn time_system(
    world: &mut World,
    time: &TimeState,
    scope_entity_id: Option<&str>,
    scope_time_override: Option<f64>,
) {
    let storages = &world.storages;
    // 1. Pre-compute auto durations for compositions
    let mut auto_durations: HashMap<String, f64> = HashMap::new();
    for entity in &world.entities {
        if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&entity.id).map(|id| &id.0) {
            let child_end = storages.get_component::<crate::scene::Lifespan>(&entity.id).copied().map(|ls| ls.end).unwrap_or(0.0);
            let entry = auto_durations.entry(pid.clone()).or_insert(0.0_f64);
            *entry = entry.max(child_end);
        }
    }

    // 2. Initialize scope_time (base time before parent transformations)
    for entity in &mut world.entities {
        entity.resolved.scope_time = time.global_time;
        entity.resolved.time.global_time = time.global_time;
        entity.resolved.time.delta_time = time.delta_time;
        entity.resolved.time.frame_index = time.frame_index;
    }

    // 3. Iterative cascade for nested compositions (up to 8 levels deep)
    for _depth in 0..8 {
        // Collect current content times of compositions
        let mut comp_states: HashMap<String, (f64, bool)> = HashMap::new();
        
        for entity in &mut world.entities {
            let scope = entity.resolved.scope_time;
            let ls = storages.get_component::<crate::scene::Lifespan>(&entity.id).copied().unwrap_or_default();
            
            // Check visibility at local scope
            let visible_at_scope = ls.contains(scope);
            
            // If it's a composition, calculate its internal content_time
            if let Some(comp) = storages.get_component::<crate::ecs::components::Composition>(&entity.id) {
                // If THIS entity is the scope entity and we have an override,
                // use the override directly as content_time (bypass speed/loop/trim)
                let is_scope_entity = scope_entity_id
                    .map(|sid| sid == entity.id.as_str())
                    .unwrap_or(false);
                
                let (content_time, duration) = if is_scope_entity && scope_time_override.is_some() {
                    let dur = match &comp.duration {
                        crate::ecs::components::composition::DurationMode::Manual(d) => *d,
                        crate::ecs::components::composition::DurationMode::Auto => {
                            auto_durations.get(&entity.id).copied().unwrap_or(5.0)
                        }
                    };
                    // Use override directly — no speed/loop/trim
                    (scope_time_override.unwrap(), dur)
                } else {
                    let local_time = scope - ls.start;
                    let raw_content = local_time * (comp.speed as f64) + comp.trim_start;
                    
                    let dur = match &comp.duration {
                        crate::ecs::components::composition::DurationMode::Manual(d) => *d,
                        crate::ecs::components::composition::DurationMode::Auto => {
                            auto_durations.get(&entity.id).copied().unwrap_or(5.0)
                        }
                    };
                    
                    let ct = apply_loop_mode(raw_content, dur, &comp.loop_mode);
                    (ct, dur)
                };
                
                entity.resolved.content_time = content_time;
                entity.resolved.max_duration = duration;
                
                comp_states.insert(entity.id.clone(), (content_time, visible_at_scope));
            }
        }

        // Propagate content_time down to children as their new scope_time
        let mut changed = false;
        for entity in &mut world.entities {
            if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&entity.id).map(|id| &id.0) {
                if let Some(&(parent_content_time, _parent_visible)) = comp_states.get(pid) {
                    if (parent_content_time - entity.resolved.scope_time).abs() > 1e-9 {
                        entity.resolved.scope_time = parent_content_time;
                        changed = true;
                    }
                    
                    // If parent composition is invisible, child should effectively be invisible, 
                    // but we handle visibility finalization in step 4.
                }
            }
        }
        
        if !changed {
            break;
        }
    }

    // 4. Finalize visibility, local time, and media playback time
    // Visibility depends on self lifespan AND parent composition visibility.
    let comp_visibility: HashMap<String, bool> = world
        .entities
        .iter()
        .filter(|e| storages.get_component::<crate::ecs::components::Composition>(&e.id).is_some())
        .map(|e| {
            // Force scope entity visible when scope_time_override is active
            let is_scope = scope_entity_id
                .map(|sid| sid == e.id.as_str())
                .unwrap_or(false);
            let vis = if is_scope && scope_time_override.is_some() {
                true
            } else {
                storages.get_component::<crate::scene::Lifespan>(&e.id).copied().unwrap_or_default().contains(e.resolved.scope_time)
            };
            (e.id.clone(), vis)
        })
        .collect();

    for entity in &mut world.entities {
        let scope = entity.resolved.scope_time;
        let ls = storages.get_component::<crate::scene::Lifespan>(&entity.id).copied().unwrap_or_default();
        
        let mut visible = ls.contains(scope);
        
        // Hide if parent composition is hidden
        if let Some(pid) = storages.get_component::<crate::ecs::components::meta::ParentId>(&entity.id).map(|id| &id.0) {
            if let Some(&parent_vis) = comp_visibility.get(pid) {
                if !parent_vis {
                    visible = false;
                }
            }
        }
        
        entity.resolved.visible = visible;
        
        if visible {
            let local_time = scope - ls.start;
            let duration = ls.end - ls.start;
            
            entity.resolved.time.local_time = local_time;
            entity.resolved.time.normalized_time = if duration > 0.0 {
                local_time / duration
            } else {
                0.0
            };
            
            // V4 Phase 3: playback_time is no longer magically inferred from local_time.
            // It defaults to 0.0 and MUST be explicitly driven by an AnimationComponent 
            // possessing a PlaybackTime FloatTrack. Core does not assume video length.
            entity.resolved.playback_time = 0.0;
        }
    }
}

/// Apply loop mode to raw content time.
pub fn apply_loop_mode(mut raw: f64, duration: f64, mode: &LoopMode) -> f64 {
    if duration <= 0.0 { return raw; }
    
    // Prevent floating point drift for exact boundaries
    if (raw - duration).abs() < 1e-9 {
        raw = duration;
    }
    if raw.abs() < 1e-9 {
        raw = 0.0;
    }
    
    match mode {
        LoopMode::Once => raw.min(duration).max(0.0),
        LoopMode::Loop => {
            if raw < 0.0 { 0.0 } else { raw % duration }
        }
        LoopMode::PingPong => {
            if raw < 0.0 { return 0.0; }
            let cycle = (raw / duration) as u64;
            let frac = raw % duration;
            if cycle % 2 == 0 { frac } else { duration - frac }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::{Entity, World};
    use crate::ecs::components::{Composition, composition::DurationMode};
    use crate::ecs::components::meta::ParentId;
    use crate::scene::Lifespan;

    fn build_world() -> World {
        let mut w = World::new();

        // Root Entity (ID: "root")
        w.add_entity(Entity {
            id: "root".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });
        w.add_component("root", Lifespan { start: 1.0, end: 5.0 });

        // Comp Entity (ID: "comp")
        w.add_entity(Entity {
            id: "comp".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });
        w.add_component("comp", Lifespan { start: 2.0, end: 10.0 });
        w.add_component("comp", Composition {
            speed: 2.0,
            trim_start: 1.0,
            trim_end: None,
            loop_mode: LoopMode::Loop,
            duration: DurationMode::Manual(4.0),
            ..Default::default()
        });

        // Child Entity of Comp (ID: "child")
        w.add_entity(Entity {
            id: "child".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });
        w.add_component("child", ParentId("comp".to_string()));
        w.add_component("child", Lifespan { start: 1.0, end: 3.0 });

        w
    }

    #[test]
    fn test_time_system_basic_visibility() {
        let mut world = build_world();
        let mut time = TimeState::default();
        
        // At t=0.0
        time.global_time = 0.0;
        time_system(&mut world, &time, None, None);
        assert!(!world.entities[0].resolved.visible); // root (1..5)
        assert!(!world.entities[1].resolved.visible); // comp (2..10)
        assert!(!world.entities[2].resolved.visible); // child

        // At t=1.5
        time.global_time = 1.5;
        time_system(&mut world, &time, None, None);
        assert!(world.entities[0].resolved.visible); // root active
        assert_eq!(world.entities[0].resolved.time.local_time, 0.5); // 1.5 - 1.0
        assert!(!world.entities[1].resolved.visible); // comp not active yet

        // At t=2.5
        time.global_time = 2.5;
        time_system(&mut world, &time, None, None);
        assert!(world.entities[1].resolved.visible); // comp is active
        assert_eq!(world.entities[1].resolved.time.local_time, 0.5); // 2.5 - 2.0
        
        // Comp content time: local_time(0.5) * speed(2.0) + trim(1.0) = 2.0
        assert_eq!(world.entities[1].resolved.content_time, 2.0);
        
        // Child scope time should be 2.0
        assert_eq!(world.entities[2].resolved.scope_time, 2.0);
        // Child lifespan is 1..3, so at scope=2.0 it is visible
        assert!(world.entities[2].resolved.visible);
        // Child local time: scope(2.0) - start(1.0) = 1.0
        assert_eq!(world.entities[2].resolved.time.local_time, 1.0);
    }
}
