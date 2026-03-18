//! ECS pipeline — runs all systems in the correct order,
//! builds draw commands, and calls the render tool.

use super::draw;
use super::systems;
use crate::ecs::World;
use crate::scene::RenderSettings;
use crate::time::TimeState;
use ifol_render::Renderer;

/// Run all ECS systems for a single frame (no rendering).
pub fn run(world: &mut World, time: &TimeState) {
    // Phase 1: Determine visibility based on timeline
    systems::timeline_system(world, time);

    // Phase 2: Resolve animation keyframes
    systems::animation_system(world, time);

    // Phase 3: Compute transform matrices
    systems::transform_system(world, time);

    // Phase 4: Process effect stacks
    systems::effects_system(world, time);

    // Future phases:
    // Phase 5: Particle system
    // Phase 6: Physics system
    // Phase 7: Bone/skeletal system
}

/// Full pipeline: run ECS systems → build draw commands → render → return pixels.
///
/// This is the main entry point for consumers (editor/CLI).
pub fn render_frame(
    world: &mut World,
    time: &TimeState,
    settings: &RenderSettings,
    renderer: &mut Renderer,
) -> Vec<u8> {
    // Step 1: Run ECS systems
    run(world, time);

    // Step 2: Build draw commands (unit→pixel conversion happens here)
    let commands = draw::build_draw_commands(world, settings);

    // Step 3: Call passive render tool
    renderer.render_frame(&commands)
}
