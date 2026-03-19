pub mod ffmpeg;
pub mod media;

// Re-exports
pub use ffmpeg::FfmpegMediaBackend;
pub use media::{MediaBackend, MediaDecoder, MediaEncoder};
