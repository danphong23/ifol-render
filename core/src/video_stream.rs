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
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

/// Maximum frames to skip forward by reading & discarding.
/// Beyond this, a full seek/restart is cheaper.
const MAX_SKIP_FRAMES: u64 = 6;

struct WorkerState {
    stop_signal: Arc<AtomicBool>,
}

/// A persistent FFmpeg decoder that reads frames sequentially using a background thread.
pub struct VideoStream {
    rx_frames: Receiver<Result<Vec<u8>, String>>,
    tx_recycle: SyncSender<Vec<u8>>,
    worker: Option<WorkerState>,
    path: String,
    width: u32,
    height: u32,
    /// Current decoded frame index (0-based from stream start).
    frames_read: u64,
    /// Timestamp (seconds) where the stream was started.
    start_secs: f64,
    fps: f64,
    /// The currently held frame pixel buffer.
    current_frame: Vec<u8>,
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
        let (worker, rx_frames, tx_recycle) =
            Self::start_worker(path, start_secs, width, height, fps, ffmpeg_bin)?;

        Ok(Self {
            rx_frames,
            tx_recycle,
            worker: Some(worker),
            path: path.to_string(),
            width,
            height,
            frames_read: 0,
            start_secs,
            fps,
            current_frame: Vec::new(),
            ffmpeg_bin: ffmpeg_bin.to_string(),
        })
    }

    fn start_worker(
        path: &str,
        start_secs: f64,
        width: u32,
        height: u32,
        fps: f64,
        ffmpeg_bin: &str,
    ) -> Result<
        (
            WorkerState,
            Receiver<Result<Vec<u8>, String>>,
            SyncSender<Vec<u8>>,
        ),
        String,
    > {
        let mut process = Self::spawn_ffmpeg(path, start_secs, width, height, fps, ffmpeg_bin)?;
        let mut stdout = process.stdout.take().ok_or("FFmpeg stdout not available")?;

        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();

        // 8 frames buffer provides ~260ms of read-ahead cushion at 30fps
        let channel_depth = 8;
        let (tx, rx) = sync_channel(channel_depth);
        let (tx_recycle, rx_recycle) = sync_channel(channel_depth);

        let frame_size = (width as usize) * (height as usize) * 4;
        for _ in 0..channel_depth {
            let _ = tx_recycle.send(vec![0u8; frame_size]);
        }

        std::thread::spawn(move || {
            let mut frames_read = 0;
            while !stop_clone.load(Ordering::Relaxed) {
                // Get a buffer to write into (blocks if main thread hasn't returned any)
                let mut buf = match rx_recycle.recv() {
                    Ok(b) => b,
                    Err(_) => break, // Main thread went away
                };

                // Read from FFmpeg
                match stdout.read_exact(&mut buf) {
                    Ok(_) => {
                        if tx.send(Ok(buf)).is_err() {
                            break; // Main thread went away
                        }
                    }
                    Err(e) => {
                        let ok_to_fail = e.kind() == std::io::ErrorKind::UnexpectedEof;
                        let err_msg = if ok_to_fail {
                            format!("EOF reached after {} frames", frames_read)
                        } else {
                            format!("Read error at frame {}: {}", frames_read, e)
                        };
                        let _ = tx.send(Err(err_msg));
                        break;
                    }
                }
                frames_read += 1;
            }
            // Cleanup process when worker thread terminates
            let _ = process.kill();
            let _ = process.wait();
        });

        Ok((WorkerState { stop_signal }, rx, tx_recycle))
    }

    /// Read the next frame from the pipe.
    ///
    /// Returns a slice of RGBA pixel data. Fast: just `read_exact()`.
    pub fn read_next_frame(&mut self) -> Result<&[u8], String> {
        // Return the previous buffer for recycling
        if !self.current_frame.is_empty() {
            let old_buf = std::mem::take(&mut self.current_frame);
            let _ = self.tx_recycle.try_send(old_buf);
        }

        let new_buf_res = self
            .rx_frames
            .recv()
            .map_err(|_| "Background decode thread disconnected".to_string())?;

        match new_buf_res {
            Ok(buf) => {
                self.current_frame = buf;
                self.frames_read += 1;
                Ok(&self.current_frame)
            }
            Err(e) => Err(e),
        }
    }

    /// Skip N frames by reading and discarding them.
    /// Used for small forward seeks to avoid expensive FFmpeg restarts.
    fn skip_frames(&mut self, count: u64) -> Result<(), String> {
        for i in 0..count {
            if let Err(e) = self.read_next_frame() {
                return Err(format!(
                    "Failed to skip frame {} (skip {}/{}): {}",
                    self.frames_read - 1,
                    i + 1,
                    count,
                    e
                ));
            }
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
        if let Some(worker) = self.worker.take() {
            worker.stop_signal.store(true, Ordering::Relaxed);
        }

        // Start new worker at the requested time
        let (worker, rx_frames, tx_recycle) = Self::start_worker(
            &self.path,
            timestamp_secs,
            self.width,
            self.height,
            self.fps,
            &self.ffmpeg_bin,
        )?;

        self.worker = Some(worker);
        self.rx_frames = rx_frames;
        self.tx_recycle = tx_recycle;
        self.start_secs = timestamp_secs;
        self.frames_read = 0;
        self.current_frame.clear(); // Reset current buffer

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
    /// - `-hwaccel auto`: use hardware-accelerated decoding (QSV/CUVID/DXVA2) if available
    /// - `-ss` before `-i`: fast input seeking
    /// - `-r fps`: force output frame rate to match scene fps
    /// - `-vf scale=WxH`: resize to target dimensions
    /// - `-an`: disable audio (video-only decode, faster)
    /// - `-threads 0`: use all available CPU threads for decode
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
            .args(["-hwaccel", "auto"]) // HW-accelerated decode (QSV/CUVID/DXVA2)
            .args(["-threads", "0"]) // Use all CPU threads for decode
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
        if let Some(worker) = self.worker.take() {
            worker.stop_signal.store(true, Ordering::Relaxed);
        }
    }
}
