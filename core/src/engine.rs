//! CoreEngine — the main entry point for the ifol-render core.
//!
//! Wraps the GPU renderer, manages shaders & textures,
//! and exposes a simple API: receive Frame → render → return pixels.

use crate::draw;
use crate::export::ffmpeg::FfmpegPipe;
use crate::export::{ExportConfig, ExportProgress};
use crate::frame::{Frame, PassType, RenderSettings, TextureUpdate};
use crate::shaders;
use crate::text;
use ifol_render::{DrawCommand, GpuCapabilities, PipelineConfig, Renderer};

/// The core rendering engine.
///
/// Stateless render machine: receives Frame → renders → returns pixels.
/// Internally caches textures and compiled shaders for performance.
pub struct CoreEngine {
    renderer: Renderer,
    settings: RenderSettings,
}

impl CoreEngine {
    /// Create a new CoreEngine with the given output settings.
    ///
    /// Initializes the GPU (headless), allocates buffers.
    pub fn new(settings: RenderSettings) -> Self {
        let renderer = Renderer::new(settings.width, settings.height);
        Self { renderer, settings }
    }

    /// Register all built-in shaders (composite, shapes, effects, ...).
    ///
    /// Call once after creating the engine. Safe to call multiple times.
    pub fn setup_builtins(&mut self) {
        shaders::setup_builtins(&mut self.renderer);
    }

    /// Change output resolution. Cached textures are preserved.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.settings.width = width;
        self.settings.height = height;
        self.renderer.resize(width, height);
    }

    /// Get current render settings.
    pub fn settings(&self) -> &RenderSettings {
        &self.settings
    }

    /// Get GPU capabilities (device name, max texture size, etc.).
    pub fn capabilities(&self) -> GpuCapabilities {
        self.renderer.capabilities()
    }

    // ── Shaders ──

    /// Register a custom shader pipeline.
    pub fn register_shader(&mut self, name: &str, wgsl: &str, config: PipelineConfig) {
        self.renderer.register_pipeline(name, wgsl, config);
    }

    /// Check if a shader is registered.
    pub fn has_shader(&self, name: &str) -> bool {
        self.renderer.has_pipeline(name)
    }

    // ── Textures ──

    /// Load an image from file into texture cache.
    /// Returns pixel dimensions [width, height].
    /// Cached: calling again with same key is a no-op.
    pub fn load_image(&mut self, key: &str, path: &str) -> Result<(), String> {
        if !self.renderer.has_texture(key) {
            self.renderer.load_image(key, path)?;
        }
        Ok(())
    }

    /// Upload raw RGBA pixels as a texture.
    /// Always replaces existing texture with same key.
    pub fn load_rgba(&mut self, key: &str, data: &[u8], width: u32, height: u32) {
        self.renderer.load_rgba(key, data, width, height);
    }

    /// Rasterize text to a texture using the built-in font.
    /// Returns pixel dimensions [width, height].
    pub fn rasterize_text(
        &mut self,
        key: &str,
        content: &str,
        font_size: f32,
        color: [f32; 4],
    ) -> Result<[u32; 2], String> {
        let font_data = text::default_font_data();
        let (pixels, tw, th) = text::rasterize_text(content, font_data, font_size, color)?;
        self.renderer.load_rgba(key, &pixels, tw, th);
        Ok([tw, th])
    }

    /// Check if a texture is in cache.
    pub fn has_texture(&self, key: &str) -> bool {
        self.renderer.has_texture(key)
    }

    /// Remove a texture from cache.
    pub fn evict_texture(&mut self, key: &str) {
        self.renderer.evict_texture(key);
    }

    /// Clear all cached textures.
    pub fn clear_textures(&mut self) {
        self.renderer.clear_textures();
    }

    // ── Render ──

    /// Render a single frame → return RGBA pixels.
    ///
    /// This is the main rendering function. Pipeline:
    /// 1. Process texture_updates
    /// 2. Execute render passes in order
    /// 3. Return final output pixels
    pub fn render_frame(&mut self, frame: &Frame) -> Vec<u8> {
        // Step 1: Process texture updates
        self.process_texture_updates(&frame.texture_updates);

        // Step 2: Execute render passes
        let mut last_pixels = Vec::new();

        for pass in &frame.passes {
            match &pass.pass_type {
                PassType::Entities {
                    entities,
                    clear_color: _,
                } => {
                    // Sort entities by (layer, z_index)
                    let mut sorted = entities.clone();
                    draw::sort_entities(&mut sorted);

                    // Build draw commands (pixel→clip + pack uniforms)
                    let commands =
                        draw::build_draw_commands(&sorted, self.settings.width, self.settings.height);

                    // Render to pixels
                    let pixels = self.renderer.render_frame(&commands);

                    // Store as intermediate texture for later passes
                    self.renderer.load_rgba(
                        &pass.output,
                        &pixels,
                        self.settings.width,
                        self.settings.height,
                    );

                    last_pixels = pixels;
                }

                PassType::Effect {
                    shader,
                    inputs,
                    params,
                } => {
                    // Build a fullscreen draw command using the effect shader
                    let commands = vec![DrawCommand {
                        pipeline: shader.clone(),
                        uniforms: params.clone(),
                        textures: inputs.clone(),
                    }];

                    let pixels = self.renderer.render_frame(&commands);

                    self.renderer.load_rgba(
                        &pass.output,
                        &pixels,
                        self.settings.width,
                        self.settings.height,
                    );

                    last_pixels = pixels;
                }

                PassType::Output { input } => {
                    // If the input texture is the last rendered, use those pixels.
                    // Otherwise, we'd need to read from texture cache.
                    // For now, the last rendered pass's pixels are the output.
                    if !last_pixels.is_empty() {
                        // last_pixels is from the previous pass
                    }
                    let _ = input; // Future: explicitly read from named texture
                }
            }
        }

        last_pixels
    }

    // ── Export ──

    /// Export a sequence of frames to video via FFmpeg.
    pub fn export_video(
        &mut self,
        frames: &[Frame],
        config: &ExportConfig,
        mut on_progress: impl FnMut(ExportProgress),
    ) -> Result<(), String> {
        let fps = config.fps.unwrap_or(30.0);
        let width = config.width.unwrap_or(self.settings.width);
        let height = config.height.unwrap_or(self.settings.height);
        let total_frames = frames.len() as u64;

        if total_frames == 0 {
            return Err("No frames to export.".into());
        }

        // Resize if export dimensions differ
        if width != self.settings.width || height != self.settings.height {
            self.resize(width, height);
        }

        let mut ffmpeg = FfmpegPipe::start(
            width,
            height,
            fps,
            &config.codec,
            &config.pixel_format,
            config.crf,
            &config.output_path,
            config.ffmpeg_path.as_deref(),
        )?;

        let start = std::time::Instant::now();

        for (i, frame) in frames.iter().enumerate() {
            let pixels = self.render_frame(frame);
            ffmpeg.write_frame(&pixels)?;

            let elapsed = start.elapsed().as_secs_f64();
            let export_fps = if elapsed > 0.0 {
                (i + 1) as f64 / elapsed
            } else {
                0.0
            };
            let remaining = total_frames - i as u64 - 1;
            let eta = if export_fps > 0.0 {
                remaining as f64 / export_fps
            } else {
                0.0
            };

            on_progress(ExportProgress {
                current_frame: i as u64,
                total_frames,
                eta_seconds: eta,
                export_fps,
            });
        }

        ffmpeg.finish()
    }

    /// Export a single frame as PNG.
    pub fn export_frame(&mut self, frame: &Frame, path: &str) -> Result<(), String> {
        let pixels = self.render_frame(frame);
        Renderer::save_png(&pixels, self.settings.width, self.settings.height, path)
            .map_err(|e| format!("Failed to save PNG: {e}"))
    }

    /// Static utility: save RGBA pixels to PNG file.
    pub fn save_png(pixels: &[u8], width: u32, height: u32, path: &str) -> Result<(), String> {
        Renderer::save_png(pixels, width, height, path)
            .map_err(|e| format!("Failed to save PNG: {e}"))
    }

    // ── Internal ──

    fn process_texture_updates(&mut self, updates: &[TextureUpdate]) {
        for update in updates {
            match update {
                TextureUpdate::LoadImage { key, path } => {
                    if !self.renderer.has_texture(key) {
                        if let Err(e) = self.renderer.load_image(key, path) {
                            log::warn!("Failed to load image '{}': {}", path, e);
                        }
                    }
                }
                TextureUpdate::UploadRgba {
                    key,
                    data,
                    width,
                    height,
                } => {
                    self.renderer.load_rgba(key, data, *width, *height);
                }
                TextureUpdate::RasterizeText {
                    key,
                    content,
                    font_size,
                    color,
                } => {
                    if let Err(e) = self.rasterize_text(key, content, *font_size, *color) {
                        log::warn!("Failed to rasterize text: {}", e);
                    }
                }
                TextureUpdate::Evict { key } => {
                    self.renderer.evict_texture(key);
                }
            }
        }
    }
}
