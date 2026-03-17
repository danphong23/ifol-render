//! GPU resource manager — texture and buffer pooling.
//!
//! Prevents per-frame allocation by reusing GPU resources.

use std::collections::HashMap;

/// Manages GPU textures and buffers with pooling.
pub struct ResourceManager {
    texture_pool: HashMap<String, wgpu::Texture>,
    // TODO: buffer pool, bind group cache, sampler cache
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            texture_pool: HashMap::new(),
        }
    }

    /// Get or create a texture from the pool.
    pub fn get_or_create_texture(
        &mut self,
        device: &wgpu::Device,
        key: &str,
        desc: &wgpu::TextureDescriptor,
    ) -> &wgpu::Texture {
        self.texture_pool.entry(key.to_string()).or_insert_with(|| {
            device.create_texture(desc)
        })
    }

    /// Release a texture back to the pool.
    pub fn release_texture(&mut self, key: &str) {
        self.texture_pool.remove(key);
    }

    /// Release all resources.
    pub fn clear(&mut self) {
        self.texture_pool.clear();
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}
