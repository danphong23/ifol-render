//! FFmpeg-based audio decoder.
//!
//! Decodes any audio format (mp3, wav, aac, video files, etc.) to raw PCM f32 samples.

#[cfg(not(target_arch = "wasm32"))]
use std::process::{Command, Stdio};

use crate::clip::AudioConfig;

/// Decode an audio file to raw PCM f32 samples using FFmpeg.
///
/// Returns interleaved f32 samples at the given sample rate and channels.
/// Supports any format FFmpeg can decode (mp3, wav, aac, video files, etc.).
#[cfg(not(target_arch = "wasm32"))]
pub fn decode_audio(
    path: &str,
    offset: f64,
    duration: Option<f64>,
    speed: f32,
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
    
    // Build atempo chain for speeds other than 1.0
    if (speed - 1.0).abs() > 0.001 {
        let mut cur_speed = speed as f64;
        let mut filters = Vec::new();
        // Stack atempo >= 100.0 (max allowed usually 100.0, but to be safe we use 2.0 or 100.0, modern ffmpeg handles 100.0)
        while cur_speed > 100.0 {
            filters.push("atempo=100.0".to_string());
            cur_speed /= 100.0;
        }
        // Min allowed is 0.5
        while cur_speed < 0.5 && cur_speed > 0.0 {
            filters.push("atempo=0.5".to_string());
            cur_speed /= 0.5;
        }
        if (cur_speed - 1.0).abs() > 0.001 && cur_speed > 0.0 {
            filters.push(format!("atempo={:.4}", cur_speed));
        }
        if !filters.is_empty() {
            cmd.args(["-af", &filters.join(",")]);
        }
    }
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

/// A real-time audio stream from FFmpeg. Reads f32 PCM samples on demand.
#[cfg(not(target_arch = "wasm32"))]
pub struct StreamingAudio {
    process: std::process::Child,
    pub sample_rate: u32,
    pub channels: u32,
}

#[cfg(not(target_arch = "wasm32"))]
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
        cmd.args(["-vn"]);
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
            let bytes_needed = buffer.len() * 4;
            let mut byte_buf = vec![0u8; bytes_needed];

            let mut total_bytes_read = 0;
            while total_bytes_read < bytes_needed {
                match stdout.read(&mut byte_buf[total_bytes_read..]) {
                    Ok(0) => break,
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

#[cfg(not(target_arch = "wasm32"))]
impl Drop for StreamingAudio {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
