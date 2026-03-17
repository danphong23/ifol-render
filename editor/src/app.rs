//! Core editor application with premium dark theme and GPU-rendered viewport.

use egui::{Color32, ColorImage, RichText, TextureHandle, TextureOptions, Vec2};
use ifol_render_core::ecs::{components, World};
use ifol_render_core::scene::{RenderSettings, SceneDescription};
use ifol_render_core::time::TimeState;

// ── Theme colors (matching workflow builder dark palette) ──
const BG_DARK: Color32 = Color32::from_rgb(17, 19, 24); // #111318
const BG_PANEL: Color32 = Color32::from_rgb(24, 27, 33); // #181b21
const BG_SURFACE: Color32 = Color32::from_rgb(30, 34, 42); // #1e222a
const BG_HOVER: Color32 = Color32::from_rgb(40, 45, 55); // #28302d
const BG_SELECTED: Color32 = Color32::from_rgb(59, 130, 246); // #3b82f6
const TEXT_PRIMARY: Color32 = Color32::from_rgb(220, 225, 235); // #dce1eb
const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 150, 170); // #8c96aa
const ACCENT_CYAN: Color32 = Color32::from_rgb(42, 157, 143); // #2a9d8f
const ACCENT_RED: Color32 = Color32::from_rgb(231, 111, 81); // #e76f51
const TRACK_COLOR: Color32 = Color32::from_rgb(47, 51, 77); // #2f334d
const TRACK_SELECTED: Color32 = Color32::from_rgb(59, 130, 246); // #3b82f6
const BORDER_SUBTLE: Color32 = Color32::from_rgb(45, 50, 60); // #2d323c
const PLAYHEAD_RED: Color32 = Color32::from_rgb(239, 68, 68); // #ef4444

/// The main editor state.
pub struct EditorApp {
    pub world: World,
    pub settings: RenderSettings,
    pub time: TimeState,
    pub playing: bool,
    pub selected_entity: Option<usize>,
    viewport_texture: Option<TextureHandle>,
    pixels: Vec<u8>,
    dirty: bool,
    file_path: Option<String>,
    status: String,
    /// Timeline zoom level.
    timeline_zoom: f32,
}

impl EditorApp {
    pub fn new() -> Self {
        let mut world = World::new();

        // Background
        let mut bg = ifol_render_core::ecs::Entity {
            id: "background".to_string(),
            components: Default::default(),
            resolved: Default::default(),
        };
        bg.components.color_source = Some(components::ColorSource {
            color: ifol_render_core::color::Color4::new(0.12, 0.13, 0.16, 1.0),
        });
        bg.components.timeline = Some(components::Timeline {
            start_time: 0.0,
            duration: 10.0,
            layer: 0,
        });
        bg.components.transform = Some(components::Transform::default());
        world.add_entity(bg);

        // Red box
        let mut red = ifol_render_core::ecs::Entity {
            id: "red_box".to_string(),
            components: Default::default(),
            resolved: Default::default(),
        };
        red.components.color_source = Some(components::ColorSource {
            color: ifol_render_core::color::Color4::new(0.9, 0.2, 0.2, 1.0),
        });
        red.components.timeline = Some(components::Timeline {
            start_time: 0.0,
            duration: 10.0,
            layer: 1,
        });
        red.components.transform = Some(components::Transform {
            position: ifol_render_core::types::Vec2 { x: -0.3, y: 0.2 },
            scale: ifol_render_core::types::Vec2 { x: 0.3, y: 0.3 },
            ..Default::default()
        });
        red.components.opacity = Some(0.8);
        world.add_entity(red);

        // Green box
        let mut green = ifol_render_core::ecs::Entity {
            id: "green_box".to_string(),
            components: Default::default(),
            resolved: Default::default(),
        };
        green.components.color_source = Some(components::ColorSource {
            color: ifol_render_core::color::Color4::new(0.2, 0.8, 0.3, 1.0),
        });
        green.components.timeline = Some(components::Timeline {
            start_time: 0.5,
            duration: 8.0,
            layer: 2,
        });
        green.components.transform = Some(components::Transform {
            position: ifol_render_core::types::Vec2 { x: 0.3, y: -0.2 },
            scale: ifol_render_core::types::Vec2 { x: 0.25, y: 0.25 },
            ..Default::default()
        });
        world.add_entity(green);

        let settings = RenderSettings {
            width: 640,
            height: 360,
            fps: 30.0,
            duration: 10.0,
            color_space: ifol_render_core::color::ColorSpace::LinearSrgb,
            output_color_space: ifol_render_core::color::ColorSpace::Srgb,
        };

        Self {
            world,
            settings,
            time: TimeState::new(30.0),
            playing: false,
            selected_entity: None,
            viewport_texture: None,
            pixels: Vec::new(),
            dirty: true,
            file_path: None,
            status: "Ready".to_string(),
            timeline_zoom: 1.0,
        }
    }

    fn render_scene(&mut self) {
        ifol_render_core::ecs::pipeline::run(&mut self.world, &self.time);
        let mut renderer = ifol_render_gpu::Renderer::new_headless(&self.settings);
        self.pixels = renderer.render_frame(&self.world, &self.time);
        self.dirty = false;
    }

    fn load_scene_from_json(&mut self, json: &str) -> Result<(), String> {
        let desc = SceneDescription::from_json(json).map_err(|e| e.to_string())?;
        self.settings = desc.settings.clone();
        self.world = desc.into_world();
        self.time = TimeState::new(self.settings.fps);
        self.selected_entity = None;
        self.dirty = true;
        Ok(())
    }
}

/// Apply premium dark theme to egui.
fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Visuals
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_SURFACE;
    visuals.extreme_bg_color = BG_DARK;
    visuals.faint_bg_color = BG_SURFACE;

    // Widgets
    visuals.widgets.noninteractive.bg_fill = BG_SURFACE;
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_SECONDARY);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, BORDER_SUBTLE);

    visuals.widgets.inactive.bg_fill = BG_SURFACE;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(0.5, BORDER_SUBTLE);

    visuals.widgets.hovered.bg_fill = BG_HOVER;
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT_CYAN);

    visuals.widgets.active.bg_fill = BG_SELECTED;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);

    visuals.selection.bg_fill = BG_SELECTED.linear_multiply(0.5);
    visuals.selection.stroke = egui::Stroke::new(1.0, BG_SELECTED);

    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(80),
    };

    style.visuals = visuals;

    // Spacing
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.indent = 16.0;

    ctx.set_style(style);
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_theme(ctx);

        // Advance time
        if self.playing {
            let dt = ctx.input(|i| i.unstable_dt) as f64;
            self.time.seek(self.time.global_time + dt);
            if self.time.global_time >= self.settings.duration {
                self.time.seek(0.0);
            }
            self.dirty = true;
            ctx.request_repaint();
        }

        if self.dirty {
            self.render_scene();
        }

        // Update viewport texture
        if !self.pixels.is_empty() {
            let image = ColorImage::from_rgba_unmultiplied(
                [self.settings.width as usize, self.settings.height as usize],
                &self.pixels,
            );
            let tex = ctx.load_texture("viewport", image, TextureOptions::LINEAR);
            self.viewport_texture = Some(tex);
        }

        // ── Top Bar (36px) ──
        egui::TopBottomPanel::top("top_bar")
            .frame(egui::Frame::new().fill(BG_DARK).inner_margin(egui::Margin::symmetric(12, 6)))
            .exact_height(36.0)
            .show(ctx, |ui| {
                self.show_top_bar(ui);
            });

        // ── Status Bar ──
        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::new().fill(BG_DARK).inner_margin(egui::Margin::symmetric(12, 4)))
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.status).color(TEXT_SECONDARY).size(11.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("Entities: {}", self.world.entities.len()))
                                .color(TEXT_SECONDARY)
                                .size(11.0),
                        );
                    });
                });
            });

        // ── Timeline (bottom) ──
        egui::TopBottomPanel::bottom("timeline")
            .resizable(true)
            .min_height(100.0)
            .default_height(180.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::same(8))
                    .stroke(egui::Stroke::new(0.5, BORDER_SUBTLE)),
            )
            .show(ctx, |ui| {
                self.show_timeline(ui);
            });

        // ── Entity List (left) ──
        egui::SidePanel::left("entity_list")
            .resizable(true)
            .default_width(220.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::same(8))
                    .stroke(egui::Stroke::new(0.5, BORDER_SUBTLE)),
            )
            .show(ctx, |ui| {
                self.show_entity_list(ui);
            });

        // ── Properties (right) ──
        egui::SidePanel::right("properties")
            .resizable(true)
            .default_width(300.0)
            .frame(
                egui::Frame::new()
                    .fill(BG_PANEL)
                    .inner_margin(egui::Margin::same(8))
                    .stroke(egui::Stroke::new(0.5, BORDER_SUBTLE)),
            )
            .show(ctx, |ui| {
                self.show_properties(ui);
            });

        // ── Viewport (center) ──
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG_DARK)
                    .inner_margin(egui::Margin::same(4)),
            )
            .show(ctx, |ui| {
                self.show_viewport(ui);
            });
    }
}

// ── Panel implementations ──
impl EditorApp {
    fn show_top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            // Left: Logo + File
            ui.label(RichText::new("◆ ifol-render").color(ACCENT_CYAN).strong().size(14.0));
            ui.separator();

            // File menu
            ui.menu_button(RichText::new("File").color(TEXT_PRIMARY).size(12.0), |ui| {
                if ui.button("📄 New Scene").clicked() {
                    *self = EditorApp::new();
                    ui.close_menu();
                }
                if ui.button("📂 Open Scene...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .pick_file()
                    {
                        if let Ok(json) = std::fs::read_to_string(&path) {
                            match self.load_scene_from_json(&json) {
                                Ok(()) => {
                                    self.file_path =
                                        Some(path.to_string_lossy().to_string());
                                    self.status = format!("Opened: {}", path.display());
                                }
                                Err(e) => self.status = format!("Error: {}", e),
                            }
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("💾 Save Scene").clicked() {
                    let json =
                        serde_json::to_string_pretty(&self.world).unwrap_or_default();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .save_file()
                    {
                        let _ = std::fs::write(&path, json);
                        self.file_path = Some(path.to_string_lossy().to_string());
                        self.status = format!("Saved: {}", path.display());
                    }
                    ui.close_menu();
                }
            });

            ui.separator();

            // Center: Scene info
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{}×{}  •  {:.0}fps  •  {:.1}s / {:.1}s",
                        self.settings.width,
                        self.settings.height,
                        self.settings.fps,
                        self.time.global_time,
                        self.settings.duration,
                    ))
                    .color(TEXT_SECONDARY)
                    .size(11.0),
                );
            });
        });
    }

    fn show_viewport(&self, ui: &mut egui::Ui) {
        if let Some(tex) = &self.viewport_texture {
            let available = ui.available_size();
            let aspect = self.settings.width as f32 / self.settings.height as f32;
            let (w, h) = if available.x / available.y > aspect {
                (available.y * aspect, available.y)
            } else {
                (available.x, available.x / aspect)
            };

            ui.centered_and_justified(|ui| {
                ui.image(egui::load::SizedTexture::new(tex.id(), Vec2::new(w, h)));
            },
            );
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("No render output").color(TEXT_SECONDARY).size(14.0));
            });
        }
    }

    fn show_entity_list(&mut self, ui: &mut egui::Ui) {
        // Header
        ui.horizontal(|ui| {
            ui.label(RichText::new("ENTITIES").color(TEXT_SECONDARY).size(11.0).strong());
        });
        ui.add_space(4.0);

        // Add buttons
        ui.horizontal(|ui| {
            let btn_style = RichText::new("+ Color").color(ACCENT_CYAN).size(11.0);
            if ui.button(btn_style).clicked() {
                let id = format!("color_{}", self.world.entities.len());
                let mut e = ifol_render_core::ecs::Entity {
                    id,
                    components: Default::default(),
                    resolved: Default::default(),
                };
                e.components.color_source = Some(components::ColorSource {
                    color: ifol_render_core::color::Color4::new(0.5, 0.5, 0.5, 1.0),
                });
                e.components.timeline = Some(components::Timeline {
                    start_time: self.time.global_time,
                    duration: 3.0,
                    layer: self.world.entities.len() as i32,
                });
                e.components.transform = Some(components::Transform {
                    scale: ifol_render_core::types::Vec2 { x: 0.2, y: 0.2 },
                    ..Default::default()
                });
                self.world.add_entity(e);
                self.selected_entity = Some(self.world.entities.len() - 1);
                self.dirty = true;
            }

            let btn2 = RichText::new("+ Image").color(ACCENT_CYAN).size(11.0);
            if ui.button(btn2).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                    .pick_file()
                {
                    let id = format!("img_{}", self.world.entities.len());
                    let mut e = ifol_render_core::ecs::Entity {
                        id,
                        components: Default::default(),
                        resolved: Default::default(),
                    };
                    e.components.image_source = Some(components::ImageSource {
                        path: path.to_string_lossy().to_string(),
                    });
                    e.components.timeline = Some(components::Timeline {
                        start_time: self.time.global_time,
                        duration: 5.0,
                        layer: self.world.entities.len() as i32,
                    });
                    e.components.transform = Some(components::Transform::default());
                    self.world.add_entity(e);
                    self.selected_entity = Some(self.world.entities.len() - 1);
                    self.dirty = true;
                }
            }
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Entity list
        let mut delete_idx: Option<usize> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut new_selection = self.selected_entity;
            for (idx, entity) in self.world.entities.iter().enumerate() {
                let selected = self.selected_entity == Some(idx);
                let icon = if entity.components.color_source.is_some() {
                    "●"
                } else if entity.components.image_source.is_some() {
                    "🖼"
                } else if entity.components.text_source.is_some() {
                    "T"
                } else if entity.components.video_source.is_some() {
                    "▶"
                } else {
                    "◻"
                };

                let label_text = format!("  {} {}", icon, entity.id);
                let resp = ui.selectable_label(
                    selected,
                    RichText::new(&label_text)
                        .color(if selected { Color32::WHITE } else { TEXT_PRIMARY })
                        .size(12.0),
                );
                if resp.clicked() {
                    new_selection = Some(idx);
                }
                if selected && resp.secondary_clicked() {
                    delete_idx = Some(idx);
                }
            }
            self.selected_entity = new_selection;
        });

        // Handle deletion outside the loop
        if let Some(idx) = delete_idx {
            self.world.entities.remove(idx);
            self.world.rebuild_index();
            self.selected_entity = None;
            self.dirty = true;
        }

        // Delete button at bottom
        ui.separator();
        if let Some(idx) = self.selected_entity {
            if idx < self.world.entities.len() {
                if ui.button(RichText::new("🗑 Delete Selected").color(ACCENT_RED).size(11.0)).clicked() {
                    self.world.entities.remove(idx);
                    self.world.rebuild_index();
                    self.selected_entity = None;
                    self.dirty = true;
                }
            }
        }
    }

    fn show_properties(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("PROPERTIES").color(TEXT_SECONDARY).size(11.0).strong());
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        let idx = match self.selected_entity {
            Some(idx) if idx < self.world.entities.len() => idx,
            _ => {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Select an entity")
                            .color(TEXT_SECONDARY)
                            .size(12.0),
                    );
                });
                return;
            }
        };

        let entity = &mut self.world.entities[idx];

        // ID
        ui.horizontal(|ui| {
            ui.label(RichText::new("ID").color(TEXT_SECONDARY).size(11.0));
            ui.text_edit_singleline(&mut entity.id);
        });
        ui.add_space(4.0);

        // Transform
        if let Some(ref mut tf) = entity.components.transform {
            let header = RichText::new("▸ Transform").color(TEXT_PRIMARY).size(12.0);
            egui::CollapsingHeader::new(header)
                .default_open(true)
                .show(ui, |ui| {
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("X").color(TEXT_SECONDARY).size(11.0));
                        changed |= ui
                            .add(egui::DragValue::new(&mut tf.position.x).speed(0.01).range(-2.0..=2.0))
                            .changed();
                        ui.label(RichText::new("Y").color(TEXT_SECONDARY).size(11.0));
                        changed |= ui
                            .add(egui::DragValue::new(&mut tf.position.y).speed(0.01).range(-2.0..=2.0))
                            .changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("W").color(TEXT_SECONDARY).size(11.0));
                        changed |= ui
                            .add(egui::DragValue::new(&mut tf.scale.x).speed(0.01).range(0.0..=4.0))
                            .changed();
                        ui.label(RichText::new("H").color(TEXT_SECONDARY).size(11.0));
                        changed |= ui
                            .add(egui::DragValue::new(&mut tf.scale.y).speed(0.01).range(0.0..=4.0))
                            .changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Rot").color(TEXT_SECONDARY).size(11.0));
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut tf.rotation)
                                    .speed(0.5)
                                    .suffix("°"),
                            )
                            .changed();
                    });
                    if changed {
                        self.dirty = true;
                    }
                });
        }

        // Opacity
        if let Some(ref mut opacity) = entity.components.opacity {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Opacity").color(TEXT_SECONDARY).size(11.0));
                if ui.add(egui::Slider::new(opacity, 0.0..=1.0).show_value(true)).changed() {
                    self.dirty = true;
                }
            });
        }

        // Color Source
        if let Some(ref mut color_src) = entity.components.color_source {
            ui.add_space(4.0);
            let header = RichText::new("▸ Color Source").color(TEXT_PRIMARY).size(12.0);
            egui::CollapsingHeader::new(header)
                .default_open(true)
                .show(ui, |ui| {
                    let mut rgb = [color_src.color.r, color_src.color.g, color_src.color.b];
                    if ui.color_edit_button_rgb(&mut rgb).changed() {
                        color_src.color.r = rgb[0];
                        color_src.color.g = rgb[1];
                        color_src.color.b = rgb[2];
                        self.dirty = true;
                    }
                });
        }

        // Image Source
        if let Some(ref img) = entity.components.image_source {
            ui.add_space(4.0);
            let header = RichText::new("▸ Image Source").color(TEXT_PRIMARY).size(12.0);
            egui::CollapsingHeader::new(header)
                .default_open(true)
                .show(ui, |ui| {
                    ui.label(RichText::new(&img.path).color(TEXT_SECONDARY).size(10.0));
                });
        }

        // Timeline
        if let Some(ref mut tl) = entity.components.timeline {
            ui.add_space(4.0);
            let header = RichText::new("▸ Timeline").color(TEXT_PRIMARY).size(12.0);
            egui::CollapsingHeader::new(header)
                .default_open(true)
                .show(ui, |ui| {
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Start").color(TEXT_SECONDARY).size(11.0));
                        changed |=
                            ui.add(egui::DragValue::new(&mut tl.start_time).speed(0.1).suffix("s")).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Dur").color(TEXT_SECONDARY).size(11.0));
                        changed |=
                            ui.add(egui::DragValue::new(&mut tl.duration).speed(0.1).suffix("s")).changed();
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Layer").color(TEXT_SECONDARY).size(11.0));
                        changed |= ui.add(egui::DragValue::new(&mut tl.layer)).changed();
                    });
                    if changed {
                        self.dirty = true;
                    }
                });
        }
    }

    fn show_timeline(&mut self, ui: &mut egui::Ui) {
        // Header with playback controls
        ui.horizontal(|ui| {
            ui.label(RichText::new("TIMELINE").color(TEXT_SECONDARY).size(11.0).strong());
            ui.separator();

            // Transport buttons
            let btn_size = Vec2::new(24.0, 20.0);
            if ui
                .add_sized(btn_size, egui::Button::new(RichText::new("⏮").size(12.0)))
                .clicked()
            {
                self.time.seek(0.0);
                self.dirty = true;
            }
            let play_text = if self.playing { "⏸" } else { "▶" };
            let play_btn = egui::Button::new(
                RichText::new(play_text).size(12.0).color(if self.playing {
                    ACCENT_RED
                } else {
                    ACCENT_CYAN
                }),
            );
            if ui.add_sized(btn_size, play_btn).clicked() {
                self.playing = !self.playing;
            }
            if ui
                .add_sized(btn_size, egui::Button::new(RichText::new("⏭").size(12.0)))
                .clicked()
            {
                self.time.seek(self.settings.duration);
                self.dirty = true;
            }

            ui.separator();

            // Time display
            ui.label(
                RichText::new(format!(
                    "{:02}:{:04.1}",
                    (self.time.global_time / 60.0) as u32,
                    self.time.global_time % 60.0
                ))
                .color(Color32::WHITE)
                .size(13.0)
                .monospace(),
            );

            ui.separator();

            // Scrubber
            let mut t = self.time.global_time;
            if ui
                .add(
                    egui::Slider::new(&mut t, 0.0..=self.settings.duration)
                        .show_value(false)
                        .trailing_fill(true),
                )
                .changed()
            {
                self.time.seek(t);
                self.dirty = true;
            }

            // Zoom
            ui.separator();
            ui.label(RichText::new("🔍").size(11.0));
            ui.add(
                egui::Slider::new(&mut self.timeline_zoom, 0.3..=4.0)
                    .show_value(false)
                    .logarithmic(true),
            );
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(2.0);

        // Collect track data to avoid borrow issues with painter
        let available_width = ui.available_width();
        let track_height = 22.0;
        let track_gap = 2.0;
        let duration = self.settings.duration;
        let pixels_per_sec = (available_width / duration as f32) * self.timeline_zoom;
        let ruler_height = 16.0;

        // Time ruler markers
        let step = if self.timeline_zoom > 2.0 {
            0.5
        } else if self.timeline_zoom > 1.0 {
            1.0
        } else {
            2.0
        };

        // Collect track rects for drawing
        struct TrackInfo {
            rect: egui::Rect,
            color: Color32,
            label: String,
        }
        let mut tracks: Vec<TrackInfo> = Vec::new();

        let ruler_origin = ui.cursor().min;
        ui.allocate_space(Vec2::new(available_width, ruler_height));

        let tracks_origin = ui.cursor().min;
        for (idx, entity) in self.world.entities.iter().enumerate() {
            if let Some(tl) = &entity.components.timeline {
                let y = tracks_origin.y + idx as f32 * (track_height + track_gap);
                let x_start = tracks_origin.x + tl.start_time as f32 * pixels_per_sec;
                let w = tl.duration as f32 * pixels_per_sec;

                let color = if self.selected_entity == Some(idx) {
                    TRACK_SELECTED
                } else {
                    TRACK_COLOR
                };

                let rect = egui::Rect::from_min_size(
                    egui::pos2(x_start, y),
                    egui::vec2(w, track_height),
                );

                tracks.push(TrackInfo {
                    rect,
                    color,
                    label: entity.id.clone(),
                });

                // Allocate interaction area
                let resp = ui.allocate_rect(rect, egui::Sense::click());
                if resp.clicked() {
                    self.selected_entity = Some(idx);
                }
            }
        }

        // Reserve remaining space
        let total_tracks = self.world.entities.len() as f32;
        let remaining = total_tracks * (track_height + track_gap)
            - tracks.len() as f32 * (track_height + track_gap);
        if remaining > 0.0 {
            ui.allocate_space(Vec2::new(available_width, remaining));
        }

        // Now paint everything (after all ui.allocate calls)
        let painter = ui.painter();

        // Ruler
        let mut t_mark = 0.0f64;
        while t_mark <= duration {
            let x = ruler_origin.x + t_mark as f32 * pixels_per_sec;
            painter.line_segment(
                [
                    egui::pos2(x, ruler_origin.y),
                    egui::pos2(x, ruler_origin.y + ruler_height),
                ],
                egui::Stroke::new(0.5, BORDER_SUBTLE),
            );
            painter.text(
                egui::pos2(x + 2.0, ruler_origin.y),
                egui::Align2::LEFT_TOP,
                format!("{:.0}s", t_mark),
                egui::FontId::monospace(9.0),
                TEXT_SECONDARY,
            );
            t_mark += step;
        }

        // Track rects
        for track in &tracks {
            painter.rect_filled(track.rect, 3.0, track.color);
            let clip_rect = track.rect.shrink(2.0);
            painter.with_clip_rect(clip_rect).text(
                egui::pos2(track.rect.min.x + 6.0, track.rect.min.y + 4.0),
                egui::Align2::LEFT_TOP,
                &track.label,
                egui::FontId::proportional(10.0),
                Color32::WHITE,
            );
        }

        // Playhead
        let total_h =
            self.world.entities.len() as f32 * (track_height + track_gap) + ruler_height;
        let playhead_x = tracks_origin.x + self.time.global_time as f32 * pixels_per_sec;
        painter.line_segment(
            [
                egui::pos2(playhead_x, ruler_origin.y),
                egui::pos2(playhead_x, ruler_origin.y + total_h),
            ],
            egui::Stroke::new(1.5, PLAYHEAD_RED),
        );
        painter.circle_filled(egui::pos2(playhead_x, ruler_origin.y), 4.0, PLAYHEAD_RED);
    }
}
