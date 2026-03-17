//! ECS pipeline — runs all systems in the correct order.

use crate::ecs::World;
use crate::time::TimeState;
use super::systems;

/// Run all ECS systems for a single frame.
pub fn run(world: &mut World, time: &TimeState) {
    // Phase 1: Determine visibility based on timeline
    systems::timeline_system(world, time);

    // Phase 2: Resolve animation keyframes
    systems::animation_system(world, time);

    // Phase 3: Compute transform matrices
    systems::transform_system(world, time);

    // Future phases:
    // Phase 4: Particle system
    // Phase 5: Physics system
    // Phase 6: Bone/skeletal system
}
