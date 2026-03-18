//! GPU Engine — owns wgpu context. Sealed, never modified.
//!
//! This is the foundation layer. Everything above (pipeline, effects)
//! borrows device/queue from here.

pub mod gpu;

/// Core GPU engine — owns the wgpu instance, adapter, device, and queue.
pub struct GpuEngine {
    #[allow(dead_code)]
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub width: u32,
    pub height: u32,
    /// Output texture for headless rendering.
    pub output_texture: Option<wgpu::Texture>,
}

impl GpuEngine {
    /// Create a headless GPU engine (no window surface).
    pub async fn new_headless(width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find GPU adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("ifol-render device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .expect("Failed to create GPU device");

        let output_texture = Some(Self::create_output_texture(&device, width, height));

        log::info!("GPU engine initialized: {:?}", adapter.get_info().name);

        Self {
            instance,
            adapter,
            device,
            queue,
            width,
            height,
            output_texture,
        }
    }

    /// Resize the output texture.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.output_texture = Some(Self::create_output_texture(&self.device, width, height));
        log::info!("Resized output to {}x{}", width, height);
    }

    fn create_output_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    /// Readback pixels from output texture to CPU.
    pub fn readback_output(&self) -> Vec<u8> {
        let texture = self.output_texture.as_ref().unwrap();
        self.readback_texture(texture, self.width, self.height)
    }

    /// Readback pixels from any GPU texture to CPU.
    pub fn readback_texture(&self, texture: &wgpu::Texture, width: u32, height: u32) -> Vec<u8> {
        let padded_bytes_per_row = Self::padded_bytes_per_row(width);
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
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
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        let buffer_slice = staging.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();
        let unpadded_bytes_per_row = width * 4;
        let buffer_size = (width * height * 4) as usize;
        let mut pixels = Vec::with_capacity(buffer_size);
        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            pixels.extend_from_slice(&data[start..end]);
        }

        drop(data);
        staging.unmap();
        pixels
    }

    /// Calculate padded bytes per row (wgpu requires 256-byte alignment).
    pub fn padded_bytes_per_row(width: u32) -> u32 {
        let unpadded = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        unpadded.div_ceil(align) * align
    }
}
