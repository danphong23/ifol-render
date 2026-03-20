use crate::audio::AudioClip;
use crate::export::ExportConfig;
use crate::types::{SysInfo, VideoInfo};

/// Represents an active video decoding stream (extracts frames).
pub trait MediaDecoder {
    /// Read the next frame into the buffer as raw RGBA data.
    /// Returns the number of bytes read (0 if EOF).
    fn read_rgba_frame(&mut self, buffer: &mut [u8]) -> usize;
    /// Close and kill the decoder.
    fn close(&mut self);
}

/// Represents an active video encoding stream (muxes and compresses frames).
pub trait MediaEncoder: Send {
    /// Write a raw RGBA frame to the encoder.
    fn write_rgba_frame(&mut self, buffer: &[u8]) -> Result<(), String>;
    /// Finalize and close the encoder (blocking wait).
    fn close(&mut self) -> Result<(), String>;
}

/// A media backend handles all OS-specific or environment-specific (Desktop FFmpeg vs WASM WebCodecs) 
/// video and audio operations.
///
/// Consumer provides an implementation when creating `CoreEngine`.
/// Core never does I/O directly — it always goes through this trait.
pub trait MediaBackend {
    /// Read file bytes from a path. 
    /// On native: default reads from filesystem.
    /// On WASM: must be overridden (e.g. fetch from HTTP cache).
    fn read_file_bytes(&self, _path: &str) -> Option<Vec<u8>> {
        #[cfg(not(target_arch = "wasm32"))]
        { std::fs::read(_path).ok() }
        #[cfg(target_arch = "wasm32")]
        { None }
    }
    
    /// Get video metadata (width, height, fps, duration, codec).
    fn get_video_info(&self, _path: &str) -> Option<VideoInfo> { None }
    
    /// Get a video frame as raw uncompressed or JPEG/PNG blob at timestamp.
    fn get_video_frame(&self, _path: &str, _timestamp: f64) -> Option<Vec<u8>> { None }
    
    /// Get a video frame as raw RGBA pixels + dimensions at timestamp.
    fn get_video_frame_rgba(&self, _path: &str, _timestamp: f64) -> Option<(Vec<u8>, u32, u32)> { None }

    /// Spawn a video decoder stream for a specific file at a specific offset.
    fn decode_video(
        &self, 
        path: &str, 
        start_secs: f64, 
        width: u32, 
        height: u32, 
        fps: f64,
    ) -> Result<Box<dyn MediaDecoder>, String>;

    /// Spawn a video encoder that will output the final muxed video.
    fn start_export(
        &self, 
        width: u32,
        height: u32,
        fps: f64,
        config: &ExportConfig, 
        sys_info: &SysInfo,
    ) -> Result<Box<dyn MediaEncoder>, String>;
    
    /// Statically mix audio clips and output to a WAV file before video muxing.
    fn export_mixed_audio(
        &self,
        clips: &[AudioClip],
        duration: f64,
        sample_rate: u32,
        channels: u32,
        out_path: &str,
    ) -> Result<(), String>;

    /// Mux a fast video track and an audio track into a single final file.
    fn mux_video_audio(
        &self,
        video_path: &str,
        audio_path: &str,
        final_path: &str,
    ) -> Result<(), String>;
}
