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
    /// Lossless PNG image sequence
    PngSequence,
}

impl VideoCodec {
    /// FFmpeg encoder name.
    pub fn encoder_name(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "libx264",
            VideoCodec::H265 => "libx265",
            VideoCodec::VP9 => "libvpx-vp9",
            VideoCodec::ProRes => "prores_ks",
            VideoCodec::PngSequence => "png",
        }
    }

    /// Recommended file extension.
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
    /// Current frame being rendered (0-indexed).
    pub current_frame: u64,
    /// Total number of frames.
    pub total_frames: u64,
    /// Estimated time remaining in seconds.
    pub eta_seconds: f64,
    /// Frames per second of the export process.
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

// ── Legacy export function (for studio migration) ──
// This uses the old ECS pipeline. New code should use CoreEngine::export_video().

/// Legacy: Export a scene to video using the old ECS pipeline.
/// Will be removed when studio migrates to CoreEngine.
pub fn export_video(
    world: &mut crate::ecs::World,
    settings: &crate::scene::RenderSettings,
    config: &ExportConfig,
    renderer: &mut crate::Renderer,
    mut on_progress: impl FnMut(ExportProgress),
) -> Result<(), String> {
    let fps = config.fps.unwrap_or(settings.fps);
    let width = config.width.unwrap_or(settings.width);
    let height = config.height.unwrap_or(settings.height);
    let total_frames = (settings.duration * fps) as u64;

    if total_frames == 0 {
        return Err("Scene duration is 0.".into());
    }

    let mut ffmpeg_pipe = ffmpeg::FfmpegPipe::start(
        width,
        height,
        fps,
        &config.codec,
        &config.pixel_format,
        config.crf,
        &config.output_path,
        config.ffmpeg_path.as_deref(),
    )?;

    let mut time = crate::time::TimeState::new(fps);
    let start = std::time::Instant::now();

    for frame in 0..total_frames {
        time.seek(frame as f64 / fps);

        let pixels = crate::ecs::pipeline::render_frame(world, &time, settings, renderer);
        ffmpeg_pipe.write_frame(&pixels)?;

        let elapsed = start.elapsed().as_secs_f64();
        let export_fps = if elapsed > 0.0 {
            (frame + 1) as f64 / elapsed
        } else {
            0.0
        };
        let remaining = total_frames - frame - 1;
        let eta = if export_fps > 0.0 {
            remaining as f64 / export_fps
        } else {
            0.0
        };

        on_progress(ExportProgress {
            current_frame: frame,
            total_frames,
            eta_seconds: eta,
            export_fps,
        });
    }

    ffmpeg_pipe.finish()
}
