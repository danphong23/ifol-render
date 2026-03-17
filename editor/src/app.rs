//! Editor application state.

use ifol_render_core::ecs::World;
use ifol_render_core::time::TimeState;

pub struct EditorApp {
    world: World,
    time: TimeState,
    selected_entity: Option<String>,
    playing: bool,
}

impl EditorApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            world: World::new(),
            time: TimeState::new(30.0),
            selected_entity: None,
            playing: false,
        }
    }
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Advance time if playing
        if self.playing {
            self.time.advance();
            ifol_render_core::ecs::pipeline::run(&mut self.world, &self.time);
            ctx.request_repaint(); // Keep animating
        }

        // ═══ Top Menu Bar ═══
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Scene...").clicked() {
                        // TODO: file dialog
                        ui.close_menu();
                    }
                    if ui.button("Save Scene").clicked() {
                        // TODO: serialize world
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Reset Layout").clicked() {
                        ui.close_menu();
                    }
                });
            });
        });

        // ═══ Left Panel: Entity List ═══
        egui::SidePanel::left("entity_list")
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("Entities");
                ui.separator();

                for entity in &self.world.entities {
                    let selected = self.selected_entity.as_ref() == Some(&entity.id);
                    if ui.selectable_label(selected, &entity.id).clicked() {
                        self.selected_entity = Some(entity.id.clone());
                    }
                }

                ui.separator();
                if ui.button("+ Add Entity").clicked() {
                    // TODO: add entity dialog
                }
            });

        // ═══ Right Panel: Properties ═══
        egui::SidePanel::right("properties")
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();

                if let Some(ref id) = self.selected_entity {
                    if let Some(entity) = self.world.get(id) {
                        ui.label(format!("ID: {}", entity.id));
                        ui.separator();

                        // Transform
                        if let Some(ref tf) = entity.components.transform {
                            ui.collapsing("Transform", |ui| {
                                ui.label(format!("Position: ({:.1}, {:.1})", tf.position.x, tf.position.y));
                                ui.label(format!("Scale: ({:.2}, {:.2})", tf.scale.x, tf.scale.y));
                                ui.label(format!("Rotation: {:.1}°", tf.rotation.to_degrees()));
                            });
                        }

                        // Timeline
                        if let Some(ref tl) = entity.components.timeline {
                            ui.collapsing("Timeline", |ui| {
                                ui.label(format!("Start: {:.2}s", tl.start_time));
                                ui.label(format!("Duration: {:.2}s", tl.duration));
                                ui.label(format!("Layer: {}", tl.layer));
                            });
                        }

                        // Opacity
                        if let Some(opacity) = entity.components.opacity {
                            ui.collapsing("Opacity", |ui| {
                                ui.label(format!("Value: {:.2}", opacity));
                            });
                        }

                        // Effects
                        if let Some(ref effects) = entity.components.effects {
                            ui.collapsing("Effects", |ui| {
                                for eff in effects {
                                    ui.label(format!("• {}", eff.effect_type));
                                }
                            });
                        }
                    }
                } else {
                    ui.label("No entity selected");
                }
            });

        // ═══ Bottom Panel: Timeline ═══
        egui::TopBottomPanel::bottom("timeline")
            .default_height(120.0)
            .show(ctx, |ui| {
                super::ui::timeline::draw(ui, &self.world, &mut self.time, &mut self.playing);
            });

        // ═══ Center: Viewport ═══
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Viewport");
            let available = ui.available_size();

            // Viewport placeholder
            let (_response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());

            // Draw dark background
            painter.rect_filled(
                painter.clip_rect(),
                0.0,
                egui::Color32::from_rgb(30, 30, 40),
            );

            // TODO: Render scene using GPU engine and display result
            painter.text(
                painter.clip_rect().center(),
                egui::Align2::CENTER_CENTER,
                format!("GPU Viewport\n{:.2}s | Frame {}", self.time.global_time, self.time.frame_index),
                egui::FontId::proportional(16.0),
                egui::Color32::from_rgb(120, 120, 140),
            );
        });
    }
}
