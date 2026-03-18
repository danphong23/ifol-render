//! # ifol-render
//!
//! Pure GPU executor. Receives shaders (WGSL strings), compiles them,
//! caches pipelines, and executes draw commands. **Does NOT own any shaders.**
//!
//! Callers (core, CLI, user code) register pipelines and effects,
//! then send `DrawCommand[]` to get pixels back.

pub mod effects;
mod engine;
pub mod vertex;

use std::collections::HashMap;
use wgpu::util::DeviceExt;

use effects::context::EffectContext;
use vertex::{QUAD_INDICES, QUAD_VERTICES, Vertex};

// Re-export
pub use engine::gpu::GpuCapabilities;

// ══════════════════════════════════════
// Public API Types
// ══════════════════════════════════════

/// Configuration for registering a draw pipeline.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Vertex entry point name.
    pub vertex_entry: String,
    /// Fragment entry point name.
    pub fragment_entry: String,
    /// Whether this pipeline uses vertex buffers (quad) or fullscreen triangle.
    pub uses_vertex_buffer: bool,
    /// Whether to enable alpha blending.
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
///
/// Generic: pipeline name determines which shader runs.
/// Uniforms are raw float data packed by the caller.
#[derive(Debug, Clone)]
pub struct DrawCommand {
    /// Name of the registered pipeline to use.
    pub pipeline: String,
    /// Raw uniform data (floats). Layout must match shader struct.
    pub uniforms: Vec<f32>,
    /// Texture keys to bind (in order: binding 1, 2, ...).
    /// First texture is bound at binding 1.
    pub textures: Vec<String>,
}

/// Effect configuration (post-processing pass).
#[derive(Debug, Clone)]
pub struct EffectConfig {
    /// Effect name (must be registered).
    pub effect_type: String,
    /// Override parameters (key → value).
    pub params: HashMap<String, f32>,
}

/// An effect entry in the registry.
pub struct EffectEntry {
    pub name: String,
    pub shader_source: String,
    pub default_params: Vec<(String, f32)>,
    pub pass_count: u32,
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
// Renderer — Pure GPU Executor
// ══════════════════════════════════════

/// The GPU renderer. Owns GPU context but NO shaders.
/// All pipelines and effects are registered from outside.
pub struct Renderer {
    engine: engine::GpuEngine,
    /// Registered draw pipelines (name → cached GPU pipeline).
    pipelines: HashMap<String, CachedPipeline>,
    /// Registered effects.
    effect_entries: HashMap<String, EffectEntry>,
    /// Cached textures by key.
    texture_cache: HashMap<String, (wgpu::Texture, wgpu::TextureView)>,
    /// 1x1 white fallback texture for solid color rendering.
    white_texture_view: wgpu::TextureView,
    /// Shared quad vertex/index buffers.
    quad_vertex_buffer: wgpu::Buffer,
    quad_index_buffer: wgpu::Buffer,
    /// Effect ping-pong context.
    effect_ctx: Option<EffectContext>,
    width: u32,
    height: u32,
}

impl Renderer {
    /// Create a new headless renderer.
    pub fn new(width: u32, height: u32) -> Self {
        let engine = pollster::block_on(engine::GpuEngine::new_headless(width, height));

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
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
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

        Self {
            engine,
            pipelines: HashMap::new(),
            effect_entries: HashMap::new(),
            texture_cache: HashMap::new(),
            white_texture_view,
            quad_vertex_buffer,
            quad_index_buffer,
            effect_ctx: None,
            width,
            height,
        }
    }

    // ── Engine ──────────────────────────

    /// Resize the output.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.engine.resize(width, height);
        self.effect_ctx = None; // recreate on next use
    }

    /// Query GPU capabilities.
    pub fn capabilities(&self) -> GpuCapabilities {
        GpuCapabilities::from_adapter(&self.engine.adapter)
    }

    // ── Texture ────────────────────────

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
    pub fn load_rgba(&mut self, key: &str, data: &[u8], width: u32, height: u32) {
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
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.engine.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
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
        self.texture_cache.insert(key.to_string(), (texture, view));
    }

    /// Check if a texture is cached.
    pub fn has_texture(&self, key: &str) -> bool {
        self.texture_cache.contains_key(key)
    }

    /// Evict a cached texture.
    pub fn evict_texture(&mut self, key: &str) {
        self.texture_cache.remove(key);
    }

    /// Clear all cached textures.
    pub fn clear_textures(&mut self) {
        self.texture_cache.clear();
    }

    // ── Pipeline (shader from outside) ──

    /// Register a draw pipeline from WGSL source.
    /// After registration, DrawCommands can reference this pipeline by name.
    pub fn register_pipeline(&mut self, name: &str, wgsl_source: &str, config: PipelineConfig) {
        let device = &self.engine.device;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Wgsl(wgsl_source.into()),
        });

        // Standard bind group layout: uniform buffer + texture + sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("{name} bgl")),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
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
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
        // Register as fullscreen pipeline
        self.register_pipeline(name, wgsl_source, PipelineConfig::fullscreen());

        // Store effect metadata
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

    // ── Draw ───────────────────────────

    /// Render a single frame from draw commands. Returns RGBA pixel data.
    pub fn render_frame(&mut self, commands: &[DrawCommand]) -> Vec<u8> {
        let output_texture = self.engine.output_texture.as_ref().unwrap();
        let output_view = output_texture.create_view(&Default::default());

        // Pre-compute per-command GPU resources
        struct GpuDrawCall {
            bind_group: wgpu::BindGroup,
            pipeline_name: String,
        }

        let mut draw_calls: Vec<GpuDrawCall> = Vec::new();

        for cmd in commands {
            let cached = match self.pipelines.get(&cmd.pipeline) {
                Some(c) => c,
                None => {
                    log::warn!("Pipeline '{}' not registered, skipping", cmd.pipeline);
                    continue;
                }
            };

            // Create uniform buffer from raw float data
            let ub = self
                .engine
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("draw_uniform"),
                    contents: bytemuck::cast_slice(&cmd.uniforms),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            // Pick first texture or white fallback
            let tex_view = if !cmd.textures.is_empty() {
                if let Some(key) = cmd.textures.first() {
                    match self.texture_cache.get(key) {
                        Some((_, view)) => view,
                        None => &self.white_texture_view,
                    }
                } else {
                    &self.white_texture_view
                }
            } else {
                &self.white_texture_view
            };

            let bg = self
                .engine
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("draw_bg"),
                    layout: &cached.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: ub.as_entire_binding(),
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

            draw_calls.push(GpuDrawCall {
                bind_group: bg,
                pipeline_name: cmd.pipeline.clone(),
            });
        }

        // Render pass
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
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            for dc in &draw_calls {
                let cached = self.pipelines.get(&dc.pipeline_name).unwrap();
                rpass.set_pipeline(&cached.pipeline);

                if cached.config.uses_vertex_buffer {
                    rpass.set_vertex_buffer(0, self.quad_vertex_buffer.slice(..));
                    rpass.set_index_buffer(
                        self.quad_index_buffer.slice(..),
                        wgpu::IndexFormat::Uint16,
                    );
                    rpass.set_bind_group(0, &dc.bind_group, &[]);
                    rpass.draw_indexed(0..6, 0, 0..1);
                } else {
                    rpass.set_bind_group(0, &dc.bind_group, &[]);
                    rpass.draw(0..3, 0..1); // fullscreen triangle
                }
            }
        }

        self.engine.queue.submit(std::iter::once(encoder.finish()));
        self.engine.readback_output()
    }

    /// Render a frame with post-processing effects applied.
    pub fn render_frame_with_effects(
        &mut self,
        commands: &[DrawCommand],
        effect_configs: &[EffectConfig],
    ) -> Vec<u8> {
        if effect_configs.is_empty() {
            return self.render_frame(commands);
        }

        // First: normal draw pass
        let _pixels = self.render_frame(commands);

        // Init effect context if needed
        if self.effect_ctx.is_none() {
            self.effect_ctx = Some(EffectContext::new(
                &self.engine.device,
                self.width,
                self.height,
            ));
        }

        let output_texture = self.engine.output_texture.as_ref().unwrap();

        // Copy composite result into ping-pong input
        let mut encoder =
            self.engine
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("effect copy"),
                });
        self.effect_ctx
            .as_ref()
            .unwrap()
            .load_from(&mut encoder, output_texture);
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

                let ub = self
                    .engine
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("effect uniform"),
                        contents: bytemuck::cast_slice(&param_values),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });

                let effect_ctx = self.effect_ctx.as_ref().unwrap();

                // SAFETY: pipeline/bgl/sampler live in self.pipelines HashMap for duration of Renderer
                let bg = self
                    .engine
                    .device
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("effect bg"),
                        layout: unsafe { &*bgl_ptr },
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: ub.as_entire_binding(),
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
                    rpass.set_bind_group(0, &bg, &[]);
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
