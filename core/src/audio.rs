//! Audio mixer — processes flat audio clip instructions and outputs mixed PCM.
//!
//! Uses FFmpeg CLI for decoding audio files, then mixes in-process.
//! Completely separate from the GPU render pipeline.
//!
//! ## JSON Input
//!
//! The audio system accepts an `AudioScene` JSON:
//! ```json
//! {
//!   "config": { "sample_rate": 44100, "channels": 2 },
//!   "total_duration": 10.0,
//!   "clips": [
//!     { "path": "audio.mp3", "start_time": 0.0, "volume": 0.8 },
//!     { "path": "video.mp4", "start_time": 2.0, "duration": 5.0, "fade_in": 0.5 }
//!   ]
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};

// ══════════════════════════════════════
// Data Types (JSON-serializable)
// ══════════════════════════════════════

/// An audio clip instruction (flat, pre-computed by frontend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioClip {
    /// Path to source audio file (supports any format FFmpeg can decode).
    pub path: String,
    /// Start time in the output timeline (seconds).
    #[serde(default)]
    pub start_time: f64,
    /// Duration to play (seconds). None = play to end of source.
    #[serde(default)]
    pub duration: Option<f64>,
    /// Offset within the source file (seconds).
    #[serde(default)]
    pub offset: f64,
    /// Volume: 0.0 (silent) to 1.0 (full).
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

// ══════════════════════════════════════
// Processing
// ══════════════════════════════════════

/// Process an AudioScene JSON string → mixed PCM f32 samples.
///
/// This is the main entry point — equivalent to `CoreEngine::render_frame()`.
pub fn process_audio_scene(
    json: &str,
    ffmpeg_bin: Option<&str>,
) -> Result<(Vec<f32>, AudioConfig), String> {
    let scene: AudioScene =
        serde_json::from_str(json).map_err(|e| format!("Invalid audio JSON: {e}"))?;

    process_audio(&scene, ffmpeg_bin)
}

/// Process an AudioScene struct → mixed PCM f32 samples.
pub fn process_audio(
    scene: &AudioScene,
    ffmpeg_bin: Option<&str>,
) -> Result<(Vec<f32>, AudioConfig), String> {
    let pcm = mix_clips(&scene.clips, scene.total_duration, &scene.config, ffmpeg_bin)?;
    Ok((pcm, scene.config.clone()))
}

// ══════════════════════════════════════
// Decode
// ══════════════════════════════════════

/// Decode an audio file to raw PCM f32 samples using FFmpeg.
///
/// Returns interleaved f32 samples at the given sample rate and channels.
/// Supports any format FFmpeg can decode (mp3, wav, aac, video files, etc.).
pub fn decode_audio(
    path: &str,
    offset: f64,
    duration: Option<f64>,
    config: &AudioConfig,
    ffmpeg_bin: Option<&str>,
) -> Result<Vec<f32>, String> {
    let bin = ffmpeg_bin.unwrap_or("ffmpeg");

    let mut cmd = Command::new(bin);

    if offset > 0.0 {
        cmd.args(["-ss", &format!("{:.4}", offset)]);
    }

    cmd.args(["-i", path]);

    if let Some(dur) = duration {
        cmd.args(["-t", &format!("{:.4}", dur)]);
    }

    cmd.args(["-vn"]); // disable video decoding (audio-only, faster)
    cmd.args(["-f", "f32le"]); // raw 32-bit float PCM
    cmd.args(["-acodec", "pcm_f32le"]);
    cmd.args(["-ar", &config.sample_rate.to_string()]);
    cmd.args(["-ac", &config.channels.to_string()]);
    cmd.arg("-v").arg("quiet");
    cmd.arg("pipe:1");

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to decode audio: {e}. Make sure FFmpeg is installed."))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg audio decode failed: {}", stderr.trim()));
    }

    // Convert raw bytes to f32
    let bytes = output.stdout;
    if bytes.len() % 4 != 0 {
        return Err("Audio decode: invalid byte count (not multiple of 4)".into());
    }

    let samples: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    log::debug!(
        "Decoded {} samples from '{}' (offset={:.2}s)",
        samples.len(),
        path,
        offset
    );

    Ok(samples)
}

// ══════════════════════════════════════
// Streaming
// ══════════════════════════════════════

/// A real-time audio stream from FFmpeg. Reads f32 PCM samples on demand.
pub struct StreamingAudio {
    process: std::process::Child,
    pub sample_rate: u32,
    pub channels: u32,
}

impl StreamingAudio {
    /// Start a new streaming audio process.
    pub fn new(
        path: &str,
        offset: f64,
        config: &AudioConfig,
        ffmpeg_bin: Option<&str>,
    ) -> Result<Self, String> {
        let bin = ffmpeg_bin.unwrap_or("ffmpeg");
        let mut cmd = Command::new(bin);

        if offset > 0.0 {
            cmd.args(["-ss", &format!("{:.4}", offset)]);
        }

        cmd.args(["-i", path]);
        cmd.args(["-vn"]); // disable video
        cmd.args(["-f", "f32le"]);
        cmd.args(["-acodec", "pcm_f32le"]);
        cmd.args(["-ar", &config.sample_rate.to_string()]);
        cmd.args(["-ac", &config.channels.to_string()]);
        cmd.arg("-v").arg("quiet");
        cmd.arg("pipe:1");

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let process = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn FFmpeg audio stream: {e}"))?;

        log::debug!(
            "Started streaming audio from '{}' (offset={:.2}s)",
            path,
            offset
        );

        Ok(Self {
            process,
            sample_rate: config.sample_rate,
            channels: config.channels,
        })
    }

    /// Read raw f32 samples. Returns 0 if EOF or error.
    pub fn read_samples(&mut self, buffer: &mut [f32]) -> usize {
        use std::io::Read;
        if let Some(stdout) = self.process.stdout.as_mut() {
            // f32 is 4 bytes
            let bytes_needed = buffer.len() * 4;
            let mut byte_buf = vec![0u8; bytes_needed];
            
            let mut total_bytes_read = 0;
            while total_bytes_read < bytes_needed {
                match stdout.read(&mut byte_buf[total_bytes_read..]) {
                    Ok(0) => break, // EOF
                    Ok(n) => total_bytes_read += n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }

            let samples_read = total_bytes_read / 4;
            for i in 0..samples_read {
                let chunk = &byte_buf[i * 4..(i + 1) * 4];
                buffer[i] = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            }
            
            samples_read
        } else {
            0
        }
    }
}

impl Drop for StreamingAudio {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

// ══════════════════════════════════════
// Mix
// ══════════════════════════════════════

/// Mix multiple audio clips into a single PCM buffer.
///
/// Returns interleaved f32 samples for the full `duration` of the output.
pub fn mix_clips(
    clips: &[AudioClip],
    total_duration: f64,
    config: &AudioConfig,
    ffmpeg_bin: Option<&str>,
) -> Result<Vec<f32>, String> {
    let total_samples =
        (total_duration * config.sample_rate as f64) as usize * config.channels as usize;
    let mut output = vec![0.0f32; total_samples];

    for (clip_idx, clip) in clips.iter().enumerate() {
        log::info!(
            "Decoding audio clip {}/{}: '{}' (start={:.2}s, vol={:.1})",
            clip_idx + 1,
            clips.len(),
            clip.path,
            clip.start_time,
            clip.volume
        );

        // Decode source audio
        let samples = decode_audio(&clip.path, clip.offset, clip.duration, config, ffmpeg_bin)?;

        if samples.is_empty() {
            log::warn!("Audio clip '{}' decoded to 0 samples", clip.path);
            continue;
        }

        // Calculate sample positions
        let start_sample =
            (clip.start_time * config.sample_rate as f64) as usize * config.channels as usize;
        let fade_in_samples =
            (clip.fade_in * config.sample_rate as f64) as usize * config.channels as usize;
        let fade_out_samples =
            (clip.fade_out * config.sample_rate as f64) as usize * config.channels as usize;

        // Mix into output
        for (i, &sample) in samples.iter().enumerate() {
            let out_idx = start_sample + i;
            if out_idx >= total_samples {
                break;
            }

            // Apply volume
            let mut vol = clip.volume;

            // Fade in
            if i < fade_in_samples && fade_in_samples > 0 {
                vol *= i as f32 / fade_in_samples as f32;
            }

            // Fade out
            let remaining = samples.len().saturating_sub(i);
            if remaining < fade_out_samples && fade_out_samples > 0 {
                vol *= remaining as f32 / fade_out_samples as f32;
            }

            output[out_idx] += sample * vol;
        }
    }

    // Clamp to [-1.0, 1.0]
    for s in &mut output {
        *s = s.clamp(-1.0, 1.0);
    }

    Ok(output)
}

// ══════════════════════════════════════
// Export
// ══════════════════════════════════════

/// Export mixed audio to a WAV file using FFmpeg.
pub fn export_wav(
    pcm_data: &[f32],
    config: &AudioConfig,
    path: &str,
    ffmpeg_bin: Option<&str>,
) -> Result<(), String> {
    let bin = ffmpeg_bin.unwrap_or("ffmpeg");

    let mut child = Command::new(bin)
        .arg("-y")
        .args(["-f", "f32le"])
        .args(["-ar", &config.sample_rate.to_string()])
        .args(["-ac", &config.channels.to_string()])
        .args(["-i", "pipe:0"])
        .args(["-c:a", "pcm_s16le"])
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start FFmpeg for WAV export: {e}"))?;

    // Write PCM data
    {
        let stdin = child.stdin.as_mut().ok_or("FFmpeg stdin not available")?;
        let bytes: Vec<u8> = pcm_data.iter().flat_map(|s| s.to_le_bytes()).collect();
        std::io::Write::write_all(stdin, &bytes)
            .map_err(|e| format!("Failed to write audio data: {e}"))?;
    }
    // Drop stdin to signal EOF
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for FFmpeg: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg WAV export failed: {}", stderr.trim()));
    }

    Ok(())
}
