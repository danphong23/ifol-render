//! FFmpeg subprocess pipe — streams raw RGBA frames to FFmpeg for encoding.

use super::VideoCodec;
use std::io::Write;
use std::process::{Child, Command, Stdio};

/// An active FFmpeg encoding process that accepts raw RGBA frames via stdin.
pub struct FfmpegPipe {
    child: Child,
}

impl FfmpegPipe {
    /// Start an FFmpeg encoding process.
    ///
    /// Frames must be written as raw RGBA bytes (width × height × 4 bytes per frame).
    pub fn start(
        width: u32,
        height: u32,
        fps: f64,
        codec: &VideoCodec,
        pixel_format: &str,
        crf: u32,
        output_path: &str,
    ) -> Result<Self, String> {
        let size = format!("{width}x{height}");
        let fps_str = format!("{fps}");

        let mut cmd = Command::new("ffmpeg");

        // Overwrite output without asking
        cmd.arg("-y");

        // Input: raw RGBA from stdin
        cmd.args(["-f", "rawvideo"]);
        cmd.args(["-pix_fmt", "rgba"]);
        cmd.args(["-s", &size]);
        cmd.args(["-r", &fps_str]);
        cmd.args(["-i", "pipe:0"]);

        // Codec-specific args
        match codec {
            VideoCodec::H264 => {
                cmd.args(["-c:v", "libx264"]);
                cmd.args(["-crf", &crf.to_string()]);
                cmd.args(["-preset", "medium"]);
                cmd.args(["-pix_fmt", pixel_format]);
            }
            VideoCodec::H265 => {
                cmd.args(["-c:v", "libx265"]);
                cmd.args(["-crf", &crf.to_string()]);
                cmd.args(["-preset", "medium"]);
                cmd.args(["-pix_fmt", pixel_format]);
            }
            VideoCodec::VP9 => {
                cmd.args(["-c:v", "libvpx-vp9"]);
                cmd.args(["-crf", &crf.to_string()]);
                cmd.args(["-b:v", "0"]); // VBR mode
                cmd.args(["-pix_fmt", pixel_format]);
            }
            VideoCodec::ProRes => {
                cmd.args(["-c:v", "prores_ks"]);
                cmd.args(["-profile:v", "3"]); // ProRes HQ
                cmd.args(["-pix_fmt", "yuva444p10le"]);
            }
            VideoCodec::PngSequence => {
                // For PNG sequence, output_path should be like "frame_%04d.png"
                cmd.args(["-c:v", "png"]);
            }
        }

        // Output
        cmd.arg(output_path);

        // Pipe stdin, suppress stderr noise
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {e}. Make sure FFmpeg is installed and in your PATH."))?;

        Ok(Self { child })
    }

    /// Write one frame of raw RGBA pixels to FFmpeg.
    pub fn write_frame(&mut self, rgba_pixels: &[u8]) -> Result<(), String> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or("FFmpeg stdin not available")?;

        stdin
            .write_all(rgba_pixels)
            .map_err(|e| format!("Failed to write frame to FFmpeg: {e}"))?;

        Ok(())
    }

    /// Close stdin and wait for FFmpeg to finish encoding.
    pub fn finish(mut self) -> Result<(), String> {
        // Drop stdin to signal EOF
        drop(self.child.stdin.take());

        let output = self
            .child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait for FFmpeg: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "FFmpeg exited with code {:?}:\n{}",
                output.status.code(),
                stderr.lines().rev().take(10).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")
            ));
        }

        Ok(())
    }
}
