#[cfg(not(target_arch = "wasm32"))]
pub mod ffmpeg;
pub mod media;

// Re-exports
#[cfg(not(target_arch = "wasm32"))]
pub use ffmpeg::FfmpegMediaBackend;
pub use media::{MediaBackend, MediaDecoder, MediaEncoder};
