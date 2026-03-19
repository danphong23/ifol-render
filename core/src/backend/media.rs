use crate::audio::AudioClip;
use crate::export::ExportConfig;
use crate::sysinfo::SysInfo;

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
pub trait MediaBackend {
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
