//! DrawCommand builder — converts resolved ECS entities into GPU draw commands.
//!
//! ## Coordinate System
//!
//! Core uses a **unit-based** coordinate system:
//! - `PPU` (Pixels Per Unit): conversion factor (default 100)
//! - Entity position/size are in **units**
//! - Images/videos auto-size based on pixel dimensions / PPU
//! - Resolution changes don't affect entity sizes
//!
//! ## Conversion: Unit → Clip Space
//!
//! GPU uses clip space (-1..1). The conversion is:
//! ```text
//! scene_width_units  = settings.width  / settings.ppu
//! scene_height_units = settings.height / settings.ppu
//! clip_x = (unit_x / scene_width_units)  * 2.0 - 1.0
//! clip_y = (unit_y / scene_height_units) * 2.0 - 1.0
//! ```
//!
//! Core OWNS the composite shader and packs uniforms to match its layout.

use crate::ecs::World;
use crate::scene::RenderSettings;
use crate::types::Mat4;
use ifol_render::{DrawCommand, Renderer};

// Composite shader uniform layout:
// [transform: f32x16, color: f32x4, opacity: f32, use_texture: f32, blend_mode: f32, _pad: f32]
// = 24 floats total

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

/// Convert unit-space transform matrix to clip-space transform matrix.
///
/// The entity's world_matrix is in unit space. We need to convert to
/// clip space (-1..1) for the GPU.
fn unit_to_clip_matrix(
    world_matrix: &Mat4,
    content_size: Option<[f32; 2]>,
    scene_w_units: f32,
    scene_h_units: f32,
) -> Mat4 {
    // Content size determines the base scale of the quad.
    // Without content_size (e.g. fullscreen color fill), use scene size.
    let (cw, ch) = match content_size {
        Some([w, h]) => (w, h),
        None => (scene_w_units, scene_h_units),
    };

    // Scale: unit size → clip space size
    // 1 unit in X = 2.0 / scene_w_units in clip space
    let sx = cw / scene_w_units;
    let sy = ch / scene_h_units;

    // Build scale matrix for content size
    let content_scale = Mat4([
        sx, 0.0, 0.0, 0.0, 0.0, sy, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);

    // The world_matrix contains position (in units), scale, rotation from the entity.
    // We need to convert position from units to clip space.
    let m = &world_matrix.0;
    let pos_x = m[12]; // translation X in units
    let pos_y = m[13]; // translation Y in units

    // Convert position to clip space: unit → normalized (0..1) → clip (-1..1)
    let clip_x = (pos_x / scene_w_units) * 2.0 - 1.0;
    let clip_y = 1.0 - (pos_y / scene_h_units) * 2.0; // Y flipped for GPU

    // Build the final matrix: content_scale * entity_scale_rotation + clip_position
    // Extract scale+rotation from world_matrix (top-left 2x2)
    let transform = Mat4([
        m[0] * sx / scene_w_units * 2.0,
        m[1] * sy / scene_h_units * 2.0,
        0.0,
        0.0,
        m[4] * sx / scene_w_units * 2.0,
        m[5] * sy / scene_h_units * 2.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        clip_x + sx * (m[0] + m[4]) / scene_w_units, // center offset
        clip_y - sy * (m[1] + m[5]) / scene_h_units,
        0.0,
        1.0,
    ]);

    // For simplicity and correctness, use a simpler approach:
    // Build the full unit→clip conversion
    let _ = transform;
    let _ = content_scale;

    // --- Simplified correct approach ---
    // The quad is drawn from (-1,-1) to (1,1) in clip space.
    // We need to transform it to the correct position and size.
    //
    // Steps:
    // 1. Scale quad by content_size / 2 (half-size because quad is -1..1)
    // 2. Apply entity transform (scale, rotation)
    // 3. Convert to clip space

    // Content half-size in units
    let hx = cw * 0.5;
    let hy = ch * 0.5;

    // Unit-to-clip conversion factors
    let ux = 2.0 / scene_w_units;
    let uy = -2.0 / scene_h_units; // Y flipped

    // Extract entity scale+rotation (columns 0,1 of world_matrix)
    let m00 = m[0]; // scale_x * cos
    let m01 = m[1]; // scale_x * sin
    let m10 = m[4]; // -scale_y * sin
    let m11 = m[5]; // scale_y * cos

    // Final matrix: combines content size + entity transform + unit-to-clip
    Mat4([
        hx * m00 * ux,
        hx * m01 * uy,
        0.0,
        0.0,
        hy * m10 * ux,
        hy * m11 * uy,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        pos_x * ux - 1.0 + hx * (m00 + m10) * ux, // X: unit→clip + center offset
        pos_y * uy + 1.0 + hy * (m01 + m11) * uy, // Y: unit→clip + center offset (flipped)
        0.0,
        1.0,
    ])
}

/// Compute content size in units from pixel dimensions and PPU.
pub fn pixels_to_units(pixel_w: u32, pixel_h: u32, ppu: f32) -> [f32; 2] {
    [pixel_w as f32 / ppu, pixel_h as f32 / ppu]
}

/// Load image resources for entities that need textures.
///
/// Must be called before `build_draw_commands` so textures are available.
pub fn load_resources(world: &mut World, settings: &RenderSettings, renderer: &mut Renderer) {
    let ppu = settings.ppu;

    for entity in &mut world.entities {
        if !entity.resolved.visible {
            continue;
        }

        // Image source: load from file, compute unit size
        if let Some(ref mut img) = entity.components.image_source {
            let tex_key = format!("img:{}", entity.id);
            if !renderer.has_texture(&tex_key)
                && let Err(e) = renderer.load_image(&tex_key, &img.path)
            {
                log::warn!("Failed to load image '{}': {}", img.path, e);
                continue;
            }
            // Compute content size from pixel dimensions
            if let Some([pw, ph]) = img.pixel_size {
                entity.resolved.content_size = Some(pixels_to_units(pw, ph, ppu));
            }
        }

        // Text source: rasterize to texture, compute unit size
        if let Some(ref mut text_src) = entity.components.text_source {
            let tex_key = format!("txt:{}", entity.id);
            let font_data = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
            match crate::text::rasterize_text(
                &text_src.content,
                font_data,
                text_src.font_size,
                [
                    text_src.color.r,
                    text_src.color.g,
                    text_src.color.b,
                    text_src.color.a,
                ],
            ) {
                Ok((pixels, tw, th)) => {
                    renderer.load_rgba(&tex_key, &pixels, tw, th);
                    text_src.pixel_size = Some([tw, th]);
                    entity.resolved.content_size = Some(pixels_to_units(tw, th, ppu));
                }
                Err(e) => {
                    log::warn!("Failed to rasterize text: {}", e);
                }
            }
        }

        // Video source: placeholder — actual frame decode will be added later.
        // For now, check if a texture is already loaded by external code.
        if let Some(ref img) = entity.components.video_source {
            let tex_key = format!("vid:{}", entity.id);
            if renderer.has_texture(&tex_key)
                && let Some([pw, ph]) = img.pixel_size
            {
                entity.resolved.content_size = Some(pixels_to_units(pw, ph, ppu));
            }
        }

        // Color source: use explicit size or fullscreen
        if let Some(ref color_src) = entity.components.color_source {
            entity.resolved.content_size = color_src.size.map(|s| [s.x, s.y]);
        }
    }
}

/// Build GPU draw commands from the resolved ECS world.
///
/// This runs AFTER all ECS systems have resolved visibility, animation,
/// and transforms, and AFTER `load_resources` has loaded textures.
///
/// Converts entity state into generic `DrawCommand` structs
/// that the passive render tool can consume.
pub fn build_draw_commands(
    world: &World,
    settings: &RenderSettings,
    camera_matrix: &Mat4,
) -> Vec<DrawCommand> {
    let sorted = world.sorted_by_layer();
    let mut commands = Vec::with_capacity(sorted.len());

    let ppu = settings.ppu;
    let scene_w_units = settings.width as f32 / ppu;
    let scene_h_units = settings.height as f32 / ppu;

    for entity in sorted {
        // Audio entities are not drawn
        if entity.components.audio_source.is_some() {
            continue;
        }

        // Determine source type and build command
        let (color, use_texture, textures) =
            if let Some(ref color_src) = entity.components.color_source {
                // Solid color fill
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
                // Image: texture mapped quad
                (
                    [1.0f32, 1.0, 1.0, 1.0],
                    1.0f32,
                    vec![format!("img:{}", entity.id)],
                )
            } else if entity.components.text_source.is_some() {
                // Text: rasterized to texture
                (
                    [1.0f32, 1.0, 1.0, 1.0],
                    1.0f32,
                    vec![format!("txt:{}", entity.id)],
                )
            } else if entity.components.video_source.is_some() {
                // Video: frame loaded as texture
                (
                    [1.0f32, 1.0, 1.0, 1.0],
                    1.0f32,
                    vec![format!("vid:{}", entity.id)],
                )
            } else {
                continue;
            };

        let blend_mode = blend_mode_to_float(entity.components.blend_mode.as_ref());

        // Apply camera transform: camera_matrix * entity_world_matrix
        let world_with_camera = camera_matrix.mul(&entity.resolved.world_matrix);

        // Convert unit-space to clip-space
        let clip_matrix = unit_to_clip_matrix(
            &world_with_camera,
            entity.resolved.content_size,
            scene_w_units,
            scene_h_units,
        );

        // Resolve opacity
        let opacity = entity.components.opacity.unwrap_or(1.0) * entity.resolved.opacity;

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
        uniforms.extend_from_slice(&clip_matrix.0); // 16
        uniforms.extend_from_slice(&color); // 4
        uniforms.push(opacity); // 1
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
