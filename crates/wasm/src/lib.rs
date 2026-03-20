use ifol_render_core::engine::CoreEngine;
use ifol_render_core::frame::{Frame, RenderSettings};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

mod web_backend;
use web_backend::WebMediaBackend;

#[wasm_bindgen]
pub struct IfolRenderWeb {
    engine: CoreEngine,
    backend: WebMediaBackend,
}

#[wasm_bindgen]
impl IfolRenderWeb {
    /// Create a new renderer attached to a canvas element.
    /// Note: The canvas must already exist in the DOM.
    #[wasm_bindgen(constructor)]
    pub async fn new(
        canvas: HtmlCanvasElement,
        width: u32,
        height: u32,
        fps: f64,
    ) -> Result<IfolRenderWeb, JsValue> {
        // Initialize logging so we can see wgpu/ifol-render panic messages in the JS console!
        console_error_panic_hook::set_once();
        
        // This initializes env_logger -> console.log
        let _ = wasm_logger::init(wasm_logger::Config::default());

        log::info!("Initializing WebGPU on canvas size {}x{}", width, height);

        let settings = RenderSettings {
            width,
            height,
            fps,
            background: [0.0, 0.0, 0.0, 1.0],
        };

        let backend = WebMediaBackend::new();
        let engine = CoreEngine::new_web(canvas, settings, Box::new(backend.clone())).await;
        
        // Register standard built-in shader pipelines on the GPU
        // engine.setup_builtins(); // Need to call if engine is mut

        Ok(Self { engine, backend })
    }

    /// Pre-inject raw bytes for an image or font asset.
    pub fn cache_image(&self, path: &str, data: &[u8]) {
        self.backend.images.write().unwrap().insert(path.to_string(), data.to_vec());
    }

    /// Pre-inject a decoded video frame as raw RGBA pixels with dimensions.
    pub fn cache_video_frame(&self, path: &str, timestamp: f64, data: &[u8], width: u32, height: u32) {
        self.backend.video_frames.write().unwrap().insert(
            format!("{}@{}", path, timestamp), 
            (data.to_vec(), width, height)
        );
    }

    /// Setup the pipeline standard builtins (Call this AFTER caching the fonts!)
    pub fn setup_builtins(&mut self) {
        self.engine.setup_builtins();
    }

    /// Render a single pre-calculated `Frame` object natively.
    pub fn render_frame(&mut self, frame_json: &str) -> Result<(), JsValue> {
        let frame: Frame = serde_json::from_str(frame_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid Frame JSON: {}", e)))?;

        // Process the frame (this bypasses CPU readback and renders directly to the canvas Surface)
        self.engine.render_frame(&frame);
        Ok(())
    }

    /// Update the resolution dynamically
    pub fn resize(&mut self, width: u32, height: u32) {
        self.engine.resize(width, height);
    }
}
