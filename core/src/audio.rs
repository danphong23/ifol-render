//! Audio types — re-exported from the standalone `ifol-audio` crate.
//!
//! Core keeps these re-exports for backward compatibility.
//! All audio processing (decode, mix, export, effects) is in `ifol-audio`.

pub use ifol_audio::clip::{AudioClip, AudioConfig, AudioScene};
#[cfg(not(target_arch = "wasm32"))]
pub use ifol_audio::decoder::StreamingAudio;
