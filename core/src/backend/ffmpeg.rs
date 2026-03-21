use std::io::Read;
use std::process::{Child, Command, Stdio};

use crate::export::ExportConfig;
use crate::sysinfo::SysInfo;

use super::media::{MediaBackend, MediaDecoder, MediaEncoder};

/// FFmpeg implementation of MediaBackend for Desktop platforms.
pub struct FfmpegMediaBackend {
    ffmpeg_bin: String,
}

impl FfmpegMediaBackend {
    pub fn new(ffmpeg_bin: &str) -> Self {
        Self {
            ffmpeg_bin: ffmpeg_bin.to_string(),
        }
    }
}

impl MediaBackend for FfmpegMediaBackend {
    fn decode_video(
        &self,
        path: &str,
        start_secs: f64,
        width: u32,
        height: u32,
        fps: f64,
    ) -> Result<Box<dyn MediaDecoder>, String> {
        let mut cmd = Command::new(&self.ffmpeg_bin);

        if start_secs > 0.0 {
            cmd.args(["-ss", &format!("{:.4}", start_secs)]);
        }

        cmd.args(["-i", path])
            .args(["-vf", &format!("scale={}:{}", width, height)])
            .args(["-r", &format!("{}", fps)])
            .args(["-an"]) // disable audio
            .args(["-f", "rawvideo"])
            .args(["-pix_fmt", "rgba"])
            .arg("-v")
            .arg("quiet")
            .arg("pipe:1");

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn FFmpeg video decoder: {e}"))?;

        Ok(Box::new(FfmpegVideoDecoder { child }))
    }

    fn start_export(
        &self,
        width: u32,
        height: u32,
        fps: f64,
        config: &ExportConfig,
        sys_info: &SysInfo,
    ) -> Result<Box<dyn MediaEncoder>, String> {
        // Use best hardware encoder detected
        let encoder = sys_info.best_h264_encoder();
        
        let mut cmd = Command::new(&self.ffmpeg_bin);
        cmd.arg("-y") // Overwrite output files without asking
            .args(["-f", "rawvideo"])
            .args(["-vcodec", "rawvideo"])
            .args(["-s", &format!("{}x{}", width, height)])
            .args(["-pix_fmt", "rgba"])
            .args(["-r", &format!("{}", fps)])
            .args(["-i", "pipe:0"]) // input from stdin
            .args(["-c:v", encoder]);

        // Encoder-specific quality/speed flags
        match encoder {
            "h264_nvenc" => {
                cmd.args(["-preset", "p4"]); // p4 is a good balance for NVENC
                cmd.args(["-cq", &config.crf.to_string()]); // NVENC uses -cq instead of -crf
            }
            "h264_qsv" | "h264_amf" => {
                cmd.args(["-preset", "fast"]); // fast preset for intel/amd
                cmd.args(["-crf", &config.crf.to_string()]);
            }
            _ => { // libx264 fallback
                cmd.args(["-preset", "ultrafast"]); 
                cmd.args(["-crf", &config.crf.to_string()]);
            }
        }

        // Output pixel format (crucial for web/player compatibility and speed)
        cmd.args(["-pix_fmt", &config.pixel_format]);
        cmd.arg(&config.output_path);

        let child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped()) // pipe stderr to read progress
            .spawn()
            .map_err(|e| format!("Failed to spawn FFmpeg video encoder: {e}"))?;

        log::info!("Started FFmpeg export using hardware encoder: {}", encoder);

        Ok(Box::new(FfmpegVideoEncoder { child: Some(child) }))
    }

}

/// Active FFmpeg Video Decoder Stream
struct FfmpegVideoDecoder {
    child: Child,
}

impl MediaDecoder for FfmpegVideoDecoder {
    fn read_rgba_frame(&mut self, buffer: &mut [u8]) -> usize {
        if let Some(stdout) = &mut self.child.stdout {
            let mut total_read = 0;
            while total_read < buffer.len() {
                match stdout.read(&mut buffer[total_read..]) {
                    Ok(0) => break,
                    Ok(n) => total_read += n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            total_read
        } else {
            0
        }
    }

    fn close(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for FfmpegVideoDecoder {
    fn drop(&mut self) {
        self.close();
    }
}

/// Active FFmpeg Video Encoder Stream
struct FfmpegVideoEncoder {
    child: Option<Child>,
}

impl MediaEncoder for FfmpegVideoEncoder {
    fn write_rgba_frame(&mut self, buffer: &[u8]) -> Result<(), String> {
        if let Some(child) = &mut self.child {
            if let Some(stdin) = &mut child.stdin {
                use std::io::Write;
                return stdin.write_all(buffer).map_err(|e| format!("Failed to pipe frame to encoder: {e}"));
            }
        }
        Err("Encoder stdin is closed".to_string())
    }

    fn close(&mut self) -> Result<(), String> {
        if let Some(mut child) = self.child.take() {
            // Drop stdin to signal EOF so FFmpeg flushes and completes
            drop(child.stdin.take());
            
            let output = child.wait_with_output().map_err(|e| format!("Failed waiting for FFmpeg: {e}"))?;
            
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("FFmpeg export failed: {}", stderr.trim()));
            }
        }
        Ok(())
    }
}
