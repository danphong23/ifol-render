//! Studio app — loads Frame JSON, renders viewport, seek/preview/play.

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

/// Scene data loaded from JSON.
struct SceneData {
    settings: RenderSettings,
    frames: Vec<Frame>,
}

/// Studio application state.
pub struct StudioApp {
    /// Loaded scene data.
    scene: Option<SceneData>,
    /// Core render engine.
    engine: Option<CoreEngine>,
    /// Current frame index.
    current_frame: usize,
    /// Whether playing.
    playing: bool,
    /// Viewport texture handle for egui.
    viewport_tex: Option<egui::TextureHandle>,
    /// Cached rendered pixels.
    pixels: Vec<u8>,
    /// Whether current frame needs re-render.
    dirty: bool,
    /// Status message.
    status: String,
    /// Playback start time (wall clock) and start frame.
    play_start_time: Option<std::time::Instant>,
    play_start_frame: usize,
    /// Scene file path.
    scene_path: Option<PathBuf>,
    /// Render timing.
    render_ms: f64,
}

impl StudioApp {
    pub fn new(_cc: &eframe::CreationContext, scene_path: Option<PathBuf>) -> Self {
        let mut app = Self {
            scene: None,
            engine: None,
            current_frame: 0,
            playing: false,
            viewport_tex: None,
            pixels: Vec::new(),
            dirty: true,
            status: "No scene loaded. File → Open to load a Frame JSON.".into(),
            play_start_time: None,
            play_start_frame: 0,
            scene_path: None,
            render_ms: 0.0,
        };

        if let Some(path) = scene_path {
            app.load_scene(&path);
        }

        app
    }

    fn load_scene(&mut self, path: &PathBuf) {
        self.status = format!("Loading {:?}...", path.file_name().unwrap_or_default());

        let json = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                self.status = format!("❌ Failed to read: {}", e);
                return;
            }
        };

        let doc: serde_json::Value = match serde_json::from_str(&json) {
            Ok(v) => v,
            Err(e) => {
                self.status = format!("❌ Invalid JSON: {}", e);
                return;
            }
        };

        let settings: RenderSettings = if let Some(s) = doc.get("settings") {
            match serde_json::from_value(s.clone()) {
                Ok(v) => v,
                Err(e) => {
                    self.status = format!("❌ Invalid settings: {}", e);
                    return;
                }
            }
        } else {
            RenderSettings::default()
        };

        // Support both single frame ("frame") and multi-frame ("frames")
        let frames: Vec<Frame> = if let Some(arr) = doc.get("frames") {
            match serde_json::from_value(arr.clone()) {
                Ok(v) => v,
                Err(e) => {
                    self.status = format!("❌ Invalid frames: {}", e);
                    return;
                }
            }
        } else if let Some(f) = doc.get("frame") {
            match serde_json::from_value(f.clone()) {
                Ok(v) => vec![v],
                Err(e) => {
                    self.status = format!("❌ Invalid frame: {}", e);
                    return;
                }
            }
        } else {
            self.status = "❌ Missing 'frames' or 'frame' key".into();
            return;
        };

        let total = frames.len();

        // Create engine
        let mut engine = CoreEngine::new(settings.clone());
        engine.setup_builtins();

        self.scene = Some(SceneData { settings, frames });
        self.engine = Some(engine);
        self.current_frame = 0;
        self.playing = false;
        self.dirty = true;
        self.scene_path = Some(path.clone());
        self.status = format!(
            "✅ Loaded: {} frames, {:.1}s",
            total,
            total as f64 / self.scene.as_ref().unwrap().settings.fps
        );
    }

    fn render_current_frame(&mut self) {
        if let (Some(scene), Some(engine)) = (&self.scene, &mut self.engine) {
            if self.current_frame < scene.frames.len() {
                let t = std::time::Instant::now();
                self.pixels = engine.render_frame(&scene.frames[self.current_frame]);
                self.render_ms = t.elapsed().as_secs_f64() * 1000.0;
                self.dirty = false;
            }
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
}

impl eframe::App for StudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Playback — wall-clock based ──
        if self.playing && self.scene.is_some() {
            let now = std::time::Instant::now();
            if let Some(start) = self.play_start_time {
                let elapsed = now.duration_since(start).as_secs_f64();
                let target_frame = self.play_start_frame + (elapsed * self.fps()) as usize;

                if target_frame >= self.total_frames() {
                    // Loop
                    self.play_start_frame = 0;
                    self.play_start_time = Some(now);
                    self.current_frame = 0;
                    self.dirty = true;
                } else if target_frame != self.current_frame {
                    self.current_frame = target_frame;
                    self.dirty = true;
                }
            } else {
                self.play_start_time = Some(now);
                self.play_start_frame = self.current_frame;
            }
            // Schedule next repaint at next frame boundary
            let frame_dur = std::time::Duration::from_secs_f64(1.0 / self.fps());
            ctx.request_repaint_after(frame_dur);
        }

        // ── Keyboard shortcuts ──
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                self.playing = !self.playing;
                if self.playing {
                    self.play_start_time = None;
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
                self.current_frame = 0;
                self.dirty = true;
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

        // ── Apply theme ──
        let mut style = (*ctx.style()).clone();
        style.visuals.window_fill = BG_APP;
        style.visuals.panel_fill = BG_PANEL;
        style.visuals.override_text_color = Some(TEXT_PRIMARY);
        style.visuals.widgets.noninteractive.bg_fill = BG_SURFACE;
        style.visuals.widgets.inactive.bg_fill = BG_SURFACE;
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 58, 66);
        style.visuals.widgets.active.bg_fill = ACCENT;
        ctx.set_style(style);

        // ── Top bar ──
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Frame JSON...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            self.load_scene(&path);
                        }
                        ui.close_menu();
                    }
                    if ui.button("Export Frame as PNG...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PNG", &["png"])
                            .set_file_name("frame.png")
                            .save_file()
                        {
                            if let Some(scene) = &self.scene {
                                let w = scene.settings.width;
                                let h = scene.settings.height;
                                match CoreEngine::save_png(&self.pixels, w, h, path.to_str().unwrap()) {
                                    Ok(()) => self.status = format!("✅ Saved: {:?}", path),
                                    Err(e) => self.status = format!("❌ Save error: {}", e),
                                }
                            }
                        }
                        ui.close_menu();
                    }
                });

                ui.separator();

                // Scene info
                if let Some(scene) = &self.scene {
                    ui.colored_label(TEXT_DIM, format!(
                        "{}×{} | {:.0}fps | {:.1}s",
                        scene.settings.width, scene.settings.height,
                        scene.settings.fps, self.duration()
                    ));
                }
            });
        });

        // ── Status bar ──
        egui::TopBottomPanel::bottom("status_bar")
            .max_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(TEXT_DIM, &self.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.colored_label(TEXT_DIM, format!(
                            "Render: {:.1}ms | GPU",
                            self.render_ms
                        ));
                    });
                });
            });

        // ── Timeline bar ──
        egui::TopBottomPanel::bottom("timeline")
            .min_height(60.0)
            .max_height(80.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    // Play/Pause button
                    let play_label = if self.playing { "⏸" } else { "▶" };
                    let btn = ui.add_sized(
                        [36.0, 36.0],
                        egui::Button::new(
                            egui::RichText::new(play_label).size(18.0).color(egui::Color32::WHITE)
                        ).fill(if self.playing { GREEN } else { ACCENT })
                    );
                    if btn.clicked() {
                        self.playing = !self.playing;
                        if self.playing {
                            self.play_start_time = None;
                        }
                    }

                    // Stop
                    if ui.add_sized([36.0, 36.0], egui::Button::new(
                        egui::RichText::new("⏹").size(18.0).color(egui::Color32::WHITE)
                    ).fill(BG_SURFACE)).clicked() {
                        self.playing = false;
                        self.current_frame = 0;
                        self.dirty = true;
                    }

                    ui.separator();

                    // Frame display
                    ui.colored_label(egui::Color32::WHITE, format!(
                        "{:>4} / {}",
                        self.current_frame, self.total_frames()
                    ));

                    // Time display
                    ui.colored_label(TEXT_DIM, format!(
                        "{:.2}s / {:.2}s",
                        self.current_time(), self.duration()
                    ));
                });

                // Seek slider
                if self.total_frames() > 0 {
                    let mut frame = self.current_frame as f64;
                    let max = (self.total_frames() - 1) as f64;
                    let response = ui.add(
                        egui::Slider::new(&mut frame, 0.0..=max)
                            .show_value(false)
                            .step_by(1.0)
                    );
                    if response.changed() {
                        self.current_frame = frame as usize;
                        self.dirty = true;
                        if self.playing {
                            self.playing = false;
                        }
                    }
                }
            });

        // ── Viewport (central panel) ──
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.scene.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.heading("No scene loaded.\n\nFile → Open to load a Frame JSON.");
                });
                return;
            }

            let scene = self.scene.as_ref().unwrap();
            let w = scene.settings.width;
            let h = scene.settings.height;

            // Update viewport texture
            if !self.pixels.is_empty() {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    &self.pixels,
                );

                if let Some(tex) = &mut self.viewport_tex {
                    tex.set(image, egui::TextureOptions::NEAREST);
                } else {
                    self.viewport_tex = Some(ctx.load_texture(
                        "viewport",
                        image,
                        egui::TextureOptions::NEAREST,
                    ));
                }
            }

            // Draw viewport centered with aspect ratio preserved
            if let Some(tex) = &self.viewport_tex {
                let available = ui.available_size();
                let aspect = w as f32 / h as f32;
                let (draw_w, draw_h) = if available.x / available.y > aspect {
                    (available.y * aspect, available.y)
                } else {
                    (available.x, available.x / aspect)
                };

                let offset_x = (available.x - draw_w) / 2.0;
                let offset_y = (available.y - draw_h) / 2.0;

                let rect = egui::Rect::from_min_size(
                    ui.min_rect().min + egui::vec2(offset_x, offset_y),
                    egui::vec2(draw_w, draw_h),
                );

                // Dark border around viewport
                ui.painter().rect_filled(rect.expand(2.0), 0.0, egui::Color32::from_rgb(15, 15, 18));
                ui.put(rect, egui::Image::new(egui::load::SizedTexture::new(
                    tex.id(), egui::vec2(draw_w, draw_h)
                )));
            }
        });
    }
}
