use crate::ecs::World;
use crate::time::TimeState;
use crate::ecs::components::composition::LoopMode;

/// Composition system — resolves time scoping for nested timelines.
///
/// Must run FIRST (before timeline_sys) because timeline needs scope_time
/// to determine visibility for children of Composition entities.
///
/// For each entity:
///   - Root entities (no Composition parent): scope_time = global_time
///   - Children of Composition parent: scope_time = parent's content_time
///
/// For entities WITH Composition component:
///   - content_time = local_time × speed + trim_start (with loop applied)
///   - This content_time becomes scope_time for their children
pub fn composition_system(world: &mut World, time: &TimeState) {
    // Phase 1: Set scope_time for all entities
    // Root entities get global_time, children of Composition get parent's content_time
    
    // First pass: collect parent composition data
    let comp_data: std::collections::HashMap<String, f64> = world
        .entities
        .iter()
        .filter(|e| e.components.composition.is_some())
        .map(|e| (e.id.clone(), e.resolved.content_time))
        .collect();
    
    // Second pass: set scope_time for all entities
    for entity in &mut world.entities {
        if let Some(pid) = &entity.components.parent_id {
            if let Some(&parent_content) = comp_data.get(pid) {
                // Parent has Composition → child uses parent's content_time
                entity.resolved.scope_time = parent_content;
            } else {
                // Parent is normal → inherit same scope (will be resolved by ancestor chain)
                entity.resolved.scope_time = time.global_time;
            }
        } else {
            // Root entity → global_time
            entity.resolved.scope_time = time.global_time;
        }
    }
    
    // Pre-compute auto durations: for each composition entity, find max(children.lifespan.end)
    let mut auto_durations: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for entity in &world.entities {
        if let Some(pid) = &entity.components.parent_id {
            let child_end = entity.components.lifespan
                .map(|ls| ls.end - ls.start)
                .unwrap_or(0.0);
            let entry = auto_durations.entry(pid.clone()).or_insert(0.0_f64);
            *entry = entry.max(child_end);
        }
    }

    // Handle multi-level nesting: iterate until stable
    // (for deeply nested compositions, scope_time must propagate down)
    for _depth in 0..8 {
        // Recollect scope data after timeline updates
        let scope_map: std::collections::HashMap<String, (f64, bool)> = world
            .entities
            .iter()
            .map(|e| {
                let has_comp = e.components.composition.is_some();
                (e.id.clone(), (e.resolved.content_time, has_comp))
            })
            .collect();
        
        let mut changed = false;
        for entity in &mut world.entities {
            if let Some(pid) = &entity.components.parent_id {
                if let Some(&(parent_ct, parent_has_comp)) = scope_map.get(pid) {
                    if parent_has_comp {
                        let new_scope = parent_ct;
                        if (new_scope - entity.resolved.scope_time).abs() > 1e-9 {
                            entity.resolved.scope_time = new_scope;
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed { break; }
        
        // Re-compute content_time for any entity with Composition
        for entity in &mut world.entities {
            if let Some(comp) = &entity.components.composition {
                let scope = entity.resolved.scope_time;
                let lifespan = entity.components.lifespan.unwrap_or_default();
                
                if !lifespan.contains(scope) {
                    continue;
                }
                
                let local_time = scope - lifespan.start;
                let raw_content = local_time * (comp.speed as f64) + comp.trim_start;
                
                let duration = match &comp.duration {
                    crate::ecs::components::composition::DurationMode::Manual(d) => *d,
                    crate::ecs::components::composition::DurationMode::Auto => {
                        // Auto: compute max(children.lifespan.end)
                        auto_durations.get(&entity.id).copied().unwrap_or(5.0)
                    }
                };
                
                entity.resolved.max_duration = duration;
                entity.resolved.content_time = apply_loop_mode(raw_content, duration, &comp.loop_mode);
            }
        }
    }
}

/// Apply loop mode to raw content time.
pub fn apply_loop_mode(raw: f64, duration: f64, mode: &LoopMode) -> f64 {
    if duration <= 0.0 { return raw; }
    match mode {
        LoopMode::Once => raw.min(duration),
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
