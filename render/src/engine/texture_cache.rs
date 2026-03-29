//! Descriptor-based GPU Texture Cache (Bevy-style).
//!
//! Reuses wgpu::Texture objects by exact descriptor match:
//! same (width, height, format, usage) → reuse existing texture.
//!
//! This avoids per-frame GPU allocations for offscreen render targets
//! used by per-entity material effects (blur, glow, shadow, etc.).
//!
//! # Design Rationale (from engine research)
//!
//! - **Bevy**: Uses `TextureCache` with exact `TextureDescriptor` matching.
//!   No "tier/slab" rounding — simpler, no pixel garbage issues.
//! - **Unity URP**: `RenderTexture` pool with descriptor matching.
//! - For a video editor, entity sizes are stable frame-to-frame,
//!   so exact-match hit rates are naturally very high.

use std::collections::HashMap;

/// Hashable key for matching texture descriptors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureKey {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub usage: u32, // wgpu::TextureUsages bits
}

impl TextureKey {
    pub fn new(width: u32, height: u32, format: wgpu::TextureFormat, usage: wgpu::TextureUsages) -> Self {
        Self {
            width,
            height,
            format,
            usage: usage.bits(),
        }
    }

    /// Standard key for offscreen render targets (TEXTURE_BINDING + RENDER_ATTACHMENT).
    pub fn render_target(width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self::new(
            width,
            height,
            format,
            wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
        )
    }
}

struct CacheEntry {
    texture: wgpu::Texture,
    key: TextureKey,
    /// Frame when this entry was last acquired.
    last_used_frame: u64,
    /// Whether currently in use this frame.
    in_use: bool,
}

/// Cache statistics for monitoring.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total textures in pool (active + idle).
    pub total_count: usize,
    /// Textures currently in use this frame.
    pub active_count: usize,
    /// Textures idle (available for reuse).
    pub idle_count: usize,
    /// Estimated total VRAM in bytes.
    pub total_bytes: u64,
    /// Number of unique descriptor keys.
    pub bucket_count: usize,
}

/// Descriptor-based GPU texture cache.
///
/// Textures are grouped by their `TextureKey` (exact descriptor match).
/// Within each group, idle textures are reused before creating new ones.
pub struct TextureCache {
    /// Map from descriptor key → list of entries.
    buckets: HashMap<TextureKey, Vec<usize>>,
    /// Flat storage of all entries.
    entries: Vec<CacheEntry>,
    /// Current frame number.
    current_frame: u64,
}

impl TextureCache {
    pub fn new() -> Self {
        Self {
            buckets: HashMap::new(),
            entries: Vec::new(),
            current_frame: 0,
        }
    }

    /// Begin a new frame. All textures become available for reuse.
    ///
    /// Call this at the start of each render frame, before any `acquire()`.
    pub fn begin_frame(&mut self) {
        self.current_frame += 1;
        for entry in &mut self.entries {
            entry.in_use = false;
        }
    }

    /// Acquire a texture matching the given key.
    ///
    /// If an idle texture with matching descriptor exists, reuse it.
    /// Otherwise, create a new one via the device.
    ///
    /// Returns a reference to the `wgpu::Texture`.
    pub fn acquire(
        &mut self,
        device: &wgpu::Device,
        key: TextureKey,
    ) -> &wgpu::Texture {
        // Look for an idle entry in the matching bucket
        if let Some(indices) = self.buckets.get(&key) {
            for &idx in indices {
                if !self.entries[idx].in_use {
                    self.entries[idx].in_use = true;
                    self.entries[idx].last_used_frame = self.current_frame;
                    return &self.entries[idx].texture;
                }
            }
        }

        // No idle match — create new texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TextureCache entry"),
            size: wgpu::Extent3d {
                width: key.width.max(1),
                height: key.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: key.format,
            usage: wgpu::TextureUsages::from_bits_truncate(key.usage),
            view_formats: &[],
        });
        
        let idx = self.entries.len();
        self.entries.push(CacheEntry {
            texture,
            key,
            last_used_frame: self.current_frame,
            in_use: true,
        });

        self.buckets.entry(key).or_default().push(idx);

        &self.entries[idx].texture
    }

    /// Evict textures that haven't been used for `max_idle_frames` frames.
    ///
    /// Call periodically (e.g., every 60 frames) to prevent VRAM leaks.
    pub fn cleanup(&mut self, max_idle_frames: u64) {
        let current = self.current_frame;
        let mut to_remove = Vec::new();

        for (idx, entry) in self.entries.iter().enumerate() {
            if !entry.in_use && current > entry.last_used_frame + max_idle_frames {
                to_remove.push(idx);
            }
        }

        if to_remove.is_empty() {
            return;
        }

        // Remove from buckets and entries (reverse order to preserve indices)
        for &idx in to_remove.iter().rev() {
            let key = self.entries[idx].key;

            // Remove from bucket
            if let Some(indices) = self.buckets.get_mut(&key) {
                indices.retain(|&i| i != idx);
                if indices.is_empty() {
                    self.buckets.remove(&key);
                }
            }

            // Drop the texture (GPU memory freed)
            self.entries.swap_remove(idx);

            // Update bucket indices that were affected by swap_remove
            if idx < self.entries.len() {
                let swapped_key = self.entries[idx].key;
                let old_idx = self.entries.len(); // was at the end before swap_remove
                if let Some(indices) = self.buckets.get_mut(&swapped_key) {
                    for i in indices.iter_mut() {
                        if *i == old_idx {
                            *i = idx;
                        }
                    }
                }
            }
        }

        if !to_remove.is_empty() {
            log::info!("TextureCache: evicted {} idle textures", to_remove.len());
        }
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let active = self.entries.iter().filter(|e| e.in_use).count();
        let total_bytes: u64 = self.entries.iter().map(|e| {
            let bpp = bytes_per_pixel(e.key.format);
            (e.key.width as u64) * (e.key.height as u64) * bpp
        }).sum();

        CacheStats {
            total_count: self.entries.len(),
            active_count: active,
            idle_count: self.entries.len() - active,
            total_bytes,
            bucket_count: self.buckets.len(),
        }
    }
}

/// Estimate bytes per pixel for common texture formats.
fn bytes_per_pixel(format: wgpu::TextureFormat) -> u64 {
    match format {
        wgpu::TextureFormat::Rgba8Unorm
        | wgpu::TextureFormat::Rgba8UnormSrgb
        | wgpu::TextureFormat::Bgra8Unorm
        | wgpu::TextureFormat::Bgra8UnormSrgb => 4,
        wgpu::TextureFormat::Rgba16Float => 8,
        wgpu::TextureFormat::Rgba32Float => 16,
        _ => 4, // conservative default
    }
}
