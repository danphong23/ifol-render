//! ECS pipeline — runs all systems in the correct order.

use super::systems;
use crate::ecs::World;
use crate::time::TimeState;

/// Run all ECS systems for a single frame.
///
/// V4 Pipeline:
/// 1. time_sys (resolve visible, local_time, content_time)
/// 2. animation_sys (copy defaults + evaluate keyframes)
/// 3. rect_sys (resolve size + scale)
/// 4. hierarchy_sys (cascade position/rotation/scale/opacity to children)
pub fn run(
    world: &mut World,
    time: &TimeState,
    scope_entity_id: Option<&str>,
    scope_time_override: Option<f64>,
) {
    if world.entities.is_empty() {
        return;
    }

    // Phase 1: Time
    systems::time_system(world, time, scope_entity_id, scope_time_override);

    // Phase 2: Animation & Value evaluation
    systems::animation_system(world);

    // Phase 3: Size
    systems::rect_system(world, time);

    // Phase 4: Hierarchy cascade (World space resolution)
    systems::hierarchy_system(world, time);
    
    // Phase 5: Culling (skip invisible bounds)
    // systems::culling_system(world, time);

    // Phase 6: Source mapping (populate primitive DrawCalls)
    systems::source_system(world);
}
