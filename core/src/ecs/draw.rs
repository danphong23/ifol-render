//! DrawCommand builder — converts resolved ECS entities into GPU draw commands.
//!
//! This is where the unit system (0..1 normalized) gets converted to
//! clip-space coordinates for the GPU.

use crate::ecs::World;
use crate::scene::RenderSettings;
use ifol_render::{DrawCommand, DrawSource};

/// Map core BlendMode to render BlendMode.
fn map_blend_mode(mode: Option<&crate::ecs::components::BlendMode>) -> ifol_render::BlendMode {
    match mode {
        Some(crate::ecs::components::BlendMode::Multiply) => ifol_render::BlendMode::Multiply,
        Some(crate::ecs::components::BlendMode::Screen) => ifol_render::BlendMode::Screen,
        Some(crate::ecs::components::BlendMode::Overlay) => ifol_render::BlendMode::Overlay,
        Some(crate::ecs::components::BlendMode::SoftLight) => ifol_render::BlendMode::SoftLight,
        Some(crate::ecs::components::BlendMode::Add) => ifol_render::BlendMode::Add,
        Some(crate::ecs::components::BlendMode::Difference) => ifol_render::BlendMode::Difference,
        _ => ifol_render::BlendMode::Normal,
    }
}

/// Build GPU draw commands from the resolved ECS world.
///
/// This runs AFTER all ECS systems have resolved visibility, animation,
/// and transforms. It converts entity state into `DrawCommand` structs
/// that the passive render tool can consume.
pub fn build_draw_commands(world: &World, _settings: &RenderSettings) -> Vec<DrawCommand> {
    let sorted = world.sorted_by_layer();
    let mut commands = Vec::with_capacity(sorted.len());

    for entity in sorted {
        // Determine draw source
        let source = if let Some(ref color_src) = entity.components.color_source {
            DrawSource::Color([
                color_src.color.r,
                color_src.color.g,
                color_src.color.b,
                color_src.color.a,
            ])
        } else if entity.components.image_source.is_some() {
            DrawSource::Texture(entity.id.clone())
        } else {
            // No visual source — skip
            continue;
        };

        commands.push(DrawCommand {
            transform: entity.resolved.world_matrix.0,
            opacity: entity.resolved.opacity,
            source,
            blend_mode: map_blend_mode(entity.components.blend_mode.as_ref()),
        });
    }

    commands
}
