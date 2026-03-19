//! Built-in shader registration.
//!
//! Core ships with these shaders. Frontend can register additional
//! custom shaders via `CoreEngine::register_shader()`.

use ifol_render::{PipelineConfig, Renderer};

/// Register all built-in shaders and effects.
///
/// Safe to call multiple times — skips already-registered pipelines.
pub fn setup_builtins(renderer: &mut Renderer) {
    // ── Entity shaders ──

    if !renderer.has_pipeline("composite") {
        renderer.register_pipeline(
            "composite",
            include_str!("../../shaders/composite.wgsl"),
            PipelineConfig::quad(),
        );
    }

    if !renderer.has_pipeline("shapes") {
        renderer.register_pipeline(
            "shapes",
            include_str!("../../shaders/shapes.wgsl"),
            PipelineConfig::quad(),
        );
    }

    if !renderer.has_pipeline("gradient") {
        renderer.register_pipeline(
            "gradient",
            include_str!("../../shaders/gradient.wgsl"),
            PipelineConfig::quad(),
        );
    }

    if !renderer.has_pipeline("mask") {
        renderer.register_pipeline(
            "mask",
            include_str!("../../shaders/mask.wgsl"),
            PipelineConfig::quad(),
        );
    }

    // ── Fullscreen effects ──

    if !renderer.has_pipeline("blur") {
        renderer.register_effect(
            "blur",
            include_str!("../../shaders/effects/blur.wgsl"),
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
            include_str!("../../shaders/effects/color_grade.wgsl"),
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
            include_str!("../../shaders/effects/vignette.wgsl"),
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
            include_str!("../../shaders/effects/chromatic_aberration.wgsl"),
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
