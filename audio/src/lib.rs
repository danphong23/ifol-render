//! # ifol-audio
//!
//! Standalone audio processing crate — decode, mix, and export audio clips via FFmpeg.
//!
//! Completely independent of the GPU render pipeline.
//! Can be used standalone as a library or CLI tool.
//!
//! ## Architecture
//!
//! ```text
//! AudioScene JSON → ifol-audio → mixed PCM → WAV/MP3/etc.
//!
//! Per-clip pipeline:
//!   Source file → FFmpeg decode → PCM f32 → [Effect Chain] → volume/fade → mix buffer
//!
//! Master pipeline:
//!   Mix buffer → [Master Effects] → clamp → output
//! ```
//!
//! ## JSON Input
//!
//! ```json
//! {
//!   "config": { "sample_rate": 48000, "channels": 2 },
//!   "total_duration": 10.0,
//!   "clips": [
//!     { "path": "audio.mp3", "start_time": 0.0, "volume": 0.8 },
//!     { "path": "video.mp4", "start_time": 2.0, "duration": 5.0, "fade_in": 0.5 }
//!   ]
//! }
//! ```

pub mod clip;
pub mod decoder;
pub mod effects;
pub mod encoder;
pub mod mixer;

// ── Public re-exports ──
pub use clip::{AudioClip, AudioConfig, AudioScene};
#[cfg(not(target_arch = "wasm32"))]
pub use decoder::{StreamingAudio, decode_audio};
pub use effects::{AudioEffect, EffectInstance, EffectRegistry};
#[cfg(not(target_arch = "wasm32"))]
pub use encoder::export_wav;
#[cfg(not(target_arch = "wasm32"))]
pub use mixer::mix_clips;

/// Process an AudioScene JSON string → mixed PCM f32 samples.
///
/// This is the main entry point for audio processing.
#[cfg(not(target_arch = "wasm32"))]
pub fn process_audio_scene(
    json: &str,
    ffmpeg_bin: Option<&str>,
) -> Result<(Vec<f32>, AudioConfig), String> {
    let scene: AudioScene =
        serde_json::from_str(json).map_err(|e| format!("Invalid audio JSON: {e}"))?;
    process_audio(&scene, ffmpeg_bin)
}

/// Process an AudioScene struct → mixed PCM f32 samples.
#[cfg(not(target_arch = "wasm32"))]
pub fn process_audio(
    scene: &AudioScene,
    ffmpeg_bin: Option<&str>,
) -> Result<(Vec<f32>, AudioConfig), String> {
    let pcm = mix_clips(
        &scene.clips,
        scene.total_duration,
        &scene.config,
        ffmpeg_bin,
    )?;
    Ok((pcm, scene.config.clone()))
}

/// Mux a video file and audio file into a final output using FFmpeg.
#[cfg(not(target_arch = "wasm32"))]
pub fn mux_video_audio(
    video_path: &str,
    audio_path: &str,
    final_path: &str,
    ffmpeg_bin: Option<&str>,
) -> Result<(), String> {
    let bin = ffmpeg_bin.unwrap_or("ffmpeg");

    let output = std::process::Command::new(bin)
        .arg("-y")
        .args(["-i", video_path])
        .args(["-i", audio_path])
        .args(["-c:v", "copy"])
        .args(["-c:a", "aac"])
        .args(["-b:a", "192k"])
        .args(["-shortest"])
        .arg(final_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to mux video+audio: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg mux failed: {}", stderr.trim()));
    }

    log::info!("Muxed video+audio → {}", final_path);
    Ok(())
}
