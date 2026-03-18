//! # ifol-render
//!
//! Passive GPU rendering tool. Receives `DrawCommand`s, outputs pixels.
//!
//! **Does NOT know about ECS, Entity, Component, World, or any business logic.**
//! It only knows how to draw textured/colored quads with transforms and opacity.

pub mod effects;
pub mod engine;
pub mod passes;
pub mod render_graph;
pub mod resource_manager;
pub mod vertex;

use std::collections::HashMap;
use wgpu::util::DeviceExt;

use passes::composite::{CompositePipeline, CompositeUniforms};

// ══════════════════════════════════════
// Public API Types (standalone — no ECS)
// ══════════════════════════════════════

/// What to draw: a solid color or a cached texture.
#[derive(Debug, Clone)]
pub enum DrawSource {
    /// Solid RGBA color fill.
    Color([f32; 4]),
    /// Reference to a previously loaded texture by key.
    Texture(String),
}

/// Blend mode for compositing (matches composite.wgsl blend_mode uniform).
#[derive(Debug, Clone, Copy, Default)]
pub enum BlendMode {
    #[default]
    Normal, // 0
    Multiply,   // 1
    Screen,     // 2
    Overlay,    // 3
    SoftLight,  // 4
    Add,        // 5
    Difference, // 6
}

impl BlendMode {
    /// Convert to GPU float for shader uniform.
    pub fn to_gpu(&self) -> f32 {
        match self {
            BlendMode::Normal => 0.0,
            BlendMode::Multiply => 1.0,
            BlendMode::Screen => 2.0,
            BlendMode::Overlay => 3.0,
            BlendMode::SoftLight => 4.0,
            BlendMode::Add => 5.0,
            BlendMode::Difference => 6.0,
        }
    }
}

/// A single draw command — everything the GPU needs to draw one quad.
///
/// The render tool does NOT decide what to draw. Callers (core/ECS)
/// build these commands and pass them in.
#[derive(Debug, Clone)]
pub struct DrawCommand {
    /// 4x4 column-major transform matrix (positions the quad in clip space).
    pub transform: [f32; 16],
    /// Opacity (0.0 = invisible, 1.0 = fully opaque).
    pub opacity: f32,
    /// What to render: color fill or texture.
    pub source: DrawSource,
    /// Blend mode.
    pub blend_mode: BlendMode,
}

// ══════════════════════════════════════
// Renderer
// ══════════════════════════════════════

/// The GPU renderer — owns the wgpu context and render resources.
///
/// Create one, load textures, then call `render_frame()` with draw commands.
pub struct Renderer {
    pub engine: engine::GpuEngine,
    composite: CompositePipeline,
    /// Cached textures by key.
    texture_cache: HashMap<String, (wgpu::Texture, wgpu::TextureView)>,
    /// Effect pass ping-pong context.
    effect_ctx: Option<effects::context::EffectContext>,
    /// Pipeline cache — avoids recreating pipelines every frame.
    pipeline_cache: effects::pipeline_cache::PipelineCache,
    /// Registry of available effects.
    effect_registry: effects::EffectRegistry,
    width: u32,
    height: u32,
}

impl Renderer {
    /// Create a new headless renderer (no window surface).
    pub fn new(width: u32, height: u32) -> Self {
        let engine = pollster::block_on(engine::GpuEngine::new_headless(width, height));

        let output_format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let composite = CompositePipeline::new(&engine.device, output_format);

        // Write white pixel to fallback texture
        engine.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &composite.white_texture,
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

        Self {
            engine,
            composite,
            texture_cache: HashMap::new(),
            effect_ctx: None,
            pipeline_cache: effects::pipeline_cache::PipelineCache::new(),
            effect_registry: effects::EffectRegistry::new(),
            width,
            height,
        }
    }

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

    /// Render a single frame from draw commands. Returns RGBA pixel data.
    pub fn render_frame(&mut self, commands: &[DrawCommand]) -> Vec<u8> {
        let output_texture = self.engine.output_texture.as_ref().unwrap();
        let output_view = output_texture.create_view(&Default::default());

        // Pre-compute per-command GPU resources
        struct GpuDrawCall {
            bind_group: wgpu::BindGroup,
        }

        let mut draw_calls: Vec<GpuDrawCall> = Vec::new();

        for cmd in commands {
            let mut uniforms = CompositeUniforms {
                transform: cmd.transform,
                opacity: cmd.opacity,
                blend_mode: cmd.blend_mode.to_gpu(),
                ..Default::default()
            };

            match &cmd.source {
                DrawSource::Color(rgba) => {
                    uniforms.color = *rgba;
                    uniforms.use_texture = 0.0;
                }
                DrawSource::Texture(key) => {
                    uniforms.use_texture = 1.0;
                    // If texture not found, skip this command
                    if !self.texture_cache.contains_key(key) {
                        continue;
                    }
                }
            }

            // Create per-command uniform buffer
            let ub = self
                .engine
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("draw_uniform"),
                    contents: bytemuck::cast_slice(&[uniforms]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            // Pick texture view
            let tex_view = match &cmd.source {
                DrawSource::Texture(key) => &self.texture_cache.get(key).unwrap().1,
                _ => &self.composite.white_texture_view,
            };

            // Create per-command bind group
            let bg = self
                .engine
                .device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("draw_bg"),
                    layout: &self.composite.bind_group_layout,
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
                            resource: wgpu::BindingResource::Sampler(&self.composite.sampler),
                        },
                    ],
                });

            draw_calls.push(GpuDrawCall { bind_group: bg });
        }

        // Render pass
        let mut encoder =
            self.engine
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Frame"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.07,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.composite.render_pipeline);
            render_pass.set_vertex_buffer(0, self.composite.vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                self.composite.index_buffer.slice(..),
                wgpu::IndexFormat::Uint16,
            );

            for dc in &draw_calls {
                render_pass.set_bind_group(0, &dc.bind_group, &[]);
                render_pass.draw_indexed(0..6, 0, 0..1);
            }
        }

        // Readback
        let padded_bytes_per_row = Self::padded_bytes_per_row(self.width);

        let staging = self.engine.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging"),
            size: (padded_bytes_per_row * self.height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.engine.queue.submit(std::iter::once(encoder.finish()));

        // Map and read pixels
        let buffer_slice = staging.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.engine.device.poll(wgpu::Maintain::Wait);
        receiver.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();

        let unpadded_bytes_per_row = self.width * 4;
        let buffer_size = (self.width * self.height * 4) as usize;
        let mut pixels = Vec::with_capacity(buffer_size);
        for row in 0..self.height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            pixels.extend_from_slice(&data[start..end]);
        }

        drop(data);
        staging.unmap();

        pixels
    }

    /// Render a frame with post-processing effects applied.
    ///
    /// Effects are applied in order after the composite pass.
    /// Uses pipeline cache — pipelines are created once and reused.
    /// Any shader following the convention (vs_fullscreen + fs_main + standard bindings)
    /// works automatically without per-effect Rust code.
    pub fn render_frame_with_effects(
        &mut self,
        commands: &[DrawCommand],
        effect_configs: &[effects::EffectConfig],
    ) -> Vec<u8> {
        if effect_configs.is_empty() {
            return self.render_frame(commands);
        }

        // First: normal composite pass
        let _composite_pixels = self.render_frame(commands);

        // Initialize effect context if needed
        if self.effect_ctx.is_none() {
            self.effect_ctx = Some(effects::context::EffectContext::new(
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

        // Run each effect using the generic engine
        for config in effect_configs {
            if let Some(entry) = self.effect_registry.get(&config.effect_type) {
                let shader_source = entry.shader_source.clone();
                let default_params = entry.default_params.clone();
                let pass_count = entry.pass_count;
                let effect_name = entry.name.clone();

                // Get or create cached pipeline
                let cached = self.pipeline_cache.get_or_create(
                    &self.engine.device,
                    &effect_name,
                    &shader_source,
                );

                for pass in 0..pass_count {
                    // Build uniform data: pack params as f32 array
                    let param_values: Vec<f32> = default_params
                        .iter()
                        .map(|(name, default)| {
                            // Special handling for blur direction per pass
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

                    let ub =
                        self.engine
                            .device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("effect uniform"),
                                contents: bytemuck::cast_slice(&param_values),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });

                    let effect_ctx = self.effect_ctx.as_ref().unwrap();

                    let bg = self
                        .engine
                        .device
                        .create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some("effect bg"),
                            layout: &cached.bind_group_layout,
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
                                    resource: wgpu::BindingResource::Sampler(&cached.sampler),
                                },
                            ],
                        });

                    let mut encoder = self.engine.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: Some("effect pass"),
                        },
                    );

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

                        rpass.set_pipeline(&cached.pipeline);
                        rpass.set_bind_group(0, &bg, &[]);
                        rpass.draw(0..3, 0..1); // fullscreen triangle
                    }

                    self.engine.queue.submit(std::iter::once(encoder.finish()));
                    self.effect_ctx.as_mut().unwrap().swap();
                }
            }
        }

        // Copy final result back to output texture
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

        // Readback the final pixels
        self.readback_pixels(output_texture)
    }

    /// Get the effect registry for querying available effects.
    pub fn effect_registry(&self) -> &effects::EffectRegistry {
        &self.effect_registry
    }

    /// Register an external shader effect at runtime.
    pub fn register_effect(
        &mut self,
        name: &str,
        shader_source: String,
        default_params: Vec<(String, f32)>,
        pass_count: u32,
    ) {
        self.effect_registry
            .register_external(name, shader_source, default_params, pass_count);
    }

    /// Readback pixels from a GPU texture into a CPU Vec<u8>.
    fn readback_pixels(&self, texture: &wgpu::Texture) -> Vec<u8> {
        let padded_bytes_per_row = Self::padded_bytes_per_row(self.width);
        let staging = self.engine.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging"),
            size: (padded_bytes_per_row * self.height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder =
            self.engine
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("readback"),
                });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.engine.queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = staging.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.engine.device.poll(wgpu::Maintain::Wait);
        receiver.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();
        let unpadded_bytes_per_row = self.width * 4;
        let buffer_size = (self.width * self.height * 4) as usize;
        let mut pixels = Vec::with_capacity(buffer_size);
        for row in 0..self.height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            pixels.extend_from_slice(&data[start..end]);
        }

        drop(data);
        staging.unmap();
        pixels
    }

    /// Calculate padded bytes per row (wgpu requires 256-byte alignment).
    fn padded_bytes_per_row(width: u32) -> u32 {
        let unpadded = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        unpadded.div_ceil(align) * align
    }

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
