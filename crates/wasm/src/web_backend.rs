use ifol_render_core::backend::media::{MediaBackend, MediaDecoder, MediaEncoder};
use ifol_render_core::export::ExportConfig;
use ifol_render_core::{SysInfo, VideoInfo};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct WebMediaBackend {
    pub images: Arc<RwLock<HashMap<String, (Vec<u8>, u32, u32)>>>,  // (RGBA, w, h)
    pub video_frames: Arc<RwLock<HashMap<String, (Vec<u8>, u32, u32)>>>,
    pub video_infos: Arc<RwLock<HashMap<String, VideoInfo>>>,
}

impl WebMediaBackend {
    pub fn new() -> Self {
        Self {
            images: Arc::new(RwLock::new(HashMap::new())),
            video_frames: Arc::new(RwLock::new(HashMap::new())),
            video_infos: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl MediaBackend for WebMediaBackend {
    fn read_file_bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.images.read().unwrap().get(path).map(|(rgba, _, _)| rgba.clone())
    }

    fn get_video_info(&self, path: &str) -> Option<VideoInfo> {
        self.video_infos.read().unwrap().get(path).cloned()
    }

    fn get_video_frame(&self, _path: &str, _timestamp: f64) -> Option<Vec<u8>> {
        None // We use get_video_frame_rgba instead
    }

    fn get_video_frame_rgba(&self, path: &str, timestamp: f64) -> Option<(Vec<u8>, u32, u32)> {
        let frames = self.video_frames.read().unwrap();
        // Find nearest cached frame for this path (exact float matching is impossible)
        let prefix = format!("{}@", path);
        let mut best: Option<(&str, f64)> = None;
        for key in frames.keys() {
            if let Some(ts_str) = key.strip_prefix(&prefix) {
                if let Ok(ts) = ts_str.parse::<f64>() {
                    let diff = (ts - timestamp).abs();
                    if best.is_none() || diff < best.unwrap().1 {
                        best = Some((key.as_str(), diff));
                    }
                }
            }
        }
        // Accept frame if within 0.5s tolerance
        if let Some((key, diff)) = best {
            if diff < 0.5 {
                return frames.get(key).cloned();
            }
        }
        None
    }

    fn decode_video(
        &self,
        _path: &str,
        _start_secs: f64,
        _width: u32,
        _height: u32,
        _fps: f64,
    ) -> Result<Box<dyn MediaDecoder>, String> {
        Err("Not supported in WASM".into())
    }

    fn start_export(
        &self,
        _width: u32,
        _height: u32,
        _fps: f64,
        _config: &ExportConfig,
        _sys_info: &SysInfo,
    ) -> Result<Box<dyn MediaEncoder>, String> {
        Err("Not supported in WASM".into())
    }
}
