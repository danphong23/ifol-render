use egui::{Ui, RichText};
use ifol_render_core::time::TimeState;
use crate::app::{EditorApp, ACCENT, TEXT_PRIMARY, TEXT_DIM, RED};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    ui.horizontal_centered(|ui| {
        ui.label(RichText::new("◆ ifol-render").color(ACCENT).strong().size(13.0));
        ui.separator();

        ui.menu_button(
            RichText::new("File").color(TEXT_PRIMARY).size(11.0),
            |ui| {
                if ui.button("  New Scene").clicked() {
                    *app = EditorApp::new();
                    ui.close_menu();
                }
                if ui.button("  Open...").clicked() {
                    if let Some(path) =
                        rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file()
                    {
                        if let Ok(json) = std::fs::read_to_string(&path) {
                            match ifol_render_core::scene::SceneDescription::from_json(&json) {
                                Ok(desc) => {
                                    app.settings = desc.settings.clone();
                                    app.world = desc.into_world();
                                    app.time = TimeState::new(app.settings.fps);
                                    app.selected = None;
                                    app.renderer = None;
                                    app.dirty = true;
                                    app.status = format!("Opened: {}", path.display());
                                }
                                Err(e) => app.status = format!("Error: {}", e),
                            }
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("  Save...").clicked() {
                    let json =
                        serde_json::to_string_pretty(&app.world).unwrap_or_default();
                    if let Some(path) =
                        rfd::FileDialog::new().add_filter("JSON", &["json"]).save_file()
                    {
                        let _ = std::fs::write(&path, &json);
                        app.status = format!("Saved: {}", path.display());
                    }
                    ui.close_menu();
                }
            },
        );

        ui.separator();
        ui.label(
            RichText::new(format!(
                "{}x{} @ {}fps",
                app.settings.width, app.settings.height, app.settings.fps
            ))
            .color(TEXT_DIM)
            .size(10.0),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if app.dirty {
                ui.label(RichText::new("●").color(RED).size(10.0));
            }
        });
    });
}
