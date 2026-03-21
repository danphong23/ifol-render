//! Audio encoder — exports PCM samples to WAV using FFmpeg.

use crate::clip::AudioConfig;

/// Export mixed PCM audio to a WAV file using FFmpeg.
#[cfg(not(target_arch = "wasm32"))]
pub fn export_wav(
    pcm_data: &[f32],
    config: &AudioConfig,
    path: &str,
    ffmpeg_bin: Option<&str>,
) -> Result<(), String> {
    use std::process::{Command, Stdio};

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
