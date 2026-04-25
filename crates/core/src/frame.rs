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
    #[serde(default)]
    pub background: [f32; 4],
    /// Whether HDR (Rgba16Float) rendering is enabled for the pipeline
    #[serde(default)]
    pub hdr_enabled: bool,
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
            hdr_enabled: false,
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
    /// Content hash representing all visual data in this entity (layout, color, texture keys, etc).
    /// Used by the RenderGraph to detect if an intermediate pass has changed.
    #[serde(default)]
    pub content_hash: u64,

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
    /// Fit mode: 0=Stretch, 1=Contain, 2=Cover.
    #[serde(default)]
    pub fit_mode: u32,
    /// UV offset for fit mode crop (computed by compiler from intrinsic aspect ratio).
    #[serde(default)]
    pub uv_offset: [f32; 2],
    /// UV scale for fit mode crop (default: [1.0, 1.0] = full texture).
    #[serde(default = "uv_scale_default")]
    pub uv_scale: [f32; 2],
    /// Native media width (for fit_mode calculations). 0 = unknown.
    #[serde(default)]
    pub intrinsic_width: f32,
    /// Native media height (for fit_mode calculations). 0 = unknown.
    #[serde(default)]
    pub intrinsic_height: f32,
}

impl FlatEntity {
    /// Computes a deterministic hash of all visual properties (layout, color, texture keys, etc).
    /// Used by the RenderGraph to detect unchanged entity content.
    pub fn calculate_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        
        hasher.write_u64(self.id);
        hasher.write_u32(self.x.to_bits());
        hasher.write_u32(self.y.to_bits());
        hasher.write_u32(self.width.to_bits());
        hasher.write_u32(self.height.to_bits());
        hasher.write_u32(self.rotation.to_bits());
        hasher.write_u32(self.opacity.to_bits());
        hasher.write_u32(self.blend_mode);
        for c in &self.color { hasher.write_u32(c.to_bits()); }
        
        self.shader.hash(&mut hasher);
        for t in &self.textures { t.hash(&mut hasher); }
        for p in &self.params { hasher.write_u32(p.to_bits()); }
        
        hasher.write_i32(self.layer);
        hasher.write_u32(self.z_index.to_bits());
        hasher.write_u32(self.fit_mode);
        
        for u in &self.uv_offset { hasher.write_u32(u.to_bits()); }
        for u in &self.uv_scale { hasher.write_u32(u.to_bits()); }
        hasher.write_u32(self.intrinsic_width.to_bits());
        hasher.write_u32(self.intrinsic_height.to_bits());
        
        hasher.finish()
    }
}

fn uv_scale_default() -> [f32; 2] {
    [1.0, 1.0]
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
    /// Frame-by-frame instantaneous audio playback queries generated by `audio_sys`.
    #[serde(default)]
    pub audio_calls: Vec<AudioCall>,
}

/// Instantaneous request to mix audio samples for the current frame step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCall {
    /// Asset URL (absolute path or resolved asset ID).
    pub url: String,
    /// The exact playhead position of the audio track right now.
    pub timestamp_secs: f64,
    /// Instantaneous volume (0.0 to 1.0+) incorporating fades.
    pub volume: f32,
    /// Instantaneous playback speed (1.0 = normal, 2.0 = double config).
    pub speed: f32,
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
                .map(|pass| RenderPass { pass_hash: 0,
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
                    target_width: pass.target_width,
                    target_height: pass.target_height,
                })
                .collect(),
            texture_updates: self.texture_updates.clone(),
            audio_calls: self.audio_calls.clone(),
        }
    }
}

/// A single render pass — produces an output texture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPass {
    /// Key for the output texture of this pass.
    /// Can be referenced as input by later passes.
    pub output: String,
    /// Hash representing the complete state of this pass (all inputs + effect params).
    /// Used by the RenderGraph to skip identical passes.
    #[serde(default)]
    pub pass_hash: u64,
    /// What this pass does.
    pub pass_type: PassType,
    /// Optional isolated target width (for grouped relative pre-comps)
    #[serde(default)]
    pub target_width: Option<u32>,
    /// Optional isolated target height (for grouped relative pre-comps)
    #[serde(default)]
    pub target_height: Option<u32>,
}

impl RenderPass {
    /// Computes a deterministic hash of this pass and all its inputs.
    /// Used by the RenderGraph to detect unchanged passes and skip GPU work.
    pub fn calculate_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        self.output.hash(&mut hasher);
        
        if let Some(w) = self.target_width { hasher.write_u32(w); }
        if let Some(h) = self.target_height { hasher.write_u32(h); }

        match &self.pass_type {
            PassType::Entities { entities, clear_color } => {
                hasher.write_u8(0);
                if let Some(c) = clear_color {
                    hasher.write_u8(1);
                    for ch in c { hasher.write_u32(ch.to_bits()); }
                } else {
                    hasher.write_u8(0);
                }
                for e in entities { hasher.write_u64(e.content_hash); }
            }
            PassType::Effect { shader, inputs, params } => {
                hasher.write_u8(1);
                shader.hash(&mut hasher);
                for i in inputs { i.hash(&mut hasher); }
                for p in params { hasher.write_u32(p.to_bits()); }
            }
            PassType::Snapshot { source_key } => {
                hasher.write_u8(2);
                source_key.hash(&mut hasher);
            }
            PassType::Output { input, entities } => {
                hasher.write_u8(3);
                input.hash(&mut hasher);
                for e in entities { hasher.write_u64(e.content_hash); }
            }
        }
        
        hasher.finish()
    }
}


/// What a render pass does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PassType {
    /// Render a list of entities to a texture.
    Entities {
        entities: Vec<FlatEntity>,
        /// Background color for this pass (RGBA).
        #[serde(default)]
        clear_color: Option<[f32; 4]>,
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
    /// Copy an existing texture to a new key without rendering.
    ///
    /// Used by the blend mode 2-pass system to snapshot the current
    /// accumulation buffer BEFORE a non-Normal blend entity is composited.
    /// The snapshot becomes the `dst` input for `blend_composite`.
    Snapshot {
        /// Key of the texture to copy FROM.
        source_key: String,
    },
    /// Mark a texture as the final output (read back to CPU).
    Output {
        /// Input texture key to read as final pixels.
        input: String,
        /// Overlay entities to draw on top of the final output (useful for editor UI).
        #[serde(default)]
        entities: Vec<FlatEntity>,
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
/// These map 1:1 to `BlendMode::as_u32()` and the `blend_composite.wgsl` shader.
pub mod blend {
    pub const NORMAL: u32 = 0;
    pub const MULTIPLY: u32 = 1;
    pub const SCREEN: u32 = 2;
    pub const OVERLAY: u32 = 3;
    pub const ADD: u32 = 4;
    pub const SUBTRACT: u32 = 5;
    pub const DARKEN: u32 = 6;
    pub const LIGHTEN: u32 = 7;
    pub const SOFT_LIGHT: u32 = 8;
    pub const HARD_LIGHT: u32 = 9;
    pub const DIFFERENCE: u32 = 10;
    pub const MASK_IN: u32 = 11;
    pub const MASK_OUT: u32 = 12;
}
