//! Text rasterization — CPU-side glyph rendering to RGBA pixels.
//!
//! Supports multi-line text, word wrapping, alignment, and custom fonts.
//! Rasterizes via ab_glyph, produces RGBA texture data.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};

/// Text alignment options.
pub const ALIGN_LEFT: u32 = 0;
pub const ALIGN_CENTER: u32 = 1;
pub const ALIGN_RIGHT: u32 = 2;

/// Text rendering options.
pub struct TextOptions {
    pub font_size: f32,
    pub color: [f32; 4],
    /// Max width in pixels for word wrapping. None = no wrap.
    pub max_width: Option<f32>,
    /// Line height multiplier (1.0 = default).
    pub line_height: f32,
    /// Alignment: 0=left, 1=center, 2=right.
    pub alignment: u32,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            font_size: 24.0,
            color: [1.0, 1.0, 1.0, 1.0],
            max_width: None,
            line_height: 1.2,
            alignment: ALIGN_LEFT,
        }
    }
}

/// Rasterize text to RGBA pixels with full layout support.
///
/// Supports:
/// - Multi-line (explicit `\n`)
/// - Word wrapping (when `max_width` is set)
/// - Text alignment (left, center, right)
/// - Custom line height
///
/// Returns (pixels, width, height).
pub fn rasterize_text(
    text: &str,
    font_data: &[u8],
    opts: &TextOptions,
) -> Result<(Vec<u8>, u32, u32), String> {
    let font =
        FontRef::try_from_slice(font_data).map_err(|e| format!("Failed to load font: {}", e))?;

    let scale = PxScale::from(opts.font_size);
    let scaled_font = font.as_scaled(scale);

    let ascent = scaled_font.ascent();
    let descent = scaled_font.descent();
    let line_gap = scaled_font.line_gap();
    let default_line_h = ascent - descent + line_gap;
    let line_h = default_line_h * opts.line_height;

    // Split text into visual lines (handle \n + word wrap)
    let lines = layout_lines(text, &scaled_font, opts.max_width);

    if lines.is_empty() {
        return Ok((vec![0u8; 16], 1, 1));
    }

    // Measure max line width
    let line_widths: Vec<f32> = lines
        .iter()
        .map(|line| measure_line(line, &scaled_font))
        .collect();
    let max_line_w = line_widths.iter().cloned().fold(0.0_f32, f32::max);

    let canvas_w = if let Some(max_w) = opts.max_width {
        max_w.ceil() as u32 + 4
    } else {
        max_line_w.ceil() as u32 + 4
    };
    let canvas_h = (line_h * lines.len() as f32).ceil() as u32 + 4;

    if canvas_w == 0 || canvas_h == 0 {
        return Ok((vec![0u8; 16], 1, 1));
    }

    let mut pixels = vec![0u8; (canvas_w * canvas_h * 4) as usize];

    let r = (opts.color[0] * 255.0) as u8;
    let g = (opts.color[1] * 255.0) as u8;
    let b = (opts.color[2] * 255.0) as u8;

    for (line_idx, line) in lines.iter().enumerate() {
        let line_w = line_widths[line_idx];
        let available_w = canvas_w as f32 - 4.0;

        // Alignment offset
        let x_offset = match opts.alignment {
            ALIGN_CENTER => ((available_w - line_w) * 0.5).max(0.0),
            ALIGN_RIGHT => (available_w - line_w).max(0.0),
            _ => 0.0, // LEFT
        };

        let baseline_y = ascent + 2.0 + line_h * line_idx as f32;
        let mut cursor_x: f32 = 2.0 + x_offset;
        let mut prev_glyph: Option<ab_glyph::GlyphId> = None;

        for ch in line.chars() {
            let glyph_id = scaled_font.glyph_id(ch);

            if let Some(prev) = prev_glyph {
                cursor_x += scaled_font.kern(prev, glyph_id);
            }

            let glyph =
                glyph_id.with_scale_and_position(scale, ab_glyph::point(cursor_x, baseline_y));

            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, coverage| {
                    let px = (bounds.min.x as i32 + gx as i32) as u32;
                    let py = (bounds.min.y as i32 + gy as i32) as u32;
                    if px < canvas_w && py < canvas_h {
                        let idx = ((py * canvas_w + px) * 4) as usize;
                        let alpha = (coverage * opts.color[3] * 255.0) as u8;
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
    }

    Ok((pixels, canvas_w, canvas_h))
}

/// Split text into visual lines, handling `\n` and word wrap.
fn layout_lines<F: Font>(
    text: &str,
    scaled_font: &ab_glyph::PxScaleFont<&F>,
    max_width: Option<f32>,
) -> Vec<String> {
    let mut result = Vec::new();

    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            result.push(String::new());
            continue;
        }

        match max_width {
            Some(max_w) if max_w > 0.0 => {
                // Word wrap
                let words: Vec<&str> = paragraph.split_whitespace().collect();
                if words.is_empty() {
                    result.push(String::new());
                    continue;
                }

                let mut current_line = String::new();
                let space_w = measure_line(" ", scaled_font);

                for word in &words {
                    let word_w = measure_line(word, scaled_font);

                    if current_line.is_empty() {
                        // First word always fits
                        current_line = word.to_string();
                    } else {
                        let test_w = measure_line(&current_line, scaled_font) + space_w + word_w;
                        if test_w <= max_w {
                            current_line.push(' ');
                            current_line.push_str(word);
                        } else {
                            result.push(current_line);
                            current_line = word.to_string();
                        }
                    }
                }

                if !current_line.is_empty() {
                    result.push(current_line);
                }
            }
            _ => {
                // No wrap — single line per paragraph
                result.push(paragraph.to_string());
            }
        }
    }

    result
}

/// Measure the pixel width of a single line of text.
fn measure_line<F: Font>(text: &str, scaled_font: &ab_glyph::PxScaleFont<&F>) -> f32 {
    let mut width: f32 = 0.0;
    let mut prev_glyph: Option<ab_glyph::GlyphId> = None;

    for ch in text.chars() {
        let glyph_id = scaled_font.glyph_id(ch);
        if let Some(prev) = prev_glyph {
            width += scaled_font.kern(prev, glyph_id);
        }
        width += scaled_font.h_advance(glyph_id);
        prev_glyph = Some(glyph_id);
    }

    width
}

/// Default embedded font (Noto Sans Regular).
pub fn default_font_data() -> &'static [u8] {
    &[]
}
