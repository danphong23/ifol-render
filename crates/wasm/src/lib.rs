use ifol_render_core::engine::CoreEngine;
use ifol_render_core::frame::{Frame, RenderSettings};
use ifol_render_core::PipelineConfig;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

mod web_backend;
use web_backend::WebMediaBackend;

mod media_manager;
use media_manager::WasmMediaManager;

#[wasm_bindgen]
pub struct IfolRenderWeb {
    engine: CoreEngine,
    backend: WebMediaBackend,
    /// Ring buffer of pre-computed frames for batch streaming (V1 Legacy).
    frame_buffer: Vec<Frame>,
    
    // ── V2 Stateful ECS ──
    v2_world: Option<ifol_render_ecs::ecs::World>,
    v2_asset_mgr: Option<ifol_render_ecs::assets::AssetManager>,

    media_manager: WasmMediaManager,

    /// Currently selected entity IDs (for rendering selection outlines)
    selected_entity_ids: Vec<String>,
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
        let _ = wasm_logger::init(wasm_logger::Config::new(log::Level::Info));

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
            v2_world: None,
            v2_asset_mgr: Some(ifol_render_ecs::assets::AssetManager::new(2.0)),
            media_manager: WasmMediaManager::new(),
            selected_entity_ids: Vec::new(),
        })
    }

    // ── Asset Cache ──────────────────────────

    /// Pre-inject raw bytes for an image or font asset.
    pub fn cache_image(&self, path: &str, data: &[u8]) {
        self.backend
            .images
            .write()
            .unwrap()
            .insert(path.to_string(), data.to_vec());
    }

    /// Pre-inject a decoded video frame as raw RGBA pixels with dimensions.
    pub fn cache_video_frame(
        &self,
        path: &str,
        timestamp: f64,
        data: &[u8],
        width: u32,
        height: u32,
    ) {
        self.backend.video_frames.write().unwrap().insert(
            format!("{}@{}", path, timestamp),
            (data.to_vec(), width, height),
        );
    }

    /// Clear ALL cached video frames from WASM memory.
    pub fn clear_video_frames(&mut self) {
        self.backend.video_frames.write().unwrap().clear();
        self.media_manager.clear();
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

    /// Register a custom entity shader (quad-based, like composite/shapes).
    /// WGSL must define vs_main and fs_main entry points.
    pub fn register_shader(&mut self, name: &str, wgsl_code: &str) -> Result<(), JsValue> {
        if self.engine.has_shader(name) {
            return Err(JsValue::from_str(&format!("Shader '{}' already registered", name)));
        }
        self.engine.register_shader(name, wgsl_code, PipelineConfig::quad());
        log::info!("Custom shader registered: '{}'", name);
        Ok(())
    }

    /// Register a custom fullscreen effect shader (like blur/vignette).
    /// `param_names` is a comma-separated list of float uniform names.
    /// WGSL must define vs_fullscreen and fs_main entry points.
    pub fn register_effect(&mut self, name: &str, wgsl_code: &str, param_names: &str) -> Result<(), JsValue> {
        if self.engine.has_shader(name) {
            return Err(JsValue::from_str(&format!("Effect '{}' already registered", name)));
        }
        let defaults: Vec<(String, f32)> = param_names
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| (s.trim().to_string(), 0.0))
            .collect();
        let pass_count = 1;
        self.engine.register_effect(name, wgsl_code, defaults, pass_count);
        log::info!("Custom effect registered: '{}'", name);
        Ok(())
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
            frame.scaled(
                rw as f64 / scene_width as f64,
                rh as f64 / scene_height as f64,
            )
        } else {
            frame
        };

        self.engine.render_frame(&rendered);
        Ok(())
    }

    // ══════════════════════════════════════════════════════════
    // V2 Stateful Render API
    // ══════════════════════════════════════════════════════════

    /// Load a complete V2 Scene Graph into WASM memory.
    /// This replaces the V1 frame-by-frame paradigm.
    pub fn load_scene_v2(&mut self, scene_json: &str) -> Result<(), JsValue> {
        let scene: ifol_render_ecs::scene::SceneV2 = serde_json::from_str(scene_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid SceneV2 JSON: {}", e)))?;
        
        let mut world = ifol_render_ecs::ecs::World::new();
        world.load_scene(&scene);
        log::info!("V2 Scene loaded: {} entities in ECS World.", world.entities.len());
        self.v2_world = Some(world);
        Ok(())
    }

    /// Patch the scene with delta changes (drag an entity, change color)
    /// without reloading the entire heavy JSON graph.
    pub fn patch_scene_v2(&mut self, _delta_json: &str) -> Result<(), JsValue> {
        // Future: parse JSON Patch or custom Delta format and update self.v2_world
        log::info!("V2 Scene patched.");
        Ok(())
    }

    /// Render exactly one frame evaluated at `time_sec` from the given `camera_id` perspective.
    pub fn render_frame_v2(
        &mut self,
        time_sec: f64,
        camera_id: &str,
        custom_cam_x: Option<f32>,
        custom_cam_y: Option<f32>,
        custom_cam_w: Option<f32>,
        custom_cam_h: Option<f32>,
    ) -> Result<(), JsValue> {
        if self.v2_world.is_none() {
            return Err(JsValue::from_str("No V2 scene loaded. Call load_scene_v2 first."));
        }
        
        // 1. Evaluate ECS timeline and animation systems at `time_sec`
        let time_state = ifol_render_ecs::time::TimeState {
            global_time: time_sec,
            delta_time: 1.0 / 60.0, // Should be computed dynamically based on the loop
            frame_index: (time_sec * 60.0) as u64,
            fps: 60.0,
        };
        
        let mut world = self.v2_world.take().unwrap();
        ifol_render_ecs::ecs::pipeline::run(&mut world, &time_state);
        
        // 2. Video frames are now injected from JS via cache_video_frame() API.
        //    WasmMediaManager disabled: async <video> seeking doesn't work in sync render.
        // self.media_manager.update_scene_videos(&world, time_sec, &self.backend);
        
        for entity in world.entities.iter() {
            if let Some(img) = &entity.components.image_source {
                let url = world.resolve_asset_url(&img.asset_id)
                    .unwrap_or(&img.asset_id);
                // The URL bytes must be preloaded in backend via `cache_image` API from JS 
                // OR ideally fetched here in media_manager. For now we just load the bytes if they exist.
                let _ = self.engine.load_image(url, url);
            }
        }
        
        // 3. Compile World to Frame
        let w = self.engine.settings().width;
        let h = self.engine.settings().height;
        let frame = ifol_render_core::compiler::compile_world_to_frame(
            &world, camera_id, w, h, time_sec,
            custom_cam_x, custom_cam_y, custom_cam_w, custom_cam_h,
            self.selected_entity_ids.iter().map(|s| s.as_str()).collect::<Vec<_>>().as_slice(),
        );
        
        self.v2_world = Some(world);
        
        // 4. Send to WGPU engine
        self.engine.render_frame(&frame);
        
        Ok(())
    }

    // ── Batch Frame Streaming API (Legacy V1) ────────────

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

        log::info!(
            "Pushed {} frames, buffer now has {}",
            count,
            self.frame_buffer.len()
        );
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

        log::info!(
            "Pushed {} frames (scaled), buffer now has {}",
            count,
            self.frame_buffer.len()
        );
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

    // ── Selection ────────────────────────────

    /// Set the currently selected entity IDs for rendering selection outlines.
    /// Pass a comma-separated string of entity IDs, or None/empty to clear.
    pub fn set_selection(&mut self, entity_ids: Option<String>) {
        self.selected_entity_ids = entity_ids
            .map(|s| s.split(',').filter(|id| !id.is_empty()).map(|id| id.trim().to_string()).collect())
            .unwrap_or_default();
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
