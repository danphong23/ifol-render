use egui::{Ui, RichText, Color32};
use ifol_render_core::commands::{AddEntity, RemoveEntity};
use ifol_render_core::ecs::{components, Entity};
use crate::app::{EditorApp, ACCENT, TEXT_DIM, TEXT_PRIMARY, RED};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    ui.label(RichText::new("ENTITIES").color(TEXT_DIM).size(10.0).strong());
    ui.add_space(2.0);

    ui.horizontal(|ui| {
        if ui
            .small_button(RichText::new("+ Color").color(ACCENT).size(10.0))
            .clicked()
        {
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
        }
        if ui
            .small_button(RichText::new("+ Image").color(ACCENT).size(10.0))
            .clicked()
        {
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
        }
    });

    ui.add_space(2.0);
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut sel = app.selected;
            for (i, e) in app.world.entities.iter().enumerate() {
                let is_sel = sel == Some(i);
                let icon = match () {
                    _ if e.components.color_source.is_some() => "●",
                    _ if e.components.image_source.is_some() => "🖼",
                    _ if e.components.video_source.is_some() => "▶",
                    _ if e.components.text_source.is_some() => "T",
                    _ => "◻",
                };
                let text = RichText::new(format!(" {} {}", icon, e.id))
                    .color(if is_sel { Color32::WHITE } else { TEXT_PRIMARY })
                    .size(11.0);
                if ui.selectable_label(is_sel, text).clicked() {
                    sel = Some(i);
                }
            }
            app.selected = sel;
        });

    if let Some(i) = app.selected {
        if i < app.world.entities.len() {
            ui.separator();
            if ui
                .small_button(RichText::new("🗑 Delete").color(RED).size(10.0))
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
        }
    }
}
