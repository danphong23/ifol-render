//! Media Backend Trait — Platform-agnostic interface for video/audio management.
//!
//! This trait abstracts the platform-specific media operations so that the
//! render orchestration logic can work identically on Web (WASM) and Native (EXE).
//!
//! Implementations:
//! - `WasmMediaBackend` (crates/wasm/) — uses HtmlVideoElement + HtmlAudioElement
//! - Future: `NativeMediaBackend` — uses FFmpeg decode + system audio

use crate::ecs::World;

/// Information about a decoded video frame, ready for GPU upload.
pub struct VideoFrameInfo {
    pub width: u32,
    pub height: u32,
}

/// Request for a video frame at a specific time.
pub struct VideoFrameRequest<'a> {
    pub entity_id: &'a str,
    pub asset_url: &'a str,
    pub seek_time: f64,
    pub is_playing: bool,
}

/// Request for audio sync.
pub struct AudioSyncRequest<'a> {
    pub entity_id: &'a str,
    pub asset_url: &'a str,
    pub seek_time: f64,
    pub volume: f32,
}

/// Platform-agnostic media backend trait.
///
/// The render orchestrator calls these methods without knowing whether
/// the underlying implementation uses browser DOM elements or FFmpeg.
pub trait MediaBackend {
    /// Request a video frame for the given entity at the given time.
    /// Returns true if a frame was successfully obtained and uploaded to GPU.
    /// Returns false if the video is still seeking/loading (frame not ready).
    fn request_video_frame(
        &mut self,
        entity_id: &str,
        asset_url: &str,
        seek_time: f64,
        is_playing: bool,
    ) -> bool;

    /// Check if a video has enough data loaded for playback at a specific time.
    fn is_video_ready(&mut self, entity_id: &str, asset_url: &str, seek_time: f64) -> bool;

    /// Pre-warm a video element for future playback (lookahead preloading).
    fn preload_video(&mut self, entity_id: &str, asset_url: &str, target_time: f64);

    /// Evict videos not found in the active entities set.
    fn cleanup_orphaned_videos(&mut self, active_entity_ids: &std::collections::HashSet<String>);

    /// Synchronize audio playback with ECS world state.
    /// `render_scope` filters audio to only the current editing scope.
    fn sync_audio(&mut self, world: &World, is_playing: bool, render_scope: Option<&str>);

    /// Clear all media resources.
    fn clear(&mut self);
}
