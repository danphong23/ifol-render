use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlAudioElement;
use ifol_render_ecs::ecs::World;

// ── Configuration Constants ──
const SYNC_TOLERANCE_PLAY: f64 = 0.25; // 250ms drift tolerance when playing (avoids stutter but keeps sync)
const SYNC_TOLERANCE_SCRUB: f64 = 0.05; // 50ms snap when scrubbing audio

pub struct AudioEntry {
    el: HtmlAudioElement,
    ready: bool,
    playing: bool,
    last_volume: f32,
    last_ecs_time: f64,
    // Store closures so they are securely dropped when AudioEntry is dropped
    _on_ready: Option<Closure<dyn FnMut(web_sys::Event)>>,
}

impl Drop for AudioEntry {
    fn drop(&mut self) {
        let _ = self.el.pause();
        self.el.set_src("");
        self.el.load();
        self.el.remove(); // Removes from the DOM tree
    }
}

pub struct WasmAudioManager {
    // Keyed by Entity ID independently
    audios: HashMap<String, Rc<RefCell<AudioEntry>>>,
}

impl WasmAudioManager {
    pub fn new() -> Self {
        Self {
            audios: HashMap::new(),
        }
    }

    pub fn sync_audio(
        &mut self,
        world: &World,
        is_engine_playing: bool,
    ) {
        let storages = &world.storages;
        
        // Track which entity paths we actively touched this frame. 
        // Any playing audio not active should be paused and cleaned up.
        let mut active_urls = HashSet::new();

        for entity in world.entities.iter() {
            // Check AudioSource or VideoSource
            let url = if let Some(audio) = storages.get_component::<ifol_render_ecs::ecs::components::AudioSource>(&entity.id) {
                world.resolve_asset_url(&audio.asset_id).unwrap_or(&audio.asset_id).to_string()
            } else if let Some(video) = storages.get_component::<ifol_render_ecs::ecs::components::VideoSource>(&entity.id) {
                world.resolve_asset_url(&video.asset_id).unwrap_or(&video.asset_id).to_string()
            } else {
                continue;
            };

            // Check if visible and volume > 0
            if !entity.resolved.visible {
                continue;
            }

            let volume = entity.resolved.volume;
            if volume <= 0.001 {
                continue;
            }
            
            // Map audio elements per Entity ID
            let entity_id = entity.id.clone();
            active_urls.insert(entity_id.clone());
            
            let entry_rc = self.get_audio_by_entity(&entity_id, &url);
            let mut entry = entry_rc.borrow_mut();
            
            if !entry.ready {
                continue;
            }

            let el = entry.el.clone();
            
            // Sync volume
            if (entry.last_volume - volume).abs() > 0.01 {
                entry.last_volume = volume;
                el.set_volume(volume as f64);
            }

            // Sync time + playback
            let ecs_time = entity.resolved.playback_time;
            
            // Determine if the entity's time is actually advancing
            let time_delta = ecs_time - entry.last_ecs_time;
            entry.last_ecs_time = ecs_time;
            
            // If the time hasn't changed (or moved backward), the entity is "paused" mechanically.
            let entity_is_playing = is_engine_playing && time_delta > 0.0;

            if entity_is_playing {
                let diff = (el.current_time() - ecs_time).abs();
                
                if diff > SYNC_TOLERANCE_PLAY {
                    el.set_current_time(ecs_time);
                }
                
                if !entry.playing {
                    let _ = el.play();
                    entry.playing = true;
                }
            } else {
                let diff = (el.current_time() - ecs_time).abs();
                if diff > SYNC_TOLERANCE_SCRUB {
                    el.set_current_time(ecs_time);
                }
                
                if entry.playing {
                    let _ = el.pause();
                    entry.playing = false;
                }
            }
        }

        // Cleanup: remove and drop anything in cache that wasn't active this frame
        self.audios.retain(|id, _| active_urls.contains(id));
    }

    fn get_audio_by_entity(&mut self, entity_id: &str, url: &str) -> Rc<RefCell<AudioEntry>> {
        if let Some(entry) = self.audios.get(entity_id) {
            return entry.clone();
        }

        let el = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .create_element("audio")
            .unwrap()
            .dyn_into::<HtmlAudioElement>()
            .unwrap();

        el.set_cross_origin(Some("anonymous"));
        el.set_preload("auto");
        el.set_src(url);
        
        let _ = el.set_attribute("style", "position: absolute; display: none;");

        if let Some(body) = web_sys::window().unwrap().document().unwrap().body() {
            let _ = body.append_child(&el);
        }

        let entry = Rc::new(RefCell::new(AudioEntry {
            el: el.clone(),
            ready: false,
            playing: false,
            last_volume: -1.0,
            last_ecs_time: -1.0,
            _on_ready: None,
        }));

        let entry_weak = Rc::downgrade(&entry);
        let ready_closure = Closure::wrap(Box::new(move |_e: web_sys::Event| {
            if let Some(entry_rc) = entry_weak.upgrade() {
                if let Ok(mut lock) = entry_rc.try_borrow_mut() {
                    lock.ready = true;
                }
            }
        }) as Box<dyn FnMut(web_sys::Event)>);
        let _ = el.add_event_listener_with_callback("canplay", ready_closure.as_ref().unchecked_ref());
        entry.borrow_mut()._on_ready = Some(ready_closure);

        self.audios.insert(entity_id.to_string(), entry.clone());
        entry
    }

    pub fn clear(&mut self) {
        self.audios.clear();
    }
}
