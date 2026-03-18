//! EffectContext — manages input/output textures for effect chaining.
//!
//! Uses a ping-pong pattern: effects read from texture A, write to texture B,
//! then swap for the next effect in the chain.

/// Context passed to each effect during execution.
pub struct EffectContext {
    /// Ping-pong texture pair.
    textures: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    /// Which texture is currently the "input" (0 or 1).
    current: usize,
    pub width: u32,
    pub height: u32,
    pub sampler: wgpu::Sampler,
}

impl EffectContext {
    /// Create a new effect context with ping-pong textures.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let create_texture = |label: &str| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };

        let tex_a = create_texture("effect_ping");
        let tex_b = create_texture("effect_pong");
        let view_a = tex_a.create_view(&Default::default());
        let view_b = tex_b.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("effect sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            textures: [tex_a, tex_b],
            views: [view_a, view_b],
            current: 0,
            width,
            height,
            sampler,
        }
    }

    /// Get the current input texture view (read from this).
    pub fn input_view(&self) -> &wgpu::TextureView {
        &self.views[self.current]
    }

    /// Get the current output texture view (write to this).
    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.views[1 - self.current]
    }

    /// Get the current input texture (for copy operations).
    pub fn input_texture(&self) -> &wgpu::Texture {
        &self.textures[self.current]
    }

    /// Get the current output texture (for copy operations).
    pub fn output_texture(&self) -> &wgpu::Texture {
        &self.textures[1 - self.current]
    }

    /// Swap input/output after an effect pass.
    pub fn swap(&mut self) {
        self.current = 1 - self.current;
    }

    /// Copy the composite output into the ping-pong input texture.
    pub fn load_from(&self, encoder: &mut wgpu::CommandEncoder, source: &wgpu::Texture) {
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.textures[self.current],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Copy the final result back to the output texture.
    pub fn store_to(&self, encoder: &mut wgpu::CommandEncoder, dest: &wgpu::Texture) {
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.textures[self.current],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: dest,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }
}
