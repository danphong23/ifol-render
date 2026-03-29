//! Pixel→clip conversion and uniform packing.
//!
//! Converts FlatEntity (pixel coordinates) to GPU DrawCommands.
//! Core's ONLY computation: pixel→clip + pack uniforms.

use crate::frame::FlatEntity;
use ifol_render::DrawCommand;
use std::collections::HashMap;

/// Composite shader uniform layout (matches composite.wgsl):
///
/// ```text
/// struct CompositeUniforms {
///     transform: mat4x4f,    // 16 floats — clip-space transform
///     color: vec4f,          //  4 floats — RGBA tint
///     opacity: f32,          //  1 float
///     use_texture: f32,      //  1 float  — 0=color only, 1=textured
///     blend_mode: f32,       //  1 float  — see blend constants
///     fit_mode: f32,         //  1 float  — 0=Stretch, 1=Contain, 2=Cover
///     uv_offset: vec2f,      //  2 floats — UV offset for fit mode
///     uv_scale: vec2f,       //  2 floats — UV scale for fit mode
/// }
/// ```


/// Convert a FlatEntity to a clip-space transform matrix.
///
/// The GPU quad spans (-1,-1) to (1,1) in clip space.
/// We build a matrix that positions and sizes the quad correctly.
///
/// # Coordinate mapping
/// ```text
/// pixel (x, y)  →  clip_x = x / width * 2 - 1
///                   clip_y = 1 - y / height * 2  (Y flipped)
/// ```
fn pixel_to_clip_matrix(entity: &FlatEntity, out_w: f32, out_h: f32) -> [f32; 16] {
    let half_w = entity.width * 0.5;
    let half_h = entity.height * 0.5;

    // Entity center in pixels
    let cx = entity.x + half_w;
    let cy = entity.y + half_h;

    // Center position in clip space (py already inverts Y)
    let px = cx / out_w * 2.0 - 1.0;
    let py = 1.0 - cy / out_h * 2.0;

    // Pixel-to-clip scale factors.
    let csx = 2.0 / out_w;
    let csy = 2.0 / out_h;

    if entity.rotation.abs() < 1e-6 {
        // No rotation — axis-aligned scaling
        [
            half_w * csx, 0.0, 0.0, 0.0,
            0.0, half_h * csy, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            px, py, 0.0, 1.0,
        ]
    } else {
        let cos = entity.rotation.cos();
        let sin = entity.rotation.sin();
        [
            half_w * cos * csx, -half_w * sin * csy, 0.0, 0.0,
            half_h * sin * csx,  half_h * cos * csy, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            px, py, 0.0, 1.0,
        ]
    }
}

/// Sort entities by (layer ascending, z_index ascending).
pub fn sort_entities(entities: &mut [FlatEntity]) {
    entities.sort_by(|a, b| {
        a.layer.cmp(&b.layer).then(
            a.z_index
                .partial_cmp(&b.z_index)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
}

/// Compute UV offset and scale for fit mode.
///
/// Given entity dimensions and texture dimensions, returns (offset_x, offset_y, scale_x, scale_y)
/// for contain/cover modes. Stretch mode returns (0, 0, 1, 1).
fn compute_uv_rect(fit_mode: u32, entity_w: f32, entity_h: f32, tex_w: f32, tex_h: f32) -> [f32; 4] {
    if fit_mode == 0 || tex_w <= 0.0 || tex_h <= 0.0 {
        // Stretch: UV covers entire quad
        return [0.0, 0.0, 1.0, 1.0];
    }

    let entity_aspect = entity_w / entity_h;
    let tex_aspect = tex_w / tex_h;

    if fit_mode == 1 {
        // Contain: scale uniformly to fit inside, letterbox
        // In UV coordinates, scale > 1.0 pushes edges outside [0..1], 
        // triggering ClampToEdge transparent padding.
        if tex_aspect > entity_aspect {
            // Texture is wider → fit width exactly, pad height (letterbox)
            let scale_y = tex_aspect / entity_aspect; // > 1.0
            let offset_y = (1.0 - scale_y) * 0.5; // negative offset
            [0.0, offset_y, 1.0, scale_y]
        } else {
            // Texture is taller → fit height exactly, pad width (pillarbox)
            let scale_x = entity_aspect / tex_aspect; // > 1.0
            let offset_x = (1.0 - scale_x) * 0.5; // negative offset
            [offset_x, 0.0, scale_x, 1.0]
        }
    } else {
        // Cover: scale uniformly to fill, crop excess
        // In UV coordinates, scale < 1.0 squishes UV sampling to interior, cropping edges.
        if tex_aspect > entity_aspect {
            // Texture is wider → fill height exactly, crop width
            let uv_scale_x = entity_aspect / tex_aspect; // < 1.0
            let uv_offset_x = (1.0 - uv_scale_x) * 0.5; // positive offset
            [uv_offset_x, 0.0, uv_scale_x, 1.0]
        } else {
            // Texture is taller → fill width exactly, crop height
            let uv_scale_y = tex_aspect / entity_aspect; // < 1.0
            let uv_offset_y = (1.0 - uv_scale_y) * 0.5; // positive offset
            [0.0, uv_offset_y, 1.0, uv_scale_y]
        }
    }
}

/// Build DrawCommands from sorted FlatEntities.
///
/// Each entity becomes one DrawCommand for the composite shader.
/// `tex_dims` provides (width, height) for texture keys (for fit_mode calculation).
pub fn build_draw_commands(
    entities: &[FlatEntity],
    out_w: u32,
    out_h: u32,
    tex_dims: &HashMap<String, (u32, u32)>,
) -> Vec<DrawCommand> {
    let w = out_w as f32;
    let h = out_h as f32;
    let mut commands = Vec::with_capacity(entities.len());

    for entity in entities {
        let transform = pixel_to_clip_matrix(entity, w, h);
        let use_texture = if entity.textures.is_empty() { 0.0 } else { 1.0 };

        // Compute UV rect for fit mode
        let uv_rect = if entity.fit_mode != 0 && !entity.textures.is_empty() {
            if let Some(&(tw, th)) = tex_dims.get(&entity.textures[0]) {
                compute_uv_rect(entity.fit_mode, entity.width, entity.height, tw as f32, th as f32)
            } else {
                [0.0, 0.0, 1.0, 1.0]
            }
        } else {
            [0.0, 0.0, 1.0, 1.0]
        };

        let mut uniforms = Vec::with_capacity(32);
        uniforms.extend_from_slice(&transform);      // 16
        uniforms.extend_from_slice(&entity.color);    // 4
        uniforms.push(entity.opacity);                // 1
        
        if !entity.params.is_empty() {
            // Specialized shape/effect shaders with custom uniform layout
            uniforms.extend_from_slice(&entity.params);
        } else {
            // Builtin composite shader layout
            uniforms.push(use_texture);                   // 1
            uniforms.push(entity.blend_mode as f32);      // 1
            uniforms.push(entity.fit_mode as f32);        // 1
            uniforms.push(uv_rect[0]); uniforms.push(uv_rect[1]); // uv_offset: 2
            uniforms.push(uv_rect[2]); uniforms.push(uv_rect[3]); // uv_scale: 2
        }

        commands.push(DrawCommand {
            pipeline: entity.shader.clone(),
            uniforms,
            textures: entity.textures.clone(),
        });
    }

    commands
}
