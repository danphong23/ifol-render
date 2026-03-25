//! ECS pipeline — runs all systems in the correct order.

use super::systems;
use crate::ecs::World;
use crate::time::TimeState;

/// Run all ECS systems for a single frame.
pub fn run(world: &mut World, time: &TimeState) {
    // Phase 1: Time — composition scoping + visibility
    systems::composition_system(world, time);  // scope_time for nested timelines
    systems::timeline_system(world, time);     // visibility + local_time (from lifespan)
    systems::speed_system(world, time);        // legacy speed + playback_time

    // Phase 2: Spatial — position + scale
    systems::transform_system(world, time);    // x, y, rotation, anchor, scale

    // Phase 3: Display — size + fit
    systems::rect_system(world, time);         // width, height, fit_mode, intrinsic, aspect

    // Phase 4: Visual — rendering properties
    systems::visual_system(world, time);       // opacity, blend_mode, volume, layer

    // Phase 5: Hierarchy — parent→child propagation
    systems::hierarchy_system(world, time);    // cascading transform + opacity
}
