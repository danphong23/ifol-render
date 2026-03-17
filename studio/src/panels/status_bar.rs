use egui::{Ui, RichText};
use crate::app::{EditorApp, TEXT_DIM};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&app.status).color(TEXT_DIM).size(10.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let undo_info = if app.commands.can_undo() {
                format!("↩{}", app.commands.undo_count())
            } else {
                "↩0".into()
            };
            let redo_info = if app.commands.can_redo() {
                format!("↪{}", app.commands.redo_count())
            } else {
                "↪0".into()
            };
            ui.label(
                RichText::new(format!(
                    "{} entities  {}  {}",
                    app.world.entities.len(), undo_info, redo_info
                ))
                .color(TEXT_DIM)
                .size(10.0),
            );
        });
    });
}
