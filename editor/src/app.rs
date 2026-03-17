//! Core editor application — a third-party consumer of ifol-render-core and ifol-render-gpu.
//!
//! Architecture:
//!   ifol-render-core  (ECS: datatypes, components, systems, pipeline)
//!   ifol-render-gpu   (Render tool: GPU shaders, composite, pixel output — NO GUI)
//!   ifol-render-editor (THIS: GUI editor that USES core + gpu as libraries)
//!
//! The editor does NOT own any rendering logic. It calls the render tool API.

use egui::{Color32, ColorImage, RichText, TextureHandle, TextureOptions, Vec2};
use ifol_render_core::ecs::{components, World};
use ifol_render_core::scene::{RenderSettings, SceneDescription};
use ifol_render_core::time::TimeState;

// ── Theme (matching VideoComposer: #18191C bg, #242529 panels, #303031 borders) ──
const BG_APP: Color32 = Color32::from_rgb(24, 25, 28);      // #18191C
const BG_PANEL: Color32 = Color32::from_rgb(36, 37, 41);     // #242529
const BG_SURFACE: Color32 = Color32::from_rgb(42, 44, 50);   // #2a2c32
const BG_HOVER: Color32 = Color32::from_rgb(55, 58, 66);     // #373a42
const BORDER: Color32 = Color32::from_rgb(48, 48, 49);       // #303031
const TEXT_PRIMARY: Color32 = Color32::from_rgb(224, 224, 224); // #E0E0E0
const TEXT_DIM: Color32 = Color32::from_rgb(130, 135, 150);    // #828796
const ACCENT: Color32 = Color32::from_rgb(88, 101, 242);      // #5865f2
const ACCENT_GREEN: Color32 = Color32::from_rgb(87, 242, 135); // #57f287
const RED: Color32 = Color32::from_rgb(237, 66, 69);          // #ed4245
const TRACK_BG: Color32 = Color32::from_rgb(54, 57, 75);      // #36394b
const TRACK_SEL: Color32 = Color32::from_rgb(88, 101, 242);   // #5865f2

/// Editor application state.
pub struct EditorApp {
    world: World,
    settings: RenderSettings,
    time: TimeState,
    playing: bool,
    selected: Option<usize>,
    viewport_tex: Option<TextureHandle>,
    pixels: Vec<u8>,
    dirty: bool,
    status: String,
    zoom: f32,
    /// Persistent renderer (avoids re-creating GPU context every frame).
    renderer: Option<ifol_render_gpu::Renderer>,
}

impl EditorApp {
    pub fn new() -> Self {
        let mut world = World::new();

        // Default scene
        Self::add_color_entity(&mut world, "background", [0.12, 0.13, 0.16, 1.0],
            [0.0, 0.0], [1.0, 1.0], 0.0, 10.0, 0, 1.0);
        Self::add_color_entity(&mut world, "red_box", [0.9, 0.2, 0.2, 1.0],
            [-0.3, 0.2], [0.3, 0.3], 0.0, 10.0, 1, 0.8);
        Self::add_color_entity(&mut world, "green_box", [0.2, 0.85, 0.35, 1.0],
            [0.3, -0.2], [0.25, 0.25], 0.5, 8.0, 2, 1.0);
        Self::add_color_entity(&mut world, "blue_box", [0.3, 0.5, 0.95, 1.0],
            [0.0, 0.0], [0.2, 0.3], 0.0, 10.0, 3, 0.6);

        let settings = RenderSettings {
            width: 640, height: 360, fps: 30.0, duration: 10.0,
            color_space: ifol_render_core::color::ColorSpace::LinearSrgb,
            output_color_space: ifol_render_core::color::ColorSpace::Srgb,
        };

        Self {
            world, settings,
            time: TimeState::new(30.0),
            playing: false, selected: None,
            viewport_tex: None, pixels: Vec::new(),
            dirty: true, status: "Ready".into(),
            zoom: 1.0, renderer: None,
        }
    }

    fn add_color_entity(world: &mut World, id: &str, rgba: [f32; 4],
        pos: [f32; 2], scale: [f32; 2], start: f64, dur: f64, layer: i32, opacity: f32,
    ) {
        let mut e = ifol_render_core::ecs::Entity {
            id: id.to_string(), components: Default::default(), resolved: Default::default(),
        };
        e.components.color_source = Some(components::ColorSource {
            color: ifol_render_core::color::Color4::new(rgba[0], rgba[1], rgba[2], rgba[3]),
        });
        e.components.timeline = Some(components::Timeline { start_time: start, duration: dur, layer });
        e.components.transform = Some(components::Transform {
            position: ifol_render_core::types::Vec2 { x: pos[0], y: pos[1] },
            scale: ifol_render_core::types::Vec2 { x: scale[0], y: scale[1] },
            ..Default::default()
        });
        if (opacity - 1.0).abs() > 0.001 {
            e.components.opacity = Some(opacity);
        }
        world.add_entity(e);
    }

    fn ensure_renderer(&mut self) {
        if self.renderer.is_none() {
            let mut r = ifol_render_gpu::Renderer::new_headless(&self.settings);
            // Load images for entities that have image_source
            for entity in &self.world.entities {
                if let Some(ref img) = entity.components.image_source {
                    if let Err(e) = r.load_image(&entity.id, &img.path) {
                        log::warn!("Failed to load image for '{}': {}", entity.id, e);
                    }
                }
            }
            self.renderer = Some(r);
        }
    }

    fn render_scene(&mut self) {
        ifol_render_core::ecs::pipeline::run(&mut self.world, &self.time);
        self.ensure_renderer();
        if let Some(ref mut r) = self.renderer {
            self.pixels = r.render_frame(&self.world, &self.time);
        }
        self.dirty = false;
    }

    fn invalidate_renderer(&mut self) {
        self.renderer = None;
        self.dirty = true;
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let v = &mut style.visuals;

    v.panel_fill = BG_PANEL;
    v.window_fill = BG_SURFACE;
    v.extreme_bg_color = BG_APP;
    v.faint_bg_color = BG_SURFACE;

    v.widgets.noninteractive.bg_fill = BG_SURFACE;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_DIM);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, BORDER);

    v.widgets.inactive.bg_fill = BG_SURFACE;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(0.5, BORDER);

    v.widgets.hovered.bg_fill = BG_HOVER;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT);

    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);

    v.selection.bg_fill = ACCENT.linear_multiply(0.4);
    v.selection.stroke = egui::Stroke::new(1.0, ACCENT);

    v.window_shadow = egui::epaint::Shadow::NONE;

    style.spacing.item_spacing = egui::vec2(6.0, 3.0);
    style.spacing.button_padding = egui::vec2(8.0, 3.0);
    ctx.set_style(style);
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_theme(ctx);

        if self.playing {
            let dt = ctx.input(|i| i.unstable_dt) as f64;
            self.time.seek(self.time.global_time + dt);
            if self.time.global_time >= self.settings.duration {
                self.time.seek(0.0);
            }
            self.dirty = true;
            ctx.request_repaint();
        }

        if self.dirty { self.render_scene(); }

        if !self.pixels.is_empty() {
            let img = ColorImage::from_rgba_unmultiplied(
                [self.settings.width as usize, self.settings.height as usize], &self.pixels,
            );
            let tex = ctx.load_texture("viewport", img, TextureOptions::LINEAR);
            self.viewport_tex = Some(tex);
        }

        // ── Top bar ──
        egui::TopBottomPanel::top("top")
            .frame(egui::Frame::new().fill(BG_APP).inner_margin(egui::Margin::symmetric(10, 5)))
            .exact_height(34.0)
            .show(ctx, |ui| { self.ui_top_bar(ui); });

        // ── Status bar ──
        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::new().fill(BG_APP).inner_margin(egui::Margin::symmetric(10, 3)))
            .exact_height(22.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).color(TEXT_DIM).size(10.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} entities", self.world.entities.len())).color(TEXT_DIM).size(10.0));
                    });
                });
            });

        // ── Timeline (bottom, fixed-height approach to avoid resize jumping) ──
        egui::TopBottomPanel::bottom("timeline")
            .frame(egui::Frame::new().fill(BG_PANEL).inner_margin(egui::Margin::same(6)).stroke(egui::Stroke::new(1.0, BORDER)))
            .resizable(true)
            .min_height(80.0)
            .default_height(160.0)
            .show(ctx, |ui| { self.ui_timeline(ui); });

        // ── Left panel ──
        egui::SidePanel::left("entities")
            .frame(egui::Frame::new().fill(BG_PANEL).inner_margin(egui::Margin::same(6)).stroke(egui::Stroke::new(1.0, BORDER)))
            .resizable(true).default_width(200.0).min_width(150.0)
            .show(ctx, |ui| { self.ui_entities(ui); });

        // ── Right panel ──
        egui::SidePanel::right("props")
            .frame(egui::Frame::new().fill(BG_PANEL).inner_margin(egui::Margin::same(6)).stroke(egui::Stroke::new(1.0, BORDER)))
            .resizable(true).default_width(280.0).min_width(200.0)
            .show(ctx, |ui| { self.ui_properties(ui); });

        // ── Center viewport ──
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG_APP).inner_margin(egui::Margin::same(2)))
            .show(ctx, |ui| { self.ui_viewport(ui); });
    }
}

impl EditorApp {
    // ── TOP BAR ──
    fn ui_top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new("◆ ifol-render").color(ACCENT).strong().size(13.0));
            ui.separator();

            ui.menu_button(RichText::new("File").color(TEXT_PRIMARY).size(11.0), |ui| {
                if ui.button("  New Scene").clicked() {
                    *self = EditorApp::new();
                    ui.close_menu();
                }
                if ui.button("  Open...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file() {
                        if let Ok(json) = std::fs::read_to_string(&path) {
                            match SceneDescription::from_json(&json) {
                                Ok(desc) => {
                                    self.settings = desc.settings.clone();
                                    self.world = desc.into_world();
                                    self.time = TimeState::new(self.settings.fps);
                                    self.selected = None;
                                    self.invalidate_renderer();
                                    self.status = format!("Opened: {}", path.display());
                                }
                                Err(e) => self.status = format!("Error: {}", e),
                            }
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("  Save...").clicked() {
                    let json = serde_json::to_string_pretty(&self.world).unwrap_or_default();
                    if let Some(path) = rfd::FileDialog::new().add_filter("JSON", &["json"]).save_file() {
                        let _ = std::fs::write(&path, &json);
                        self.status = format!("Saved: {}", path.display());
                    }
                    ui.close_menu();
                }
            });

            ui.separator();
            ui.label(RichText::new(
                format!("{}×{}  {:.0}fps  {:.1}s/{:.1}s",
                    self.settings.width, self.settings.height, self.settings.fps,
                    self.time.global_time, self.settings.duration)
            ).color(TEXT_DIM).size(10.0));
        });
    }

    // ── VIEWPORT ──
    fn ui_viewport(&self, ui: &mut egui::Ui) {
        if let Some(tex) = &self.viewport_tex {
            let avail = ui.available_size();
            let aspect = self.settings.width as f32 / self.settings.height as f32;
            let (w, h) = if avail.x / avail.y > aspect {
                (avail.y * aspect, avail.y)
            } else {
                (avail.x, avail.x / aspect)
            };
            ui.centered_and_justified(|ui| {
                ui.image(egui::load::SizedTexture::new(tex.id(), Vec2::new(w, h)));
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("No output").color(TEXT_DIM));
            });
        }
    }

    // ── ENTITY LIST ──
    fn ui_entities(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("ENTITIES").color(TEXT_DIM).size(10.0).strong());
        ui.add_space(2.0);

        // Add buttons
        ui.horizontal(|ui| {
            if ui.small_button(RichText::new("+ Color").color(ACCENT).size(10.0)).clicked() {
                let n = self.world.entities.len();
                Self::add_color_entity(&mut self.world, &format!("color_{}", n),
                    [0.5, 0.5, 0.5, 1.0], [0.0, 0.0], [0.2, 0.2],
                    self.time.global_time, 3.0, n as i32, 1.0);
                self.selected = Some(n);
                self.invalidate_renderer();
            }
            if ui.small_button(RichText::new("+ Image").color(ACCENT).size(10.0)).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_file()
                {
                    let n = self.world.entities.len();
                    let mut e = ifol_render_core::ecs::Entity {
                        id: format!("img_{}", n), components: Default::default(), resolved: Default::default(),
                    };
                    e.components.image_source = Some(components::ImageSource {
                        path: path.to_string_lossy().to_string(),
                    });
                    e.components.timeline = Some(components::Timeline {
                        start_time: self.time.global_time, duration: 5.0, layer: n as i32,
                    });
                    e.components.transform = Some(components::Transform::default());
                    self.world.add_entity(e);
                    self.selected = Some(n);
                    self.invalidate_renderer();
                }
            }
        });

        ui.add_space(2.0);
        ui.separator();

        // List
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut sel = self.selected;
            for (i, e) in self.world.entities.iter().enumerate() {
                let is_sel = sel == Some(i);
                let icon = match () {
                    _ if e.components.color_source.is_some() => "●",
                    _ if e.components.image_source.is_some() => "🖼",
                    _ if e.components.video_source.is_some() => "▶",
                    _ if e.components.text_source.is_some() => "T",
                    _ => "◻",
                };
                let text = RichText::new(format!(" {} {}", icon, e.id))
                    .color(if is_sel { Color32::WHITE } else { TEXT_PRIMARY }).size(11.0);
                if ui.selectable_label(is_sel, text).clicked() {
                    sel = Some(i);
                }
            }
            self.selected = sel;
        });

        // Delete
        if let Some(i) = self.selected {
            if i < self.world.entities.len() {
                ui.separator();
                if ui.small_button(RichText::new("🗑 Delete").color(RED).size(10.0)).clicked() {
                    self.world.entities.remove(i);
                    self.world.rebuild_index();
                    self.selected = None;
                    self.invalidate_renderer();
                }
            }
        }
    }

    // ── PROPERTIES ──
    fn ui_properties(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("PROPERTIES").color(TEXT_DIM).size(10.0).strong());
        ui.add_space(2.0);
        ui.separator();

        let i = match self.selected {
            Some(i) if i < self.world.entities.len() => i,
            _ => {
                ui.add_space(20.0);
                ui.label(RichText::new("Select an entity").color(TEXT_DIM).size(11.0));
                return;
            }
        };

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let e = &mut self.world.entities[i];

            // ID
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("ID").color(TEXT_DIM).size(10.0));
                ui.text_edit_singleline(&mut e.id);
            });

            // Transform
            if let Some(ref mut tf) = e.components.transform {
                ui.add_space(6.0);
                ui.label(RichText::new("TRANSFORM").color(TEXT_DIM).size(10.0).strong());
                let mut ch = false;
                ui.horizontal(|ui| {
                    ui.label(RichText::new("X").color(TEXT_DIM).size(10.0));
                    ch |= ui.add(egui::DragValue::new(&mut tf.position.x).speed(0.01)).changed();
                    ui.label(RichText::new("Y").color(TEXT_DIM).size(10.0));
                    ch |= ui.add(egui::DragValue::new(&mut tf.position.y).speed(0.01)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("W").color(TEXT_DIM).size(10.0));
                    ch |= ui.add(egui::DragValue::new(&mut tf.scale.x).speed(0.01).range(0.0..=4.0)).changed();
                    ui.label(RichText::new("H").color(TEXT_DIM).size(10.0));
                    ch |= ui.add(egui::DragValue::new(&mut tf.scale.y).speed(0.01).range(0.0..=4.0)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Rot").color(TEXT_DIM).size(10.0));
                    ch |= ui.add(egui::DragValue::new(&mut tf.rotation).speed(0.5).suffix("°")).changed();
                });
                if ch { self.dirty = true; }
            }

            // Opacity
            if let Some(ref mut op) = e.components.opacity {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Opacity").color(TEXT_DIM).size(10.0));
                    if ui.add(egui::Slider::new(op, 0.0..=1.0).show_value(true)).changed() {
                        self.dirty = true;
                    }
                });
            }

            // Color
            if let Some(ref mut cs) = e.components.color_source {
                ui.add_space(6.0);
                ui.label(RichText::new("COLOR").color(TEXT_DIM).size(10.0).strong());
                let mut rgb = [cs.color.r, cs.color.g, cs.color.b];
                if ui.color_edit_button_rgb(&mut rgb).changed() {
                    cs.color.r = rgb[0]; cs.color.g = rgb[1]; cs.color.b = rgb[2];
                    self.dirty = true;
                }
            }

            // Image
            if let Some(ref img) = e.components.image_source {
                ui.add_space(6.0);
                ui.label(RichText::new("IMAGE").color(TEXT_DIM).size(10.0).strong());
                ui.label(RichText::new(&img.path).color(TEXT_DIM).size(9.0));
            }

            // Timeline
            if let Some(ref mut tl) = e.components.timeline {
                ui.add_space(6.0);
                ui.label(RichText::new("TIMELINE").color(TEXT_DIM).size(10.0).strong());
                let mut ch = false;
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Start").color(TEXT_DIM).size(10.0));
                    ch |= ui.add(egui::DragValue::new(&mut tl.start_time).speed(0.1).suffix("s")).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Dur").color(TEXT_DIM).size(10.0));
                    ch |= ui.add(egui::DragValue::new(&mut tl.duration).speed(0.1).suffix("s")).changed();
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Layer").color(TEXT_DIM).size(10.0));
                    ch |= ui.add(egui::DragValue::new(&mut tl.layer)).changed();
                });
                if ch { self.dirty = true; }
            }
        });
    }

    // ── TIMELINE ──
    fn ui_timeline(&mut self, ui: &mut egui::Ui) {
        // Transport controls
        ui.horizontal(|ui| {
            ui.label(RichText::new("TIMELINE").color(TEXT_DIM).size(10.0).strong());
            ui.separator();

            if ui.small_button("⏮").clicked() { self.time.seek(0.0); self.dirty = true; }

            let play_label = if self.playing {
                RichText::new("⏸").color(RED).size(12.0)
            } else {
                RichText::new("▶").color(ACCENT_GREEN).size(12.0)
            };
            if ui.small_button(play_label).clicked() { self.playing = !self.playing; }

            if ui.small_button("⏭").clicked() { self.time.seek(self.settings.duration); self.dirty = true; }

            ui.separator();
            ui.label(RichText::new(format!("{:02}:{:04.1}",
                (self.time.global_time / 60.0) as u32, self.time.global_time % 60.0
            )).color(Color32::WHITE).size(12.0).monospace());

            ui.separator();
            let mut t = self.time.global_time;
            if ui.add(egui::Slider::new(&mut t, 0.0..=self.settings.duration).show_value(false).trailing_fill(true)).changed() {
                self.time.seek(t); self.dirty = true;
            }

            ui.separator();
            ui.add(egui::Slider::new(&mut self.zoom, 0.3..=4.0).show_value(false).logarithmic(true)
                .text(RichText::new("Zoom").color(TEXT_DIM).size(9.0)));
        });

        ui.add_space(2.0);

        // Track area inside ScrollArea (fixes resize jumping)
        let avail_w = ui.available_width();
        let dur = self.settings.duration;
        let pps = (avail_w / dur as f32) * self.zoom;  // pixels per second
        let track_h = 22.0f32;
        let gap = 2.0f32;

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // Reserve total height for ruler + tracks
            let ruler_h = 14.0;
            let total_tracks = self.world.entities.len();
            let total_h = ruler_h + total_tracks as f32 * (track_h + gap) + 8.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, total_h), egui::Sense::click());

            let painter = ui.painter_at(rect);
            let origin = rect.min;

            // Ruler
            let step = if self.zoom > 2.0 { 0.5 } else if self.zoom > 1.0 { 1.0 } else { 2.0 };
            let mut tm = 0.0f64;
            while tm <= dur {
                let x = origin.x + tm as f32 * pps;
                painter.line_segment(
                    [egui::pos2(x, origin.y), egui::pos2(x, origin.y + ruler_h)],
                    egui::Stroke::new(0.5, BORDER),
                );
                painter.text(egui::pos2(x + 2.0, origin.y), egui::Align2::LEFT_TOP,
                    format!("{:.0}s", tm), egui::FontId::monospace(8.0), TEXT_DIM);
                tm += step;
            }

            // Tracks
            let tracks_y = origin.y + ruler_h;
            for (i, e) in self.world.entities.iter().enumerate() {
                if let Some(tl) = &e.components.timeline {
                    let y = tracks_y + i as f32 * (track_h + gap);
                    let x0 = origin.x + tl.start_time as f32 * pps;
                    let w = tl.duration as f32 * pps;

                    let color = if self.selected == Some(i) { TRACK_SEL } else { TRACK_BG };
                    let r = egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(w, track_h));
                    painter.rect_filled(r, 3.0, color);
                    painter.with_clip_rect(r.shrink(2.0)).text(
                        egui::pos2(x0 + 4.0, y + 4.0), egui::Align2::LEFT_TOP,
                        &e.id, egui::FontId::proportional(10.0), Color32::WHITE,
                    );
                }
            }

            // Playhead
            let ph_x = origin.x + self.time.global_time as f32 * pps;
            painter.line_segment(
                [egui::pos2(ph_x, origin.y), egui::pos2(ph_x, origin.y + total_h)],
                egui::Stroke::new(1.5, RED),
            );
            painter.circle_filled(egui::pos2(ph_x, origin.y), 4.0, RED);

            // Track click detection
            for (i, e) in self.world.entities.iter().enumerate() {
                if let Some(tl) = &e.components.timeline {
                    let y = tracks_y + i as f32 * (track_h + gap);
                    let x0 = origin.x + tl.start_time as f32 * pps;
                    let w = tl.duration as f32 * pps;
                    let r = egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(w, track_h));

                    if ui.input(|inp| inp.pointer.any_click()) {
                        if let Some(pos) = ui.input(|inp| inp.pointer.hover_pos()) {
                            if r.contains(pos) {
                                self.selected = Some(i);
                            }
                        }
                    }
                }
            }
        });
    }
}
