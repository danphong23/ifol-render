use egui::{Ui, Vec2, RichText};
use crate::app::{EditorApp, TEXT_DIM};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    if let Some(tex) = &app.viewport_tex {
        let avail = ui.available_size();
        let aspect = app.settings.width as f32 / app.settings.height as f32;
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
