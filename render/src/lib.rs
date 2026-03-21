//! # ifol-render
//!
//! Pure GPU executor. Receives shaders (WGSL strings), compiles them,
//! caches pipelines, and executes draw commands. **Does NOT own any shaders.**
//!
//! ## Performance Features
//! - **Uniform ring buffer**: pre-allocated, zero-alloc per draw
//! - **Draw call batching**: minimizes pipeline state switches
//! - **Texture cache LRU**: automatic eviction when VRAM budget exceeded
//! - **VRAM tracking**: real-time memory usage monitoring
//! - **Single command encoder**: all draws in one submission

pub mod effects;
mod engine;
pub mod vertex;

use std::collections::HashMap;
use wgpu::util::DeviceExt;

use effects::context::EffectContext;
use vertex::{QUAD_INDICES, QUAD_VERTICES, Vertex};

// Re-export
use engine::GpuEngine;
pub use engine::gpu::GpuCapabilities;

// ══════════════════════════════════════
// Public API Types
// ══════════════════════════════════════

/// Configuration for registering a draw pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub vertex_entry: String,
    pub fragment_entry: String,
    pub uses_vertex_buffer: bool,
    pub alpha_blend: bool,
}

impl PipelineConfig {
    /// Config for standard quad rendering (vertex buffer + alpha blend).
    pub fn quad() -> Self {
        Self {
            vertex_entry: "vs_main".into(),
            fragment_entry: "fs_main".into(),
            uses_vertex_buffer: true,
            alpha_blend: true,
        }
    }

    /// Config for fullscreen effect pass (no vertex buffer, no blend).
    pub fn fullscreen() -> Self {
        Self {
            vertex_entry: "vs_fullscreen".into(),
            fragment_entry: "fs_main".into(),
            uses_vertex_buffer: false,
            alpha_blend: false,
        }
    }
}

/// A single draw command — everything the GPU needs for one draw call.
#[derive(Debug, Clone)]
pub struct DrawCommand {
    /// Name of the registered pipeline to use.
    pub pipeline: String,
    /// Raw uniform data (floats). Layout must match shader struct.
    pub uniforms: Vec<f32>,
    /// Texture keys to bind.
    pub textures: Vec<String>,
}

/// Effect configuration (post-processing pass).
#[derive(Debug, Clone)]
pub struct EffectConfig {
    pub effect_type: String,
    pub params: HashMap<String, f32>,
}

/// An effect entry in the registry.
pub struct EffectEntry {
    pub name: String,
    pub shader_source: String,
    pub default_params: Vec<(String, f32)>,
    pub pass_count: u32,
}

/// VRAM usage statistics.
#[derive(Debug, Clone)]
pub struct VramStats {
    /// Total texture cache VRAM (bytes).
    pub texture_cache_bytes: u64,
    /// Number of cached textures.
    pub texture_count: usize,
    /// Uniform ring buffer size (bytes).
    pub uniform_buffer_bytes: u64,
    /// Max texture cache budget (bytes). 0 = unlimited.
    pub max_cache_bytes: u64,
}

/// Texture cache stats.
#[derive(Debug, Clone)]
pub struct TextureCacheStats {
    pub count: usize,
    pub total_bytes: u64,
    pub max_bytes: u64,
    /// Keys sorted by last-used (oldest first).
    pub keys_by_age: Vec<String>,
}

// ══════════════════════════════════════
// Cached Pipeline
// ══════════════════════════════════════

struct CachedPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    config: PipelineConfig,
}

// ══════════════════════════════════════
// Uniform Ring Buffer
// ══════════════════════════════════════

/// Pre-allocated GPU buffer for per-frame uniform data.
/// Eliminates per-draw buffer allocations.
struct UniformRingBuffer {
    buffer: wgpu::Buffer,
    /// Total capacity in bytes.
    capacity: u64,
    /// Current write offset (reset each frame).
    offset: u64,
    /// Minimum uniform alignment (256 bytes for wgpu).
    alignment: u64,
}

impl UniformRingBuffer {
    fn new(device: &wgpu::Device, capacity_bytes: u64) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Ring Buffer"),
            size: capacity_bytes,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            capacity: capacity_bytes,
            offset: 0,
            alignment: 256, // wgpu requires 256-byte alignment for dynamic offsets
        }
    }

    /// Reset for a new frame.
    fn reset(&mut self) {
        self.offset = 0;
    }

    /// Allocate space for uniform data, return the byte offset.
    /// Returns None if buffer is full.
    fn allocate(&mut self, data_bytes: u64) -> Option<u64> {
        let aligned_offset = self.offset;
        let aligned_size = data_bytes.div_ceil(self.alignment) * self.alignment;
        let new_offset = aligned_offset + aligned_size;

        if new_offset > self.capacity {
            return None;
        }

        self.offset = new_offset;
        Some(aligned_offset)
    }

    /// Write uniform data at the given offset.
    fn write(&self, queue: &wgpu::Queue, offset: u64, data: &[u8]) {
        queue.write_buffer(&self.buffer, offset, data);
    }
}

// ══════════════════════════════════════
// Texture Cache Entry (LRU)
// ══════════════════════════════════════

struct CachedTexture {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Size in bytes (w * h * 4).
    size_bytes: u64,
    /// Frame number when last used.
    last_used_frame: u64,
}

// ══════════════════════════════════════
// Renderer — Pure GPU Executor
// ══════════════════════════════════════

/// The GPU renderer. Owns GPU context but NO shaders.
pub struct Renderer {
    engine: engine::GpuEngine,
    /// Registered draw pipelines (name → cached GPU pipeline).
    pipelines: HashMap<String, CachedPipeline>,
    /// Registered effects.
    effect_entries: HashMap<String, EffectEntry>,
    /// Cached textures with LRU tracking.
    texture_cache: HashMap<String, CachedTexture>,
    /// Total texture cache VRAM.
    texture_cache_bytes: u64,
    /// Max texture cache size (0 = unlimited).
    max_cache_bytes: u64,
    /// 1x1 white fallback texture for solid color rendering.
    white_texture_view: wgpu::TextureView,
    /// Shared quad vertex/index buffers.
    quad_vertex_buffer: wgpu::Buffer,
    quad_index_buffer: wgpu::Buffer,
    /// Uniform ring buffer (reused every frame).
    uniform_ring: UniformRingBuffer,
    /// Effect ping-pong context.
    effect_ctx: Option<EffectContext>,
    width: u32,
    height: u32,
    /// Current frame number for LRU tracking.
    frame_number: u64,
}

impl Renderer {
    /// Create a headless renderer (Native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(width: u32, height: u32) -> Self {
        let engine = pollster::block_on(GpuEngine::new_headless(width, height));
        Self::from_engine(engine)
    }

    /// Create an asynchronous headless renderer (for environments where block_on is not allowed).
    pub async fn new_async(width: u32, height: u32) -> Self {
        let engine = GpuEngine::new_headless(width, height).await;
        Self::from_engine(engine)
    }

    /// Create a web renderer attached to an HTML canvas.
    #[cfg(target_arch = "wasm32")]
    pub async fn new_web(canvas: web_sys::HtmlCanvasElement, width: u32, height: u32) -> Self {
        let engine = GpuEngine::new_web(canvas, width, height).await;
        Self::from_engine(engine)
    }

    /// Internal engine builder.
    fn from_engine(engine: GpuEngine) -> Self {
        let width = engine.width;
        let height = engine.height;

        // Quad geometry (shared by all quad-based pipelines)
        let quad_vertex_buffer =
            engine
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("quad vertices"),
                    contents: bytemuck::cast_slice(QUAD_VERTICES),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let quad_index_buffer =
            engine
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("quad indices"),
                    contents: bytemuck::cast_slice(QUAD_INDICES),
                    usage: wgpu::BufferUsages::INDEX,
                });

        // 1x1 white fallback texture
        let white_texture = engine.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("white 1x1"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: engine.texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        engine.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &white_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white_texture_view = white_texture.create_view(&Default::default());

        // Uniform ring buffer: 2MB should handle ~8000 draw commands per frame
        // (256 bytes aligned × 8000 = 2MB)
        let uniform_ring = UniformRingBuffer::new(&engine.device, 2 * 1024 * 1024);

        Self {
            engine,
            pipelines: HashMap::new(),
            effect_entries: HashMap::new(),
            texture_cache: HashMap::new(),
            texture_cache_bytes: 0,
            max_cache_bytes: 0, // unlimited by default
            white_texture_view,
            quad_vertex_buffer,
            quad_index_buffer,
            uniform_ring,
            effect_ctx: None,
            width,
            height,
            frame_number: 0,
        }
    }

    // ── Engine ──────────────────────────

    /// Resize the output.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.engine.resize(width, height);
        self.effect_ctx = None;
    }

    /// Query GPU capabilities.
    pub fn capabilities(&self) -> GpuCapabilities {
        GpuCapabilities::from_adapter(&self.engine.adapter)
    }

    // ── Texture Cache ──────────────────

    /// Load an image file into GPU texture cache.
    pub fn load_image(&mut self, key: &str, path: &str) -> Result<(), String> {
        if self.texture_cache.contains_key(key) {
            return Ok(());
        }
        let img = image::open(path)
            .map_err(|e| format!("Failed to load image '{}': {}", path, e))?
            .to_rgba8();
        let (w, h) = img.dimensions();
        self.load_rgba(key, &img, w, h);
        log::info!("Loaded image '{}' ({}x{}) as '{}'", path, w, h, key);
        Ok(())
    }

    /// Load raw RGBA bytes as a texture.
    ///
    /// Input data is always RGBA byte order. If the GPU surface format is BGRA,
    /// this method swaps R↔B channels before uploading to maintain correct colors.
    pub fn load_rgba(&mut self, key: &str, data: &[u8], width: u32, height: u32) {
        let size_bytes = (width as u64) * (height as u64) * 4;

        // Evict LRU textures if over budget
        self.evict_if_needed(size_bytes);

        let texture = self.engine.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(key),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.engine.texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        // Swap RGBA → BGRA if GPU surface format expects BGRA byte order.
        // This is the case on WebGPU (Chrome/Edge) where surface is Bgra8Unorm.
        let needs_swap = matches!(
            self.engine.texture_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );

        let upload_data;
        let final_data = if needs_swap {
            upload_data = rgba_to_bgra(data);
            &upload_data[..]
        } else {
            data
        };

        self.engine.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            final_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());

        // Remove old entry if exists
        if let Some(old) = self.texture_cache.remove(key) {
            self.texture_cache_bytes -= old.size_bytes;
        }

        self.texture_cache.insert(
            key.to_string(),
            CachedTexture {
                texture,
                view,
                size_bytes,
                last_used_frame: self.frame_number,
            },
        );
        self.texture_cache_bytes += size_bytes;
    }

    /// Update an existing texture's pixel data WITHOUT recreating it.
    ///
    /// If the texture exists with matching dimensions, just writes new data (fast).
    /// If dimensions changed or texture doesn't exist, falls back to full load.
    ///
    /// This is the key optimization for video frames — avoids GPU texture
    /// creation/destruction at 30fps (8MB allocation per frame).
    pub fn update_rgba(&mut self, key: &str, data: &[u8], width: u32, height: u32) {
        if let Some(entry) = self.texture_cache.get_mut(key) {
            let expected_size = (width as u64) * (height as u64) * 4;
            if entry.size_bytes == expected_size {
                // Swap RGBA → BGRA if needed (same logic as load_rgba)
                let needs_swap = matches!(
                    self.engine.texture_format,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
                );
                let upload_data;
                let final_data = if needs_swap {
                    upload_data = rgba_to_bgra(data);
                    &upload_data[..]
                } else {
                    data
                };

                // Same dimensions — just update the data (fast path)
                self.engine.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &entry.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    final_data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * width),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                entry.last_used_frame = self.frame_number;
                return;
            }
        }
        // Dimensions changed or texture doesn't exist — full recreation
        self.load_rgba(key, data, width, height);
    }

    /// Check if a texture is cached.
    pub fn has_texture(&self, key: &str) -> bool {
        self.texture_cache.contains_key(key)
    }

    /// Evict a cached texture.
    pub fn evict_texture(&mut self, key: &str) {
        if let Some(entry) = self.texture_cache.remove(key) {
            self.texture_cache_bytes -= entry.size_bytes;
        }
    }

    /// Clear all cached textures.
    pub fn clear_textures(&mut self) {
        self.texture_cache.clear();
        self.texture_cache_bytes = 0;
    }

    /// Set maximum texture cache size in bytes. 0 = unlimited.
    pub fn set_max_cache_size(&mut self, max_bytes: u64) {
        self.max_cache_bytes = max_bytes;
        if max_bytes > 0 {
            self.evict_if_needed(0);
        }
    }

    /// Get texture cache statistics.
    pub fn texture_cache_stats(&self) -> TextureCacheStats {
        let mut keys_by_age: Vec<(&String, u64)> = self
            .texture_cache
            .iter()
            .map(|(k, v)| (k, v.last_used_frame))
            .collect();
        keys_by_age.sort_by_key(|(_, frame)| *frame);

        TextureCacheStats {
            count: self.texture_cache.len(),
            total_bytes: self.texture_cache_bytes,
            max_bytes: self.max_cache_bytes,
            keys_by_age: keys_by_age.into_iter().map(|(k, _)| k.clone()).collect(),
        }
    }

    /// Evict LRU textures until we're under budget.
    fn evict_if_needed(&mut self, incoming_bytes: u64) {
        if self.max_cache_bytes == 0 {
            return; // unlimited
        }

        let target = self.max_cache_bytes;
        while self.texture_cache_bytes + incoming_bytes > target && !self.texture_cache.is_empty() {
            // Find LRU (oldest last_used_frame)
            let oldest_key = self
                .texture_cache
                .iter()
                .min_by_key(|(_, v)| v.last_used_frame)
                .map(|(k, _)| k.clone());

            if let Some(key) = oldest_key {
                log::info!(
                    "Evicting texture '{}' (LRU frame {})",
                    key,
                    self.texture_cache[&key].last_used_frame
                );
                self.evict_texture(&key);
            } else {
                break;
            }
        }
    }

    /// Get VRAM usage statistics.
    pub fn vram_usage(&self) -> VramStats {
        VramStats {
            texture_cache_bytes: self.texture_cache_bytes,
            texture_count: self.texture_cache.len(),
            uniform_buffer_bytes: self.uniform_ring.capacity,
            max_cache_bytes: self.max_cache_bytes,
        }
    }

    // ── Pipeline (shader from outside) ──

    /// Register a draw pipeline from WGSL source.
    pub fn register_pipeline(&mut self, name: &str, wgsl_source: &str, config: PipelineConfig) {
        let device = &self.engine.device;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Wgsl(wgsl_source.into()),
        });

        // Standard bind group layout: uniform buffer (dynamic offset) + texture + sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{name} bgl")),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{name} layout")),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_buffers: Vec<wgpu::VertexBufferLayout> = if config.uses_vertex_buffer {
            vec![Vertex::layout()]
        } else {
            vec![]
        };

        let blend = if config.alpha_blend {
            Some(wgpu::BlendState::ALPHA_BLENDING)
        } else {
            None
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("{name} pipeline")),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some(&config.vertex_entry),
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(&config.fragment_entry),
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.engine.texture_format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some(&format!("{name} sampler")),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.pipelines.insert(
            name.to_string(),
            CachedPipeline {
                pipeline,
                bind_group_layout,
                sampler,
                config,
            },
        );

        log::info!("Pipeline registered: '{}'", name);
    }

    /// Register an effect pipeline from WGSL source.
    pub fn register_effect(
        &mut self,
        name: &str,
        wgsl_source: &str,
        default_params: Vec<(String, f32)>,
        pass_count: u32,
    ) {
        self.register_pipeline(name, wgsl_source, PipelineConfig::fullscreen());

        self.effect_entries.insert(
            name.to_string(),
            EffectEntry {
                name: name.to_string(),
                shader_source: wgsl_source.to_string(),
                default_params,
                pass_count,
            },
        );

        log::info!("Effect registered: '{}'", name);
    }

    /// Check if a pipeline is registered.
    pub fn has_pipeline(&self, name: &str) -> bool {
        self.pipelines.contains_key(name)
    }

    /// List available registered pipelines.
    pub fn available_pipelines(&self) -> Vec<&str> {
        self.pipelines.keys().map(|s| s.as_str()).collect()
    }

    /// List available registered effects.
    pub fn available_effects(&self) -> Vec<&str> {
        self.effect_entries.keys().map(|s| s.as_str()).collect()
    }

    // ── Draw (Optimized) ─────────────────

    fn get_or_create_render_target(
        &mut self,
        key: &str,
        width: u32,
        height: u32,
    ) -> wgpu::TextureView {
        let expected_size = (width as u64) * (height as u64) * 4;
        let needs_create = match self.texture_cache.get(key) {
            Some(entry) => entry.size_bytes != expected_size,
            None => true,
        };
        if needs_create {
            self.load_rgba(key, &vec![0; expected_size as usize], width, height);
        }
        let view = self
            .texture_cache
            .get(key)
            .unwrap()
            .texture
            .create_view(&Default::default());
        self.texture_cache.get_mut(key).unwrap().last_used_frame = self.frame_number;
        view
    }

    /// Render draw commands to the main output.
    pub fn render_frame(&mut self, commands: &[DrawCommand], clear_color: [f32; 4]) -> Vec<u8> {
        self.render_frame_to(commands, clear_color, None);
        if self.engine.surface.is_none() {
            self.engine.readback_output()
        } else {
            Vec::new() // No readback support over surface queues
        }
    }

    /// If `output_key` is Some(key), the result is rendered ONLY into the `texture_cache` map.
    /// If `output_key` is None, the result is rendered to the final engine output (WebGPU Surface or Native texture)
    /// and `engine.readback_output()` can be called.
    pub fn render_frame_to(
        &mut self,
        commands: &[DrawCommand],
        clear_color: [f32; 4],
        output_key: Option<&str>,
    ) {
        if commands.is_empty() && output_key.is_some() {
            // Nothing to draw to intermediate, skip to save GPU time
            return;
        }

        // Determine the target texture view.
        // For offscreen (output_key is Some), grab/create cached texture.
        // For final (output_key is None), use surface or headless output_texture.
        let output_view;
        let mut surface_frame = None;

        if let Some(key) = output_key {
            output_view =
                self.get_or_create_render_target(key, self.engine.width, self.engine.height);
        } else if let Some(ref surface) = self.engine.surface {
            // WebGPU canvas rendering!
            let frame = surface
                .get_current_texture()
                .expect("Failed to acquire next surface texture");
            output_view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            surface_frame = Some(frame);
        } else {
            // Headless Native rendering!
            let tex = self.engine.output_texture.as_ref().unwrap();
            output_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        }

        self.frame_number += 1;
        self.uniform_ring.reset();

        // Phase 1: Write all uniforms to ring buffer, build draw list
        struct PreparedDraw {
            pipeline_name: String,
            uniform_offset: u32,
            texture_view_key: Option<String>, // None = white fallback
        }

        let mut prepared: Vec<PreparedDraw> = Vec::with_capacity(commands.len());

        for cmd in commands {
            if !self.pipelines.contains_key(&cmd.pipeline) {
                log::warn!("Pipeline '{}' not registered, skipping", cmd.pipeline);
                continue;
            }

            let data_bytes = (cmd.uniforms.len() * 4) as u64;
            let offset = match self.uniform_ring.allocate(data_bytes) {
                Some(o) => o,
                None => {
                    log::error!("Uniform ring buffer full! Dropping draw command.");
                    continue;
                }
            };

            self.uniform_ring.write(
                &self.engine.queue,
                offset,
                bytemuck::cast_slice(&cmd.uniforms),
            );

            // Update LRU for textures
            let tex_key = cmd.textures.first().cloned();
            if let Some(ref key) = tex_key
                && let Some(entry) = self.texture_cache.get_mut(key)
            {
                entry.last_used_frame = self.frame_number;
            }

            prepared.push(PreparedDraw {
                pipeline_name: cmd.pipeline.clone(),
                uniform_offset: offset as u32,
                texture_view_key: tex_key,
            });
        }

        // Phase 2: Create bind groups, batched by texture
        // We need one bind group per (pipeline, texture) combination
        struct DrawCall {
            pipeline_name: String,
            bind_group: wgpu::BindGroup,
            uniform_offset: u32,
        }

        let mut draw_calls: Vec<DrawCall> = Vec::with_capacity(prepared.len());

        for prep in &prepared {
            let cached = self.pipelines.get(&prep.pipeline_name).unwrap();

            let tex_view = match &prep.texture_view_key {
                Some(key) => match self.texture_cache.get(key) {
                    Some(entry) => &entry.view,
                    None => &self.white_texture_view,
                },
                None => &self.white_texture_view,
            };

            // Bind group with the ENTIRE ring buffer + dynamic offset
            let bg = self
                .engine
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None, // Skip label for perf
                    layout: &cached.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &self.uniform_ring.buffer,
                                offset: 0,
                                size: Some(
                                    std::num::NonZeroU64::new(self.uniform_ring.alignment).unwrap(),
                                ),
                            }),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(tex_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&cached.sampler),
                        },
                    ],
                });

            draw_calls.push(DrawCall {
                pipeline_name: prep.pipeline_name.clone(),
                bind_group: bg,
                uniform_offset: prep.uniform_offset,
            });
        }

        // Phase 3: Single render pass, minimize pipeline switches
        let mut encoder =
            self.engine
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Frame"),
                });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color[0] as f64,
                            g: clear_color[1] as f64,
                            b: clear_color[2] as f64,
                            a: clear_color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            let mut current_pipeline: Option<&str> = None;

            for dc in &draw_calls {
                let cached = self.pipelines.get(&dc.pipeline_name).unwrap();

                // Only set pipeline if it changed
                if current_pipeline != Some(&dc.pipeline_name) {
                    rpass.set_pipeline(&cached.pipeline);
                    current_pipeline = Some(&dc.pipeline_name);

                    // Set vertex/index buffers when pipeline changes
                    if cached.config.uses_vertex_buffer {
                        rpass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                        rpass.set_index_buffer(
                            self.quad_index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                    }
                }

                // Dynamic offset into uniform ring buffer
                rpass.set_bind_group(0, &dc.bind_group, &[dc.uniform_offset]);

                if cached.config.uses_vertex_buffer {
                    rpass.draw_indexed(0..6, 0, 0..1);
                } else {
                    rpass.draw(0..3, 0..1);
                }
            }
        } // encoder.begin_render_pass is dropped here

        self.engine.queue.submit(std::iter::once(encoder.finish()));

        // If we rendered to a WebGPU surface, we MUST present it.
        if let Some(frame) = surface_frame {
            frame.present();
        }

        // If intermediate pass, mark the texture as written and update LRU
        if let Some(key) = output_key
            && let Some(entry) = self.texture_cache.get_mut(key)
        {
            entry.last_used_frame = self.frame_number;
        }
    }

    /// Render a frame with post-processing effects applied.
    pub fn render_frame_with_effects(
        &mut self,
        commands: &[DrawCommand],
        effect_configs: &[EffectConfig],
    ) -> Vec<u8> {
        if effect_configs.is_empty() {
            return self.render_frame(commands, [0.0, 0.0, 0.0, 1.0]);
        }

        // First: normal draw pass
        self.render_frame_to(commands, [0.0, 0.0, 0.0, 1.0], Some("effect_input_temp"));

        // Init effect context if needed
        if self.effect_ctx.is_none() {
            self.effect_ctx = Some(EffectContext::new(
                &self.engine.device,
                self.width,
                self.height,
                self.engine.texture_format,
            ));
        }

        let _output_texture = self.engine.output_texture.as_ref();

        // Copy composite result into ping-pong input
        let mut encoder =
            self.engine
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("effect copy"),
                });
        self.effect_ctx.as_ref().unwrap().load_from(
            &mut encoder,
            &self.texture_cache.get("effect_input_temp").unwrap().texture,
        );
        self.engine.queue.submit(std::iter::once(encoder.finish()));

        // Run each effect
        for config in effect_configs {
            let entry = match self.effect_entries.get(&config.effect_type) {
                Some(e) => e,
                None => {
                    log::warn!("Effect '{}' not registered", config.effect_type);
                    continue;
                }
            };
            let default_params = entry.default_params.clone();
            let pass_count = entry.pass_count;
            let effect_name = entry.name.clone();

            let cached = match self.pipelines.get(&effect_name) {
                Some(c) => c,
                None => continue,
            };
            let pipeline_ptr = &cached.pipeline as *const wgpu::RenderPipeline;
            let bgl_ptr = &cached.bind_group_layout as *const wgpu::BindGroupLayout;
            let sampler_ptr = &cached.sampler as *const wgpu::Sampler;

            for pass in 0..pass_count {
                let param_values: Vec<f32> = default_params
                    .iter()
                    .map(|(name, default)| {
                        if effect_name == "blur" {
                            match name.as_str() {
                                "direction_x" => {
                                    if pass == 0 {
                                        1.0
                                    } else {
                                        0.0
                                    }
                                }
                                "direction_y" => {
                                    if pass == 0 {
                                        0.0
                                    } else {
                                        1.0
                                    }
                                }
                                "texel_size" => {
                                    if pass == 0 {
                                        1.0 / self.width as f32
                                    } else {
                                        1.0 / self.height as f32
                                    }
                                }
                                _ => *config.params.get(name).unwrap_or(default),
                            }
                        } else {
                            *config.params.get(name).unwrap_or(default)
                        }
                    })
                    .collect();

                // Use ring buffer for effect uniforms too
                let data_bytes = (param_values.len() * 4) as u64;
                let offset = self.uniform_ring.allocate(data_bytes).unwrap_or(0);
                self.uniform_ring.write(
                    &self.engine.queue,
                    offset,
                    bytemuck::cast_slice(&param_values),
                );

                let effect_ctx = self.effect_ctx.as_ref().unwrap();

                let bg = self
                    .engine
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: unsafe { &*bgl_ptr },
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                    buffer: &self.uniform_ring.buffer,
                                    offset: 0,
                                    size: Some(
                                        std::num::NonZeroU64::new(self.uniform_ring.alignment)
                                            .unwrap(),
                                    ),
                                }),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(
                                    effect_ctx.input_view(),
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(unsafe { &*sampler_ptr }),
                            },
                        ],
                    });

                let mut encoder =
                    self.engine
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("effect pass"),
                        });

                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("effect render pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: effect_ctx.output_view(),
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    rpass.set_pipeline(unsafe { &*pipeline_ptr });
                    rpass.set_bind_group(0, &bg, &[offset as u32]);
                    rpass.draw(0..3, 0..1);
                }

                self.engine.queue.submit(std::iter::once(encoder.finish()));
                self.effect_ctx.as_mut().unwrap().swap();
            }
        }

        // Copy final result back to output
        let output_texture = self.engine.output_texture.as_ref().unwrap();
        let mut encoder =
            self.engine
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("effect store"),
                });
        self.effect_ctx
            .as_ref()
            .unwrap()
            .store_to(&mut encoder, output_texture);
        self.engine.queue.submit(std::iter::once(encoder.finish()));

        self.engine.readback_output()
    }

    // ── Export ──────────────────────────

    /// Save rendered pixels to a PNG file.
    pub fn save_png(pixels: &[u8], width: u32, height: u32, path: &str) -> Result<(), String> {
        let img = image::RgbaImage::from_raw(width, height, pixels.to_vec())
            .ok_or("Failed to create image from pixels")?;
        img.save(path)
            .map_err(|e| format!("Failed to save PNG: {}", e))?;
        log::info!("Saved PNG: {} ({}x{})", path, width, height);
        Ok(())
    }
}

// ══════════════════════════════════════
// Color channel swap helpers
// ══════════════════════════════════════

/// Swap R and B channels in RGBA pixel data → BGRA byte order.
/// Used when GPU surface format is BGRA but input data is RGBA.
#[inline]
fn rgba_to_bgra(data: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    for chunk in out.chunks_exact_mut(4) {
        chunk.swap(0, 2); // R ↔ B
    }
    out
}
