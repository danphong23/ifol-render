//! PCM audio mixer — combines multiple decoded clips into a single buffer.

use crate::clip::{AudioClip, AudioConfig};
#[cfg(not(target_arch = "wasm32"))]
use crate::decoder::decode_audio;

/// Mix multiple audio clips into a single PCM buffer.
///
/// Returns interleaved f32 samples for the full `duration` of the output.
#[cfg(not(target_arch = "wasm32"))]
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

        // Decode source audio with dynamic speed
        let samples = decode_audio(&clip.path, clip.offset, clip.duration, clip.speed, config, ffmpeg_bin)?;

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
