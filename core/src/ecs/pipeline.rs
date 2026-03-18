//! ECS pipeline — runs all systems in the correct order,
//! builds draw commands, and calls the render tool.
//!
//! Core ALSO owns shader registration: it provides WGSL sources
//! to render via `setup_renderer()`.

use super::draw;
use super::systems;
use crate::ecs::World;
use crate::scene::RenderSettings;
use crate::time::TimeState;
use ifol_render::{PipelineConfig, Renderer};

/// Register core's built-in shaders into the renderer.
///
/// This must be called once before rendering.
/// Core OWNS the shader files and provides them to render.
pub fn setup_renderer(renderer: &mut Renderer) {
    // Composite pipeline (quad rendering with blend modes)
    if !renderer.has_pipeline("composite") {
        renderer.register_pipeline(
            "composite",
            include_str!("../../../shaders/composite.wgsl"),
            PipelineConfig::quad(),
        );
    }

    // SDF shapes pipeline
    if !renderer.has_pipeline("shapes") {
        renderer.register_pipeline(
            "shapes",
            include_str!("../../../shaders/shapes.wgsl"),
            PipelineConfig::quad(),
        );
    }

    // Gradient pipeline
    if !renderer.has_pipeline("gradient") {
        renderer.register_pipeline(
            "gradient",
            include_str!("../../../shaders/gradient.wgsl"),
            PipelineConfig::quad(),
        );
    }

    // Mask/clip pipeline
    if !renderer.has_pipeline("mask") {
        renderer.register_pipeline(
            "mask",
            include_str!("../../../shaders/mask.wgsl"),
            PipelineConfig::quad(),
        );
    }

    // Built-in effects
    if !renderer.has_pipeline("blur") {
        renderer.register_effect(
            "blur",
            include_str!("../../../shaders/effects/blur.wgsl"),
            vec![
                ("direction_x".into(), 1.0),
                ("direction_y".into(), 0.0),
                ("radius".into(), 4.0),
                ("texel_size".into(), 0.001),
            ],
            2,
        );
    }

    if !renderer.has_pipeline("color_grade") {
        renderer.register_effect(
            "color_grade",
            include_str!("../../../shaders/effects/color_grade.wgsl"),
            vec![
                ("brightness".into(), 0.0),
                ("contrast".into(), 1.0),
                ("saturation".into(), 1.0),
                ("_pad".into(), 0.0),
            ],
            1,
        );
    }

    if !renderer.has_pipeline("vignette") {
        renderer.register_effect(
            "vignette",
            include_str!("../../../shaders/effects/vignette.wgsl"),
            vec![
                ("intensity".into(), 0.5),
                ("smoothness".into(), 0.5),
                ("_pad0".into(), 0.0),
                ("_pad1".into(), 0.0),
            ],
            1,
        );
    }

    if !renderer.has_pipeline("chromatic_aberration") {
        renderer.register_effect(
            "chromatic_aberration",
            include_str!("../../../shaders/effects/chromatic_aberration.wgsl"),
            vec![
                ("intensity".into(), 0.005),
                ("_pad0".into(), 0.0),
                ("_pad1".into(), 0.0),
                ("_pad2".into(), 0.0),
            ],
            1,
        );
    }
}

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
    // Ensure shaders are registered
    setup_renderer(renderer);

    // Step 1: Run ECS systems
    run(world, time);

    // Step 2: Build draw commands (unit→pixel conversion happens here)
    let commands = draw::build_draw_commands(world, settings);

    // Step 3: Call passive render tool
    renderer.render_frame(&commands)
}
