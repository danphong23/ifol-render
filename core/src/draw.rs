//! Pixel→clip conversion and uniform packing.
//!
//! Converts FlatEntity (pixel coordinates) to GPU DrawCommands.
//! Core's ONLY computation: pixel→clip + pack uniforms.

use crate::frame::FlatEntity;
use ifol_render::DrawCommand;

/// Composite shader uniform layout (matches composite.wgsl):
///
/// ```text
/// struct CompositeUniforms {
///     transform: mat4x4f,    // 16 floats — clip-space transform
///     color: vec4f,          //  4 floats — RGBA tint
///     opacity: f32,          //  1 float
///     use_texture: f32,      //  1 float  — 0=color only, 1=textured
///     blend_mode: f32,       //  1 float  — see blend constants
///     _pad: f32,             //  1 float  — padding to 16-byte alignment
/// }
/// ```
const COMPOSITE_UNIFORM_FLOATS: usize = 24;

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
    // BOTH positive — py handles Y inversion, so column vectors
    // must NOT also invert Y or the image flips upside down.
    let csx = 2.0 / out_w;
    let csy = 2.0 / out_h;

    if entity.rotation.abs() < 1e-6 {
        // No rotation — axis-aligned scaling
        [
            half_w * csx, 0.0,          0.0, 0.0, // column 0
            0.0,          half_h * csy, 0.0, 0.0, // column 1
            0.0,          0.0,          1.0, 0.0, // column 2
            px,           py,           0.0, 1.0, // column 3
        ]
    } else {
        let cos = entity.rotation.cos();
        let sin = entity.rotation.sin();

        // Decomposition: Translate(px,py) × ClipScale × Rotate(θ) × PixelSize
        //
        // The vertex y-axis (up) is opposite to pixel y-axis (down).
        // Rotation is in PIXEL space (uniform, no aspect distortion).
        // The mapping from vertex → pixel → rotated pixel → clip gives:
        //
        //   col0 = [halfW·cos·csx,  -halfW·sin·csy,  0, 0]
        //   col1 = [halfH·sin·csx,   halfH·cos·csy,  0, 0]
        [
            half_w *  cos * csx,   -half_w * sin * csy,    0.0, 0.0, // column 0
            half_h *  sin * csx,    half_h * cos * csy,    0.0, 0.0, // column 1
            0.0,                    0.0,                   1.0, 0.0, // column 2
            px,                     py,                    0.0, 1.0, // column 3
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

/// Build DrawCommands from sorted FlatEntities.
///
/// Each entity becomes one DrawCommand for the composite shader.
/// Output width/height needed for pixel→clip conversion.
pub fn build_draw_commands(entities: &[FlatEntity], out_w: u32, out_h: u32) -> Vec<DrawCommand> {
    let w = out_w as f32;
    let h = out_h as f32;
    let mut commands = Vec::with_capacity(entities.len());

    for entity in entities {
        let transform = pixel_to_clip_matrix(entity, w, h);
        let use_texture = if entity.textures.is_empty() { 0.0 } else { 1.0 };

        let mut uniforms = Vec::with_capacity(COMPOSITE_UNIFORM_FLOATS);
        uniforms.extend_from_slice(&transform); // 16
        uniforms.extend_from_slice(&entity.color); // 4
        uniforms.push(entity.opacity); // 1
        uniforms.push(use_texture); // 1
        uniforms.push(entity.blend_mode as f32); // 1
        uniforms.push(0.0); // _pad

        commands.push(DrawCommand {
            pipeline: entity.shader.clone(),
            uniforms,
            textures: entity.textures.clone(),
        });
    }

    commands
}
