//! Time bindings automatically injected into shaders every frame.

use serde::{Deserialize, Serialize};

/// Time information available to shaders and systems each frame.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TimeState {
    /// Seconds since scene start.
    pub global_time: f64,
    /// Current frame index.
    pub frame_index: u64,
    /// Time since last frame (seconds).
    pub delta_time: f64,
    /// Frames per second.
    pub fps: f64,
}

impl TimeState {
    pub fn new(fps: f64) -> Self {
        Self {
            fps,
            ..Default::default()
        }
    }

    /// Advance to the next frame.
    pub fn advance(&mut self) {
        self.frame_index += 1;
        let new_time = self.frame_index as f64 / self.fps;
        self.delta_time = new_time - self.global_time;
        self.global_time = new_time;
    }

    /// Seek to a specific timestamp.
    pub fn seek(&mut self, time: f64) {
        let old_time = self.global_time;
        self.global_time = time;
        self.frame_index = (time * self.fps) as u64;
        self.delta_time = time - old_time;
    }
}

/// Time bindings for a specific entity (relative to entity's timeline).
#[derive(Debug, Clone, Copy, Default)]
pub struct EntityTime {
    /// Seconds since this entity's start on the timeline.
    pub local_time: f64,
    /// 0.0 to 1.0 over the entity's duration.
    pub normalized_time: f64,
    /// Global time reference.
    pub global_time: f64,
    /// Delta time.
    pub delta_time: f64,
    /// Frame index.
    pub frame_index: u64,
}
