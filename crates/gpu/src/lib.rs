//! # ifol-render-gpu
//!
//! GPU rendering backend using wgpu. Provides:
//! - **Renderer**: Full GPU rendering pipeline
//! - **Composite Pipeline**: Draw textured/colored quads with transform + opacity
//! - **Image Loader**: Load PNG/JPG to GPU textures
//! - **Resource Manager**: GPU texture/buffer pooling

pub mod engine;
pub mod passes;
pub mod render_graph;
pub mod resource_manager;
pub mod vertex;

use std::collections::HashMap;
use wgpu::util::DeviceExt;

use ifol_render_core::ecs::World;
use ifol_render_core::scene::RenderSettings;
use ifol_render_core::time::TimeState;
use passes::composite::{CompositePipeline, CompositeUniforms};

/// The GPU renderer — owns the wgpu context and render resources.
pub struct Renderer {
    pub engine: engine::GpuEngine,
    composite: CompositePipeline,
    /// Cached textures by entity ID.
    texture_cache: HashMap<String, (wgpu::Texture, wgpu::TextureView)>,
    /// Staging buffer for pixel readback.
    #[allow(dead_code)]
    staging_buffer: Option<wgpu::Buffer>,
    width: u32,
    height: u32,
}

impl Renderer {
    /// Create a new headless renderer (for CLI/backend/editor).
    pub fn new_headless(settings: &RenderSettings) -> Self {
        let engine = pollster::block_on(engine::GpuEngine::new_headless(
            settings.width,
            settings.height,
        ));

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
            staging_buffer: None,
            width: settings.width,
            height: settings.height,
        }
    }

    /// Load an image file into GPU texture cache.
    pub fn load_image(&mut self, entity_id: &str, path: &str) -> Result<(), String> {
        if self.texture_cache.contains_key(entity_id) {
            return Ok(());
        }

        let img = image::open(path)
            .map_err(|e| format!("Failed to load image '{}': {}", path, e))?
            .to_rgba8();

        let (w, h) = img.dimensions();

        let texture = self.engine.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(entity_id),
            size: wgpu::Extent3d {
                width: w,
                height: h,
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
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&Default::default());
        self.texture_cache
            .insert(entity_id.to_string(), (texture, view));

        log::info!("Loaded image '{}' ({}x{}) for entity '{}'", path, w, h, entity_id);
        Ok(())
    }

    /// Load raw RGBA bytes as a texture.
    pub fn load_rgba(
        &mut self,
        entity_id: &str,
        data: &[u8],
        width: u32,
        height: u32,
    ) {
        let texture = self.engine.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(entity_id),
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
        self.texture_cache
            .insert(entity_id.to_string(), (texture, view));
    }

    /// Render a single frame and return RGBA pixel data.
    pub fn render_frame(&mut self, world: &World, _time: &TimeState) -> Vec<u8> {
        let output_texture = self.engine.output_texture.as_ref().unwrap();
        let output_view = output_texture.create_view(&Default::default());

        // Pre-compute per-entity draw data (uniform buffers + bind groups)
        // Must be done BEFORE the render pass so GPU buffers are ready.
        let sorted = world.sorted_by_layer();

        struct DrawCall {
            bind_group: wgpu::BindGroup,
        }

        let mut draw_calls: Vec<DrawCall> = Vec::new();

        for entity in &sorted {
            let mut uniforms = CompositeUniforms::default();
            uniforms.transform = entity.resolved.world_matrix.0;
            uniforms.opacity = entity.resolved.opacity;

            // Determine source type
            let has_color = entity.components.color_source.is_some();
            let has_texture = self.texture_cache.contains_key(&entity.id);

            if !has_color && !has_texture {
                continue; // No visual source
            }

            if let Some(ref color_src) = entity.components.color_source {
                uniforms.color = [
                    color_src.color.r,
                    color_src.color.g,
                    color_src.color.b,
                    color_src.color.a,
                ];
                uniforms.use_texture = 0.0;
            } else {
                uniforms.use_texture = 1.0;
            }

            // Create per-entity uniform buffer
            let ub = self.engine.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("uniform_{}", entity.id)),
                    contents: bytemuck::cast_slice(&[uniforms]),
                    usage: wgpu::BufferUsages::UNIFORM,
                },
            );

            // Pick texture view
            let tex_view = if has_texture {
                &self.texture_cache.get(&entity.id).unwrap().1
            } else {
                &self.composite.white_texture_view
            };

            // Create per-entity bind group
            let bg = self.engine.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("bg_{}", entity.id)),
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

            draw_calls.push(DrawCall { bind_group: bg });
        }

        // Now do the render pass with pre-built bind groups
        let mut encoder = self
            .engine
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

        // Copy output texture to staging buffer for readback
        let buffer_size = (self.width * self.height * 4) as u64;
        let padded_bytes_per_row = Self::padded_bytes_per_row(self.width);

        let staging = self
            .engine
            .device
            .create_buffer(&wgpu::BufferDescriptor {
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

        // Remove padding from rows
        let unpadded_bytes_per_row = self.width * 4;
        let mut pixels = Vec::with_capacity(buffer_size as usize);
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
        (unpadded + align - 1) / align * align
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
