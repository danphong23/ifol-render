use ifol_render_core::PipelineConfig;
use ifol_render_core::engine::CoreEngine;
use ifol_render_core::frame::{Frame, RenderSettings};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

mod web_backend;
use web_backend::WebMediaBackend;

mod media_manager;
use media_manager::WasmMediaManager;

mod audio_manager;
use audio_manager::WasmAudioManager;

mod gizmo_overlay;

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
    /// Previous render scope — used to detect scope transitions for cleanup
    previous_scope: Option<String>,
    /// Scope time override: local time for scoped composition (bypasses speed/loop/trim)
    scope_time: Option<f64>,
    /// Last effective render time — used to detect time changes for graph invalidation
    /// Visual style for selected entities ("rect" or "content")
    select_mode: String,

    /// JavaScript callback for events (progress, buffers, metrics, errors).
    on_event_callback: Option<js_sys::Function>,

    /// When true, camera post-processing effects (post_effects on CameraComponent)
    /// are skipped during rendering. Useful for editor preview without color grading.
    post_effects_enabled: bool,
    /// Global render quality scale (0.0 to 1.0)
    render_quality: f32,
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
            previous_scope: None,
            scope_time: None,
            select_mode: "rect".to_string(),
            on_event_callback: None,
            post_effects_enabled: true,
            render_quality: 1.0,
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
        self.dispatch_event("asset_loaded", &format!("Font cached: {}", key));
    }

    /// Set an event listener callback function passed from Javascript
    #[wasm_bindgen]
    pub fn set_event_listener(&mut self, callback: js_sys::Function) {
        self.on_event_callback = Some(callback);
    }

    /// Internal method to dispatch events to Javascript
    fn dispatch_event(&self, event_type: &str, payload_json: &str) {
        if let Some(cb) = &self.on_event_callback {
            let event = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&event, &"type".into(), &event_type.into());
            let _ = js_sys::Reflect::set(&event, &"payload".into(), &payload_json.into());
            let _ = cb.call1(&JsValue::NULL, &event);
        }
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

    /// Set the maximum size of the GPU texture cache in Megabytes.
    /// If actual usage exceeds this limit, LRU eviction runs automatically.
    /// Defaults to 0 (Unlimited).
    #[wasm_bindgen]
    pub fn set_vram_limit_mb(&mut self, mb: f64) {
        let max_bytes = (mb * 1024.0 * 1024.0) as u64;
        self.engine.set_vram_limit(max_bytes);
        self.dispatch_event("info", &format!("VRAM Limit set to: {} MB", mb));
    }

    /// Explicitly clear all cached textures and GPU buffers (forces reload of assets).
    #[wasm_bindgen]
    pub fn clear_textures(&mut self) {
        self.engine.renderer_mut().clear_textures();
        self.dispatch_event("info", "VRAM textures cleared.");
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
            return Err(JsValue::from_str(&format!(
                "Shader '{}' already registered",
                name
            )));
        }
        self.engine
            .register_shader(name, wgsl_code, PipelineConfig::quad());
        log::info!("Custom shader registered: '{}'", name);
        Ok(())
    }

    /// Register a custom fullscreen effect shader (like blur/vignette).
    /// `param_names` is a comma-separated list of float uniform names.
    /// WGSL must define vs_fullscreen and fs_main entry points.
    pub fn register_effect(
        &mut self,
        name: &str,
        wgsl_code: &str,
        param_names: &str,
    ) -> Result<(), JsValue> {
        if self.engine.has_shader(name) {
            return Err(JsValue::from_str(&format!(
                "Effect '{}' already registered",
                name
            )));
        }
        let defaults: Vec<(String, f32)> = param_names
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| (s.trim().to_string(), 0.0))
            .collect();
        let pass_count = 1;
        self.engine
            .register_effect(name, wgsl_code, defaults, pass_count);
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
        log::info!(
            "V2 Scene loaded: {} entities in ECS World.",
            world.entities.len()
        );
        self.v2_world = Some(world);

        // Full state cleanup on scene change to prevent stale GPU artifacts
        self.engine.invalidate_render_graph();
        self.engine.evict_scope_textures();
        self.media_manager.clear();
        self.audio_manager.clear();
        self.render_scope = None;
        self.previous_scope = None;
        self.scope_time = None;
        self.selected_entity_ids.clear();
        self.is_playing = false;

        Ok(())
    }

    /// Patch the scene with delta changes without reloading the entire scene JSON.
    ///
    /// `delta_json` is an array of patch operations:
    /// ```json
    /// [
    ///   {
    ///     "id": "entity_001",
    ///     "components": {
    ///       "Transform": { "x": 100.0, "y": 200.0 }
    ///     }
    ///   }
    /// ]
    /// ```
    /// Each entry patches only the listed components for the given entity ID.
    /// Non-listed components are left unchanged.
    /// If the entity ID is not found in the world, that entry is silently skipped.
    pub fn patch_scene_v2(&mut self, delta_json: &str) -> Result<(), JsValue> {
        let world = match self.v2_world.as_mut() {
            Some(w) => w,
            None => return Err(JsValue::from_str("No V2 scene loaded. Call load_scene_v2 first.")),
        };

        // Parse delta as an array of EntityV2 patches
        let patches: Vec<ifol_render_ecs::schema::v2::EntityV2> =
            serde_json::from_str(delta_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid patch JSON: {}", e)))?;

        let mut patched = 0usize;
        for patch in &patches {
            // Check entity exists
            if world.get(&patch.id).is_none() {
                log::warn!("patch_scene_v2: entity '{}' not found, skipping", patch.id);
                continue;
            }

            // Re-inject each patched component using the existing registry loaders.
            // This reuses the exact same load path as load_scene — no code duplication.
            for (comp_key, comp_value) in &patch.components {
                let loader_opt = world.registry.loaders.get(comp_key.as_str()).copied();
                if let Some(loader) = loader_opt {
                    if let Err(e) = loader(world, &patch.id, comp_value) {
                        log::warn!(
                            "patch_scene_v2: failed to patch component '{}' for entity '{}': {}",
                            comp_key, patch.id, e
                        );
                    }
                } else {
                    log::warn!(
                        "patch_scene_v2: unknown component '{}' for entity '{}', skipping",
                        comp_key, patch.id
                    );
                }
            }
            patched += 1;
        }

        log::debug!("patch_scene_v2: patched {} entities", patched);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn parse_v2_json(scene_json: &str) -> String {
        match serde_json::from_str::<ifol_render_ecs::scene::SceneV2>(scene_json) {
            Ok(scene) => format!("Success. Parsed {} assets.", scene.assets.len()),
            Err(e) => format!("Error parsing: {}", e),
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
            return Err(JsValue::from_str(
                "No V2 scene loaded. Call load_scene_v2 first.",
            ));
        }

        // 1. Evaluate ECS timeline and animation systems at `time_sec`
        let scene_fps = self.engine.settings().fps;
        let time_state = ifol_render_ecs::time::TimeState {
            global_time: time_sec,
            delta_time: 1.0 / scene_fps,
            frame_index: (time_sec * scene_fps) as u64,
            fps: scene_fps,
        };

        let mut world = self.v2_world.take().unwrap();

        // ── Scope change detection: evict stale render targets ──
        // When the user drills into or out of a composition, intermediate
        // GPU textures from the previous scope session become stale and
        // must be invalidated before the new scope renders.
        let scope_changed = self.render_scope != self.previous_scope;
        if scope_changed {
            self.engine.evict_scope_textures();
            self.previous_scope = self.render_scope.clone();
            // Always clear overrides on scope switch — the "time has changed"
            // check below may not trigger if time_sec is the same (e.g. both 0.0)
            world.editor_overrides.clear();
            world.override_time = Some(time_sec);
        }

        // Reset transient Editor Overrides when time jumps (scrubbing/playing)
        if world.override_time != Some(time_sec) {
            world.editor_overrides.clear();
            world.override_time = Some(time_sec);
        }

        // ── RenderGraph cache invalidation ──
        // Always clear graph node cache on every render call. The per-pass
        // hash system (actual_hash in engine.render_frame) will still skip
        // unchanged GPU work — this only ensures we never serve stale cached
        // nodes from a previous frame (which caused composition freezing
        // on backward seeks where effective_time matched a previously-rendered time).
        self.engine.invalidate_render_graph();

        ifol_render_ecs::ecs::pipeline::run(
            &mut world,
            &time_state,
            self.render_scope.as_deref(),
            self.scope_time,
        );

        // Sync HTML5 <audio> tags with ECS time and volume
        self.audio_manager.sync_audio(&world, self.is_playing, self.render_scope.as_deref());

        // ---- Asset Discovery Scans (Buffering & Preload) ----
        let mut buffering_assets = Vec::new();
        let mut preload_assets = Vec::new();
        let mut active_video_entities = std::collections::HashSet::new();
        let mut has_pending_video = false; // Track if any visible video entity has no frame ready
        let preload_window = 3.0; // 3 seconds lookahead
        let mut intrinsic_updates: Vec<(String, f32, f32)> = Vec::new();

        {
            let images_cache = self.backend.images.read().unwrap();
            let storages = &world.storages;

            for entity in world.entities.iter() {
                // Preload Scan (Lookahead)
                if let Some(lifespan) =
                    storages.get_component::<ifol_render_ecs::scene::Lifespan>(&entity.id)
                {
                    if lifespan.start > time_sec && lifespan.start <= time_sec + preload_window {
                        // Gather future assets
                        if let Some(video) = storages
                            .get_component::<ifol_render_ecs::ecs::components::VideoSource>(
                                &entity.id,
                            )
                        {
                            let url = world
                                .resolve_asset_url(&video.asset_id)
                                .unwrap_or(&video.asset_id)
                                .to_string();
                            self.media_manager.preload_video(&entity.id, &url, 0.0);
                            preload_assets.push(format!("video:{}", url));
                        }
                        if let Some(image) = storages
                            .get_component::<ifol_render_ecs::ecs::components::ImageSource>(
                                &entity.id,
                            )
                        {
                            let url = world
                                .resolve_asset_url(&image.asset_id)
                                .unwrap_or(&image.asset_id)
                                .to_string();
                            if !images_cache.contains_key(&url) {
                                preload_assets.push(format!("image:{}", url));
                            }
                        }
                    }
                }

                if !entity.resolved.visible {
                    continue;
                }

                // Current Frame Scan (Buffering & Loading)
                if let Some(video_source) = storages
                    .get_component::<ifol_render_ecs::ecs::components::VideoSource>(&entity.id)
                {
                    active_video_entities.insert(entity.id.clone());
                    let url = world
                        .resolve_asset_url(&video_source.asset_id)
                        .unwrap_or(&video_source.asset_id);
                    let seek_time = entity.resolved.playback_time;

                    if !self
                        .media_manager
                        .is_video_ready(&entity.id, url, seek_time)
                    {
                        buffering_assets.push(format!("video:{}", url));
                    }

                    if let Some((el, w, h)) = self.media_manager.get_video_frame(
                        &entity.id,
                        url,
                        seek_time,
                        self.is_playing,
                    ) {
                        self.engine.load_video_texture_web(url, &el, w, h);
                        if video_source.intrinsic_width <= 0.0
                            || video_source.intrinsic_height <= 0.0
                        {
                            intrinsic_updates.push((entity.id.clone(), w as f32, h as f32));
                        }
                    } else {
                        // Video entity is visible but frame not ready (async seek pending)
                        has_pending_video = true;
                    }
                }

                if let Some(img) = storages
                    .get_component::<ifol_render_ecs::ecs::components::ImageSource>(&entity.id)
                {
                    let asset_key = world
                        .resolve_asset_url(&img.asset_id)
                        .unwrap_or(&img.asset_id);
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
            if let Some(img) = world
                .storages
                .get_component_mut::<ifol_render_ecs::ecs::components::ImageSource>(&id)
            {
                img.intrinsic_width = w;
                img.intrinsic_height = h;
            } else if let Some(vid) = world
                .storages
                .get_component_mut::<ifol_render_ecs::ecs::components::VideoSource>(&id)
            {
                vid.intrinsic_width = w;
                vid.intrinsic_height = h;
            }
        }

        // 3. Compile World to Frame
        let w = self.engine.settings().width;
        let h = self.engine.settings().height;

        // 3.1. Editor Phase (Gizmos)
        let selected_refs: Vec<&str> = self
            .selected_entity_ids
            .iter()
            .map(|s| s.as_str())
            .collect();
        // Wait, editor_gizmo_system MUST run AFTER render_to_frame because it appends to the Frame!

        let mut selected_ids = std::collections::HashSet::new();
        for s in &self.selected_entity_ids {
            selected_ids.insert(s.clone());
        }
        let context = world.build_context(self.render_scope.as_deref(), selected_ids, self.select_mode.clone());

        let cam_ids: Vec<String> = world.entities.iter()
            .filter(|e| world.storages.get_component::<ifol_render_ecs::ecs::components::CameraComponent>(&e.id).is_some())
            .map(|e| e.id.clone())
            .collect();
            
        // Save original camera state before transient modifications.
        // We must NOT mutate persistent component data — only apply overrides for this frame.
        let mut cam_originals: Vec<(String, f32, Vec<ifol_render_ecs::scene::MaterialV2>)> = Vec::new();
        for cam_id in &cam_ids {
            if let Some(cam) = world.storages.get_component_mut::<ifol_render_ecs::ecs::components::CameraComponent>(cam_id) {
                // Save originals before any mutation
                cam_originals.push((cam_id.clone(), cam.render_scale, cam.post_effects.clone()));

                if !self.post_effects_enabled {
                    cam.post_effects.clear();
                }
                // Apply global preview quality scale (transient, will be restored)
                cam.render_scale *= self.render_quality;
            }
        }
        
        // 3.2.1 Virtual Editor Camera injection
        // Pre-compute gizmo_layer and max_content_layer as plain i32 values BEFORE
        // any mutable world operations (add_entity/add_component).
        // The sort Vec borrows world — we must drop it before mutation.
        let (editor_gizmo_layer, editor_max_content_layer) = if is_editor_mode {
            let sorted = world.sorted_by_layer();
            let max_non_cam = sorted
                .iter()
                .filter(|e| world.storages
                    .get_component::<ifol_render_ecs::ecs::components::CameraComponent>(&e.id)
                    .is_none())
                .map(|e| e.resolved.layer)
                .max()
                .unwrap_or(0);
            (max_non_cam + 1, max_non_cam)
            // `sorted` dropped here — world borrow released
        } else {
            (1, 0)
        };

        let mut actual_camera_id = camera_id.to_string();
        if is_editor_mode {
            let gizmo_layer = editor_gizmo_layer;
            
            // Upsert __editor_cam__ virtual camera (persistent across frames)
            world.upsert_entity(ifol_render_ecs::ecs::Entity {
                id: "__editor_cam__".to_string(),
                resolved: {
                    let mut r = ifol_render_ecs::ecs::ResolvedState::default();
                    r.visible = true;
                    // Note: We don't have center_x, center_y, width, height yet during this initialization.
                    // We will update it directly after calculating!
                    r
                },
                draw: Default::default(),
            });
            let mut editor_cam = ifol_render_ecs::ecs::components::CameraComponent::default();
            editor_cam.render_mode = ifol_render_ecs::ecs::components::camera::CameraRenderMode::Cameras;
            editor_cam.target_cameras = vec!["__editor_cam_content__".to_string(), "__gizmo_cam__".to_string()];
            world.add_component("__editor_cam__", editor_cam);
            
            let (mut center_x, mut center_y, mut width, mut height) = if let Some(c) = world.find_camera(camera_id) {
                (c.resolved.x, c.resolved.y, c.resolved.width, c.resolved.height)
            } else {
                (0.0, 0.0, 1280.0, 720.0)
            };

            // custom_cam_w and custom_cam_h are provided directly
            if let Some(w_ov) = custom_cam_w { width = w_ov; }
            if let Some(h_ov) = custom_cam_h { height = h_ov; }

            // custom_cam_x and custom_cam_y are passed from JS as the TOP-LEFT corner of the viewport!
            // We must convert them to the ECS Transform coordinate space (CENTER).
            if let Some(x) = custom_cam_x { center_x = x + width * 0.5; }
            if let Some(y) = custom_cam_y { center_y = y + height * 0.5; }

            // MANUALLY seed the resolved state for __editor_cam__ since we added the ECS entity 
            // *after* the pipeline systems have finished executing. 
            // Ohterwise, it will be mapped with root_cam_x = 0 causing panning failures.
            if let Some(editor_ent) = world.get_mut("__editor_cam__") {
                editor_ent.resolved.x = center_x;
                editor_ent.resolved.y = center_y;
                editor_ent.resolved.width = width;
                editor_ent.resolved.height = height;
            }

            // Clone the original camera to safely override its Transform without modifying the user's scene data permanently
            let mut clone_cam = ifol_render_ecs::ecs::components::CameraComponent::default();
            if let Some(c) = world.storages.get_component::<ifol_render_ecs::ecs::components::CameraComponent>(camera_id) {
                clone_cam = c.clone();
            }
            
            world.upsert_entity(ifol_render_ecs::ecs::Entity {
                id: "__editor_cam_content__".to_string(),
                resolved: {
                    let mut r = ifol_render_ecs::ecs::ResolvedState::default();
                    r.visible = true;
                    r.x = center_x; r.y = center_y;
                    r.width = width; r.height = height;
                    r
                },
                draw: Default::default(),
            });
            world.add_component("__editor_cam_content__", clone_cam);
            world.add_component("__editor_cam_content__", ifol_render_ecs::ecs::components::Transform {
                x: center_x, y: center_y, 
                scale_x: 1.0, scale_y: 1.0, 
                rotation: 0.0, anchor_x: 0.5, anchor_y: 0.5,
            });
            world.add_component("__editor_cam_content__", ifol_render_ecs::ecs::components::Rect {
                width: width, height: height, 
                fit_mode: ifol_render_ecs::ecs::components::FitMode::Stretch,
                align_x: 0.5, align_y: 0.5,
            });

            world.add_component("__editor_cam__", ifol_render_ecs::ecs::components::Transform {
                x: center_x, y: center_y, 
                scale_x: 1.0, scale_y: 1.0, 
                rotation: 0.0, anchor_x: 0.5, anchor_y: 0.5,
            });
            world.add_component("__editor_cam__", ifol_render_ecs::ecs::components::Rect {
                width: width, height: height, 
                fit_mode: ifol_render_ecs::ecs::components::FitMode::Stretch,
                align_x: 0.5, align_y: 0.5,
            });
            
            // Setup gizmo sub-camera pointing to gizmo_layer
            world.upsert_entity(ifol_render_ecs::ecs::Entity {
                id: "__gizmo_cam__".to_string(),
                resolved: {
                    let mut r = ifol_render_ecs::ecs::ResolvedState::default();
                    r.visible = true;
                    r.x = center_x; r.y = center_y;
                    r.width = width; r.height = height;
                    r
                },
                draw: Default::default(),
            });
            let mut gizmo_cam = ifol_render_ecs::ecs::components::CameraComponent::default();
            gizmo_cam.render_mode = ifol_render_ecs::ecs::components::camera::CameraRenderMode::Layers;
            gizmo_cam.target_layers = Some(vec![gizmo_layer]);
            gizmo_cam.render_order = 999;
            world.add_component("__gizmo_cam__", gizmo_cam);

            world.add_component("__gizmo_cam__", ifol_render_ecs::ecs::components::Transform {
                x: center_x, y: center_y, 
                scale_x: 1.0, scale_y: 1.0, 
                rotation: 0.0, anchor_x: 0.5, anchor_y: 0.5,
            });
            world.add_component("__gizmo_cam__", ifol_render_ecs::ecs::components::Rect {
                width: width, height: height, 
                fit_mode: ifol_render_ecs::ecs::components::FitMode::Stretch,
                align_x: 0.5, align_y: 0.5,
            });
            
            // Redirect render to use virtual master camera
            actual_camera_id = "__editor_cam__".to_string();
        }

        // 3.3. Core Render Phase
        let mut frame = ifol_render_ecs::ecs::systems::render_to_frame(
            &world,
            &actual_camera_id,
            w,
            h,
            time_sec,
            &context,
        );

        if is_editor_mode {
            let cam = world.find_camera(camera_id);
            let cam_x = custom_cam_x.unwrap_or_else(|| cam.map(|c| c.resolved.x - c.resolved.width * 0.5).unwrap_or(0.0));
            let cam_y = custom_cam_y.unwrap_or_else(|| cam.map(|c| c.resolved.y - c.resolved.height * 0.5).unwrap_or(0.0));
            let cam_w = custom_cam_w
                .unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0))
                .max(1.0);
            let cam_h = custom_cam_h
                .unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0))
                .max(1.0);

            let sx = w as f32 / cam_w;
            let sy = h as f32 / cam_h;

            // Reuse the pre-computed integer (no additional sort needed).
            let gizmo_base_layer = editor_max_content_layer + 1;

            let gizmos = crate::gizmo_overlay::editor_gizmo_system(
                &world,
                &selected_refs,
                &self.select_mode,
                cam_x,
                cam_y,
                sx,
                sy,
                w,
                h,
                &context,
                gizmo_base_layer,
            );

            // T3.2: Only run the gizmo system when something is actually selected.
            // With no selection, gizmos produce zero entities — skip the whole system.
            if !gizmos.is_empty() {
                if let Some(pass) = frame.passes.iter_mut().find(|p| p.output == "final" || matches!(p.pass_type, ifol_render_ecs::frame::PassType::Output { .. })) {
                    if let ifol_render_ecs::frame::PassType::Output { entities, .. } = &mut pass.pass_type {
                        entities.extend(gizmos);
                    }
                }
            }
        }

        // Restore original camera state after rendering (undo transient mutations)
        for (cam_id, orig_scale, orig_effects) in cam_originals {
            if let Some(cam) = world.storages.get_component_mut::<ifol_render_ecs::ecs::components::CameraComponent>(&cam_id) {
                cam.render_scale = orig_scale;
                cam.post_effects = orig_effects;
            }
        }

        // Editor entities are now persistent (upsert_entity) — no cleanup needed.

        self.v2_world = Some(world);

        // 4. Send to WGPU engine — ALWAYS render
        // Previously this had a "Frame Readiness Gate" that would SKIP
        // engine.render_frame() when has_pending_video was true and not playing.
        // That caused ALL compositions to freeze when ANY single video entity
        // was pending (e.g. after leaving/re-entering a comp's lifespan).
        // Now we always render: non-video entities render immediately,
        // video entities show stale/placeholder until their frame arrives.
        let frame_complete = !has_pending_video;
        self.engine.render_frame(&frame);

        // 5. Build and return the EngineStatus JSON manually
        let mut json = String::from("{");

        let status_str = if buffering_assets.is_empty() {
            "\"ready\""
        } else {
            "\"buffering\""
        };
        json.push_str(&format!("\"status\":{},", status_str));
        json.push_str(&format!("\"frame_complete\":{},", frame_complete));

        let buff_join = buffering_assets
            .iter()
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(",");
        json.push_str(&format!("\"buffering_assets\":[{}],", buff_join));

        let pre_join = preload_assets
            .iter()
            .map(|s| format!("\"{}\"", s))
            .collect::<Vec<_>>()
            .join(",");
        json.push_str(&format!("\"preload_assets\":[{}]", pre_join));

        // VRAM Stats
        let vram = self.engine.vram_usage();
        json.push_str(&format!(
            ",\"vram_bytes\":{},\"vram_count\":{}",
            vram.texture_cache_bytes, vram.texture_count
        ));

        json.push('}');

        self.dispatch_event("render_metrics", &json);

        Ok(JsValue::from_str(&json))
    }

    /// Set the render scope to only show descendants of this entity.
    /// Pass None to show all entities (root scope).
    #[wasm_bindgen]
    pub fn set_render_scope(&mut self, entity_id: Option<String>) {
        self.render_scope = entity_id;
    }

    /// Get inner camera viewport params for the current render scope composition.
    /// Returns JSON: { x, y, w, h } representing the inner camera's world-space view origin and size.
    /// Returns None if not in a scoped composition or no camera found.
    #[wasm_bindgen]
    pub fn get_scope_camera_params(&self) -> Option<String> {
        let scope_id = self.render_scope.as_deref()?;
        let world = self.v2_world.as_ref()?;
        let storages = &world.storages;

        // Find the comp entity
        let comp_ent = world.entities.iter().find(|e| e.id == scope_id)?;

        // Find DIRECT child camera  
        let cam_ent = world.entities.iter().find(|c| {
            if storages.get_component::<ifol_render_ecs::ecs::components::CameraComponent>(&c.id).is_none() {
                return false;
            }
            storages.get_component::<ifol_render_ecs::ecs::components::meta::ParentId>(&c.id)
                .map(|pid| pid.0 == scope_id)
                .unwrap_or(false)
        })?;

        let cw = cam_ent.resolved.width.max(1.0);
        let ch = cam_ent.resolved.height.max(1.0);
        // Inner cam view origin = cam world pos - half cam size
        let inner_cam_x = cam_ent.resolved.x - cw * 0.5;
        let inner_cam_y = cam_ent.resolved.y - ch * 0.5;

        Some(format!(
            "{{\"x\":{},\"y\":{},\"w\":{},\"h\":{}}}",
            inner_cam_x, inner_cam_y, cw, ch
        ))
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

    /// Enable or disable camera post-processing effects for the current viewport.
    /// When disabled, `post_effects` defined on CameraComponent are stripped before rendering.
    /// Useful for editor preview modes where post-grading would obscure the true scene.
    /// Default: enabled (true).
    #[wasm_bindgen]
    pub fn set_post_effects_enabled(&mut self, enabled: bool) {
        self.post_effects_enabled = enabled;
    }

    /// Returns whether camera post-processing is currently enabled.
    #[wasm_bindgen]
    pub fn is_post_effects_enabled(&self) -> bool {
        self.post_effects_enabled
    }

    #[wasm_bindgen]
    pub fn set_render_quality(&mut self, quality: f32) {
        self.render_quality = quality.clamp(0.01, 1.0);
    }

    #[wasm_bindgen]
    pub fn get_render_quality(&self) -> f32 {
        self.render_quality
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
            let cam_top_left_x = cam.map(|c| c.resolved.x - c.resolved.width * 0.5).unwrap_or(0.0);
            let cam_top_left_y = cam.map(|c| c.resolved.y - c.resolved.height * 0.5).unwrap_or(0.0);
            
            let cam_x = custom_cam_x.unwrap_or(cam_top_left_x);
            let cam_y = custom_cam_y.unwrap_or(cam_top_left_y);
            let cam_w = custom_cam_w
                .unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0))
                .max(1.0);
            let cam_h = custom_cam_h
                .unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0))
                .max(1.0);

            let screen_width = self.engine.settings().width as f32;
            let screen_height = self.engine.settings().height as f32;
            let sx = screen_width / cam_w;
            let sy = screen_height / cam_h;

            let candidates = ifol_render_ecs::ecs::systems::hit_test::pick_entity_at(
                world, screen_x, screen_y, cam_x, cam_y, sx, sy, true,
                self.render_scope.as_deref(),
            );

            self.evaluate_hits_recursive(world, candidates)
        } else {
            None
        }
    }

    fn evaluate_hits_recursive(
        &self,
        world: &ifol_render_ecs::ecs::World,
        candidates: Vec<ifol_render_ecs::ecs::systems::hit_test::HitResult>,
    ) -> Option<String> {
        for hit in candidates {
            if self.select_mode == "rect" {
                return Some(hit.entity_id);
            }

            let is_comp = world.storages.get_component::<ifol_render_ecs::ecs::components::Composition>(&hit.entity_id).is_some();
            if is_comp && self.select_mode == "content" {
                if let Some(cam_ent) = world.entities.iter().find(|c| {
                    world.storages.get_component::<ifol_render_ecs::ecs::components::CameraComponent>(&c.id).is_some() &&
                    world.storages.get_component::<ifol_render_ecs::ecs::components::meta::ParentId>(&c.id).map_or(false, |pid| pid.0 == hit.entity_id)
                }) {
                    let inner_cw = cam_ent.resolved.width.max(1.0);
                    let inner_ch = cam_ent.resolved.height.max(1.0);
                    let comp_ent = world.entities.iter().find(|e| e.id == hit.entity_id).unwrap();
                    // Fix: The inner camera offset must subtract half the Camera's size to get top-left
                    let inner_cam_x = cam_ent.resolved.x - inner_cw * 0.5;
                    let inner_cam_y = cam_ent.resolved.y - inner_ch * 0.5;
                    
                    let inner_sx = hit.u * inner_cw;
                    let inner_sy = hit.v * inner_ch;
                    
                    let inner_hits = ifol_render_ecs::ecs::systems::hit_test::pick_entity_at(
                        world, inner_sx, inner_sy, inner_cam_x, inner_cam_y, 1.0, 1.0, true,
                        Some(&hit.entity_id),
                    );
                    
                    if self.evaluate_hits_recursive(world, inner_hits).is_some() {
                        return Some(hit.entity_id);
                    } else {
                        // User requirement: if click is empty space inside composition, select composition itself
                        return Some(hit.entity_id);
                    }
                }
            }

            // Cameras are non-visual but their gizmo triangle defines their hit area in Editor Mode.
            if world.storages.get_component::<ifol_render_ecs::ecs::components::CameraComponent>(&hit.entity_id).is_some() {
                return Some(hit.entity_id);
            }

            // If it's an image, do an alpha pixel lookup
            if let Some(img) = world
                .storages
                .get_component::<ifol_render_ecs::ecs::components::ImageSource>(&hit.entity_id)
            {
                let asset_key = world
                    .resolve_asset_url(&img.asset_id)
                    .unwrap_or(&img.asset_id);
                if let Some((rgba, w, h)) = self.backend.images.read().unwrap().get(asset_key) {
                    let px = (hit.u * (*w as f32)) as u32;
                    let py = (hit.v * (*h as f32)) as u32;
                    let px = px.clamp(0, w.saturating_sub(1));
                    let py = py.clamp(0, h.saturating_sub(1));

                    let idx = ((py * *w + px) * 4) as usize;
                    if idx + 3 < rgba.len() {
                        let alpha = rgba[idx + 3];
                        if alpha < 10 {
                            continue;
                        }
                    }
                }
            }

            return Some(hit.entity_id);
        }
        None
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
            let cam_w = custom_cam_w
                .unwrap_or_else(|| cam.map(|c| c.resolved.width).unwrap_or(1280.0))
                .max(1.0);
            let cam_h = custom_cam_h
                .unwrap_or_else(|| cam.map(|c| c.resolved.height).unwrap_or(720.0))
                .max(1.0);

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
            let mut parent_x = 0.0f32;
            let mut parent_y = 0.0f32;

            if let Some(entity) = world.entities.iter().find(|e| e.id == entity_id) {
                let storages = &world.storages;
                if let Some(pid) = storages
                    .get_component::<ifol_render_ecs::ecs::components::meta::ParentId>(&entity.id)
                    .map(|id| &id.0)
                {
                    if let Some(parent) = world.entities.iter().find(|e| &e.id == pid) {
                        // Check if parent is a Composition (world isolation boundary)
                        let parent_is_comp = storages
                            .get_component::<ifol_render_ecs::ecs::components::Composition>(pid)
                            .is_some();
                        if parent_is_comp {
                            // Comp boundary: children live in LOCAL space, no parent transform
                            parent_rot = 0.0;
                            parent_sx = 1.0;
                            parent_sy = 1.0;
                            parent_x = 0.0;
                            parent_y = 0.0;
                        } else {
                            // Normal hierarchy: use parent's accumulated world transforms
                            parent_rot = parent.resolved.rotation;
                            parent_sx = parent.resolved.scale_x;
                            parent_sy = parent.resolved.scale_y;
                            parent_x = parent.resolved.x;
                            parent_y = parent.resolved.y;
                        }
                    }
                }
            }

            // Inverse-rotate world delta through parent rotation to get local delta
            let cos_r = (-parent_rot).cos();
            let sin_r = (-parent_rot).sin();
            let local_dx = (world_dx * cos_r - world_dy * sin_r) / parent_sx.max(0.001);
            let local_dy = (world_dx * sin_r + world_dy * cos_r) / parent_sy.max(0.001);

            let has_anim_x = world
                .get_component::<ifol_render_ecs::ecs::components::AnimationComponent>(entity_id)
                .map(|a| {
                    a.float_tracks.iter().any(|t| {
                        t.target
                            == ifol_render_ecs::ecs::components::animation::AnimTarget::TransformX
                            && !t.track.keyframes.is_empty()
                    })
                })
                .unwrap_or(false);

            let has_anim_y = world
                .get_component::<ifol_render_ecs::ecs::components::AnimationComponent>(entity_id)
                .map(|a| {
                    a.float_tracks.iter().any(|t| {
                        t.target
                            == ifol_render_ecs::ecs::components::animation::AnimTarget::TransformY
                            && !t.track.keyframes.is_empty()
                    })
                })
                .unwrap_or(false);

            let mut global_x = 0.0;
            let mut global_y = 0.0;
            if let Some(entity) = world.entities.iter().find(|e| e.id == entity_id) {
                let anchor_dx = (0.5 - entity.resolved.anchor_x) * entity.resolved.width;
                let anchor_dy = (0.5 - entity.resolved.anchor_y) * entity.resolved.height;
                let final_cos_r = entity.resolved.rotation.cos();
                let final_sin_r = entity.resolved.rotation.sin();
                
                // Reverse the visual center offset baked by hierarchy_sys.rs back to the true world anchor!
                global_x = entity.resolved.x - (anchor_dx * final_cos_r - anchor_dy * final_sin_r);
                global_y = entity.resolved.y - (anchor_dx * final_sin_r + anchor_dy * final_cos_r);
            }

            // Inverse-transform world coordinates back to local coordinates
            let dx_world = global_x - parent_x;
            let dy_world = global_y - parent_y;
            
            // Rotate back by -parent_rot
            let cos_r_inv = (-parent_rot).cos();
            let sin_r_inv = (-parent_rot).sin();
            let current_local_x = (dx_world * cos_r_inv - dy_world * sin_r_inv) / parent_sx.max(0.001);
            let current_local_y = (dx_world * sin_r_inv + dy_world * cos_r_inv) / parent_sy.max(0.001);

            if has_anim_x {
                world.set_transient_override(
                    entity_id,
                    ifol_render_ecs::ecs::components::animation::AnimTarget::TransformX,
                    ifol_render_ecs::ecs::OverrideValue::Float(current_local_x + local_dx),
                );
            } else if let Some(t) = world
                .storages
                .get_component_mut::<ifol_render_ecs::ecs::components::Transform>(entity_id)
            {
                t.x += local_dx;
            }

            if has_anim_y {
                world.set_transient_override(
                    entity_id,
                    ifol_render_ecs::ecs::components::animation::AnimTarget::TransformY,
                    ifol_render_ecs::ecs::OverrideValue::Float(current_local_y + local_dy),
                );
            } else if let Some(t) = world
                .storages
                .get_component_mut::<ifol_render_ecs::ecs::components::Transform>(entity_id)
            {
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
            .map(|s| {
                s.split(',')
                    .filter(|id| !id.is_empty())
                    .map(|id| id.trim().to_string())
                    .collect()
            })
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
