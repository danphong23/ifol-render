//! WASM implementation of the MediaBackend trait.
//!
//! Combines WasmMediaManager (video) and WasmAudioManager (audio)
//! into a single struct implementing the platform-agnostic trait.

use crate::media_manager::WasmMediaManager;
use crate::audio_manager::WasmAudioManager;
use ifol_render_ecs::media::MediaBackend;
use ifol_render_ecs::ecs::World;
use std::collections::HashSet;
use web_sys::HtmlVideoElement;

/// Combined media backend for WASM (browser) platform.
///
/// Wraps the existing WasmMediaManager and WasmAudioManager,
/// exposing them through the platform-agnostic MediaBackend trait.
pub struct WasmMediaBackendImpl {
    pub video: WasmMediaManager,
    pub audio: WasmAudioManager,
    /// Stores the last successfully obtained video element + dimensions per entity
    /// for the caller (lib.rs) to upload to GPU.
    last_video_frames: std::collections::HashMap<String, (HtmlVideoElement, u32, u32)>,
}

impl WasmMediaBackendImpl {
    pub fn new() -> Self {
        Self {
            video: WasmMediaManager::new(),
            audio: WasmAudioManager::new(),
            last_video_frames: std::collections::HashMap::new(),
        }
    }

    /// After calling request_video_frame(), retrieve the HtmlVideoElement
    /// for GPU texture upload. This is WASM-specific (not part of trait).
    pub fn take_video_frame(&mut self, entity_id: &str) -> Option<(HtmlVideoElement, u32, u32)> {
        self.last_video_frames.remove(entity_id)
    }
}

impl MediaBackend for WasmMediaBackendImpl {
    fn request_video_frame(
        &mut self,
        entity_id: &str,
        asset_url: &str,
        seek_time: f64,
        is_playing: bool,
    ) -> bool {
        if let Some((el, w, h)) = self.video.get_video_frame(entity_id, asset_url, seek_time, is_playing) {
            self.last_video_frames.insert(entity_id.to_string(), (el, w, h));
            true
        } else {
            false
        }
    }

    fn is_video_ready(&mut self, entity_id: &str, asset_url: &str, seek_time: f64) -> bool {
        self.video.is_video_ready(entity_id, asset_url, seek_time)
    }

    fn preload_video(&mut self, entity_id: &str, asset_url: &str, target_time: f64) {
        self.video.preload_video(entity_id, asset_url, target_time);
    }

    fn cleanup_orphaned_videos(&mut self, active_entity_ids: &HashSet<String>) {
        self.video.cleanup_orphaned(active_entity_ids);
    }

    fn sync_audio(&mut self, world: &World, is_playing: bool) {
        self.audio.sync_audio(world, is_playing);
    }

    fn clear(&mut self) {
        self.video.clear();
        self.audio.clear();
    }
}
