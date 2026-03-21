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
    /// Ring buffer of pre-computed frames for batch streaming.
    frame_buffer: Vec<Frame>,
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
        
        Ok(Self {
            engine,
            backend,
            frame_buffer: Vec::new(),
        })
    }

    // ── Asset Cache ──────────────────────────

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

    /// Clear ALL cached video frames from WASM memory.
    /// Call this periodically during playback to prevent unbounded memory growth.
    /// (Each 1280×720 frame = 3.7MB, hundreds accumulate quickly.)
    pub fn clear_video_frames(&self) {
        self.backend.video_frames.write().unwrap().clear();
    }

    /// Remove a specific texture from the GPU cache.
    pub fn evict_texture(&mut self, key: &str) {
        self.engine.evict_texture(key);
    }

    // ── Setup ────────────────────────────────

    /// Setup the pipeline standard builtins (Call this AFTER caching the fonts!)
    pub fn setup_builtins(&mut self) {
        self.engine.setup_builtins();
    }

    // ── Single-Frame Render (backward compatible) ──

    /// Render a single pre-calculated `Frame` object natively.
    pub fn render_frame(&mut self, frame_json: &str) -> Result<(), JsValue> {
        let frame: Frame = serde_json::from_str(frame_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid Frame JSON: {}", e)))?;

        // Process the frame (this bypasses CPU readback and renders directly to the canvas Surface)
        self.engine.render_frame(&frame);
        Ok(())
    }

    /// Render a frame with automatic coordinate scaling.
    ///
    /// JSON pixel coords are authored at `scene_width × scene_height` (export resolution).
    /// If the engine's current render size differs (e.g. preview at 1280×720),
    /// this method scales all entity coordinates proportionally before rendering.
    pub fn render_frame_scaled(
        &mut self,
        frame_json: &str,
        scene_width: u32,
        scene_height: u32,
    ) -> Result<(), JsValue> {
        let frame: Frame = serde_json::from_str(frame_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid Frame JSON: {}", e)))?;

        let rw = self.engine.settings().width;
        let rh = self.engine.settings().height;

        let rendered = if rw != scene_width || rh != scene_height {
            frame.scaled(rw as f64 / scene_width as f64, rh as f64 / scene_height as f64)
        } else {
            frame
        };

        self.engine.render_frame(&rendered);
        Ok(())
    }

    // ── Batch Frame Streaming API ────────────

    /// Push a batch of pre-computed frames into the internal buffer.
    ///
    /// `frames_json` must be a JSON array of Frame objects: `[{passes:..., texture_updates:...}, ...]`
    /// Frames are APPENDED to the existing buffer (call `clear_frames()` first if replacing).
    /// Returns the total number of frames now buffered.
    ///
    /// Typical usage pattern:
    /// 1. Frontend flattens N frames from playhead (time-budgeted)
    /// 2. `push_frames(batch)` — append to buffer
    /// 3. `render_at(index)` — render during playback
    /// 4. When viewport/entity changes: `clear_frames()` → re-push
    pub fn push_frames(&mut self, frames_json: &str) -> Result<u32, JsValue> {
        let frames: Vec<Frame> = serde_json::from_str(frames_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid frames JSON: {}", e)))?;
        
        let count = frames.len();
        self.frame_buffer.extend(frames);
        
        log::info!("Pushed {} frames, buffer now has {}", count, self.frame_buffer.len());
        Ok(self.frame_buffer.len() as u32)
    }

    /// Push a batch of frames with automatic coordinate scaling.
    ///
    /// Same as `push_frames` but scales entity coords from `scene_width × scene_height`
    /// to the current engine render resolution.
    pub fn push_frames_scaled(
        &mut self,
        frames_json: &str,
        scene_width: u32,
        scene_height: u32,
    ) -> Result<u32, JsValue> {
        let frames: Vec<Frame> = serde_json::from_str(frames_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid frames JSON: {}", e)))?;
        
        let rw = self.engine.settings().width;
        let rh = self.engine.settings().height;
        let needs_scale = rw != scene_width || rh != scene_height;
        
        let count = frames.len();
        if needs_scale {
            let sx = rw as f64 / scene_width as f64;
            let sy = rh as f64 / scene_height as f64;
            for frame in frames {
                self.frame_buffer.push(frame.scaled(sx, sy));
            }
        } else {
            self.frame_buffer.extend(frames);
        }
        
        log::info!("Pushed {} frames (scaled), buffer now has {}", count, self.frame_buffer.len());
        Ok(self.frame_buffer.len() as u32)
    }

    /// Render a frame from the buffer at the given index.
    ///
    /// Processes texture updates (video decode, image load, text raster) then renders
    /// to the canvas. Returns `false` if the index is out of range.
    pub fn render_at(&mut self, index: u32) -> bool {
        let idx = index as usize;
        if idx >= self.frame_buffer.len() {
            return false;
        }
        
        // Clone the frame to satisfy borrow checker (frame_buffer borrowed, engine needs &mut self)
        let frame = self.frame_buffer[idx].clone();
        self.engine.render_frame(&frame);
        true
    }

    /// Clear all buffered frames. Call when viewport/entity changes
    /// invalidate the pre-computed batch (zoom, pan, entity drag, seek).
    pub fn clear_frames(&mut self) {
        let prev = self.frame_buffer.len();
        self.frame_buffer.clear();
        if prev > 0 {
            log::info!("Cleared frame buffer ({} frames dropped)", prev);
        }
    }

    /// Get the number of frames currently in the buffer.
    pub fn buffered_count(&self) -> u32 {
        self.frame_buffer.len() as u32
    }

    // ── Resize ───────────────────────────────

    /// Update the resolution dynamically.
    ///
    /// **Important**: This clears the frame buffer because all pre-computed
    /// pixel coordinates are invalid at the new resolution.
    /// After calling resize(), push new frames computed for the new size.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.engine.resize(width, height);
        // Frame buffer coords are invalid at new resolution → must clear
        self.clear_frames();
        log::info!("Resized to {}x{}, frame buffer cleared", width, height);
    }
}
