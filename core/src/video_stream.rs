//! Persistent video stream decoder — keeps FFmpeg running for sequential frame reading.
//!
//! Instead of spawning a new FFmpeg process per frame (~273ms),
//! this keeps a single process alive and reads frames sequentially (~5-10ms).
//!
//! When seeking backwards or jumping, it restarts the process at the new position.

use std::io::Read;
use std::process::{Child, Command, Stdio};

/// A persistent FFmpeg decoder that reads frames sequentially from a pipe.
pub struct VideoStream {
    process: Child,
    path: String,
    width: u32,
    height: u32,
    _frame_size: usize,
    /// Current decoded frame index (0-based from stream start).
    frames_read: u64,
    /// Timestamp (seconds) where the stream was started.
    start_secs: f64,
    fps: f64,
    /// Reusable frame buffer.
    buf: Vec<u8>,
    /// FFmpeg binary path.
    ffmpeg_bin: String,
}

impl VideoStream {
    /// Start a new persistent decoder at the given timestamp.
    ///
    /// Spawns FFmpeg to output continuous raw RGBA frames from `start_secs`.
    pub fn start(
        path: &str,
        start_secs: f64,
        width: u32,
        height: u32,
        fps: f64,
        ffmpeg_bin: &str,
    ) -> Result<Self, String> {
        let frame_size = (width as usize) * (height as usize) * 4;

        let process = Self::spawn_ffmpeg(path, start_secs, width, height, ffmpeg_bin)?;

        Ok(Self {
            process,
            path: path.to_string(),
            width,
            height,
            _frame_size: frame_size,
            frames_read: 0,
            start_secs,
            fps,
            buf: vec![0u8; frame_size],
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

    /// Get the timestamp of the current position (last read frame).
    pub fn current_timestamp(&self) -> f64 {
        self.start_secs + (self.frames_read as f64 / self.fps)
    }

    /// Check if we can serve the requested timestamp without restarting.
    ///
    /// Returns `true` if the requested timestamp is the next sequential frame
    /// (within tolerance).
    pub fn can_serve_sequential(&self, requested_secs: f64) -> bool {
        let next_ts = self.current_timestamp();
        let tolerance = 0.5 / self.fps; // half a frame duration
        (requested_secs - next_ts).abs() < tolerance
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
            &self.ffmpeg_bin,
        )?;
        self.start_secs = timestamp_secs;
        self.frames_read = 0;

        Ok(())
    }

    /// Get a frame at a specific timestamp.
    ///
    /// If sequential, reads from pipe (fast).
    /// If non-sequential, seeks first (slower, but necessary).
    pub fn frame_at(&mut self, timestamp_secs: f64) -> Result<&[u8], String> {
        if !self.can_serve_sequential(timestamp_secs) {
            self.seek(timestamp_secs)?;
        }
        self.read_next_frame()
    }

    /// Spawn FFmpeg subprocess for continuous raw frame output.
    fn spawn_ffmpeg(
        path: &str,
        start_secs: f64,
        width: u32,
        height: u32,
        ffmpeg_bin: &str,
    ) -> Result<Child, String> {
        let ts = format!("{:.4}", start_secs);

        let child = Command::new(ffmpeg_bin)
            .args(["-ss", &ts])
            .args(["-i", path])
            .args(["-vf", &format!("scale={}:{}", width, height)])
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
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
