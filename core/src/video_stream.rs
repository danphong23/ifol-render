//! Persistent video stream decoder — keeps FFmpeg running for sequential frame reading.
//!
//! Instead of spawning a new FFmpeg process per frame (~273ms),
//! this keeps a single process alive and reads frames sequentially (~5-10ms).
//!
//! Optimizations:
//! - Forces output fps via `-r` flag so timestamps match scene fps exactly
//! - Forward-skip: reads & discards frames for small forward jumps (avoids re-seek)
//! - Only restarts FFmpeg for backward seeks or large forward jumps

use std::io::Read;
use std::process::{Child, Command, Stdio};

/// Maximum frames to skip forward by reading & discarding.
/// Beyond this, a full seek/restart is cheaper.
const MAX_SKIP_FRAMES: u64 = 5;

/// A persistent FFmpeg decoder that reads frames sequentially from a pipe.
pub struct VideoStream {
    process: Child,
    path: String,
    width: u32,
    height: u32,
    /// Current decoded frame index (0-based from stream start).
    frames_read: u64,
    /// Timestamp (seconds) where the stream was started.
    start_secs: f64,
    fps: f64,
    /// Reusable frame buffer.
    buf: Vec<u8>,
    /// Discard buffer (for forward-skip reads).
    discard_buf: Vec<u8>,
    /// FFmpeg binary path.
    ffmpeg_bin: String,
}

impl VideoStream {
    /// Start a new persistent decoder at the given timestamp.
    ///
    /// Spawns FFmpeg to output continuous raw RGBA frames from `start_secs`
    /// at the exact `fps` rate specified.
    pub fn start(
        path: &str,
        start_secs: f64,
        width: u32,
        height: u32,
        fps: f64,
        ffmpeg_bin: &str,
    ) -> Result<Self, String> {
        let frame_size = (width as usize) * (height as usize) * 4;

        let process = Self::spawn_ffmpeg(path, start_secs, width, height, fps, ffmpeg_bin)?;

        Ok(Self {
            process,
            path: path.to_string(),
            width,
            height,
            frames_read: 0,
            start_secs,
            fps,
            buf: vec![0u8; frame_size],
            discard_buf: vec![0u8; frame_size],
            ffmpeg_bin: ffmpeg_bin.to_string(),
        })
    }

    /// Read the next frame from the pipe.
    ///
    /// Returns a slice of RGBA pixel data. Fast: just `read_exact()`.
    pub fn read_next_frame(&mut self) -> Result<&[u8], String> {
        let stdout = self
            .process
            .stdout
            .as_mut()
            .ok_or("FFmpeg stdout not available")?;

        stdout.read_exact(&mut self.buf).map_err(|e| {
            format!(
                "Failed to read video frame {} from pipe: {}",
                self.frames_read, e
            )
        })?;

        self.frames_read += 1;
        Ok(&self.buf)
    }

    /// Skip N frames by reading and discarding them.
    /// Used for small forward seeks to avoid expensive FFmpeg restarts.
    fn skip_frames(&mut self, count: u64) -> Result<(), String> {
        let stdout = self
            .process
            .stdout
            .as_mut()
            .ok_or("FFmpeg stdout not available")?;

        for i in 0..count {
            stdout.read_exact(&mut self.discard_buf).map_err(|e| {
                format!(
                    "Failed to skip frame {} (skip {}/{}): {}",
                    self.frames_read,
                    i + 1,
                    count,
                    e
                )
            })?;
            self.frames_read += 1;
        }

        log::debug!("Skipped {} frames (forward seek)", count);
        Ok(())
    }

    /// Get the timestamp of the current position (next frame to read).
    pub fn current_timestamp(&self) -> f64 {
        self.start_secs + (self.frames_read as f64 / self.fps)
    }

    /// Determine what action is needed for the requested timestamp.
    ///
    /// Returns:
    /// - `SeekAction::Sequential` — next frame matches, just read
    /// - `SeekAction::Skip(n)` — skip n frames forward, then read
    /// - `SeekAction::Restart` — need full FFmpeg restart at new position
    fn classify_seek(&self, requested_secs: f64) -> SeekAction {
        let next_ts = self.current_timestamp();
        let delta = requested_secs - next_ts;
        let frame_duration = 1.0 / self.fps;
        let tolerance = frame_duration * 0.5;

        if delta.abs() < tolerance {
            // Exact match (within half a frame)
            SeekAction::Sequential
        } else if delta > 0.0 {
            // Forward seek
            let frames_ahead = (delta / frame_duration).round() as u64;
            if frames_ahead <= MAX_SKIP_FRAMES {
                SeekAction::Skip(frames_ahead)
            } else {
                SeekAction::Restart
            }
        } else {
            // Backward seek — must restart
            SeekAction::Restart
        }
    }

    /// Seek to a new timestamp by restarting FFmpeg at that position.
    pub fn seek(&mut self, timestamp_secs: f64) -> Result<(), String> {
        // Kill old process
        let _ = self.process.kill();
        let _ = self.process.wait();

        // Start new process at the requested time
        self.process = Self::spawn_ffmpeg(
            &self.path,
            timestamp_secs,
            self.width,
            self.height,
            self.fps,
            &self.ffmpeg_bin,
        )?;
        self.start_secs = timestamp_secs;
        self.frames_read = 0;

        log::debug!("VideoStream seeked to {:.3}s", timestamp_secs);
        Ok(())
    }

    /// Get a frame at a specific timestamp.
    ///
    /// Uses smart seek strategy:
    /// - Sequential: just read next (fast, ~5ms)
    /// - Small forward: skip & discard frames (medium, ~5ms × skip count)
    /// - Backward or big jump: restart FFmpeg (slow, ~100-200ms)
    pub fn frame_at(&mut self, timestamp_secs: f64) -> Result<&[u8], String> {
        match self.classify_seek(timestamp_secs) {
            SeekAction::Sequential => {
                // Perfect — just read the next frame
            }
            SeekAction::Skip(n) => {
                // Small forward jump — skip frames instead of restarting
                self.skip_frames(n)?;
            }
            SeekAction::Restart => {
                // Large jump or backward — must restart FFmpeg
                self.seek(timestamp_secs)?;
            }
        }
        self.read_next_frame()
    }

    /// Spawn FFmpeg subprocess for continuous raw frame output.
    ///
    /// Key flags:
    /// - `-ss` before `-i`: fast input seeking
    /// - `-r fps`: force output frame rate to match scene fps
    /// - `-vf scale=WxH`: resize to target dimensions
    /// - `-an`: disable audio (video-only decode, faster)
    fn spawn_ffmpeg(
        path: &str,
        start_secs: f64,
        width: u32,
        height: u32,
        fps: f64,
        ffmpeg_bin: &str,
    ) -> Result<Child, String> {
        let ts = format!("{:.4}", start_secs);
        let fps_str = format!("{}", fps);

        let child = Command::new(ffmpeg_bin)
            .args(["-ss", &ts])
            .args(["-i", path])
            .args(["-an"]) // disable audio decoding (faster)
            .args(["-vf", &format!("scale={}:{}", width, height)])
            .args(["-r", &fps_str]) // force output frame rate
            .args(["-f", "rawvideo"])
            .args(["-pix_fmt", "rgba"])
            .arg("-v")
            .arg("quiet")
            .arg("pipe:1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                format!(
                    "Failed to start FFmpeg video stream: {e}. \
                     Make sure FFmpeg is installed."
                )
            })?;

        Ok(child)
    }

    /// Get video dimensions.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get frames read count (for diagnostics).
    pub fn frames_read(&self) -> u64 {
        self.frames_read
    }
}

/// Internal seek classification.
enum SeekAction {
    /// Next pipe frame matches — just read.
    Sequential,
    /// Skip N frames forward (read & discard).
    Skip(u64),
    /// Full FFmpeg restart needed.
    Restart,
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
