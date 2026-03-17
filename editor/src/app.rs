//! Core editor application with GPU-rendered viewport.

use egui::{Color32, ColorImage, TextureHandle, TextureOptions, Vec2};
use ifol_render_core::ecs::{components, World};
use ifol_render_core::scene::{RenderSettings, SceneDescription};
use ifol_render_core::time::TimeState;

/// The main editor state.
pub struct EditorApp {
    /// The scene world (ECS).
    pub world: World,
    /// Render settings (resolution, fps, etc.).
    pub settings: RenderSettings,
    /// Current playback time.
    pub time: TimeState,
    /// Whether the timeline is playing.
    pub playing: bool,
    /// Currently selected entity index.
    pub selected_entity: Option<usize>,
    /// The GPU-rendered viewport image cached as egui texture.
    viewport_texture: Option<TextureHandle>,
    /// Rendered pixel data (updated each frame when playing or when scene changes).
    pixels: Vec<u8>,
    /// Whether scene needs re-render.
    dirty: bool,
    /// Scene file path (if loaded from file).
    file_path: Option<String>,
    /// Status message.
    status: String,
}

impl EditorApp {
    pub fn new() -> Self {
        // Create a default scene with a few entities for testing
        let mut world = World::new();

        // Background
        let mut bg = ifol_render_core::ecs::Entity {
            id: "background".to_string(),
            components: Default::default(),
            resolved: Default::default(),
        };
        bg.components.color_source = Some(components::ColorSource {
            color: ifol_render_core::color::Color4::new(0.15, 0.15, 0.2, 1.0),
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
        }
    }

    /// Re-render the scene using the GPU.
    fn render_scene(&mut self) {
        // Run ECS pipeline
        ifol_render_core::ecs::pipeline::run(&mut self.world, &self.time);

        // Create headless renderer and render frame
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

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Advance time if playing
        if self.playing {
            let dt = ctx.input(|i| i.unstable_dt) as f64;
            self.time.seek(self.time.global_time + dt);
            if self.time.global_time >= self.settings.duration {
                self.time.seek(0.0);
            }
            self.dirty = true;
            ctx.request_repaint();
        }

        // Re-render if dirty
        if self.dirty {
            self.render_scene();
        }

        // Update viewport texture from pixels
        if !self.pixels.is_empty() {
            let image = ColorImage::from_rgba_unmultiplied(
                [self.settings.width as usize, self.settings.height as usize],
                &self.pixels,
            );
            // Always recreate texture to avoid borrow issues
            let tex = ctx.load_texture("viewport", image, TextureOptions::LINEAR);
            self.viewport_texture = Some(tex);
        }

        // ── Menu bar ──
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Scene").clicked() {
                        *self = EditorApp::new();
                        ui.close_menu();
                    }
                    if ui.button("Open Scene...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            if let Ok(json) = std::fs::read_to_string(&path) {
                                match self.load_scene_from_json(&json) {
                                    Ok(()) => {
                                        self.file_path = Some(path.to_string_lossy().to_string());
                                        self.status = format!("Opened: {}", path.display());
                                    }
                                    Err(e) => self.status = format!("Error: {}", e),
                                }
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Save Scene").clicked() {
                        let json = serde_json::to_string_pretty(&self.world)
                            .unwrap_or_default();
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
                ui.label(format!(
                    "{}x{} | {:.0}fps | {:.1}s / {:.1}s",
                    self.settings.width,
                    self.settings.height,
                    self.settings.fps,
                    self.time.global_time,
                    self.settings.duration,
                ));
            });
        });

        // ── Status bar ──
        egui::TopBottomPanel::bottom("status_bar")
            .max_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(&self.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("Entities: {}", self.world.entities.len()));
                    });
                });
            });

        // ── Timeline panel (bottom) ──
        egui::TopBottomPanel::bottom("timeline")
            .resizable(true)
            .min_height(100.0)
            .default_height(150.0)
            .show(ctx, |ui| {
                self.show_timeline(ui);
            });

        // ── Entity list (left) ──
        egui::SidePanel::left("entity_list")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                self.show_entity_list(ui);
            });

        // ── Properties (right) ──
        egui::SidePanel::right("properties")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                self.show_properties(ui);
            });

        // ── Viewport (center) ──
        egui::CentralPanel::default().show(ctx, |ui| {
            self.show_viewport(ui);
        });
    }
}

impl EditorApp {
    fn show_viewport(&self, ui: &mut egui::Ui) {
        ui.heading("Viewport");
        if let Some(tex) = &self.viewport_texture {
            let available = ui.available_size();
            let aspect =
                self.settings.width as f32 / self.settings.height as f32;
            let (w, h) = if available.x / available.y > aspect {
                (available.y * aspect, available.y)
            } else {
                (available.x, available.x / aspect)
            };
            ui.centered_and_justified(|ui| {
                ui.image(egui::load::SizedTexture::new(tex.id(), Vec2::new(w, h)));
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("No render output");
            });
        }
    }

    fn show_entity_list(&mut self, ui: &mut egui::Ui) {
        ui.heading("Entities");
        ui.separator();

        // Add entity buttons
        ui.horizontal(|ui| {
            if ui.button("+ Color").clicked() {
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
            if ui.button("+ Image").clicked() {
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

        ui.separator();

        // Entity list
        let mut new_selection = self.selected_entity;
        for (idx, entity) in self.world.entities.iter().enumerate() {
            let selected = self.selected_entity == Some(idx);
            let icon = if entity.components.color_source.is_some() {
                "🟥"
            } else if entity.components.image_source.is_some() {
                "🖼"
            } else if entity.components.text_source.is_some() {
                "📝"
            } else if entity.components.video_source.is_some() {
                "🎬"
            } else {
                "⬜"
            };

            let label = format!("{} {}", icon, entity.id);
            if ui.selectable_label(selected, label).clicked() {
                new_selection = Some(idx);
            }
        }
        self.selected_entity = new_selection;

        // Delete button
        ui.separator();
        if let Some(idx) = self.selected_entity {
            if ui.button("🗑 Delete Selected").clicked() {
                self.world.entities.remove(idx);
                self.world.rebuild_index();
                self.selected_entity = None;
                self.dirty = true;
            }
        }
    }

    fn show_properties(&mut self, ui: &mut egui::Ui) {
        ui.heading("Properties");
        ui.separator();

        let idx = match self.selected_entity {
            Some(idx) if idx < self.world.entities.len() => idx,
            _ => {
                ui.label("Select an entity to edit properties.");
                return;
            }
        };

        let entity = &mut self.world.entities[idx];

        // ID
        ui.horizontal(|ui| {
            ui.label("ID:");
            ui.text_edit_singleline(&mut entity.id);
        });

        ui.separator();

        // Transform
        if let Some(ref mut tf) = entity.components.transform {
            ui.collapsing("Transform", |ui| {
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("Pos X:");
                    changed |= ui.add(egui::DragValue::new(&mut tf.position.x).speed(0.01)).changed();
                    ui.label("Y:");
                    changed |= ui.add(egui::DragValue::new(&mut tf.position.y).speed(0.01)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Scale X:");
                    changed |= ui.add(egui::DragValue::new(&mut tf.scale.x).speed(0.01)).changed();
                    ui.label("Y:");
                    changed |= ui.add(egui::DragValue::new(&mut tf.scale.y).speed(0.01)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Rotation:");
                    changed |= ui.add(egui::DragValue::new(&mut tf.rotation).speed(0.01).suffix("°")).changed();
                });
                if changed {
                    self.dirty = true;
                }
            });
        }

        // Opacity
        if let Some(ref mut opacity) = entity.components.opacity {
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                if ui.add(egui::Slider::new(opacity, 0.0..=1.0)).changed() {
                    self.dirty = true;
                }
            });
        }

        // Color Source
        if let Some(ref mut color_src) = entity.components.color_source {
            ui.collapsing("Color Source", |ui| {
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
            ui.collapsing("Image Source", |ui| {
                ui.label(format!("Path: {}", img.path));
            });
        }

        // Timeline
        if let Some(ref mut tl) = entity.components.timeline {
            ui.collapsing("Timeline", |ui| {
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("Start:");
                    changed |= ui.add(egui::DragValue::new(&mut tl.start_time).speed(0.1).suffix("s")).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Duration:");
                    changed |= ui.add(egui::DragValue::new(&mut tl.duration).speed(0.1).suffix("s")).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("Layer:");
                    changed |= ui.add(egui::DragValue::new(&mut tl.layer)).changed();
                });
                if changed {
                    self.dirty = true;
                }
            });
        }
    }

    fn show_timeline(&mut self, ui: &mut egui::Ui) {
        // Playback controls
        ui.horizontal(|ui| {
            if ui.button("⏮").clicked() {
                self.time.seek(0.0);
                self.dirty = true;
            }
            if ui.button(if self.playing { "⏸" } else { "▶" }).clicked() {
                self.playing = !self.playing;
            }
            if ui.button("⏭").clicked() {
                self.time.seek(self.settings.duration);
                self.dirty = true;
            }

            ui.separator();

            // Time scrubber
            let mut t = self.time.global_time;
            let slider = egui::Slider::new(&mut t, 0.0..=self.settings.duration)
                .text("Time")
                .suffix("s");
            if ui.add(slider).changed() {
                self.time.seek(t);
                self.dirty = true;
            }
        });

        ui.separator();

        // Timeline tracks visualization
        let available_width = ui.available_width();
        let track_height = 24.0;
        let duration = self.settings.duration;
        let pixels_per_sec = available_width / duration as f32;

        let painter = ui.painter();
        let origin = ui.cursor().min;

        // Draw tracks
        for (idx, entity) in self.world.entities.iter().enumerate() {
            if let Some(tl) = &entity.components.timeline {
                let y = origin.y + idx as f32 * (track_height + 2.0);
                let x_start = origin.x + tl.start_time as f32 * pixels_per_sec;
                let x_end = origin.x + (tl.start_time + tl.duration) as f32 * pixels_per_sec;

                let color = if self.selected_entity == Some(idx) {
                    Color32::from_rgb(100, 150, 255)
                } else {
                    Color32::from_rgb(70, 90, 120)
                };

                let rect = egui::Rect::from_min_max(
                    egui::pos2(x_start, y),
                    egui::pos2(x_end, y + track_height),
                );
                painter.rect_filled(rect, 4.0, color);
                painter.text(
                    egui::pos2(x_start + 4.0, y + 4.0),
                    egui::Align2::LEFT_TOP,
                    &entity.id,
                    egui::FontId::proportional(12.0),
                    Color32::WHITE,
                );

                // Click to select
                if ui.input(|i| i.pointer.any_click()) {
                    if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                        if rect.contains(pos) {
                            self.selected_entity = Some(idx);
                        }
                    }
                }
            }
        }

        // Draw playhead
        let playhead_x = origin.x + self.time.global_time as f32 * pixels_per_sec;
        let total_tracks = self.world.entities.len() as f32;
        painter.line_segment(
            [
                egui::pos2(playhead_x, origin.y),
                egui::pos2(playhead_x, origin.y + total_tracks * (track_height + 2.0)),
            ],
            egui::Stroke::new(2.0, Color32::RED),
        );

        // Reserve space for tracks
        ui.allocate_space(Vec2::new(
            available_width,
            total_tracks * (track_height + 2.0),
        ));
    }
}
