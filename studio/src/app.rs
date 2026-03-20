//! Studio app — loads Frame JSON, renders viewport, seek/preview/play/export.

use eframe::egui;
use ifol_render_core::{AudioClip, AudioConfig, CoreEngine, Frame, RenderSettings};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ── Audio playback ──
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

// ── Theme ──
const BG_APP: egui::Color32 = egui::Color32::from_rgb(24, 25, 28);
const BG_PANEL: egui::Color32 = egui::Color32::from_rgb(36, 37, 41);
const BG_SURFACE: egui::Color32 = egui::Color32::from_rgb(42, 44, 50);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(88, 101, 242);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(224, 224, 224);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(130, 135, 150);
const GREEN: egui::Color32 = egui::Color32::from_rgb(87, 242, 135);
const RED: egui::Color32 = egui::Color32::from_rgb(237, 66, 69);

/// Scene data loaded from JSON.
struct SceneData {
    settings: RenderSettings,
    frames: Vec<Frame>,
}

/// Playback mode.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PlaybackMode {
    /// Skip to correct frame based on wall-clock time (drop frames if behind).
    Realtime,
    /// Play every frame at exact fps. If render is slow, playback waits.
    Smooth,
}

/// Preview resolution scale.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PreviewScale {
    /// Adapt to viewport display size (most efficient).
    Auto,
    /// Fixed percentage of output resolution.
    Percent(u32),
}

impl PreviewScale {
    fn label(&self) -> &'static str {
        match self {
            PreviewScale::Auto => "Auto (viewport)",
            PreviewScale::Percent(25) => "25%",
            PreviewScale::Percent(50) => "50%",
            PreviewScale::Percent(75) => "75%",
            PreviewScale::Percent(100) => "100%",
            _ => "Custom",
        }
    }
}

/// Export settings dialog state.
struct ExportSettings {
    output_path: String,
    codec_index: usize,
    crf: u32,
    preset_index: usize,
    pixel_format: String,
    ffmpeg_path: String,
    use_custom_resolution: bool,
    export_width: u32,
    export_height: u32,
}

const CODECS: &[(&str, &str)] = &[
    ("H.264 (MP4)", "h264"),
    ("H.265/HEVC (MP4)", "h265"),
    ("VP9 (WebM)", "vp9"),
    ("ProRes (MOV)", "prores"),
    ("PNG Sequence", "png"),
];

const PRESETS: &[(&str, &str)] = &[
    ("Ultrafast (Fastest, Largest)", "ultrafast"),
    ("Superfast", "superfast"),
    ("Veryfast", "veryfast"),
    ("Faster", "faster"),
    ("Fast", "fast"),
    ("Medium (Balanced)", "medium"),
    ("Slow", "slow"),
    ("Slower", "slower"),
];

impl ExportSettings {
    fn new(width: u32, height: u32) -> Self {
        // Auto-detect FFmpeg from tool/ directory relative to executable
        let ffmpeg_path = Self::detect_ffmpeg();
        Self {
            output_path: "output.mp4".into(),
            codec_index: 0,
            crf: 23,
            preset_index: 5, // "medium" defaults to index 5
            pixel_format: "yuv420p".into(),
            ffmpeg_path,
            use_custom_resolution: false,
            export_width: width,
            export_height: height,
        }
    }

    /// Try to find ffmpeg in common locations relative to executable.
    fn detect_ffmpeg() -> String {
        // Check tool/ directory relative to current working directory
        let candidates = [
            "tool/ffmpeg.exe",
            "tool/ffmpeg",
            "../tool/ffmpeg.exe",
            "../tool/ffmpeg",
        ];

        for candidate in &candidates {
            let path = std::path::Path::new(candidate);
            if path.exists() {
                if let Ok(abs) = std::fs::canonicalize(path) {
                    return abs.to_string_lossy().to_string();
                }
            }
        }

        // Also check relative to executable location
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                for name in &["ffmpeg.exe", "ffmpeg"] {
                    let tool_path = exe_dir.join("tool").join(name);
                    if tool_path.exists() {
                        return tool_path.to_string_lossy().to_string();
                    }
                    // One level up
                    if let Some(parent) = exe_dir.parent() {
                        let tool_path = parent.join("tool").join(name);
                        if tool_path.exists() {
                            return tool_path.to_string_lossy().to_string();
                        }
                    }
                }
            }
        }

        "ffmpeg".to_string() // fallback: use system PATH
    }

    fn codec(&self) -> ifol_render_core::VideoCodec {
        match CODECS[self.codec_index].1 {
            "h264" => ifol_render_core::VideoCodec::H264,
            "h265" => ifol_render_core::VideoCodec::H265,
            "vp9" => ifol_render_core::VideoCodec::VP9,
            "prores" => ifol_render_core::VideoCodec::ProRes,
            "png" => ifol_render_core::VideoCodec::PngSequence,
            _ => ifol_render_core::VideoCodec::H264,
        }
    }

    fn extension(&self) -> &str {
        self.codec().extension()
    }
}

/// Active export state — background thread does all rendering.
struct ExportState {
    /// Shared progress counter (updated by background thread).
    progress: Arc<AtomicUsize>,
    total_frames: usize,
    start_time: std::time::Instant,
    output_path: String,
    /// Set to true when background thread finishes.
    done: Arc<AtomicBool>,
    /// Error message from background thread (if any).
    error: Arc<std::sync::Mutex<Option<String>>>,
    /// Cancel flag — main thread sets, background thread checks.
    cancel: Arc<AtomicBool>,
    /// Thread join handle.
    handle: Option<std::thread::JoinHandle<()>>,
}

/// Audio player — streams audio clips dynamically, plays via rodio in sync with timeline.
struct AudioPlayer {
    /// rodio output stream (must be kept alive).
    _stream: OutputStream,
    /// Stream handle for creating sinks.
    stream_handle: OutputStreamHandle,
    /// Current sinks for playback (one per playing clip).
    sinks: Vec<Sink>,
    /// Stored from scene
    clips: Vec<AudioClip>,
    /// Audio configuration (sample rate, channels).
    config: AudioConfig,
    /// FFmpeg binary path.
    ffmpeg_bin: Option<String>,
    /// Total scene duration in seconds.
    total_duration: f64,
}

impl AudioPlayer {
    /// Create a new AudioPlayer (initializes audio output device).
    fn new() -> Option<Self> {
        match OutputStream::try_default() {
            Ok((stream, handle)) => Some(Self {
                _stream: stream,
                stream_handle: handle,
                sinks: Vec::new(),
                clips: Vec::new(),
                config: AudioConfig::default(),
                ffmpeg_bin: None,
                total_duration: 0.0,
            }),
            Err(e) => {
                log::warn!("Failed to open audio device: {}", e);
                None
            }
        }
    }

    /// Load audio from clips (metadata only, no decoding).
    fn load_clips(
        &mut self,
        clips: &[AudioClip],
        total_duration: f64,
        ffmpeg_bin: Option<&str>,
    ) {
        self.stop();
        self.clips = clips.to_vec();
        self.ffmpeg_bin = ffmpeg_bin.map(|s| s.to_string());
        self.total_duration = total_duration;
        log::info!("Audio initialized for streaming ({} clips)", clips.len());
    }

    /// Check if audio clips are present.
    fn has_audio(&self) -> bool {
        !self.clips.is_empty()
    }

    /// Start or resume playback from a specific time position.
    fn play_from(&mut self, time_secs: f64) {
        if !self.has_audio() {
            return;
        }
        self.stop();

        for clip in &self.clips {
            let clip_dur = clip.duration.unwrap_or(self.total_duration);
            let end_time = clip.start_time + clip_dur;

            if time_secs >= clip.start_time && time_secs < end_time {
                // Determine offset into the source clip
                let play_offset = clip.offset + (time_secs - clip.start_time);
                
                // Spawn streaming audio
                if let Ok(stream) = ifol_render_core::StreamingAudio::new(
                    &clip.path, 
                    play_offset, 
                    &self.config, 
                    self.ffmpeg_bin.as_deref()
                ) {
                    let source = StreamingAudioSource {
                        sample_rate: stream.sample_rate,
                        channels: stream.channels as u16,
                        stream,
                        buffer: Vec::new(),
                        pos: 0,
                    };
                    
                    if let Ok(sink) = Sink::try_new(&self.stream_handle) {
                        sink.set_volume(clip.volume);
                        sink.append(source);
                        self.sinks.push(sink);
                    }
                }
            }
        }
    }

    /// Pause audio playback.
    fn pause(&mut self) {
        for sink in &self.sinks {
            sink.pause();
        }
    }

    /// Resume audio playback.
    fn resume(&mut self) {
        for sink in &self.sinks {
            sink.play();
        }
    }

    /// Stop audio playback completely.
    fn stop(&mut self) {
        self.sinks.clear(); // Drop all sinks to stop playback and kill FFmpeg process
    }

    /// Check if any sink is currently playing.
    fn is_playing(&self) -> bool {
        self.sinks.iter().any(|s| !s.is_paused() && !s.empty())
    }
}

/// Custom PCM audio source for rodio using in-flight streaming FFmpeg decoder.
struct StreamingAudioSource {
    stream: ifol_render_core::StreamingAudio,
    buffer: Vec<f32>,
    pos: usize,
    sample_rate: u32,
    channels: u16,
}

impl Iterator for StreamingAudioSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pos >= self.buffer.len() {
            let mut chunk = vec![0.0; 4096];
            let n = self.stream.read_samples(&mut chunk);
            if n == 0 {
                return None; // EOF Stream ended
            }
            chunk.truncate(n);
            self.buffer = chunk;
            self.pos = 0;
        }
        let sample = self.buffer[self.pos];
        self.pos += 1;
        Some(sample)
    }
}

impl Source for StreamingAudioSource {
    fn current_frame_len(&self) -> Option<usize> {
        None // Indicates continuous streaming
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None // Unknown streaming duration
    }
}

/// Studio application state.
pub struct StudioApp {
    scene: Option<SceneData>,
    engine: Option<CoreEngine>,
    current_frame: usize,
    playing: bool,
    playback_mode: PlaybackMode,
    preview_scale: PreviewScale,
    viewport_tex: Option<egui::TextureHandle>,
    pixels: Vec<u8>,
    /// Actual render dimensions (may differ from output).
    render_w: u32,
    render_h: u32,
    dirty: bool,
    status: String,
    /// Playback timing.
    play_start_time: Option<std::time::Instant>,
    play_start_frame: usize,
    /// For smooth mode: time of last rendered frame.
    smooth_last_render: Option<std::time::Instant>,
    scene_path: Option<PathBuf>,
    render_ms: f64,
    /// Last known viewport display size (for auto resolution).
    viewport_display_size: [f32; 2],
    /// Export dialog.
    show_export_dialog: bool,
    export_settings: ExportSettings,
    /// Active export (non-blocking, renders one frame per update).
    export_state: Option<ExportState>,
    /// Audio player for real-time audio playback.
    audio_player: Option<AudioPlayer>,
    /// Audio clips from scene (stored separately for reload).
    audio_clips: Vec<AudioClip>,
    /// Async channel for receiving a parsed scene from background thread.
    loading_scene_rx: Option<std::sync::mpsc::Receiver<Result<ParsedScene, String>>>,
}

/// Structure returned by background thread parsing the JSON scene.
struct ParsedScene {
    settings: RenderSettings,
    frames: Vec<Frame>,
    audio_clips: Vec<AudioClip>,
    path: PathBuf,
}

impl StudioApp {
    pub fn new(_cc: &eframe::CreationContext, scene_path: Option<PathBuf>) -> Self {
        let audio_player = AudioPlayer::new();
        if audio_player.is_some() {
            log::info!("Audio output device initialized");
        }

        let mut app = Self {
            scene: None,
            engine: None,
            current_frame: 0,
            playing: false,
            playback_mode: PlaybackMode::Realtime,
            preview_scale: PreviewScale::Auto,
            viewport_tex: None,
            pixels: Vec::new(),
            render_w: 0,
            render_h: 0,
            dirty: true,
            status: "No scene loaded. File → Open".into(),
            play_start_time: None,
            play_start_frame: 0,
            smooth_last_render: None,
            scene_path: None,
            render_ms: 0.0,
            viewport_display_size: [640.0, 360.0],
            show_export_dialog: false,
            export_settings: ExportSettings::new(1920, 1080),
            export_state: None,
            audio_player,
            audio_clips: Vec::new(),
            loading_scene_rx: None,
        };

        if let Some(path) = scene_path {
            app.load_scene(&path);
        }
        app
    }

    fn load_scene(&mut self, path: &PathBuf) {
        let p = path.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        self.loading_scene_rx = Some(rx);
        self.status = "Loading and parsing Scene JSON...".into();
        
        std::thread::spawn(move || {
            let json = match std::fs::read_to_string(&p) {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(Err(format!("Read error: {}", e)));
                    return;
                }
            };
            let doc: serde_json::Value = match serde_json::from_str(&json) {
                Ok(v) => v,
                Err(e) => {
                    let _ = tx.send(Err(format!("JSON error: {}", e)));
                    return;
                }
            };

            let settings: RenderSettings = doc
                .get("settings")
                .and_then(|s| serde_json::from_value(s.clone()).ok())
                .unwrap_or_default();

            let frames: Vec<Frame> = if let Some(arr) = doc.get("frames") {
                serde_json::from_value(arr.clone()).unwrap_or_default()
            } else if let Some(f) = doc.get("frame") {
                serde_json::from_value(f.clone())
                    .map(|v| vec![v])
                    .unwrap_or_default()
            } else {
                let _ = tx.send(Err("Missing 'frames' key".into()));
                return;
            };

            let audio_clips: Vec<AudioClip> = doc
                .get("audio_clips")
                .and_then(|a| serde_json::from_value(a.clone()).ok())
                .unwrap_or_default();

            let _ = tx.send(Ok(ParsedScene {
                settings,
                frames,
                audio_clips,
                path: p,
            }));
        });
    }

    /// Compute preview render dimensions based on scale mode.
    fn compute_render_size(&self) -> (u32, u32) {
        let scene = match &self.scene {
            Some(s) => s,
            None => return (800, 600),
        };
        let out_w = scene.settings.width;
        let out_h = scene.settings.height;

        match self.preview_scale {
            PreviewScale::Auto => {
                // Use viewport display size (clamped to output)
                let vw = (self.viewport_display_size[0] as u32).max(64).min(out_w);
                let vh = (self.viewport_display_size[1] as u32).max(64).min(out_h);
                // Round to nearest even for GPU compatibility
                let w = (vw / 2) * 2;
                let h = (vh / 2) * 2;
                (w.max(64), h.max(64))
            }
            PreviewScale::Percent(pct) => {
                let w = (out_w * pct / 100).max(64);
                let h = (out_h * pct / 100).max(64);
                ((w / 2) * 2, (h / 2) * 2)
            }
        }
    }

    fn render_current_frame(&mut self) {
        let (rw, rh) = self.compute_render_size();
        let (out_w, out_h) = self.output_size();

        if let (Some(scene), Some(engine)) = (&self.scene, &mut self.engine) {
            if self.current_frame >= scene.frames.len() {
                return;
            }

            // Resize if needed
            if rw != self.render_w || rh != self.render_h {
                engine.resize(rw, rh);
                self.render_w = rw;
                self.render_h = rh;
            }

            // Scale entity coordinates if preview resolution differs from output
            let scale_x = rw as f64 / out_w as f64;
            let scale_y = rh as f64 / out_h as f64;
            let needs_scale = (scale_x - 1.0).abs() > 0.001 || (scale_y - 1.0).abs() > 0.001;

            let frame_data = if needs_scale {
                scene.frames[self.current_frame].scaled(scale_x, scale_y)
            } else {
                scene.frames[self.current_frame].clone()
            };

            let t = std::time::Instant::now();
            self.pixels = engine.render_frame(&frame_data);
            self.render_ms = t.elapsed().as_secs_f64() * 1000.0;
            self.dirty = false;
        }
    }



    fn total_frames(&self) -> usize {
        self.scene.as_ref().map(|s| s.frames.len()).unwrap_or(0)
    }
    fn fps(&self) -> f64 {
        self.scene.as_ref().map(|s| s.settings.fps).unwrap_or(30.0)
    }
    fn current_time(&self) -> f64 {
        self.current_frame as f64 / self.fps()
    }
    fn duration(&self) -> f64 {
        self.total_frames() as f64 / self.fps()
    }
    fn output_size(&self) -> (u32, u32) {
        self.scene
            .as_ref()
            .map(|s| (s.settings.width, s.settings.height))
            .unwrap_or((1280, 720))
    }

    fn start_export(&mut self) {
        let Some(scene) = &self.scene else {
            return;
        };

        let es = &self.export_settings;
        let out_w = if es.use_custom_resolution {
            es.export_width
        } else {
            scene.settings.width
        };
        let out_h = if es.use_custom_resolution {
            es.export_height
        } else {
            scene.settings.height
        };

        let ffmpeg_path = if es.ffmpeg_path.trim().is_empty() {
            None
        } else {
            Some(es.ffmpeg_path.trim().to_string())
        };

        let codec = es.codec();
        let pixel_format = es.pixel_format.clone();
        let crf = es.crf;
        let preset_flag = PRESETS[es.preset_index].1.to_string();
        let fps = scene.settings.fps;
        let output_path = es.output_path.clone();
        let total = scene.frames.len();

        // Clone frame data for the background thread
        let frames: Vec<Frame> = scene.frames.clone();
        let audio_clips = self.audio_clips.clone();

        // Shared state
        let progress = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));
        let error: Arc<std::sync::Mutex<Option<String>>> = Arc::new(std::sync::Mutex::new(None));
        let cancel = Arc::new(AtomicBool::new(false));

        let p = progress.clone();
        let d = done.clone();
        let e = error.clone();
        let c = cancel.clone();
        let out_path = output_path.clone();

        let settings = RenderSettings {
            width: out_w,
            height: out_h,
            fps,
            ..Default::default()
        };

        // Clone ffmpeg path for the background thread
        let export_ffmpeg = ffmpeg_path.clone();

        let handle = std::thread::spawn(move || {
            // Create a dedicated CoreEngine for export (own GPU context)
            let mut engine = CoreEngine::new(settings);
            engine.setup_builtins();
            if let Some(ref fp) = export_ffmpeg {
                engine.set_ffmpeg_path(fp);
            }

            let export_config = ifol_render_core::export::ExportConfig {
                output_path: out_path,
                codec,
                pixel_format,
                crf,
                preset: Some(preset_flag),
                fps: Some(fps),
                width: Some(out_w),
                height: Some(out_h),
                ffmpeg_path: export_ffmpeg,
            };

            if let Err(err) = engine.export_video(
                frames.into_iter(),
                total,
                &audio_clips,
                &export_config,
                |prog| {
                    if c.load(Ordering::Relaxed) {
                        return false; 
                    }
                    p.store(prog.current_frame as usize + 1, Ordering::Release);
                    true
                }
            ) {
                *e.lock().unwrap() = Some(err);
            }

            d.store(true, Ordering::Release);
        });

        self.playing = false;
        self.export_state = Some(ExportState {
            progress,
            total_frames: total,
            start_time: std::time::Instant::now(),
            output_path,
            done,
            error,
            cancel,
            handle: Some(handle),
        });
        self.status = format!("Exporting 0/{} frames...", total);
    }

    fn cancel_export(&mut self) {
        if let Some(state) = &self.export_state {
            state.cancel.store(true, Ordering::Release);
        }
    }
}

impl eframe::App for StudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Handle Async JSON Scene Loading ──
        if let Some(rx) = &self.loading_scene_rx {
            match rx.try_recv() {
                Ok(res) => {
                    self.loading_scene_rx = None;
                    match res {
                        Ok(parsed) => {
                            let total = parsed.frames.len();
                            let mut engine = CoreEngine::new(parsed.settings.clone());
                            engine.setup_builtins();
                            
                            let fpath = self.export_settings.ffmpeg_path.trim();
                            if !fpath.is_empty() {
                                engine.set_ffmpeg_path(fpath);
                            }

                            let duration = total as f64 / parsed.settings.fps;

                            self.scene = Some(SceneData { settings: parsed.settings, frames: parsed.frames });
                            self.engine = Some(engine);
                            self.current_frame = 0;
                            self.playing = false;
                            self.dirty = true;
                            self.scene_path = Some(parsed.path);

                            let audio_count = parsed.audio_clips.len();
                            if !parsed.audio_clips.is_empty() {
                                let ffmpeg_bin = if fpath.is_empty() { None } else { Some(fpath) };
                                if let Some(player) = &mut self.audio_player {
                                    player.load_clips(&parsed.audio_clips, duration, ffmpeg_bin);
                                }
                            }
                            self.audio_clips = parsed.audio_clips;

                            let audio_status = if audio_count > 0 {
                                format!(" | 🔊 {} audio clip(s)", audio_count)
                            } else {
                                String::new()
                            };
                            self.status = format!(
                                "✅ {} frames, {:.1}s @ {:.0}fps{}",
                                total,
                                duration,
                                self.fps(),
                                audio_status
                            );
                        }
                        Err(e) => {
                            self.status = format!("❌ {}", e);
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Still loading... show spinner overlaid
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.spinner();
                            ui.heading("Loading and Parsing Scene JSON...");
                        });
                    });
                    ctx.request_repaint(); // keep animating spinner
                    return; // halt drawing the normal UI while loading!
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.loading_scene_rx = None;
                    self.status = "❌ Load thread panicked".into();
                }
            }
        }

        // ── Export progress polling (background thread does GPU work) ──
        if let Some(state) = &self.export_state {
            let current = state.progress.load(Ordering::Relaxed);
            let is_done = state.done.load(Ordering::Relaxed);

            let elapsed = state.start_time.elapsed().as_secs_f64();
            let fps = if elapsed > 0.0 {
                current as f64 / elapsed
            } else {
                0.0
            };
            let remaining = state.total_frames.saturating_sub(current);
            let eta = if fps > 0.0 {
                remaining as f64 / fps
            } else {
                0.0
            };
            let pct = current as f64 / state.total_frames.max(1) as f64 * 100.0;
            self.status = format!(
                "Exporting {}/{} ({:.0}%) | {:.1}s | ETA {:.1}s | {:.1} fps",
                current, state.total_frames, pct, elapsed, eta, fps
            );

            if is_done {
                let error = state.error.lock().unwrap().take();
                let total = state.total_frames;
                let output = state.output_path.clone();
                let elapsed_final = elapsed;

                // Join the thread
                if let Some(mut state) = self.export_state.take()
                    && let Some(handle) = state.handle.take()
                {
                    let _ = handle.join();
                }

                if let Some(e) = error {
                    if e == "Cancelled" {
                        self.status = format!("⚠️ Export cancelled at {}/{}", current, total);
                    } else {
                        self.status = format!("❌ {}", e);
                    }
                } else {
                    self.status = format!(
                        "✅ Exported {} frames → {} ({:.1}s)",
                        total, output, elapsed_final
                    );
                }
                self.dirty = true;
            } else {
                ctx.request_repaint(); // keep polling
            }
        }

        // ── Playback ──
        if self.playing && self.scene.is_some() && self.export_state.is_none() {
            match self.playback_mode {
                PlaybackMode::Realtime => {
                    // Jump to the correct frame based on wall-clock time
                    let now = std::time::Instant::now();
                    if let Some(start) = self.play_start_time {
                        let elapsed = now.duration_since(start).as_secs_f64();
                        let target = self.play_start_frame + (elapsed * self.fps()) as usize;
                        if target >= self.total_frames() {
                            self.play_start_frame = 0;
                            self.play_start_time = Some(now);
                            self.current_frame = 0;
                            self.dirty = true;
                        } else if target != self.current_frame {
                            self.current_frame = target;
                            self.dirty = true;
                        }
                    } else {
                        self.play_start_time = Some(now);
                        self.play_start_frame = self.current_frame;
                    }
                }
                PlaybackMode::Smooth => {
                    // Render every frame sequentially at exact fps
                    let now = std::time::Instant::now();
                    let frame_dur = 1.0 / self.fps();
                    let should_advance = match self.smooth_last_render {
                        Some(last) => now.duration_since(last).as_secs_f64() >= frame_dur,
                        None => true,
                    };
                    if should_advance {
                        self.current_frame += 1;
                        if self.current_frame >= self.total_frames() {
                            self.current_frame = 0;
                        }
                        self.dirty = true;
                        self.smooth_last_render = Some(now);
                    }
                }
            }
            // Realtime: repaint immediately (wall-clock handles frame skipping)
            // Smooth: schedule at frame interval
            match self.playback_mode {
                PlaybackMode::Realtime => ctx.request_repaint(),
                PlaybackMode::Smooth => {
                    let frame_dur = std::time::Duration::from_secs_f64(1.0 / self.fps());
                    ctx.request_repaint_after(frame_dur);
                }
            }
        }

        // ── Keyboard ──
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                self.playing = !self.playing;
                if self.playing {
                    self.play_start_time = None;
                    self.smooth_last_render = None;
                    // Start audio from current position
                    let time = self.current_time();
                    if let Some(player) = &mut self.audio_player {
                        player.play_from(time);
                    }
                } else {
                    // Pause audio
                    if let Some(player) = &mut self.audio_player {
                        player.pause();
                    }
                }
            }
            if i.key_pressed(egui::Key::ArrowRight)
                && !self.playing
                && self.current_frame + 1 < self.total_frames()
            {
                self.current_frame += 1;
                self.dirty = true;
            }
            if i.key_pressed(egui::Key::ArrowLeft) && !self.playing && self.current_frame > 0 {
                self.current_frame -= 1;
                self.dirty = true;
            }
            if i.key_pressed(egui::Key::Home) {
                self.current_frame = 0;
                self.dirty = true;
                if let Some(player) = &mut self.audio_player {
                    player.stop();
                }
            }
            if i.key_pressed(egui::Key::End) {
                self.current_frame = self.total_frames().saturating_sub(1);
                self.dirty = true;
                if let Some(player) = &mut self.audio_player {
                    player.stop();
                }
            }
        });

        // ── Render if dirty ──
        if self.dirty && self.scene.is_some() {
            self.render_current_frame();
        }

        // ── Theme ──
        let mut style = (*ctx.style()).clone();
        style.visuals.window_fill = BG_APP;
        style.visuals.panel_fill = BG_PANEL;
        style.visuals.override_text_color = Some(TEXT_PRIMARY);
        style.visuals.widgets.noninteractive.bg_fill = BG_SURFACE;
        style.visuals.widgets.inactive.bg_fill = BG_SURFACE;
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 58, 66);
        style.visuals.widgets.active.bg_fill = ACCENT;
        ctx.set_style(style);

        // ═══════════════ TOP BAR ═══════════════
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("📂 Open Frame JSON...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            self.load_scene(&path);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("💾 Export Frame (PNG)...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PNG", &["png"])
                            .set_file_name("frame.png")
                            .save_file()
                            && let (Some(scene), Some(engine)) = (&self.scene, &mut self.engine)
                        {
                            // Export at full resolution
                            let (ow, oh) = (scene.settings.width, scene.settings.height);
                            engine.resize(ow, oh);
                            let pixels = engine.render_frame(&scene.frames[self.current_frame]);
                            match CoreEngine::save_png(&pixels, ow, oh, path.to_str().unwrap()) {
                                Ok(()) => self.status = format!("✅ Saved: {:?}", path),
                                Err(e) => self.status = format!("❌ {}", e),
                            }
                            self.render_w = 0;
                            self.render_h = 0;
                            self.dirty = true;
                        }
                        ui.close_menu();
                    }
                    if ui.button("🎬 Export Video...").clicked() {
                        ui.close_menu();
                        // Initialize export settings from scene, but PRESERVE ffmpeg_path
                        // (otherwise opening the dialog wipes the user's custom FFmpeg setting)
                        if let Some(scene) = &self.scene {
                            let saved_ffmpeg = self.export_settings.ffmpeg_path.clone();
                            self.export_settings =
                                ExportSettings::new(scene.settings.width, scene.settings.height);
                            if !saved_ffmpeg.trim().is_empty() {
                                self.export_settings.ffmpeg_path = saved_ffmpeg;
                            }
                        }
                        self.show_export_dialog = true;
                    }
                });

                ui.separator();

                // Playback mode selector
                ui.label("Mode:");
                let rt_label = if self.playback_mode == PlaybackMode::Realtime {
                    "⏩ Realtime ✓"
                } else {
                    "⏩ Realtime"
                };
                if ui
                    .selectable_label(self.playback_mode == PlaybackMode::Realtime, rt_label)
                    .clicked()
                {
                    self.playback_mode = PlaybackMode::Realtime;
                }
                let sm_label = if self.playback_mode == PlaybackMode::Smooth {
                    "🎞 Smooth ✓"
                } else {
                    "🎞 Smooth"
                };
                if ui
                    .selectable_label(self.playback_mode == PlaybackMode::Smooth, sm_label)
                    .clicked()
                {
                    self.playback_mode = PlaybackMode::Smooth;
                }

                ui.separator();

                // Preview resolution
                ui.menu_button(format!("Res: {}", self.preview_scale.label()), |ui| {
                    if ui
                        .selectable_label(
                            self.preview_scale == PreviewScale::Auto,
                            "Auto (viewport)",
                        )
                        .clicked()
                    {
                        self.preview_scale = PreviewScale::Auto;
                        self.render_w = 0;
                        self.render_h = 0;
                        self.dirty = true;
                        ui.close_menu();
                    }
                    for pct in [25, 50, 75, 100] {
                        let ps = PreviewScale::Percent(pct);
                        if ui
                            .selectable_label(self.preview_scale == ps, format!("{}%", pct))
                            .clicked()
                        {
                            self.preview_scale = ps;
                            self.render_w = 0;
                            self.render_h = 0;
                            self.dirty = true;
                            ui.close_menu();
                        }
                    }
                });

                ui.separator();

                // Scene info
                if let Some(scene) = &self.scene {
                    let (ow, oh) = (scene.settings.width, scene.settings.height);
                    ui.colored_label(
                        TEXT_DIM,
                        format!(
                            "Output: {}×{} | Preview: {}×{} | {:.0}fps | {:.1}s",
                            ow,
                            oh,
                            self.render_w,
                            self.render_h,
                            scene.settings.fps,
                            self.duration()
                        ),
                    );
                }
            });
        });

        // ═══════════════ EXPORT DIALOG ═══════════════
        let mut start_export = false;
        if self.show_export_dialog {
            let mut open = true;
            egui::Window::new("🎬 Export Video")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .min_width(420.0)
                .show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;

                    // ── Output path ──
                    ui.horizontal(|ui| {
                        ui.label("Output:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.export_settings.output_path)
                                .desired_width(280.0),
                        );
                        if ui.button("📂").clicked() {
                            let ext = self.export_settings.extension().to_string();
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter(ext.to_uppercase(), &[&ext])
                                .set_file_name(format!("output.{}", ext))
                                .save_file()
                            {
                                self.export_settings.output_path =
                                    path.to_string_lossy().to_string();
                            }
                        }
                    });

                    ui.separator();

                    // ── Codec ──
                    ui.horizontal(|ui| {
                        ui.label("Codec:");
                        egui::ComboBox::from_id_salt("codec_select")
                            .selected_text(CODECS[self.export_settings.codec_index].0)
                            .show_ui(ui, |ui| {
                                for (i, (label, _)) in CODECS.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut self.export_settings.codec_index,
                                        i,
                                        *label,
                                    );
                                }
                            });
                    });

                    // Update extension when codec changes
                    {
                        let ext = self.export_settings.extension().to_string();
                        if let Some(dot_pos) = self.export_settings.output_path.rfind('.') {
                            let current_ext = &self.export_settings.output_path[dot_pos + 1..];
                            if current_ext != ext {
                                self.export_settings.output_path.truncate(dot_pos + 1);
                                self.export_settings.output_path.push_str(&ext);
                            }
                        }
                    }

                    // ── CRF ──
                    ui.horizontal(|ui| {
                        ui.label("Quality (CRF):");
                        let mut crf = self.export_settings.crf as f32;
                        let quality = if crf < 18.0 {
                            "high"
                        } else if crf < 28.0 {
                            "medium"
                        } else {
                            "low"
                        };
                        ui.add(egui::Slider::new(&mut crf, 0.0..=51.0).step_by(1.0));
                        ui.colored_label(TEXT_DIM, format!("({})", quality));
                        self.export_settings.crf = crf as u32;
                    });

                    // ── Preset ──
                    ui.horizontal(|ui| {
                        ui.label("Speed Preset:");
                        egui::ComboBox::from_id_salt("preset_select")
                            .selected_text(PRESETS[self.export_settings.preset_index].0)
                            .show_ui(ui, |ui| {
                                for (i, (label, _)) in PRESETS.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut self.export_settings.preset_index,
                                        i,
                                        *label,
                                    );
                                }
                            });
                    });

                    // ── Pixel Format ──
                    ui.horizontal(|ui| {
                        ui.label("Pixel Format:");
                        egui::ComboBox::from_id_salt("pix_fmt")
                            .selected_text(&self.export_settings.pixel_format)
                            .show_ui(ui, |ui| {
                                for fmt in &["yuv420p", "yuv444p", "rgb24", "rgba"] {
                                    ui.selectable_value(
                                        &mut self.export_settings.pixel_format,
                                        fmt.to_string(),
                                        *fmt,
                                    );
                                }
                            });
                    });

                    ui.separator();

                    // ── Resolution ──
                    ui.checkbox(
                        &mut self.export_settings.use_custom_resolution,
                        "Custom resolution",
                    );
                    if self.export_settings.use_custom_resolution {
                        ui.horizontal(|ui| {
                            ui.label("Width:");
                            ui.add(
                                egui::DragValue::new(&mut self.export_settings.export_width)
                                    .range(64..=7680)
                                    .speed(2),
                            );
                            ui.label("Height:");
                            ui.add(
                                egui::DragValue::new(&mut self.export_settings.export_height)
                                    .range(64..=4320)
                                    .speed(2),
                            );
                        });
                    } else if let Some(scene) = &self.scene {
                        ui.colored_label(
                            TEXT_DIM,
                            format!(
                                "Resolution: {}×{} (from scene)",
                                scene.settings.width, scene.settings.height
                            ),
                        );
                    }

                    ui.separator();

                    // ── FFmpeg path ──
                    ui.horizontal(|ui| {
                        ui.label("FFmpeg:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.export_settings.ffmpeg_path)
                                .desired_width(250.0)
                                .hint_text("(system PATH)"),
                        );
                        if ui.button("📂").clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Executable", &["exe", ""])
                                .pick_file()
                        {
                            self.export_settings.ffmpeg_path = path.to_string_lossy().to_string();
                        }
                    });

                    ui.separator();

                    // ── Info ──
                    if let Some(scene) = &self.scene {
                        ui.colored_label(
                            TEXT_DIM,
                            format!(
                                "{} frames | {:.1}s | {:.0}fps",
                                scene.frames.len(),
                                self.duration(),
                                scene.settings.fps
                            ),
                        );
                    }

                    // ── Buttons ──
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized(
                                [120.0, 32.0],
                                egui::Button::new(
                                    egui::RichText::new("🚀 Export")
                                        .size(14.0)
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(GREEN),
                            )
                            .clicked()
                        {
                            start_export = true;
                        }
                        if ui
                            .add_sized(
                                [100.0, 32.0],
                                egui::Button::new(egui::RichText::new("Cancel").size(14.0))
                                    .fill(BG_SURFACE),
                            )
                            .clicked()
                        {
                            self.show_export_dialog = false;
                        }
                    });
                });
            if !open {
                self.show_export_dialog = false;
            }
        }
        // Handle export after dialog is done rendering (avoids borrow issues)
        if start_export {
            self.show_export_dialog = false;
            self.start_export();
        }
        // ═══════════════ STATUS BAR ═══════════════
        egui::TopBottomPanel::bottom("status_bar")
            .max_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(TEXT_DIM, &self.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mode_str = match self.playback_mode {
                            PlaybackMode::Realtime => "Realtime",
                            PlaybackMode::Smooth => "Smooth",
                        };
                        ui.colored_label(
                            TEXT_DIM,
                            format!("{:.1}ms | {} | GPU", self.render_ms, mode_str),
                        );
                    });
                });
            });

        // ═══════════════ TIMELINE ═══════════════
        egui::TopBottomPanel::bottom("timeline")
            .min_height(56.0)
            .max_height(72.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    // Play/Pause
                    let play_label = if self.playing { "⏸" } else { "▶" };
                    let play_color = if self.playing { GREEN } else { ACCENT };
                    if ui
                        .add_sized(
                            [34.0, 34.0],
                            egui::Button::new(
                                egui::RichText::new(play_label)
                                    .size(16.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(play_color),
                        )
                        .clicked()
                    {
                        self.playing = !self.playing;
                        if self.playing {
                            self.play_start_time = None;
                            self.smooth_last_render = None;
                            // Start audio from current position
                            let time = self.current_time();
                            if let Some(player) = &mut self.audio_player {
                                player.play_from(time);
                            }
                        } else {
                            // Pause audio
                            if let Some(player) = &mut self.audio_player {
                                player.pause();
                            }
                        }
                    }

                    // Stop
                    if ui
                        .add_sized(
                            [34.0, 34.0],
                            egui::Button::new(
                                egui::RichText::new("⏹")
                                    .size(16.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(BG_SURFACE),
                        )
                        .clicked()
                    {
                        self.playing = false;
                        self.current_frame = 0;
                        self.dirty = true;
                        // Stop audio
                        if let Some(player) = &mut self.audio_player {
                            player.stop();
                        }
                    }

                    ui.separator();

                    // Frame / time
                    ui.colored_label(
                        egui::Color32::WHITE,
                        format!("{:>4} / {}", self.current_frame, self.total_frames()),
                    );
                    ui.colored_label(
                        TEXT_DIM,
                        format!("{:.2}s / {:.2}s", self.current_time(), self.duration()),
                    );
                });

                // Seek slider
                if self.total_frames() > 0 {
                    let mut frame = self.current_frame as f64;
                    let max = (self.total_frames() - 1) as f64;
                    let resp = ui.add(
                        egui::Slider::new(&mut frame, 0.0..=max)
                            .show_value(false)
                            .step_by(1.0),
                    );
                    if resp.changed() {
                        self.current_frame = frame as usize;
                        self.dirty = true;
                        if self.playing {
                            self.playing = false;
                            // Stop audio on seek
                            if let Some(player) = &mut self.audio_player {
                                player.stop();
                            }
                        }
                    }
                }
            });

        // ═══════════════ VIEWPORT ═══════════════
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.scene.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.heading("No scene loaded.\n\nFile → Open");
                });
                return;
            }

            let available = ui.available_size();

            // Track viewport display size for auto resolution
            let (out_w, out_h) = self.output_size();
            let aspect = out_w as f32 / out_h as f32;
            let (draw_w, draw_h) = if available.x / available.y > aspect {
                (available.y * aspect, available.y)
            } else {
                (available.x, available.x / aspect)
            };

            // Update viewport display size for auto resolution computation
            if (self.viewport_display_size[0] - draw_w).abs() > 8.0
                || (self.viewport_display_size[1] - draw_h).abs() > 8.0
            {
                self.viewport_display_size = [draw_w, draw_h];
                if self.preview_scale == PreviewScale::Auto {
                    self.render_w = 0;
                    self.render_h = 0;
                    self.dirty = true;
                }
            }

            // Update texture
            if !self.pixels.is_empty() && self.render_w > 0 && self.render_h > 0 {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [self.render_w as usize, self.render_h as usize],
                    &self.pixels,
                );
                if let Some(tex) = &mut self.viewport_tex {
                    tex.set(image, egui::TextureOptions::LINEAR);
                } else {
                    self.viewport_tex =
                        Some(ctx.load_texture("viewport", image, egui::TextureOptions::LINEAR));
                }
            }

            // Draw centered with aspect ratio
            if let Some(tex) = &self.viewport_tex {
                let offset_x = (available.x - draw_w) / 2.0;
                let offset_y = (available.y - draw_h) / 2.0;
                let rect = egui::Rect::from_min_size(
                    ui.min_rect().min + egui::vec2(offset_x, offset_y),
                    egui::vec2(draw_w, draw_h),
                );

                // Border
                ui.painter().rect_filled(
                    rect.expand(1.0),
                    0.0,
                    egui::Color32::from_rgb(15, 15, 18),
                );
                ui.put(
                    rect,
                    egui::Image::new(egui::load::SizedTexture::new(
                        tex.id(),
                        egui::vec2(draw_w, draw_h),
                    )),
                );
            }

            // Export progress overlay
            if let Some(state) = &self.export_state {
                let rect = ui.max_rect();
                // Dark overlay
                ui.painter()
                    .rect_filled(rect, 0.0, egui::Color32::from_black_alpha(200));

                let center = rect.center();
                let current = state.progress.load(Ordering::Relaxed);
                let pct = current as f64 / state.total_frames.max(1) as f64;
                let elapsed = state.start_time.elapsed().as_secs_f64();
                let fps = if elapsed > 0.0 {
                    current as f64 / elapsed
                } else {
                    0.0
                };
                let remaining = state.total_frames.saturating_sub(current);
                let eta = if fps > 0.0 {
                    remaining as f64 / fps
                } else {
                    0.0
                };

                // Title
                ui.painter().text(
                    center + egui::vec2(0.0, -60.0),
                    egui::Align2::CENTER_CENTER,
                    "Exporting...",
                    egui::FontId::proportional(28.0),
                    egui::Color32::WHITE,
                );

                // Progress bar background
                let bar_w = 360.0_f32;
                let bar_h = 16.0_f32;
                let bar_rect = egui::Rect::from_center_size(
                    center + egui::vec2(0.0, -20.0),
                    egui::vec2(bar_w, bar_h),
                );
                ui.painter().rect_filled(bar_rect, 4.0, BG_SURFACE);

                // Progress bar fill
                let fill_w = bar_w * pct as f32;
                let fill_rect = egui::Rect::from_min_size(bar_rect.min, egui::vec2(fill_w, bar_h));
                ui.painter().rect_filled(fill_rect, 4.0, ACCENT);

                // Percent text on bar
                ui.painter().text(
                    bar_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{:.0}%", pct * 100.0),
                    egui::FontId::proportional(11.0),
                    egui::Color32::WHITE,
                );

                // Stats line
                ui.painter().text(
                    center + egui::vec2(0.0, 10.0),
                    egui::Align2::CENTER_CENTER,
                    format!(
                        "{} / {} frames  |  {:.1}s elapsed  |  ETA {:.1}s  |  {:.1} fps",
                        current, state.total_frames, elapsed, eta, fps
                    ),
                    egui::FontId::proportional(13.0),
                    TEXT_DIM,
                );

                // Cancel button
                let cancel_rect = egui::Rect::from_center_size(
                    center + egui::vec2(0.0, 45.0),
                    egui::vec2(100.0, 30.0),
                );
                let cancel_resp = ui.put(
                    cancel_rect,
                    egui::Button::new(
                        egui::RichText::new("Cancel")
                            .size(13.0)
                            .color(egui::Color32::WHITE),
                    )
                    .fill(RED),
                );
                if cancel_resp.clicked() {
                    self.cancel_export();
                }
            }
        });
    }
}
