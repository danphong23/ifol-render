//! Text rasterization — CPU-side glyph rendering to RGBA pixels.
//!
//! Core owns text rendering: rasterizes glyphs via ab_glyph,
//! produces RGBA texture data, then uses render's composite pipeline
//! to draw it. Render sees text as just another texture.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

/// Rasterize a text string to RGBA pixels.
///
/// Returns (pixels, width, height).
pub fn rasterize_text(
    text: &str,
    font_data: &[u8],
    font_size: f32,
    color: [f32; 4],
) -> Result<(Vec<u8>, u32, u32), String> {
    let font =
        FontRef::try_from_slice(font_data).map_err(|e| format!("Failed to load font: {}", e))?;

    let scale = PxScale::from(font_size);
    let scaled_font = font.as_scaled(scale);

    // Measure text dimensions
    let mut total_width: f32 = 0.0;
    let mut max_ascent: f32 = 0.0;
    let mut max_descent: f32 = 0.0;

    let ascent = scaled_font.ascent();
    let descent = scaled_font.descent();
    max_ascent = max_ascent.max(ascent);
    max_descent = max_descent.min(descent);

    let mut prev_glyph: Option<ab_glyph::GlyphId> = None;
    for ch in text.chars() {
        let glyph_id = scaled_font.glyph_id(ch);
        if let Some(prev) = prev_glyph {
            total_width += scaled_font.kern(prev, glyph_id);
        }
        total_width += scaled_font.h_advance(glyph_id);
        prev_glyph = Some(glyph_id);
    }

    let width = total_width.ceil() as u32 + 4; // 2px padding each side
    let height = (max_ascent - max_descent).ceil() as u32 + 4;

    if width == 0 || height == 0 {
        return Ok((vec![0u8; 16], 1, 1));
    }

    // Rasterize
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let baseline_y = max_ascent + 2.0; // 2px top padding

    let mut cursor_x: f32 = 2.0; // 2px left padding
    let mut prev_glyph: Option<ab_glyph::GlyphId> = None;

    let r = (color[0] * 255.0) as u8;
    let g = (color[1] * 255.0) as u8;
    let b = (color[2] * 255.0) as u8;

    for ch in text.chars() {
        let glyph_id = scaled_font.glyph_id(ch);

        if let Some(prev) = prev_glyph {
            cursor_x += scaled_font.kern(prev, glyph_id);
        }

        let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(cursor_x, baseline_y));

        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                let px = (bounds.min.x as i32 + gx as i32) as u32;
                let py = (bounds.min.y as i32 + gy as i32) as u32;
                if px < width && py < height {
                    let idx = ((py * width + px) * 4) as usize;
                    let alpha = (coverage * color[3] * 255.0) as u8;
                    // Alpha blend
                    let existing_a = pixels[idx + 3];
                    if alpha > existing_a {
                        pixels[idx] = r;
                        pixels[idx + 1] = g;
                        pixels[idx + 2] = b;
                        pixels[idx + 3] = alpha;
                    }
                }
            });
        }

        cursor_x += scaled_font.h_advance(glyph_id);
        prev_glyph = Some(glyph_id);
    }

    Ok((pixels, width, height))
}

/// Default embedded font (DejaVu Sans Mono subset).
/// This is a fallback; callers should provide their own fonts.
pub fn default_font_data() -> &'static [u8] {
    // Use the built-in font from ab_glyph's test data or embed a minimal one.
    // For now, we use a public domain font embedded at compile time.
    // Users should provide their own font via the API.
    include_bytes!("../../assets/fonts/NotoSans-Regular.ttf")
}
