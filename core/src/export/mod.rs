//! Export pipeline — render frames to video/image files.
//!
//! Uses FFmpeg for video encoding.
//! Receives pre-computed frames (no ECS, no timeline logic).

pub mod ffmpeg;

/// Export configuration.
#[derive(Debug, Clone)]
pub struct ExportConfig {
    /// Output file path (e.g., "output.mp4", "output.webm").
    pub output_path: String,
    /// Video codec to use.
    pub codec: VideoCodec,
    /// Pixel format for FFmpeg.
    pub pixel_format: String,
    /// Constant Rate Factor (quality). Lower = better. Typical: 18–28.
    pub crf: u32,
    /// Encoding preset (e.g., "fast", "medium", "ultrafast"). Maps to FFmpeg -preset.
    pub preset: Option<String>,
    /// FPS for the output video.
    pub fps: Option<f64>,
    /// Override width (uses engine width if None).
    pub width: Option<u32>,
    /// Override height (uses engine height if None).
    pub height: Option<u32>,
    /// Path to the FFmpeg binary. If None, searches system PATH.
    pub ffmpeg_path: Option<String>,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            output_path: "output.mp4".into(),
            codec: VideoCodec::H264,
            pixel_format: "yuv420p".into(),
            crf: 23,
            preset: None,
            fps: None,
            width: None,
            height: None,
            ffmpeg_path: None,
        }
    }
}

/// Supported video codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
    VP9,
    ProRes,
    PngSequence,
}

impl VideoCodec {
    pub fn encoder_name(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "libx264",
            VideoCodec::H265 => "libx265",
            VideoCodec::VP9 => "libvpx-vp9",
            VideoCodec::ProRes => "prores_ks",
            VideoCodec::PngSequence => "png",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            VideoCodec::H264 | VideoCodec::H265 => "mp4",
            VideoCodec::VP9 => "webm",
            VideoCodec::ProRes => "mov",
            VideoCodec::PngSequence => "png",
        }
    }

    pub fn parse_codec(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "h264" | "x264" | "libx264" => Some(VideoCodec::H264),
            "h265" | "x265" | "hevc" | "libx265" => Some(VideoCodec::H265),
            "vp9" | "libvpx-vp9" | "webm" => Some(VideoCodec::VP9),
            "prores" => Some(VideoCodec::ProRes),
            "png" | "pngsequence" | "png_sequence" => Some(VideoCodec::PngSequence),
            _ => None,
        }
    }
}

/// Progress info sent during export.
#[derive(Debug, Clone)]
pub struct ExportProgress {
    pub current_frame: u64,
    pub total_frames: u64,
    pub eta_seconds: f64,
    pub export_fps: f64,
}

impl ExportProgress {
    pub fn percent(&self) -> f64 {
        if self.total_frames == 0 {
            0.0
        } else {
            (self.current_frame as f64 / self.total_frames as f64) * 100.0
        }
    }
}
