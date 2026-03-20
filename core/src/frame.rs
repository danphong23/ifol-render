//! Data types for the Core render pipeline.
//!
//! These types define the API contract between Frontend and Core.
//! Frontend builds them. Core consumes them. Core never modifies them.
//!
//! All positions and sizes are in **pixels**, pre-computed by Frontend.

use serde::{Deserialize, Serialize};

// ══════════════════════════════════════
// Render Settings
// ══════════════════════════════════════

/// Output configuration for the render engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderSettings {
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Frames per second for playback/export.
    #[serde(default = "default_fps")]
    pub fps: f64,
    /// Background color (RGBA, 0..1). Default: transparent black.
    #[serde(default)]
    pub background: [f32; 4],
}

fn default_fps() -> f64 {
    30.0
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30.0,
            background: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

// ══════════════════════════════════════
// FlatEntity
// ══════════════════════════════════════

/// A single drawable element — fully resolved, pixel-based.
///
/// Frontend computes all positions, sizes, opacity, etc.
/// Core only reads this and packs it into GPU uniforms.
///
/// # Coordinate System
/// - Origin: top-left of output (0,0)
/// - X: increases right
/// - Y: increases down
/// - All units: **pixels**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatEntity {
    /// Unique ID for dirty tracking & caching.
    pub id: u64,

    // ── Spatial (pixels, top-left origin) ──
    /// Top-left X position in pixels.
    pub x: f32,
    /// Top-left Y position in pixels.
    pub y: f32,
    /// Rendered width in pixels.
    pub width: f32,
    /// Rendered height in pixels.
    pub height: f32,
    /// Rotation in radians (around entity center).
    #[serde(default)]
    pub rotation: f32,

    // ── Appearance ──
    /// Opacity: 0.0 (transparent) to 1.0 (opaque).
    #[serde(default = "one")]
    pub opacity: f32,
    /// Blend mode index. See [BlendMode] for values.
    #[serde(default)]
    pub blend_mode: u32,
    /// RGBA color tint (multiplied with texture). Default: white (no tint).
    #[serde(default = "white")]
    pub color: [f32; 4],

    // ── Rendering ──
    /// Registered shader/pipeline name (e.g. "composite").
    pub shader: String,
    /// Texture cache keys to bind.
    #[serde(default)]
    pub textures: Vec<String>,
    /// Extra shader uniform parameters.
    #[serde(default)]
    pub params: Vec<f32>,

    // ── Ordering ──
    /// Layer index for sorting (ascending: 0 = behind).
    #[serde(default)]
    pub layer: i32,
    /// Z-index within the same layer (ascending: 0 = behind).
    #[serde(default)]
    pub z_index: f32,
}

fn one() -> f32 {
    1.0
}

fn white() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

// ══════════════════════════════════════
// Frame & Render Passes
// ══════════════════════════════════════

/// Data for rendering a single frame.
///
/// Contains an ordered list of render passes and
/// texture updates to process before rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    /// Ordered render passes. Executed sequentially.
    pub passes: Vec<RenderPass>,
    /// Texture updates for this frame (load, upload, rasterize, evict).
    #[serde(default)]
    pub texture_updates: Vec<TextureUpdate>,
}

impl Frame {
    /// Scale all entity coordinates by the given factors.
    ///
    /// Use when the render resolution differs from the scene's authored resolution.
    /// For example, if scene is authored at 1920×1080 but previewing at 640×360:
    /// ```rust,ignore
    /// let scaled = frame.scaled(640.0 / 1920.0, 360.0 / 1080.0);
    /// engine.render_frame(&scaled);
    /// ```
    pub fn scaled(&self, sx: f64, sy: f64) -> Frame {
        let sx = sx as f32;
        let sy = sy as f32;
        Frame {
            passes: self
                .passes
                .iter()
                .map(|pass| RenderPass {
                    output: pass.output.clone(),
                    pass_type: match &pass.pass_type {
                        PassType::Entities {
                            clear_color,
                            entities,
                        } => PassType::Entities {
                            clear_color: *clear_color,
                            entities: entities
                                .iter()
                                .map(|e| FlatEntity {
                                    x: e.x * sx,
                                    y: e.y * sy,
                                    width: e.width * sx,
                                    height: e.height * sy,
                                    ..e.clone()
                                })
                                .collect(),
                        },
                        other => other.clone(),
                    },
                })
                .collect(),
            texture_updates: self.texture_updates.clone(),
        }
    }
}

/// A single render pass — produces an output texture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPass {
    /// Key for the output texture of this pass.
    /// Can be referenced as input by later passes.
    pub output: String,
    /// What this pass does.
    pub pass_type: PassType,
}

/// What a render pass does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PassType {
    /// Render a list of entities to a texture.
    Entities {
        entities: Vec<FlatEntity>,
        /// Background color for this pass (RGBA).
        #[serde(default)]
        clear_color: [f32; 4],
    },
    /// Apply a fullscreen shader effect on input texture(s).
    Effect {
        /// Shader/pipeline name.
        shader: String,
        /// Input texture keys from previous passes.
        inputs: Vec<String>,
        /// Shader uniform parameters.
        #[serde(default)]
        params: Vec<f32>,
    },
    /// Mark a texture as the final output (read back to CPU).
    Output {
        /// Input texture key to read as final pixels.
        input: String,
    },
}

// ══════════════════════════════════════
// Texture Updates
// ══════════════════════════════════════

/// Instructions for Core to load/update textures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextureUpdate {
    /// Load image from file path. Cached — skips if key already exists.
    LoadImage { key: String, path: String },
    /// Upload raw RGBA pixels directly (video frames, procedural content).
    /// Always replaces existing texture with same key.
    UploadRgba {
        key: String,
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// Load a font file into the font cache. Cached by key.
    LoadFont { key: String, path: String },
    /// Rasterize text to a texture. Core handles font rendering.
    RasterizeText {
        key: String,
        content: String,
        font_size: f32,
        color: [f32; 4],
        /// Font cache key (from LoadFont). None = built-in default.
        #[serde(default)]
        font_key: Option<String>,
        /// Max width in pixels for word wrapping. None = no wrap.
        #[serde(default)]
        max_width: Option<f32>,
        /// Line height multiplier (1.0 = default spacing).
        #[serde(default)]
        line_height: Option<f32>,
        /// Text alignment: 0 = left (default), 1 = center, 2 = right.
        #[serde(default)]
        alignment: u32,
    },
    /// Decode a single video frame to texture via FFmpeg.
    DecodeVideoFrame {
        /// Texture cache key for the decoded frame.
        key: String,
        /// Path to video file.
        path: String,
        /// Timestamp in seconds to extract.
        timestamp_secs: f64,
        /// Optional output width (None = native video width).
        #[serde(default)]
        width: Option<u32>,
        /// Optional output height (None = native video height).
        #[serde(default)]
        height: Option<u32>,
    },
    /// Remove a texture from cache.
    Evict { key: String },
}

/// Text alignment constants.
pub mod text_align {
    pub const LEFT: u32 = 0;
    pub const CENTER: u32 = 1;
    pub const RIGHT: u32 = 2;
}

// ══════════════════════════════════════
// Blend Mode Constants
// ══════════════════════════════════════

/// Blend mode values for `FlatEntity.blend_mode`.
///
/// These map 1:1 to the composite shader's blend logic.
pub mod blend {
    pub const NORMAL: u32 = 0;
    pub const MULTIPLY: u32 = 1;
    pub const SCREEN: u32 = 2;
    pub const OVERLAY: u32 = 3;
    pub const SOFT_LIGHT: u32 = 4;
    pub const ADD: u32 = 5;
    pub const DIFFERENCE: u32 = 6;
}
