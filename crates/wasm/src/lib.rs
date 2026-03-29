use ifol_render_core::engine::CoreEngine;
use ifol_render_core::frame::{Frame, RenderSettings};
use ifol_render_core::PipelineConfig;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;


mod web_backend;
use web_backend::WebMediaBackend;

mod media_manager;
use media_manager::WasmMediaManager;

mod audio_manager;
use audio_manager::WasmAudioManager;

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
    audio_manager: WasmAudioManager,
    is_playing: bool,

    /// Currently selected entity IDs (for rendering selection outlines)
    selected_entity_ids: Vec<String>,
    /// Render scope: if set, only render descendants of this entity
    render_scope: Option<String>,
    /// Scope time override: local time for scoped composition (bypasses speed/loop/trim)
    scope_time: Option<f64>,
    /// Visual style for selected entities ("rect" or "content")
    select_mode: String,
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
            hdr_enabled: false,
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
            audio_manager: WasmAudioManager::new(),
            is_playing: false,
            selected_entity_ids: Vec::new(),
            render_scope: None,
            scope_time: None,
            select_mode: "rect".to_string(),
        })
    }

    // ── Asset Cache ──────────────────────────

    /// Inject decoded RGBA pixels into WASM memory.
    pub fn cache_image(&self, key: &str, data: &[u8], width: u32, height: u32) {
        let mut images = self.backend.images.write().unwrap();
        images.insert(key.to_string(), (data.to_vec(), width, height));
    }

    /// Inject Font TTF Bytes directly into WASM RAM bypassing the FileSystem.
    pub fn cache_font(&mut self, key: &str, data: &[u8]) {
        self.engine.load_font_bytes(key, data.to_vec());
        log::info!("Font '{}' cached in WASM memory.", key);
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

    /// Clear ALL cached video frames and audio elements from WASM memory.
    pub fn clear_media_cache(&mut self) {
        self.backend.video_frames.write().unwrap().clear();
        self.backend.images.write().unwrap().clear();
        self.media_manager.clear();
        self.audio_manager.clear();
        self.engine.clear_textures();
        log::info!("Media cache successfully cleared.");
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

    #[wasm_bindgen]
    pub fn parse_v2_json(scene_json: &str) -> String {
        match serde_json::from_str::<ifol_render_ecs::scene::SceneV2>(scene_json) {
            Ok(scene) => format!("Success. Parsed {} assets.", scene.assets.len()),
            Err(e) => format!("Error parsing: {}", e)
        }
    }

    /// Render exactly one frame evaluated at `time_sec` from the given `camera_id` perspective.
    pub fn render_frame_v2(
        &mut self,
        time_sec: f64,
        camera_id: &str,
        is_editor_mode: bool,
        custom_cam_x: Option<f32>,
        custom_cam_y: Option<f32>,
        custom_cam_w: Option<f32>,
        custom_cam_h: Option<f32>,
    ) -> Result<JsValue, JsValue> {
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
        ifol_render_ecs::ecs::pipeline::run(
            &mut world,
            &time_state,
            self.render_scope.as_deref(),
            self.scope_time,
        );
        
        // Sync HTML5 <audio> tags with ECS time and volume
        self.audio_manager.sync_audio(&world, self.is_playing);
        
        // ---- Asset Discovery Scans (Buffering & Preload) ----
        let mut buffering_assets = Vec::new();
        let mut preload_assets = Vec::new();
        let mut active_video_entities = std::collections::HashSet::new();
        let preload_window = 3.0; // 3 seconds lookahead
        let mut intrinsic_updates: Vec<(String, f32, f32)> = Vec::new();

        {
            let images_cache = self.backend.images.read().unwrap();
            let storages = &world.storages;
            
            for entity in world.entities.iter() {
                // Preload Scan (Lookahead)
                if let Some(lifespan) = storages.get_component::<ifol_render_ecs::scene::Lifespan>(&entity.id) {
                    if lifespan.start > time_sec && lifespan.start <= time_sec + preload_window {
                        // Gather future assets
                        if let Some(video) = storages.get_component::<ifol_render_ecs::ecs::components::VideoSource>(&entity.id) {
                            let url = world.resolve_asset_url(&video.asset_id).unwrap_or(&video.asset_id).to_string();
                            self.media_manager.preload_video(&entity.id, &url, 0.0);
                            preload_assets.push(format!("video:{}", url));
                        }
                        if let Some(image) = storages.get_component::<ifol_render_ecs::ecs::components::ImageSource>(&entity.id) {
                            let url = world.resolve_asset_url(&image.asset_id).unwrap_or(&image.asset_id).to_string();
                            if !images_cache.contains_key(&url) {
                                preload_assets.push(format!("image:{}", url));
                            }
                        }
                    }
                }

                if !entity.resolved.visible { continue; }
                
                // Current Frame Scan (Buffering & Loading)
                if let Some(video_source) = storages.get_component::<ifol_render_ecs::ecs::components::VideoSource>(&entity.id) {
                    active_video_entities.insert(entity.id.clone());
                    let url = world.resolve_asset_url(&video_source.asset_id).unwrap_or(&video_source.asset_id);
                    let seek_time = entity.resolved.playback_time;
                    
                    if !self.media_manager.is_video_ready(&entity.id, url, seek_time) {
                        buffering_assets.push(format!("video:{}", url));
                    }
                    
                    if let Some((el, w, h)) = self.media_manager.get_video_frame(&entity.id, url, seek_time, self.is_playing) {
                        self.engine.load_video_texture_web(url, &el, w, h);
                        if video_source.intrinsic_width <= 0.0 || video_source.intrinsic_height <= 0.0 {
                            intrinsic_updates.push((entity.id.clone(), w as f32, h as f32));
                        }
                    }
                }
                
                if let Some(img) = storages.get_component::<ifol_render_ecs::ecs::components::ImageSource>(&entity.id) {
                    let asset_key = world.resolve_asset_url(&img.asset_id).unwrap_or(&img.asset_id);
                    if let Some((_rgba, w, h)) = images_cache.get(asset_key) {
                        if img.intrinsic_width <= 0.0 || img.intrinsic_height <= 0.0 {
                            intrinsic_updates.push((entity.id.clone(), *w as f32, *h as f32));
                        }
                        if !self.engine.has_texture(asset_key) {
                            self.engine.load_rgba(asset_key, _rgba, *w, *h);
                        }
                    } else {
                        // Image missing, buffer it
                        buffering_assets.push(format!("image:{}", asset_key));
                    }
                }
            }
        }

        self.media_manager.cleanup_orphaned(&active_video_entities);

        for (id, w, h) in intrinsic_updates {
            if let Some(img) = world.storages.get_component_mut::<ifol_render_ecs::ecs::components::ImageSource>(&id) {
                img.intrinsic_width = w;
                img.intrinsic_height = h;
            } else if let Some(vid) = world.storages.get_component_mut::<ifol_render_ecs::ecs::components::VideoSource>(&id) {
                vid.intrinsic_width = w;
                vid.intrinsic_height = h;
            }
        }
        
        // 3. Compile World to Frame
        let w = self.engine.settings().width;
        let h = self.engine.settings().height;
        
        // 3.1. Editor Phase (Gizmos)
        let selected_refs: Vec<&str> = self.selected_entity_ids.iter().map(|s| s.as_str()).collect();
        // Wait, editor_gizmo_system MUST run AFTER render_to_frame because it appends to the Frame!


        // 3.2. Core Render Phase
        let mut frame = ifol_render_ecs::ecs::systems::render_to_frame(
            &world, camera_id, w, h, time_sec,
            custom_cam_x, custom_cam_y, custom_cam_w, custom_cam_h,
            self.render_scope.as_deref(),
        );
        
        if is_editor_mode {
            let cam = world.find_camera(camera_id);
            let cam_x = custom_cam_x.unwrap_or_else(|| cam.map(|c| c.resolved.x).unwrap_or(0.0));
            let cam_y = custom_cam_y.unwrap_or_else(|| cam.map(|c| c.resolved.y).unwrap_or(0.0));
            let cam_w = custom_cam_w.unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0)).max(1.0);
            let cam_h = custom_cam_h.unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0)).max(1.0);
            
            let sx = w as f32 / cam_w;
            let sy = h as f32 / cam_h;

            ifol_render_ecs::ecs::systems::editor_gizmo_system(
                &world, &mut frame, &selected_refs, &self.select_mode,
                cam_x, cam_y, sx, sy, w, h
            );
        }
        
        self.v2_world = Some(world);
        
        // 4. Send to WGPU engine
        self.engine.render_frame(&frame);
        
        // 5. Build and return the EngineStatus JSON manually
        let mut json = String::from("{");
        
        let status_str = if buffering_assets.is_empty() { "\"ready\"" } else { "\"buffering\"" };
        json.push_str(&format!("\"status\":{},", status_str));
        
        let buff_join = buffering_assets.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(",");
        json.push_str(&format!("\"buffering_assets\":[{}],", buff_join));
        
        let pre_join = preload_assets.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(",");
        json.push_str(&format!("\"preload_assets\":[{}]", pre_join));
        
        json.push('}');

        Ok(JsValue::from_str(&json))
    }

    /// Set the render scope to only show descendants of this entity.
    /// Pass None to show all entities (root scope).
    #[wasm_bindgen]
    pub fn set_render_scope(&mut self, entity_id: Option<String>) {
        self.render_scope = entity_id;
    }

    /// Set engine play state (orchestrates <audio> synced playback)
    #[wasm_bindgen]
    pub fn set_playing(&mut self, is_playing: bool) {
        self.is_playing = is_playing;
    }

    /// Set scope time override (local time for the scoped composition).
    /// When set, the scoped composition's children are evaluated at this
    /// local time directly, bypassing speed/loop/trim mapping.
    #[wasm_bindgen]
    pub fn set_scope_time(&mut self, time: Option<f64>) {
        self.scope_time = time;
    }

    #[wasm_bindgen]
    pub fn select_entity_v2(&mut self, entity_id: Option<String>) {
        self.selected_entity_ids.clear();
        if let Some(id) = entity_id {
            self.selected_entity_ids.push(id);
        }
    }

    #[wasm_bindgen]
    pub fn set_select_mode(&mut self, mode: &str) {
        self.select_mode = mode.to_string();
    }

    #[wasm_bindgen]
    pub fn pick_entity_v2(
        &self,
        screen_x: f32,
        screen_y: f32,
        camera_id: &str,
        custom_cam_x: Option<f32>,
        custom_cam_y: Option<f32>,
        custom_cam_w: Option<f32>,
        custom_cam_h: Option<f32>,
    ) -> Option<String> {
        if let Some(world) = &self.v2_world {
            let cam = world.find_camera(camera_id);
            let cam_x = custom_cam_x.unwrap_or_else(|| cam.map(|c| c.resolved.x).unwrap_or(0.0));
            let cam_y = custom_cam_y.unwrap_or_else(|| cam.map(|c| c.resolved.y).unwrap_or(0.0));
            let cam_w = custom_cam_w.unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0)).max(1.0);
            let cam_h = custom_cam_h.unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0)).max(1.0);
            
            let screen_width = self.engine.settings().width as f32;
            let screen_height = self.engine.settings().height as f32;
            let sx = screen_width / cam_w;
            let sy = screen_height / cam_h;

            let candidates = ifol_render_ecs::ecs::systems::hit_test::pick_entity_at(
                world, screen_x, screen_y, cam_x, cam_y, sx, sy, true
            );

            for hit in candidates {
                // If it's an image, do an alpha pixel lookup
                if let Some(img) = world.storages.get_component::<ifol_render_ecs::ecs::components::ImageSource>(&hit.entity_id) {
                    let asset_key = world.resolve_asset_url(&img.asset_id).unwrap_or(&img.asset_id);
                    if let Some((rgba, w, h)) = self.backend.images.read().unwrap().get(asset_key) {
                        // Map normalized (u, v) into physical pixel coordinates
                        let px = (hit.u * (*w as f32)) as u32;
                        let py = (hit.v * (*h as f32)) as u32;
                        let px = px.clamp(0, w.saturating_sub(1));
                        let py = py.clamp(0, h.saturating_sub(1));
                        
                        let idx = ((py * *w + px) * 4) as usize;
                        if idx + 3 < rgba.len() {
                            let alpha = rgba[idx + 3];
                            // If pixel is transparent, skip this entity and fall through to the next candidate
                            if alpha < 10 {
                                continue;
                            }
                        }
                    }
                }
                
                // If we reach here, either it's opaque, or it's a solid/text entity, or texture not found
                return Some(hit.entity_id);
            }
            None
        } else {
            None
        }
    }

    #[wasm_bindgen]
    pub fn drag_entity_v2(
        &mut self,
        entity_id: &str,
        screen_dx: f32,
        screen_dy: f32,
        camera_id: &str,
        custom_cam_w: Option<f32>,
        custom_cam_h: Option<f32>,
    ) {
        if let Some(world) = &mut self.v2_world {
            let cam = world.find_camera(camera_id);
            let cam_w = custom_cam_w.unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0)).max(1.0);
            let cam_h = custom_cam_h.unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0)).max(1.0);
            
            let screen_width = self.engine.settings().width as f32;
            let screen_height = self.engine.settings().height as f32;
            let sx = screen_width / cam_w;
            let sy = screen_height / cam_h;

            let world_dx = screen_dx / sx;
            let world_dy = screen_dy / sy;

            // Find the entity's immediate parent accumulated rotation + scale
            // After hierarchy_sys, parent.resolved already contains all ancestor transforms
            let mut parent_rot = 0.0f32;
            let mut parent_sx = 1.0f32;
            let mut parent_sy = 1.0f32;
            
            if let Some(entity) = world.entities.iter().find(|e| e.id == entity_id) {
                let storages = &world.storages;
                if let Some(pid) = storages.get_component::<ifol_render_ecs::ecs::components::meta::ParentId>(&entity.id).map(|id| &id.0) {
                    if let Some(parent) = world.entities.iter().find(|e| &e.id == pid) {
                        // parent.resolved already has full accumulated transform from hierarchy_sys
                        parent_rot = parent.resolved.rotation;
                        parent_sx = parent.resolved.scale_x;
                        parent_sy = parent.resolved.scale_y;
                    }
                }
            }
            
            // Inverse-rotate world delta through parent rotation to get local delta
            let cos_r = (-parent_rot).cos();
            let sin_r = (-parent_rot).sin();
            let local_dx = (world_dx * cos_r - world_dy * sin_r) / parent_sx.max(0.001);
            let local_dy = (world_dx * sin_r + world_dy * cos_r) / parent_sy.max(0.001);

            if let Some(t) = world.storages.get_component_mut::<ifol_render_ecs::ecs::components::Transform>(entity_id) {
                t.x += local_dx;
                t.y += local_dy;
            }
        }
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
