//! Composite pass — draws textured quads with transform and opacity.

use super::super::render_graph::{PassContext, RenderPass};

pub struct CompositePass;

impl RenderPass for CompositePass {
    fn name(&self) -> &str {
        "composite"
    }

    fn execute(&self, _ctx: &mut PassContext) {
        // TODO: For each visible entity (sorted by layer):
        // 1. Bind entity's texture
        // 2. Set transform matrix uniform
        // 3. Set opacity uniform
        // 4. Draw fullscreen quad with passthrough/effect shader
        log::trace!("CompositePass executed");
    }
}
