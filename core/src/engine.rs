//! CoreEngine — the main entry point for the ifol-render core.
//!
//! Wraps the GPU renderer, manages shaders & textures,
//! and exposes a simple API: receive Frame → render → return pixels.

#[cfg(not(target_arch = "wasm32"))]
use crate::backend::FfmpegMediaBackend;
use crate::backend::MediaBackend;
use crate::draw;
use crate::frame::{Frame, PassType, RenderSettings, TextureUpdate};
use crate::shaders;
use crate::text::{self, TextOptions};
use ifol_render::{DrawCommand, GpuCapabilities, PipelineConfig, Renderer};
use std::collections::HashMap;
use std::sync::Arc;

// Native-only imports
#[cfg(not(target_arch = "wasm32"))]
use crate::export::{ExportConfig, ExportProgress};
#[cfg(not(target_arch = "wasm32"))]
use crate::sysinfo::SysInfo;
#[cfg(not(target_arch = "wasm32"))]
use crate::video;
#[cfg(not(target_arch = "wasm32"))]
use crate::video_stream::VideoStream;

/// The core rendering engine.
///
/// Stateless render machine: receives Frame → renders → returns pixels.
/// Internally caches textures and compiled shaders for performance.
pub struct CoreEngine {
    renderer: Renderer,
    settings: RenderSettings,
    /// Cached font data (key → raw font bytes).
    font_cache: HashMap<String, Vec<u8>>,
    /// Cached video metadata (path → VideoInfo).
    #[cfg(not(target_arch = "wasm32"))]
    video_info_cache: HashMap<String, video::VideoInfo>,
    /// Persistent video stream decoders (stream_key → VideoStream).
    #[cfg(not(target_arch = "wasm32"))]
    video_streams: HashMap<String, VideoStream>,
    /// Path to FFmpeg binary. Engine-level config.
    #[cfg(not(target_arch = "wasm32"))]
    ffmpeg_path: Option<String>,
    /// Text content cache — skip re-rasterization when content hasn't changed.
    /// Maps texture key → (content, font_size, alignment) signature.
    text_cache: HashMap<String, (String, u32, u32)>,
    /// Polymorphic Media Backend
    pub backend: Arc<Box<dyn MediaBackend>>,
}

impl CoreEngine {
    /// Create a new CoreEngine with the given output settings (Native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(settings: RenderSettings) -> Self {
        let renderer = Renderer::new(settings.width, settings.height);
        let default_backend = Box::new(FfmpegMediaBackend::new("ffmpeg")) as Box<dyn MediaBackend>;
        Self::build(renderer, settings, default_backend)
    }

    /// Create a headless async CoreEngine with a caller-provided backend.
    ///
    /// On native, pass `FfmpegMediaBackend`. On WASM, pass `WebMediaBackend`.
    pub async fn new_async(settings: RenderSettings, backend: Box<dyn MediaBackend>) -> Self {
        let renderer = Renderer::new_async(settings.width, settings.height).await;
        Self::build(renderer, settings, backend)
    }

    /// Create a web renderer bound to an HTML Canvas
    #[cfg(target_arch = "wasm32")]
    pub async fn new_web(
        canvas: web_sys::HtmlCanvasElement, 
        settings: RenderSettings,
        backend: Box<dyn MediaBackend>,
    ) -> Self {
        let renderer = Renderer::new_web(canvas, settings.width, settings.height).await;
        Self::build(renderer, settings, backend)
    }

    fn build(renderer: Renderer, settings: RenderSettings, backend: Box<dyn MediaBackend>) -> Self {
        Self {
            renderer,
            settings,
            font_cache: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            video_info_cache: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            video_streams: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            ffmpeg_path: None,
            text_cache: HashMap::new(),
            backend: Arc::new(backend),
        }
    }

    /// Set the FFmpeg binary path (engine-level config). Native only.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_ffmpeg_path(&mut self, path: &str) {
        self.ffmpeg_path = Some(path.to_string());
        self.backend = Arc::new(Box::new(FfmpegMediaBackend::new(path)) as Box<dyn MediaBackend>);
    }

    /// Get the configured FFmpeg binary path. Native only.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn ffmpeg_bin(&self) -> &str {
        self.ffmpeg_path.as_deref().unwrap_or("ffmpeg")
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
            let data = self.backend.read_file_bytes(path)
                .ok_or_else(|| format!("Failed to read asset '{}'", path))?;
            let img = image::load_from_memory(&data)
                .map_err(|e| format!("Failed to decode image '{}': {}", path, e))?;
            let rgba = img.into_rgba8();
            self.renderer.load_rgba(key, &rgba, rgba.width(), rgba.height());
        }
        Ok(())
    }

    /// Upload raw RGBA pixels as a texture.
    /// Always replaces existing texture with same key.
    pub fn load_rgba(&mut self, key: &str, data: &[u8], width: u32, height: u32) {
        self.renderer.load_rgba(key, data, width, height);
    }

    /// Rasterize text to a texture with full layout options.
    ///
    /// Supports custom fonts, multi-line, word wrap, alignment.
    pub fn rasterize_text(
        &mut self,
        key: &str,
        content: &str,
        opts: &TextOptions,
        font_key: Option<&str>,
    ) -> Result<[u32; 2], String> {
        let font_data = match font_key {
            Some(fk) => self
                .font_cache
                .get(fk)
                .map(|v| v.as_slice())
                .ok_or_else(|| format!("Font '{}' not loaded", fk))?,
            None => text::default_font_data(),
        };
        let (pixels, tw, th) = text::rasterize_text(content, font_data, opts)?;
        self.renderer.update_rgba(key, &pixels, tw, th);
        Ok([tw, th])
    }

    /// Load a font file into the font cache.
    pub fn load_font(&mut self, key: &str, path: &str) -> Result<(), String> {
        if !self.font_cache.contains_key(key) {
            let data = self.backend.read_file_bytes(path)
                .ok_or_else(|| format!("Failed to read font '{}'", path))?;
            self.font_cache.insert(key.to_string(), data);
        }
        Ok(())
    }

    /// Decode a video frame and upload as texture.
    ///
    /// On native: uses persistent VideoStream for fast sequential reads (~5ms).
    /// Falls back to single-frame decode for random access.
    /// On WASM: delegates entirely to MediaBackend (Canvas2D / ffmpeg.wasm).
    pub fn decode_video_frame(
        &mut self,
        key: &str,
        path: &str,
        timestamp_secs: f64,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Result<[u32; 2], String> {
        #[allow(unused_variables)]
        let w = width.unwrap_or(self.settings.width);
        #[allow(unused_variables)]
        let h = height.unwrap_or(self.settings.height);

        // ── WASM path: delegate to MediaBackend (JS provides decoded frames) ──
        // Only check backend overrides on WASM — on native these always return None
        // but cost 2× Arc<RwLock> + HashMap lookup per frame for nothing.
        #[cfg(target_arch = "wasm32")]
        {
            // Try raw RGBA path first (from HTML5 Canvas getImageData or similar)
            if let Some((pixels, fw, fh)) = self.backend.get_video_frame_rgba(path, timestamp_secs) {
                self.renderer.update_rgba(key, &pixels, fw, fh);
                return Ok([fw, fh]);
            }
            // Try encoded image path (JPEG/PNG)
            if let Some(pixels) = self.backend.get_video_frame(path, timestamp_secs) {
                if let Ok(img) = image::load_from_memory(&pixels) {
                    let rgba = img.into_rgba8();
                    let actual_w = rgba.width();
                    let actual_h = rgba.height();
                    self.renderer.update_rgba(key, &rgba, actual_w, actual_h);
                    return Ok([actual_w, actual_h]);
                }
            }
            return Err(format!("No video frame available for '{}' at {:.2}s — backend did not provide frame data", path, timestamp_secs));
        }

        // ── Native path: VideoStream for fast sequential FFmpeg pipe decoding ──
        #[cfg(not(target_arch = "wasm32"))]
        {
            let stream_key = format!("{}:{}x{}", path, w, h);
            let ffmpeg_bin = self.ffmpeg_bin().to_string();
            let fps = self.settings.fps;

            if !self.video_streams.contains_key(&stream_key) {
                let stream = VideoStream::start(path, timestamp_secs, w, h, fps, &ffmpeg_bin)?;
                self.video_streams.insert(stream_key.clone(), stream);
            }

            let stream = self.video_streams.get_mut(&stream_key).unwrap();
            let pixels = stream.frame_at(timestamp_secs)?;
            // update_rgba: reuse existing GPU texture, avoid 8MB alloc/dealloc per frame
            self.renderer.update_rgba(key, pixels, w, h);
            return Ok([w, h]);
        }
    }

    /// Get cached video info, probing if not yet cached. Native only.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn video_info(&mut self, path: &str) -> Result<&video::VideoInfo, String> {
        if !self.video_info_cache.contains_key(path) {
            let info = if let Some(m) = self.backend.get_video_info(path) {
                m
            } else {
                let probe_path = self
                    .ffmpeg_path
                    .as_ref()
                    .map(|p| p.replace("ffmpeg", "ffprobe"));
                video::probe(path, probe_path.as_deref())?
            };
            self.video_info_cache.insert(path.to_string(), info);
        }
        Ok(&self.video_info_cache[path])
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
                    clear_color,
                } => {
                    // Sort entities by (layer, z_index)
                    let mut sorted = entities.clone();
                    draw::sort_entities(&mut sorted);

                    // Build draw commands (pixel→clip + pack uniforms)
                    let commands = draw::build_draw_commands(
                        &sorted,
                        self.settings.width,
                        self.settings.height,
                    );

                    // ZERO-COPY: Render directly to intermediate target in VRAM
                    let _ = self.renderer.render_frame_to(&commands, *clear_color, Some(&pass.output));
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

                    // ZERO-COPY: Render directly to intermediate target in VRAM
                    let _ = self.renderer.render_frame_to(&commands, [0.0, 0.0, 0.0, 0.0], Some(&pass.output));
                }

                PassType::Output { input } => {
                    // Output pass: Draws VRAM `input` texture back into the CPU mapped Buffer!
                    let commands = vec![DrawCommand {
                        pipeline: "copy".to_string(),
                        uniforms: vec![0.0], // Padding to fulfill minimal binding size
                        textures: vec![input.clone()],
                    }];
                    
                    // Sending None performs the CPU synchronization and mapped Download
                    last_pixels = self.renderer.render_frame(&commands, [0.0, 0.0, 0.0, 1.0]);
                }
            }
        }

        last_pixels
    }

    // ── Export (Native only) ──

    /// Export a sequence of frames to video via FFmpeg.
    ///
    /// Returns the video output path on success.
    /// Audio is NOT handled here — use `ifol-audio` crate for mixing,
    /// then `ifol_audio::mux_video_audio()` to combine.
    ///
    /// `frames` is an Iterator, allowing infinite-length batch generation to save memory.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn export_video<I>(
        &mut self,
        frames: I,
        total_frames: usize,
        config: &ExportConfig,
        mut on_progress: impl FnMut(ExportProgress) -> bool,
    ) -> Result<String, String> 
    where
        I: IntoIterator<Item = Frame>,
    {
        let fps = config.fps.unwrap_or(30.0);
        let width = config.width.unwrap_or(self.settings.width);
        let height = config.height.unwrap_or(self.settings.height);

        if total_frames == 0 {
            return Err("No frames to export.".into());
        }

        // Resize if export dimensions differ
        if width != self.settings.width || height != self.settings.height {
            self.resize(width, height);
        }

        let sys_info = SysInfo::probe(self.ffmpeg_bin());
        log::info!("Export Hardware detected: {:?}", sys_info);

        let output_path = config.output_path.clone();
        let mut encoder = self.backend.start_export(width, height, fps, config, &sys_info)?;

        // GPU-CPU pipeline: buffer up to 3 frames.
        // GPU renders -> pushes to this channel.
        // Background thread -> pops from channel -> encodes via FFmpeg.
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(3);

        let encode_thread = std::thread::spawn(move || {
            let mut result = Ok(());
            for pixels in rx {
                if let Err(e) = encoder.write_rgba_frame(&pixels) {
                    result = Err(e);
                    break;
                }
            }
            if result.is_ok() {
                if let Err(e) = encoder.close() {
                    result = Err(e);
                }
            }
            result
        });

        let start = std::time::Instant::now();

        for (i, frame) in frames.into_iter().enumerate() {
            let pixels = self.render_frame(&frame);
            
            // Push pixels to encode thread. Blocks if queue is full.
            // If encode thread hit an error, the queue is closed and `send` fails.
            if tx.send(pixels).is_err() {
                break;
            }

            let elapsed = start.elapsed().as_secs_f64();
            let export_fps = if elapsed > 0.0 {
                (i + 1) as f64 / elapsed
            } else {
                0.0
            };
            let remaining: u64 = if total_frames > i + 1 { (total_frames - i - 1) as u64 } else { 0 };
            let eta = if export_fps > 0.0 {
                remaining as f64 / export_fps
            } else {
                0.0
            };

            if !on_progress(ExportProgress {
                current_frame: i as u64,
                total_frames: total_frames as u64,
                eta_seconds: eta,
                export_fps,
            }) {
                log::info!("Export cancelled via progress callback.");
                break;
            }
        }

        // Close the channel, signalling the encoder thread to finish remaining frames.
        drop(tx);

        // Propagate any FFmpeg IO errors that occurred in the encode thread.
        encode_thread.join().map_err(|_| "Encode thread panicked".to_string())??;

        Ok(output_path)
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
                    if !self.renderer.has_texture(key)
                        && let Err(e) = self.load_image(key, path)
                    {
                        log::warn!("Failed to load image '{}': {}", path, e);
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
                TextureUpdate::LoadFont { key, path } => {
                    if let Err(e) = self.load_font(key, path) {
                        log::warn!("Failed to load font: {}", e);
                    }
                }
                TextureUpdate::RasterizeText {
                    key,
                    content,
                    font_size,
                    color,
                    font_key,
                    max_width,
                    line_height,
                    alignment,
                } => {
                    // Cache check: skip re-rasterization if content hasn't changed
                    let cache_sig = (
                        content.clone(),
                        (*font_size * 100.0) as u32, // quantize font_size
                        *alignment,
                    );
                    if let Some(cached) = self.text_cache.get(key.as_str()) {
                        if *cached == cache_sig && self.renderer.has_texture(key) {
                            continue;
                        }
                    }

                    let opts = TextOptions {
                        font_size: *font_size,
                        color: *color,
                        max_width: *max_width,
                        line_height: line_height.unwrap_or(1.2),
                        alignment: *alignment,
                    };
                    if let Err(e) = self.rasterize_text(key, content, &opts, font_key.as_deref()) {
                        log::warn!("Failed to rasterize text: {}", e);
                    } else {
                        self.text_cache.insert(key.clone(), cache_sig);
                    }
                }
                TextureUpdate::DecodeVideoFrame {
                    key,
                    path,
                    timestamp_secs,
                    width,
                    height,
                } => {
                    if let Err(e) =
                        self.decode_video_frame(key, path, *timestamp_secs, *width, *height)
                    {
                        log::error!(
                            "⚠ Video decode FAILED for key='{}' path='{}' t={:.2}s ({}×{}): {}",
                            key, path, timestamp_secs,
                            width.unwrap_or(self.settings.width),
                            height.unwrap_or(self.settings.height),
                            e
                        );
                    }
                }
                TextureUpdate::Evict { key } => {
                    self.renderer.evict_texture(key);
                }
            }
        }
    }
}
