use egui::{Ui, RichText, Color32, Frame, Margin, Align, Layout, Sense};
use ifol_render_core::commands::{AddEntity, RemoveEntity};
use ifol_render_core::ecs::{components, Entity};
use crate::app::{EditorApp, ACCENT, TEXT_DIM, TEXT_PRIMARY, RED};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    // Header: "ENTITIES" + Add dropdown
    ui.horizontal(|ui| {
        ui.label(RichText::new("ENTITIES").color(TEXT_DIM).strong().size(11.0));
        
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.menu_button(RichText::new("+ Add").color(ACCENT).size(11.0), |ui| {
                if ui.button("🎨 Color Solid").clicked() {
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
                    app.renderer = None;
                    app.dirty = true;
                    ui.close_menu();
                }
                if ui.button("🖼 Image Layer").clicked() {
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
                        app.renderer = None;
                        app.dirty = true;
                    }
                    ui.close_menu();
                }
            });
        });
    });

    ui.add_space(2.0);
    ui.separator();

    // Entity List — scroll area fills remaining space

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let mut sel = app.selected;
            
            for (i, e) in app.world.entities.iter().enumerate() {
                let is_sel = sel == Some(i);
                
                let (icon, color) = match () {
                    _ if e.components.color_source.is_some() => ("●", Color32::from_rgb(147, 51, 234)),
                    _ if e.components.image_source.is_some() => ("🖼", Color32::from_rgb(234, 88, 12)),
                    _ if e.components.video_source.is_some() => ("▶", Color32::from_rgb(234, 88, 12)),
                    _ if e.components.text_source.is_some() => ("T", Color32::from_rgb(22, 163, 74)),
                    _ => ("◻", TEXT_DIM),
                };

                let bg = if is_sel { ACCENT.linear_multiply(0.25) } else { Color32::TRANSPARENT };
                
                let resp = Frame::NONE
                    .fill(bg)
                    .inner_margin(Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icon).color(color).size(12.0));
                            ui.add_space(6.0);
                            let tc = if is_sel { Color32::WHITE } else { TEXT_PRIMARY };
                            ui.label(RichText::new(&e.id).color(tc).size(12.0));
                        });
                    })
                    .response
                    .interact(Sense::click());

                if resp.clicked() {
                    sel = Some(i);
                }
            }
            app.selected = sel;

            // Delete button at bottom of scroll area (not bottom_up layout)
            if let Some(i) = app.selected {
                if i < app.world.entities.len() {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
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
                        app.renderer = None;
                        app.dirty = true;
                    }
                }
            }
        });
}
