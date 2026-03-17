//! WASM bindings for ifol-render.
//!
//! Provides JavaScript API for browser-based preview rendering via WebGPU.

use wasm_bindgen::prelude::*;

/// The main WASM API entry point.
#[wasm_bindgen]
pub struct IfolRender {
    // TODO: hold wgpu surface + renderer
}

#[wasm_bindgen]
impl IfolRender {
    /// Create a new renderer attached to a canvas element.
    #[wasm_bindgen(constructor)]
    pub fn new(_canvas_id: &str) -> Result<IfolRender, JsValue> {
        // TODO: Get canvas, create wgpu surface, init renderer
        Ok(Self {})
    }

    /// Load a scene from JSON.
    pub fn load_scene(&mut self, _scene_json: &str) {
        // TODO: Parse SceneDescription, build World
    }

    /// Render a single frame at the given timestamp.
    pub fn render_frame(&mut self, _timestamp: f64) {
        // TODO: Run ECS pipeline, execute render graph, present to canvas
    }

    /// Register a custom shader.
    pub fn register_shader(&mut self, _id: &str, _wgsl_code: &str) {
        // TODO: Compile and cache shader
    }

    /// Update an entity's component data.
    pub fn update_entity(&mut self, _entity_id: &str, _component_json: &str) {
        // TODO: Parse and update component
    }
}
