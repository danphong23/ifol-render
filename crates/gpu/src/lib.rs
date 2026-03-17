//! # ifol-render-gpu
//!
//! GPU rendering backend using wgpu. Provides:
//! - **Engine**: wgpu device/queue initialization
//! - **Render Graph**: DAG-based pass execution
//! - **Resource Manager**: GPU texture/buffer pooling
//! - **Shader Runtime**: WGSL loading, compilation, caching
//! - **Render Passes**: Composite, text, effects

pub mod engine;
pub mod passes;
pub mod render_graph;
pub mod resource_manager;

use ifol_render_core::ecs::World;
use ifol_render_core::scene::RenderSettings;
use ifol_render_core::time::TimeState;

/// The GPU renderer — owns the wgpu context and render resources.
pub struct Renderer {
    engine: engine::GpuEngine,
    // render_graph: render_graph::RenderGraph,
    // resource_manager: resource_manager::ResourceManager,
}

impl Renderer {
    /// Create a new headless renderer (for CLI/backend).
    pub fn new_headless(settings: &RenderSettings) -> Self {
        let engine = pollster::block_on(engine::GpuEngine::new_headless(
            settings.width,
            settings.height,
        ));
        Self { engine }
    }

    /// Render a single frame, returning RGBA pixel data.
    pub fn render_frame(&mut self, world: &World, _time: &TimeState) -> Vec<u8> {
        let sorted = world.sorted_by_layer();

        // TODO: Build render graph from sorted entities
        // TODO: Execute render passes
        // TODO: Read pixels from output texture

        // Placeholder: return empty frame
        let size = (self.engine.width * self.engine.height * 4) as usize;
        let mut pixels = vec![0u8; size];

        // Clear to dark background
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[0] = 30; // R
            chunk[1] = 30; // G
            chunk[2] = 40; // B
            chunk[3] = 255; // A
        }

        // TODO: For each entity in sorted order:
        // 1. Load/cache texture (video frame, image, or text render)
        // 2. Create draw command with world_matrix and opacity
        // 3. Apply effects (shader pipeline)
        // 4. Composite to output

        let _ = sorted;
        pixels
    }
}
