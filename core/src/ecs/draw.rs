//! DrawCommand builder — converts resolved ECS entities into GPU draw commands.
//!
//! This is where the unit system (0..1 normalized) gets converted to
//! clip-space coordinates for the GPU.

use crate::ecs::World;
use crate::scene::RenderSettings;
use ifol_render::{DrawCommand, DrawSource};

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
            blend_mode: Default::default(),
        });
    }

    commands
}
