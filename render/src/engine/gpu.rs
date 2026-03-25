//! GPU capabilities and hardware detection.

/// GPU hardware capabilities — render detects, outside reads.
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
    pub gpu_name: String,
    pub backend: String,
    pub max_texture_size: u32,
    pub max_buffer_size: u64,
}

impl GpuCapabilities {
    /// Detect capabilities from wgpu adapter.
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        let info = adapter.get_info();
        let limits = adapter.limits();

        Self {
            gpu_name: info.name.clone(),
            backend: format!("{:?}", info.backend),
            max_texture_size: limits.max_texture_dimension_2d,
            max_buffer_size: limits.max_buffer_size,
        }
    }
}
