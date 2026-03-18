//! DrawCommand builder — converts resolved ECS entities into GPU draw commands.
//!
//! This is where the unit system (0..1 normalized) gets converted to
//! clip-space coordinates for the GPU.
//!
//! Core OWNS the composite shader and packs uniforms to match its layout.

use crate::ecs::World;
use crate::scene::RenderSettings;
use ifol_render::DrawCommand;

/// Composite shader uniform layout:
/// [transform: f32x16, color: f32x4, opacity: f32, use_texture: f32, blend_mode: f32, _pad: f32]
/// = 24 floats total

/// Map core BlendMode to shader float.
fn blend_mode_to_float(mode: Option<&crate::ecs::components::BlendMode>) -> f32 {
    match mode {
        Some(crate::ecs::components::BlendMode::Multiply) => 1.0,
        Some(crate::ecs::components::BlendMode::Screen) => 2.0,
        Some(crate::ecs::components::BlendMode::Overlay) => 3.0,
        Some(crate::ecs::components::BlendMode::SoftLight) => 4.0,
        Some(crate::ecs::components::BlendMode::Add) => 5.0,
        Some(crate::ecs::components::BlendMode::Difference) => 6.0,
        _ => 0.0, // Normal
    }
}

/// Build GPU draw commands from the resolved ECS world.
///
/// This runs AFTER all ECS systems have resolved visibility, animation,
/// and transforms. It converts entity state into generic `DrawCommand` structs
/// that the passive render tool can consume.
pub fn build_draw_commands(world: &World, _settings: &RenderSettings) -> Vec<DrawCommand> {
    let sorted = world.sorted_by_layer();
    let mut commands = Vec::with_capacity(sorted.len());

    for entity in sorted {
        // Determine draw source and pack uniforms
        let (color, use_texture, textures) =
            if let Some(ref color_src) = entity.components.color_source {
                (
                    [
                        color_src.color.r,
                        color_src.color.g,
                        color_src.color.b,
                        color_src.color.a,
                    ],
                    0.0f32,
                    vec![],
                )
            } else if entity.components.image_source.is_some() {
                ([1.0f32, 1.0, 1.0, 1.0], 1.0f32, vec![entity.id.clone()])
            } else {
                continue;
            };

        let blend_mode = blend_mode_to_float(entity.components.blend_mode.as_ref());

        // Pack uniforms to match composite.wgsl layout:
        // struct CompositeUniforms {
        //     transform: mat4x4f,      // 16 floats
        //     color: vec4f,            // 4 floats
        //     opacity: f32,            // 1 float
        //     use_texture: f32,        // 1 float
        //     blend_mode: f32,         // 1 float
        //     _pad: f32,              // 1 float (padding)
        // }
        let mut uniforms = Vec::with_capacity(24);
        uniforms.extend_from_slice(&entity.resolved.world_matrix.0); // 16
        uniforms.extend_from_slice(&color); // 4
        uniforms.push(entity.resolved.opacity); // 1
        uniforms.push(use_texture); // 1
        uniforms.push(blend_mode); // 1
        uniforms.push(0.0); // _pad

        commands.push(DrawCommand {
            pipeline: "composite".into(),
            uniforms,
            textures,
        });
    }

    commands
}
