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

    if !renderer.has_pipeline("mask") {
        renderer.register_pipeline(
            "mask",
            include_str!("../../shaders/mask.wgsl"),
            PipelineConfig::quad(),
        );
    }

    if !renderer.has_pipeline("copy") {
        renderer.register_effect(
            "copy",
            "
            struct VertexOutput {
                @builtin(position) clip_position: vec4f,
                @location(0) uv: vec2f,
            }

            @vertex
            fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> VertexOutput {
                var out: VertexOutput;
                let x = f32(i32(vi) / 2) * 4.0 - 1.0;
                let y = f32(i32(vi) % 2) * 4.0 - 1.0;
                out.clip_position = vec4f(x, y, 0.0, 1.0);
                out.uv = vec2f((x + 1.0) * 0.5, (1.0 - y) * 0.5);
                return out;
            }

            @group(0) @binding(1) var t_color: texture_2d<f32>;
            @group(0) @binding(2) var s_color: sampler;

            @fragment
            fn fs_main(in: VertexOutput) -> @location(0) vec4f {
                return textureSample(t_color, s_color, in.uv);
            }
            ",
            vec![("_pad".into(), 0.0)],
            1,
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
