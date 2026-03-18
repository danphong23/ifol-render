use crate::app::{ACCENT, EditorApp, RED, TEXT_DIM, TEXT_PRIMARY};
use egui::{Align, Frame, Layout, Margin, RichText, Ui};
use ifol_render_core::time::TimeState;

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    // 3-Zone Flex Layout container
    let height = 36.0;

    Frame::NONE
        .inner_margin(Margin::symmetric(12, 4))
        .show(ui, |ui| {
            ui.set_height(height);

            ui.allocate_ui_with_layout(
                ui.available_size(),
                Layout::left_to_right(Align::Center),
                |ui| {
                    // ==========================================
                    // ZONE 1: LEFT (Brand & Identity & File Menu)
                    // ==========================================
                    ui.label(RichText::new("🏠").size(14.0).color(TEXT_PRIMARY));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("ifol-render")
                            .color(ACCENT)
                            .strong()
                            .size(14.0),
                    );

                    if app.dirty {
                        ui.label(
                            RichText::new("●")
                                .color(egui::Color32::from_rgb(100, 150, 255))
                                .size(10.0),
                        );
                    }

                    ui.add_space(12.0);

                    ui.menu_button(RichText::new("File").color(TEXT_PRIMARY).size(12.0), |ui| {
                        if ui.button("New Scene").clicked() {
                            let ffmpeg = app.ffmpeg_path.clone();
                            *app = EditorApp::new();
                            app.ffmpeg_path = ffmpeg; // preserve ffmpeg path
                            ui.close_menu();
                        }
                        if ui.button("Open...").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("JSON", &["json"])
                                .pick_file()
                                && let Ok(json) = std::fs::read_to_string(&path)
                            {
                                match ifol_render_core::scene::SceneDescription::from_json(&json) {
                                    Ok(desc) => {
                                        app.settings = desc.settings.clone();
                                        app.world = desc.into_world();
                                        app.time = TimeState::new(app.settings.fps);
                                        app.selected = None;
                                        app.selected_indices.clear();
                                        app.renderer = None;
                                        app.needs_render = true;
                                        app.dirty = false;
                                        app.scene_path = Some(path.clone());
                                        app.status = format!("Opened: {}", path.display());
                                    }
                                    Err(e) => app.status = format!("Error: {}", e),
                                }
                            }
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Save").clicked() {
                            save_scene(app, false);
                            ui.close_menu();
                        }
                        if ui.button("Save As...").clicked() {
                            save_scene(app, true);
                            ui.close_menu();
                        }
                    });

                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!(
                            "{}x{} @ {}fps",
                            app.settings.width, app.settings.height, app.settings.fps
                        ))
                        .color(TEXT_DIM)
                        .size(11.0),
                    );

                    // ==========================================
                    // ZONE 3: RIGHT (Actions & Execution)
                    // ==========================================
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Overflow menu (Settings, Export, etc)
                        ui.menu_button(RichText::new("⋮").size(16.0).color(TEXT_PRIMARY), |ui| {
                            if ui.button("Export Video...").clicked() {
                                export_video(app);
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("FFmpeg Settings...").clicked() {
                                // Simple text input for ffmpeg path
                                ui.close_menu();
                                // We'll handle this as a sub-menu instead
                            }
                            // Inline FFmpeg path edit
                            ui.label(RichText::new("FFmpeg Path:").color(TEXT_DIM).size(10.0));
                            let mut path_str = app.ffmpeg_path.clone().unwrap_or_default();
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut path_str)
                                    .desired_width(200.0)
                                    .hint_text("ffmpeg (in PATH)"),
                            );
                            if r.changed() {
                                app.ffmpeg_path = if path_str.is_empty() {
                                    None
                                } else {
                                    Some(path_str)
                                };
                            }
                            if ui.button("Browse...").clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("FFmpeg", &["exe", ""])
                                    .pick_file()
                            {
                                app.ffmpeg_path = Some(path.to_string_lossy().to_string());
                            }
                        });

                        ui.add_space(8.0);

                        // Save (Status-Aware) — quick save
                        let save_color = if app.dirty {
                            egui::Color32::from_rgb(100, 150, 255)
                        } else {
                            TEXT_DIM
                        };
                        if ui
                            .button(RichText::new("💾").color(save_color).size(14.0))
                            .clicked()
                        {
                            save_scene(app, false);
                        }

                        ui.add_space(8.0);

                        // Redo
                        let redo_color = if app.commands.can_redo() {
                            TEXT_PRIMARY
                        } else {
                            TEXT_DIM
                        };
                        if ui
                            .button(RichText::new("↪").color(redo_color).size(14.0))
                            .clicked()
                            && app.commands.can_redo()
                        {
                            app.commands.redo(&mut app.world);
                            app.needs_render = true;
                            app.dirty = true;
                        }

                        // Undo
                        let undo_color = if app.commands.can_undo() {
                            TEXT_PRIMARY
                        } else {
                            TEXT_DIM
                        };
                        if ui
                            .button(RichText::new("↩").color(undo_color).size(14.0))
                            .clicked()
                            && app.commands.can_undo()
                        {
                            app.commands.undo(&mut app.world);
                            app.needs_render = true;
                            app.dirty = true;
                        }

                        ui.add_space(16.0);

                        // Run / Stop (Execution)
                        let is_playing = app.playing;
                        if is_playing {
                            let btn = egui::Button::new(
                                RichText::new("■ Stop").color(egui::Color32::WHITE).strong(),
                            )
                            .fill(RED);
                            if ui.add(btn).clicked() {
                                app.playing = false;
                            }
                        } else {
                            let btn = egui::Button::new(
                                RichText::new("▶ Run").color(egui::Color32::BLACK).strong(),
                            )
                            .fill(egui::Color32::from_rgb(100, 220, 120));
                            if ui.add(btn).clicked() {
                                app.playing = true;
                            }
                        }

                        // ==========================================
                        // ZONE 2: CENTER (Workspaces)
                        // ==========================================
                        ui.with_layout(
                            Layout::centered_and_justified(egui::Direction::LeftToRight),
                            |ui| {
                                ui.label(
                                    RichText::new("Compositing")
                                        .color(TEXT_PRIMARY)
                                        .strong()
                                        .size(12.0),
                                );
                            },
                        );
                    });
                },
            );
        });
}

/// Save the current scene (Save or Save As).
fn save_scene(app: &mut EditorApp, force_dialog: bool) {
    if !force_dialog && let Some(ref path) = app.scene_path {
        let desc = ifol_render_core::scene::SceneDescription::from_world(&app.world, &app.settings);
        if let Ok(json) = desc.to_json() {
            let _ = std::fs::write(path, &json);
            app.status = format!("Saved: {}", path.display());
            app.dirty = false;
        }
        return;
    }
    // Save As dialog
    let desc = ifol_render_core::scene::SceneDescription::from_world(&app.world, &app.settings);
    if let Ok(json) = desc.to_json()
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .save_file()
    {
        let _ = std::fs::write(&path, &json);
        app.status = format!("Saved: {}", path.display());
        app.scene_path = Some(path);
        app.dirty = false;
    }
}

/// Export video using the export pipeline.
fn export_video(app: &mut EditorApp) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("MP4 Video", &["mp4"])
        .add_filter("WebM Video", &["webm"])
        .add_filter("MOV Video", &["mov"])
        .save_file()
    {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
        let codec = match ext {
            "webm" => ifol_render_core::export::VideoCodec::VP9,
            "mov" => ifol_render_core::export::VideoCodec::ProRes,
            _ => ifol_render_core::export::VideoCodec::H264,
        };
        let config = ifol_render_core::export::ExportConfig {
            output_path: path.to_string_lossy().into(),
            codec,
            ffmpeg_path: app.ffmpeg_path.clone(),
            ..Default::default()
        };
        let mut renderer = ifol_render_core::Renderer::new(app.settings.width, app.settings.height);
        // Load image sources
        for entity in &app.world.entities {
            if let Some(ref img) = entity.components.image_source {
                let _ = renderer.load_image(&entity.id, &img.path);
            }
        }
        app.status = format!("Exporting to {}...", path.display());
        match ifol_render_core::export::export_video(
            &mut app.world,
            &app.settings,
            &config,
            &mut renderer,
            |p| {
                log::info!(
                    "Export: {:.0}% ({}/{})",
                    p.percent(),
                    p.current_frame,
                    p.total_frames
                );
            },
        ) {
            Ok(()) => app.status = format!("✅ Exported: {}", path.display()),
            Err(e) => app.status = format!("❌ Export error: {}", e),
        }
    }
}
