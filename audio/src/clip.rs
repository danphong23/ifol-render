//! Audio clip data types (JSON-serializable).
//!
//! These are the audio equivalent of `FlatEntity` for the render engine.

use serde::{Deserialize, Serialize};

/// An audio clip instruction (flat, pre-computed by frontend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioClip {
    /// Path to the audio file (any format FFmpeg can decode, including video files).
    pub path: String,
    /// Start time in the output timeline (seconds).
    #[serde(default)]
    pub start_time: f64,
    /// Duration to play (seconds). None = play to end.
    pub duration: Option<f64>,
    /// Offset within the source file — skip N seconds (seconds).
    #[serde(default)]
    pub offset: f64,
    /// Playback speed multiplier. Default: 1.0
    #[serde(default = "default_speed")]
    pub speed: f32,
    /// Volume: 0.0 (silent) to 1.0 (full). Default: 1.0
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// Fade in duration (seconds).
    #[serde(default)]
    pub fade_in: f64,
    /// Fade out duration (seconds).
    #[serde(default)]
    pub fade_out: f64,
}

fn default_volume() -> f32 {
    1.0
}

fn default_speed() -> f32 {
    1.0
}

/// Audio output configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_channels")]
    pub channels: u32,
}

fn default_sample_rate() -> u32 {
    44100
}
fn default_channels() -> u32 {
    2
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 2,
        }
    }
}

/// Complete audio scene — the JSON input format for the audio system.
///
/// This is the audio equivalent of `Frame` for the render engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioScene {
    /// Audio output configuration.
    #[serde(default)]
    pub config: AudioConfig,
    /// Total output duration in seconds.
    pub total_duration: f64,
    /// Audio clips to mix together.
    #[serde(default)]
    pub clips: Vec<AudioClip>,
}
