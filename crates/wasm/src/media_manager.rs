use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, HtmlVideoElement, CanvasRenderingContext2d};
use ifol_render_ecs::ecs::World;
use ifol_render_core::backend::media::MediaBackend;

const MAX_CACHE: usize = 60;

#[derive(Clone)]
pub struct FrameData {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

struct VideoEntry {
    el: HtmlVideoElement,
    ready: bool,
    seeking: bool,
    playing: bool,
    pending_seek: Option<f64>,
    // Keep closures alive
    _on_seeked: Option<Closure<dyn FnMut(web_sys::Event)>>,
    _on_error: Option<Closure<dyn FnMut(web_sys::Event)>>,
}

pub struct WasmMediaManager {
    videos: HashMap<String, Rc<RefCell<VideoEntry>>>,
    cache: HashMap<String, FrameData>,
    lru: VecDeque<String>,
    last_good_frames: HashMap<String, FrameData>,
    canvas: Option<HtmlCanvasElement>,
    ctx: Option<CanvasRenderingContext2d>,
}

impl WasmMediaManager {
    pub fn new() -> Self {
        Self {
            videos: HashMap::new(),
            cache: HashMap::new(),
            lru: VecDeque::new(),
            last_good_frames: HashMap::new(),
            canvas: None,
            ctx: None,
        }
    }

    /// Update required video elements for the given world at the specified time.
    /// Injects successfully extracted frames into the backend.
    pub fn update_scene_videos(
        &mut self,
        world: &World,
        _time_sec: f64,
        backend: &crate::web_backend::WebMediaBackend,
    ) {
        // Clear backend's frame map for the new frame
        backend.video_frames.write().unwrap().clear();

        for entity in world.entities.iter() {
            if let Some(video_source) = &entity.components.video_source {
                let asset_id = &video_source.asset_id;
                // Resolve url
                let url = world.resolve_asset_url(asset_id).unwrap_or(asset_id);

                if !entity.resolved.visible {
                    continue;
                }

                let seek_time = entity.resolved.playback_time;

                self.request_frame(url, seek_time, backend);
            }
        }
    }

    fn get_video(&mut self, url: &str) -> Rc<RefCell<VideoEntry>> {
        if let Some(entry) = self.videos.get(url) {
            return entry.clone();
        }

        let el = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .create_element("video")
            .unwrap()
            .dyn_into::<HtmlVideoElement>()
            .unwrap();

        el.set_cross_origin(Some("anonymous"));
        el.set_preload("auto");
        el.set_muted(true);
        let _ = el.set_attribute("playsInline", "true");
        el.set_src(url);

        let entry = Rc::new(RefCell::new(VideoEntry {
            el: el.clone(),
            ready: false,
            seeking: false,
            playing: false,
            pending_seek: None,
            _on_seeked: None,
            _on_error: None,
        }));

        let entry_clone = entry.clone();
        let ready_closure = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            if let Ok(mut lock) = entry_clone.try_borrow_mut() {
                lock.ready = true;
            }
        }) as Box<dyn FnMut(web_sys::Event)>);
        let _ = el.add_event_listener_with_callback("loadedmetadata", ready_closure.as_ref().unchecked_ref());
        ready_closure.forget(); // leak it since video lives forever mostly

        self.videos.insert(url.to_string(), entry.clone());
        entry
    }

    fn request_frame(
        &mut self,
        url: &str,
        time: f64,
        backend: &crate::web_backend::WebMediaBackend,
    ) {
        // Round to nearest 30fps to match frontend cache behavior
        let rounded_time = (time * 30.0).round() / 30.0;
        let cache_key = format!("{}@{:.3}", url, rounded_time);

        // 1. Check cache
        if let Some(frame) = self.cache.get(&cache_key) {
            backend.video_frames.write().unwrap().insert(
                cache_key.clone(),
                (frame.rgba.clone(), frame.width, frame.height),
            );
            return;
        }

        // 2. Fetch video element
        let entry_rc = self.get_video(url);
        let mut entry = entry_rc.borrow_mut();

        // If not ready, use last good frame
        if !entry.ready {
            if let Some(frame) = self.last_good_frames.get(url) {
                backend.video_frames.write().unwrap().insert(
                    cache_key.clone(),
                    (frame.rgba.clone(), frame.width, frame.height),
                );
            }
            return;
        }

        let el = entry.el.clone();
        let diff = (el.current_time() - rounded_time).abs();

        if diff > 0.05 && !entry.seeking && !entry.playing {
            // Need to seek
            entry.seeking = true;
            entry.pending_seek = Some(rounded_time);
            
            let _url_clone = url.to_string();
            let entry_clone = entry_rc.clone();
            
            let on_seeked = Closure::wrap(Box::new(move |_e: web_sys::Event| {
                if let Ok(mut lock) = entry_clone.try_borrow_mut() {
                    lock.seeking = false;
                }
            }) as Box<dyn FnMut(web_sys::Event)>);
            
            let _ = el.add_event_listener_with_callback_and_bool("seeked", on_seeked.as_ref().unchecked_ref(), true);
            entry._on_seeked = Some(on_seeked);
            
            el.set_current_time(rounded_time);
        }

        // Extract current frame if readyState >= 2 (HAVE_CURRENT_DATA)
        if el.ready_state() >= 2 {
            let w = el.video_width();
            let h = el.video_height();
            if w > 0 && h > 0 {
                if let Some(rgba) = self.extract_rgba(&el, w, h) {
                    let frame = FrameData {
                        rgba: rgba.clone(),
                        width: w,
                        height: h,
                    };
                    self.cache.insert(cache_key.clone(), frame.clone());
                    self.lru.push_back(cache_key.clone());
                    self.last_good_frames.insert(url.to_string(), frame.clone());
                    
                    if self.lru.len() > MAX_CACHE {
                        if let Some(old_key) = self.lru.pop_front() {
                            self.cache.remove(&old_key);
                        }
                    }

                    backend.video_frames.write().unwrap().insert(
                        cache_key.clone(),
                        (rgba, w, h),
                    );
                    return;
                }
            }
        }

        // Fallback to last good frame if seeking or extracting failed
        if let Some(frame) = self.last_good_frames.get(url) {
            backend.video_frames.write().unwrap().insert(
                cache_key.clone(),
                (frame.rgba.clone(), frame.width, frame.height),
            );
        }
    }

    fn extract_rgba(&mut self, el: &HtmlVideoElement, width: u32, height: u32) -> Option<Vec<u8>> {
        if self.canvas.is_none() || self.canvas.as_ref().unwrap().width() != width || self.canvas.as_ref().unwrap().height() != height {
            let document = web_sys::window()?.document()?;
            let canvas = document.create_element("canvas").ok()?.dyn_into::<HtmlCanvasElement>().ok()?;
            canvas.set_width(width);
            canvas.set_height(height);
            let options = js_sys::Object::new();
            js_sys::Reflect::set(&options, &JsValue::from_str("willReadFrequently"), &JsValue::from_bool(true)).unwrap();
            let ctx = canvas
                .get_context_with_context_options("2d", &options)
                .ok()??
                .dyn_into::<CanvasRenderingContext2d>()
                .ok()?;
            self.canvas = Some(canvas);
            self.ctx = Some(ctx);
        }

        if let Some(ctx) = &self.ctx {
            ctx.draw_image_with_html_video_element_and_dw_and_dh(
                el, 0.0, 0.0, width as f64, height as f64
            ).ok()?;
            
            let img_data = ctx.get_image_data(0.0, 0.0, width as f64, height as f64).ok()?;
            return Some(img_data.data().0);
        }
        None
    }

    pub fn clear(&mut self) {
        for entry in self.videos.values() {
            let lock = entry.borrow_mut();
            let _ = lock.el.pause();
            lock.el.set_src("");
            lock.el.load();
        }
        self.videos.clear();
        self.cache.clear();
        self.lru.clear();
        self.last_good_frames.clear();
    }
}
