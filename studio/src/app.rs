//! Studio app — loads Frame JSON, renders viewport, seek/preview/play/export.

use eframe::egui;
use ifol_render_core::{CoreEngine, Frame, RenderSettings};
use std::path::PathBuf;

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
    /// Export state.
    exporting: bool,
    export_progress: f64,
}

impl StudioApp {
    pub fn new(_cc: &eframe::CreationContext, scene_path: Option<PathBuf>) -> Self {
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
            exporting: false,
            export_progress: 0.0,
        };

        if let Some(path) = scene_path {
            app.load_scene(&path);
        }
        app
    }

    fn load_scene(&mut self, path: &PathBuf) {
        let json = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => { self.status = format!("❌ Read error: {}", e); return; }
        };
        let doc: serde_json::Value = match serde_json::from_str(&json) {
            Ok(v) => v,
            Err(e) => { self.status = format!("❌ JSON error: {}", e); return; }
        };

        let settings: RenderSettings = doc.get("settings")
            .and_then(|s| serde_json::from_value(s.clone()).ok())
            .unwrap_or_default();

        let frames: Vec<Frame> = if let Some(arr) = doc.get("frames") {
            serde_json::from_value(arr.clone()).unwrap_or_default()
        } else if let Some(f) = doc.get("frame") {
            serde_json::from_value(f.clone()).map(|v| vec![v]).unwrap_or_default()
        } else {
            self.status = "❌ Missing 'frames' key".into(); return;
        };

        let total = frames.len();
        let mut engine = CoreEngine::new(settings.clone());
        engine.setup_builtins();

        self.scene = Some(SceneData { settings, frames });
        self.engine = Some(engine);
        self.current_frame = 0;
        self.playing = false;
        self.dirty = true;
        self.scene_path = Some(path.clone());
        self.status = format!("✅ {} frames, {:.1}s @ {:.0}fps",
            total, total as f64 / self.fps(), self.fps());
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

        if let (Some(scene), Some(engine)) = (&self.scene, &mut self.engine) {
            if self.current_frame >= scene.frames.len() { return; }

            // Resize if needed
            if rw != self.render_w || rh != self.render_h {
                engine.resize(rw, rh);
                self.render_w = rw;
                self.render_h = rh;
            }

            let t = std::time::Instant::now();
            self.pixels = engine.render_frame(&scene.frames[self.current_frame]);
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
        self.scene.as_ref().map(|s| (s.settings.width, s.settings.height)).unwrap_or((1280, 720))
    }

    fn do_export(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("MP4", &["mp4"])
            .add_filter("WebM", &["webm"])
            .set_file_name("output.mp4")
            .save_file()
        else { return; };

        let (Some(scene), Some(engine)) = (&self.scene, &mut self.engine)
        else { return; };

        let (out_w, out_h) = (scene.settings.width, scene.settings.height);

        // Resize engine to full output resolution for export
        engine.resize(out_w, out_h);

        let config = ifol_render_core::ExportConfig {
            output_path: path.to_string_lossy().to_string(),
            fps: Some(scene.settings.fps),
            width: Some(out_w),
            height: Some(out_h),
            ..Default::default()
        };

        self.exporting = true;
        self.status = format!("Exporting {} frames → {:?}...", scene.frames.len(), path.file_name().unwrap_or_default());

        match engine.export_video(&scene.frames, &config, |p| {
            log::info!("Export: {:.0}% ({}/{})", p.percent(), p.current_frame, p.total_frames);
        }) {
            Ok(()) => {
                self.status = format!("✅ Exported: {:?}", path);
            }
            Err(e) => {
                self.status = format!("❌ Export error: {}", e);
            }
        }

        // Restore preview resolution
        self.render_w = 0; // force re-compute on next render
        self.render_h = 0;
        self.exporting = false;
        self.dirty = true;
    }
}

impl eframe::App for StudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Playback ──
        if self.playing && self.scene.is_some() && !self.exporting {
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
            let frame_dur = std::time::Duration::from_secs_f64(1.0 / self.fps());
            ctx.request_repaint_after(frame_dur);
        }

        // ── Keyboard ──
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                self.playing = !self.playing;
                if self.playing {
                    self.play_start_time = None;
                    self.smooth_last_render = None;
                }
            }
            if i.key_pressed(egui::Key::ArrowRight) && !self.playing {
                if self.current_frame + 1 < self.total_frames() {
                    self.current_frame += 1;
                    self.dirty = true;
                }
            }
            if i.key_pressed(egui::Key::ArrowLeft) && !self.playing {
                if self.current_frame > 0 {
                    self.current_frame -= 1;
                    self.dirty = true;
                }
            }
            if i.key_pressed(egui::Key::Home) {
                self.current_frame = 0; self.dirty = true;
            }
            if i.key_pressed(egui::Key::End) {
                self.current_frame = self.total_frames().saturating_sub(1);
                self.dirty = true;
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
                        {
                            if let (Some(scene), Some(engine)) = (&self.scene, &mut self.engine) {
                                // Export at full resolution
                                let (ow, oh) = (scene.settings.width, scene.settings.height);
                                engine.resize(ow, oh);
                                let pixels = engine.render_frame(&scene.frames[self.current_frame]);
                                match CoreEngine::save_png(&pixels, ow, oh, path.to_str().unwrap()) {
                                    Ok(()) => self.status = format!("✅ Saved: {:?}", path),
                                    Err(e) => self.status = format!("❌ {}", e),
                                }
                                self.render_w = 0; self.render_h = 0; self.dirty = true;
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("🎬 Export Video (MP4)...").clicked() {
                        ui.close_menu();
                        self.do_export();
                    }
                });

                ui.separator();

                // Playback mode selector
                ui.label("Mode:");
                let rt_label = if self.playback_mode == PlaybackMode::Realtime { "⏩ Realtime ✓" } else { "⏩ Realtime" };
                if ui.selectable_label(self.playback_mode == PlaybackMode::Realtime, rt_label).clicked() {
                    self.playback_mode = PlaybackMode::Realtime;
                }
                let sm_label = if self.playback_mode == PlaybackMode::Smooth { "🎞 Smooth ✓" } else { "🎞 Smooth" };
                if ui.selectable_label(self.playback_mode == PlaybackMode::Smooth, sm_label).clicked() {
                    self.playback_mode = PlaybackMode::Smooth;
                }

                ui.separator();

                // Preview resolution
                ui.menu_button(format!("Res: {}", self.preview_scale.label()), |ui| {
                    if ui.selectable_label(self.preview_scale == PreviewScale::Auto, "Auto (viewport)").clicked() {
                        self.preview_scale = PreviewScale::Auto;
                        self.render_w = 0; self.render_h = 0; self.dirty = true;
                        ui.close_menu();
                    }
                    for pct in [25, 50, 75, 100] {
                        let ps = PreviewScale::Percent(pct);
                        if ui.selectable_label(self.preview_scale == ps, format!("{}%", pct)).clicked() {
                            self.preview_scale = ps;
                            self.render_w = 0; self.render_h = 0; self.dirty = true;
                            ui.close_menu();
                        }
                    }
                });

                ui.separator();

                // Scene info
                if let Some(scene) = &self.scene {
                    let (ow, oh) = (scene.settings.width, scene.settings.height);
                    ui.colored_label(TEXT_DIM, format!(
                        "Output: {}×{} | Preview: {}×{} | {:.0}fps | {:.1}s",
                        ow, oh, self.render_w, self.render_h,
                        scene.settings.fps, self.duration()
                    ));
                }
            });
        });

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
                        ui.colored_label(TEXT_DIM, format!(
                            "{:.1}ms | {} | GPU", self.render_ms, mode_str
                        ));
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
                    if ui.add_sized([34.0, 34.0], egui::Button::new(
                        egui::RichText::new(play_label).size(16.0).color(egui::Color32::WHITE)
                    ).fill(play_color)).clicked() {
                        self.playing = !self.playing;
                        if self.playing {
                            self.play_start_time = None;
                            self.smooth_last_render = None;
                        }
                    }

                    // Stop
                    if ui.add_sized([34.0, 34.0], egui::Button::new(
                        egui::RichText::new("⏹").size(16.0).color(egui::Color32::WHITE)
                    ).fill(BG_SURFACE)).clicked() {
                        self.playing = false;
                        self.current_frame = 0;
                        self.dirty = true;
                    }

                    ui.separator();

                    // Frame / time
                    ui.colored_label(egui::Color32::WHITE, format!(
                        "{:>4} / {}", self.current_frame, self.total_frames()
                    ));
                    ui.colored_label(TEXT_DIM, format!(
                        "{:.2}s / {:.2}s", self.current_time(), self.duration()
                    ));
                });

                // Seek slider
                if self.total_frames() > 0 {
                    let mut frame = self.current_frame as f64;
                    let max = (self.total_frames() - 1) as f64;
                    let resp = ui.add(
                        egui::Slider::new(&mut frame, 0.0..=max)
                            .show_value(false).step_by(1.0)
                    );
                    if resp.changed() {
                        self.current_frame = frame as usize;
                        self.dirty = true;
                        if self.playing { self.playing = false; }
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
                    self.render_w = 0; self.render_h = 0;
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
                    self.viewport_tex = Some(ctx.load_texture(
                        "viewport", image, egui::TextureOptions::LINEAR,
                    ));
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
                ui.painter().rect_filled(rect.expand(1.0), 0.0, egui::Color32::from_rgb(15, 15, 18));
                ui.put(rect, egui::Image::new(egui::load::SizedTexture::new(
                    tex.id(), egui::vec2(draw_w, draw_h)
                )));
            }

            // Exporting overlay
            if self.exporting {
                let rect = ui.max_rect();
                ui.painter().rect_filled(rect, 0.0, egui::Color32::from_black_alpha(180));
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Exporting...",
                    egui::FontId::proportional(32.0),
                    egui::Color32::WHITE,
                );
            }
        });
    }
}
