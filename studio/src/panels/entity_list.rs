use egui::{Ui, RichText, Color32, Frame, Margin, Align, Layout, Sense};
use ifol_render_core::commands::{AddEntity, RemoveEntity};
use ifol_render_core::ecs::{components, Entity};
use crate::app::{EditorApp, ACCENT, TEXT_DIM, TEXT_PRIMARY, RED, BG_SURFACE};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    // Header Panel
    Frame::NONE
        .inner_margin(Margin::symmetric(12, 8))
        .fill(BG_SURFACE)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("ENTITIES").color(TEXT_DIM).strong().size(11.0));
                
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button(RichText::new("➕ Add").color(ACCENT).size(11.0), |ui| {
                        if ui.button("Color Solid").clicked() {
                            let n = app.world.entities.len();
                            let mut e = Entity {
                                id: format!("color_{}", n),
                                components: Default::default(),
                                resolved: Default::default(),
                            };
                            e.components.color_source = Some(components::ColorSource {
                                color: ifol_render_core::color::Color4::new(0.5, 0.5, 0.5, 1.0),
                            });
                            e.components.timeline = Some(components::Timeline {
                                start_time: app.time.global_time,
                                duration: 3.0,
                                layer: n as i32,
                            });
                            e.components.transform = Some(components::Transform::default());
                            app.commands.execute(
                                Box::new(AddEntity::new(e)),
                                &mut app.world,
                            );
                            app.selected = Some(n);
                            app.renderer = None; // invalidate renderer
                            app.dirty = true;
                            ui.close_menu();
                        }
                        if ui.button("Image Layer").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                                .pick_file()
                            {
                                let n = app.world.entities.len();
                                let mut e = Entity {
                                    id: format!("img_{}", n),
                                    components: Default::default(),
                                    resolved: Default::default(),
                                };
                                e.components.image_source = Some(components::ImageSource {
                                    path: path.to_string_lossy().to_string(),
                                });
                                e.components.timeline = Some(components::Timeline {
                                    start_time: app.time.global_time,
                                    duration: 5.0,
                                    layer: n as i32,
                                });
                                e.components.transform = Some(components::Transform::default());
                                app.commands.execute(
                                    Box::new(AddEntity::new(e)),
                                    &mut app.world,
                                );
                                app.selected = Some(n);
                                app.renderer = None; // invalidate renderer
                                app.dirty = true;
                            }
                            ui.close_menu();
                        }
                    });
                });
            });
        });

    ui.add_space(4.0);

    // Entity List Tree View
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut sel = app.selected;
            
            for (i, e) in app.world.entities.iter().enumerate() {
                let is_sel = sel == Some(i);
                
                let (icon, color) = match () {
                    _ if e.components.color_source.is_some() => ("●", Color32::from_rgb(123, 31, 162)), // Purple
                    _ if e.components.image_source.is_some() => ("🖼", Color32::from_rgb(216, 67, 21)), // Orange
                    _ if e.components.video_source.is_some() => ("▶", Color32::from_rgb(216, 67, 21)),
                    _ if e.components.text_source.is_some() => ("T", Color32::from_rgb(46, 125, 50)), // Green
                    _ => ("◻", TEXT_DIM),
                };

                let bg_color = if is_sel { ACCENT.linear_multiply(0.3) } else { Color32::TRANSPARENT };
                
                let response = Frame::NONE
                    .fill(bg_color)
                    .inner_margin(Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon).color(color).size(12.0));
                            ui.add_space(8.0);
                            let text_color = if is_sel { Color32::WHITE } else { TEXT_PRIMARY };
                            ui.label(RichText::new(&e.id).color(text_color).size(12.0));
                        });
                    })
                    .response
                    .interact(Sense::click());

                if response.hovered() && !is_sel {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                if response.clicked() {
                    sel = Some(i);
                }
            }
            app.selected = sel;
        });

    // Footer actions
    if let Some(i) = app.selected {
        if i < app.world.entities.len() {
            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.add_space(12.0);
                if ui
                    .button(RichText::new("🗑 Delete Selected").color(RED).size(11.0))
                    .clicked()
                {
                    let eid = app.world.entities[i].id.clone();
                    app.commands.execute(
                        Box::new(RemoveEntity::new(eid)),
                        &mut app.world,
                    );
                    app.selected = None;
                    app.renderer = None; // invalidate renderer
                    app.dirty = true;
                }
            });
        }
    }
}
