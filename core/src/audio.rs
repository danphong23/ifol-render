//! Audio mixer — processes flat audio clip instructions and outputs mixed PCM.
//!
//! Uses FFmpeg CLI for decoding audio files, then mixes in-process.
//! Completely separate from the GPU render pipeline.

use std::process::{Command, Stdio};

/// An audio clip instruction (flat, pre-computed by frontend).
#[derive(Debug, Clone)]
pub struct AudioClip {
    /// Path to source audio file.
    pub path: String,
    /// Start time in the output timeline (seconds).
    pub start_time: f64,
    /// Duration to play (seconds). None = play to end.
    pub duration: Option<f64>,
    /// Offset within the source file (seconds).
    pub offset: f64,
    /// Volume: 0.0 (silent) to 1.0 (full).
    pub volume: f32,
    /// Fade in duration (seconds).
    pub fade_in: f64,
    /// Fade out duration (seconds).
    pub fade_out: f64,
}

/// Audio output configuration.
#[derive(Debug, Clone)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 2,
        }
    }
}

/// Decode an audio file to raw PCM f32 samples using FFmpeg.
///
/// Returns interleaved f32 samples at the given sample rate and channels.
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

    Ok(samples)
}

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

    for clip in clips {
        // Decode source audio
        let samples = decode_audio(&clip.path, clip.offset, clip.duration, config, ffmpeg_bin)?;

        if samples.is_empty() {
            continue;
        }

        // Calculate sample positions
        let start_sample =
            (clip.start_time * config.sample_rate as f64) as usize * config.channels as usize;
        let _clip_duration = clip
            .duration
            .unwrap_or(samples.len() as f64 / (config.sample_rate as f64 * config.channels as f64));
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
