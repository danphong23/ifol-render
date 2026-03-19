//! Video frame decoder — extracts individual frames from video files via FFmpeg.
//!
//! Uses FFmpeg CLI for maximum compatibility. Each call extracts a single frame
//! at a given timestamp, returning raw RGBA pixels.
//!
//! Design: stateless per-call (no long-running process). Fast for random access
//! because FFmpeg uses `-ss` before `-i` for input seeking.

use std::process::{Command, Stdio};

/// Video metadata from ffprobe.
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_secs: f64,
    pub codec: String,
}

/// Probe video metadata using ffprobe.
///
/// Returns width, height, fps, duration, codec.
pub fn probe(path: &str, ffmpeg_bin: Option<&str>) -> Result<VideoInfo, String> {
    let bin = ffmpeg_bin.unwrap_or("ffprobe");

    let output = Command::new(bin)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
            "-select_streams",
            "v:0",
            path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            format!("Failed to run ffprobe: {e}. Make sure FFmpeg is installed and in your PATH.")
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe failed: {}", stderr.trim()));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse ffprobe output: {e}"))?;

    let stream = json["streams"]
        .as_array()
        .and_then(|s| s.first())
        .ok_or("No video stream found")?;

    let width = stream["width"].as_u64().ok_or("Missing video width")? as u32;
    let height = stream["height"].as_u64().ok_or("Missing video height")? as u32;

    // Parse FPS from r_frame_rate "30/1" or "30000/1001"
    let fps_str = stream["r_frame_rate"].as_str().unwrap_or("30/1");
    let fps = parse_fps(fps_str);

    // Duration from format or stream
    let duration_secs = json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| {
            stream["duration"]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
        })
        .unwrap_or(0.0);

    let codec = stream["codec_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    Ok(VideoInfo {
        width,
        height,
        fps,
        duration_secs,
        codec,
    })
}

/// Decode a single video frame at the given timestamp.
///
/// Uses `-ss` before `-i` for fast input seeking.
/// Returns (RGBA pixels, width, height).
///
/// If `out_width`/`out_height` are specified, the frame is scaled.
pub fn decode_frame(
    path: &str,
    timestamp_secs: f64,
    out_width: Option<u32>,
    out_height: Option<u32>,
    ffmpeg_bin: Option<&str>,
) -> Result<(Vec<u8>, u32, u32), String> {
    let bin = ffmpeg_bin.unwrap_or("ffmpeg");

    let ts = format!("{:.4}", timestamp_secs);

    let mut cmd = Command::new(bin);
    cmd.args(["-ss", &ts]);
    cmd.args(["-i", path]);
    cmd.args(["-frames:v", "1"]);

    // Scale filter if requested
    if let (Some(w), Some(h)) = (out_width, out_height) {
        cmd.args(["-vf", &format!("scale={}:{}", w, h)]);
    }

    cmd.args(["-f", "rawvideo"]);
    cmd.args(["-pix_fmt", "rgba"]);
    cmd.arg("pipe:1");

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().map_err(|e| {
        format!("Failed to run FFmpeg for frame decode: {e}. Make sure FFmpeg is installed.")
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Take last few lines of stderr for error context
        let err_lines: String = stderr
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!("FFmpeg frame decode failed: {}", err_lines));
    }

    let pixels = output.stdout;

    // Determine actual dimensions
    let (w, h) = if let (Some(w), Some(h)) = (out_width, out_height) {
        (w, h)
    } else {
        // Need to probe for actual dimensions
        let info = probe(path, Some(bin)).or_else(|_| {
            // Try with ffprobe binary name
            let probe_bin = if bin.contains("ffmpeg") {
                bin.replace("ffmpeg", "ffprobe")
            } else {
                "ffprobe".to_string()
            };
            probe(path, Some(&probe_bin))
        })?;
        (info.width, info.height)
    };

    let expected_size = (w * h * 4) as usize;
    if pixels.len() != expected_size {
        return Err(format!(
            "Frame size mismatch: got {} bytes, expected {} ({}x{}x4)",
            pixels.len(),
            expected_size,
            w,
            h
        ));
    }

    Ok((pixels, w, h))
}

/// Parse FPS from ffprobe "r_frame_rate" format: "30/1" or "30000/1001".
fn parse_fps(s: &str) -> f64 {
    if let Some((num_s, den_s)) = s.split_once('/') {
        let num: f64 = num_s.parse().unwrap_or(30.0);
        let den: f64 = den_s.parse().unwrap_or(1.0);
        if den > 0.0 { num / den } else { 30.0 }
    } else {
        s.parse().unwrap_or(30.0)
    }
}
