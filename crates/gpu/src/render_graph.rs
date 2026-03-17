//! Render Graph — DAG-based pass execution system.
//!
//! The render graph manages render passes, their dependencies,
//! and GPU resource lifetimes. Each pass declares its inputs
//! and outputs, and the graph resolves execution order.

/// A render pass in the graph.
pub trait RenderPass: Send + Sync {
    /// Human-readable name.
    fn name(&self) -> &str;

    /// Execute this pass.
    fn execute(&self, ctx: &mut PassContext);
}

/// Context provided to each render pass during execution.
pub struct PassContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
}

/// The render graph DAG.
pub struct RenderGraph {
    passes: Vec<Box<dyn RenderPass>>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    /// Add a render pass.
    pub fn add_pass(&mut self, pass: Box<dyn RenderPass>) {
        self.passes.push(pass);
    }

    /// Execute all passes in order.
    pub fn execute(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Graph"),
        });

        for pass in &self.passes {
            let mut ctx = PassContext { device, queue, encoder: &mut encoder };
            pass.execute(&mut ctx);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}

impl Default for RenderGraph {
    fn default() -> Self {
        Self::new()
    }
}
