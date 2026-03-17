//! Clear pass — clears the output framebuffer.

use super::super::render_graph::{PassContext, RenderPass};

pub struct ClearPass {
    pub color: wgpu::Color,
}

impl RenderPass for ClearPass {
    fn name(&self) -> &str {
        "clear"
    }

    fn execute(&self, _ctx: &mut PassContext) {
        // TODO: create render pass with clear color on the output texture
        log::trace!("ClearPass executed");
    }
}
