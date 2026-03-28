use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlVideoElement};
use ifol_render_ecs::ecs::World;

// ── Configuration Constants ──
const SYNC_TOLERANCE_PLAY: f64 = 0.3; // 300ms drift tolerance when playing (avoids stutter but keeps sync)
const SYNC_TOLERANCE_SCRUB: f64 = 0.02; // 20ms frame-perfect snap when scrubbing

pub struct VideoEntry {
    pub el: HtmlVideoElement,
    ready: bool,
    playing: bool,
    last_ecs_time: f64,
    // Store closures so they are safely dropped when VideoEntry is dropped (no more memory leaks)
    _on_ready: Option<Closure<dyn FnMut(web_sys::Event)>>,
    _on_seeked: Option<Closure<dyn FnMut(web_sys::Event)>>,
}

impl Drop for VideoEntry {
    fn drop(&mut self) {
        let _ = self.el.pause();
        self.el.set_src("");
        self.el.load();
        self.el.remove(); // Removes from the DOM tree entirely
    }
}

pub struct WasmMediaManager {
    // Key is now the Entity ID, NOT the URL, so entities can share URLs freely.
    videos: HashMap<String, Rc<RefCell<VideoEntry>>>,
}

impl WasmMediaManager {
    pub fn new() -> Self {
        Self {
            videos: HashMap::new(),
        }
    }

    fn get_video(&mut self, entity_id: &str, url: &str) -> Rc<RefCell<VideoEntry>> {
        if let Some(entry) = self.videos.get(entity_id) {
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
        let _ = el.set_attribute("style", "position: absolute; opacity: 0; pointer-events: none; width: 1px; height: 1px;");
        el.set_src(url);

        if let Some(body) = web_sys::window().unwrap().document().unwrap().body() {
            let _ = body.append_child(&el);
        }

        let entry = Rc::new(RefCell::new(VideoEntry {
            el: el.clone(),
            ready: false,
            playing: false,
            last_ecs_time: -1.0,
            _on_ready: None,
            _on_seeked: None,
        }));

        // Ready Callback
        let entry_clone = entry.clone();
        let ready_closure = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            if let Ok(mut lock) = entry_clone.try_borrow_mut() {
                lock.ready = true;
            }
        }) as Box<dyn FnMut(web_sys::Event)>);
        let _ = el.add_event_listener_with_callback("loadedmetadata", ready_closure.as_ref().unchecked_ref());
        entry.borrow_mut()._on_ready = Some(ready_closure);

        // Seeked Callback (For pushing Dirty frame updates during scrubbing)
        let seeked_closure = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            if let Some(window) = web_sys::window() {
                if let Ok(event) = web_sys::Event::new("ifol_video_seeked") {
                    let _ = window.dispatch_event(&event);
                }
            }
        }) as Box<dyn FnMut(web_sys::Event)>);
        let _ = el.add_event_listener_with_callback("seeked", seeked_closure.as_ref().unchecked_ref());
        entry.borrow_mut()._on_seeked = Some(seeked_closure);

        self.videos.insert(entity_id.to_string(), entry.clone());
        entry
    }

    /// Requests a video frame for the given Entity.
    /// Returns the active HtmlVideoElement, its width, and height.
    /// WebGPU will copy directly from the HtmlVideoElement to VRAM.
    pub fn get_video_frame(
        &mut self,
        entity_id: &str,
        url: &str,
        time: f64,
        is_engine_playing: bool,
    ) -> Option<(HtmlVideoElement, u32, u32)> {
        // Round to nearest 30fps to match timeline expectation
        let rounded_time = (time * 30.0).round() / 30.0;

        let entry_rc = self.get_video(entity_id, url);
        let mut entry = entry_rc.borrow_mut();

        if !entry.ready {
            return None;
        }

        let el = entry.el.clone();
        
        let time_delta = time - entry.last_ecs_time;
        entry.last_ecs_time = time;
        let entity_is_playing = is_engine_playing && time_delta > 0.0;
        let diff = (el.current_time() - rounded_time).abs();

        if entity_is_playing {
            if diff > SYNC_TOLERANCE_PLAY {
                el.set_current_time(rounded_time);
            }
            if !entry.playing {
                let _ = el.play();
                entry.playing = true;
            }
        } else {
            if diff > SYNC_TOLERANCE_SCRUB {
                el.set_current_time(rounded_time);
            }
            if entry.playing {
                let _ = el.pause();
                entry.playing = false;
            }
        }

        if el.ready_state() >= 2 {
            let w = el.video_width();
            let h = el.video_height();
            if w > 0 && h > 0 {
                return Some((el, w, h));
            }
        }

        None
    }

    /// Check if a video has enough data loaded for a specific timestamp WITHOUT altering playback state.
    pub fn is_video_ready(&mut self, entity_id: &str, url: &str, time: f64) -> bool {
        let entry_rc = self.get_video(entity_id, url);
        let entry = entry_rc.borrow();
        
        if !entry.ready {
            return false;
        }

        let el = entry.el.clone();
        
        // Wait, HtmlVideoElement's ready_state >= 3 (HAVE_FUTURE_DATA) means we can play smoothly. 
        // 2 (HAVE_CURRENT_DATA) is enough for the precise frame, but for robust buffering we check >= 3.
        if el.ready_state() >= 3 {
            // Also check if we have dimensions
            if el.video_width() > 0 && el.video_height() > 0 {
                return true;
            }
        }
        
        false
    }

    /// Pre-warm a video element for future playback.
    pub fn preload_video(&mut self, entity_id: &str, url: &str, target_time: f64) {
        // Just calling get_video forces the creation of the <video> DOM element with preload="auto"
        let entry_rc = self.get_video(entity_id, url);
        let entry = entry_rc.borrow();
        
        // Optionally, if not playing, we can seek to the target_time to force buffer loading around that area.
        if entry.ready && !entry.playing {
            let diff = (entry.el.current_time() - target_time).abs();
            if diff > 1.0 { // Prevent thrashing the seek head
                entry.el.set_current_time(target_time);
            }
        }
    }

    /// Evict videos not found in active entities set.
    pub fn cleanup_orphaned(&mut self, active_entity_ids: &HashSet<String>) {
        // `retain` automatically calls `Drop` on removed items (which removes them from the DOM)
        self.videos.retain(|id, _| active_entity_ids.contains(id));
    }

    /// Clear entirely. Drops all VideoEntry, triggering DOM remove.
    pub fn clear(&mut self) {
        self.videos.clear();
    }
}
