use egui::{Ui, RichText, Color32, Vec2};
use crate::app::{EditorApp, TEXT_DIM, RED, ACCENT_GREEN, BORDER, TRACK_SEL, TRACK_BG};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("TIMELINE").color(TEXT_DIM).size(10.0).strong());
        ui.separator();

        if ui.small_button("⏮").clicked() {
            app.time.seek(0.0);
            app.dirty = true;
        }

        let play_label = if app.playing {
            RichText::new("⏸").color(RED).size(12.0)
        } else {
            RichText::new("▶").color(ACCENT_GREEN).size(12.0)
        };
        if ui.small_button(play_label).clicked() {
            app.playing = !app.playing;
        }

        if ui.small_button("⏭").clicked() {
            app.time.seek(app.settings.duration);
            app.dirty = true;
        }

        ui.separator();
        ui.label(
            RichText::new(format!(
                "{:02}:{:04.1}",
                (app.time.global_time / 60.0) as u32,
                app.time.global_time % 60.0
            ))
            .color(Color32::WHITE)
            .size(12.0)
            .monospace(),
        );

        ui.separator();
        let mut t = app.time.global_time;
        if ui
            .add(
                egui::Slider::new(&mut t, 0.0..=app.settings.duration)
                    .show_value(false)
                    .trailing_fill(true),
            )
            .changed()
        {
            app.time.seek(t);
            app.dirty = true;
        }

        ui.separator();
        ui.add(
            egui::Slider::new(&mut app.zoom, 0.3..=4.0)
                .show_value(false)
                .logarithmic(true)
                .text(RichText::new("Zoom").color(TEXT_DIM).size(9.0)),
        );
    });

    ui.add_space(2.0);

    let avail_w = ui.available_width();
    let dur = app.settings.duration;
    let pps = (avail_w / dur as f32) * app.zoom;
    let track_h = 22.0f32;
    let gap = 2.0f32;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let ruler_h = 14.0;
            let total_tracks = app.world.entities.len();
            let total_h = ruler_h + total_tracks as f32 * (track_h + gap) + 8.0;
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(avail_w, total_h), egui::Sense::click());

            let painter = ui.painter_at(rect);
            let origin = rect.min;

            // Ruler
            let step = if app.zoom > 2.0 {
                0.5
            } else if app.zoom > 1.0 {
                1.0
            } else {
                2.0
            };
            let mut tm = 0.0f64;
            while tm <= dur {
                let x = origin.x + tm as f32 * pps;
                painter.line_segment(
                    [egui::pos2(x, origin.y), egui::pos2(x, origin.y + ruler_h)],
                    egui::Stroke::new(0.5, BORDER),
                );
                painter.text(
                    egui::pos2(x + 2.0, origin.y),
                    egui::Align2::LEFT_TOP,
                    format!("{:.0}s", tm),
                    egui::FontId::monospace(8.0),
                    TEXT_DIM,
                );
                tm += step;
            }

            // Tracks
            let tracks_y = origin.y + ruler_h;
            for (i, e) in app.world.entities.iter().enumerate() {
                if let Some(tl) = &e.components.timeline {
                    let y = tracks_y + i as f32 * (track_h + gap);
                    let x0 = origin.x + tl.start_time as f32 * pps;
                    let w = tl.duration as f32 * pps;

                    let color = if app.selected == Some(i) {
                        TRACK_SEL
                    } else {
                        TRACK_BG
                    };
                    let r =
                        egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(w, track_h));
                    painter.rect_filled(r, 3.0, color);
                    painter.with_clip_rect(r.shrink(2.0)).text(
                        egui::pos2(x0 + 4.0, y + 4.0),
                        egui::Align2::LEFT_TOP,
                        &e.id,
                        egui::FontId::proportional(10.0),
                        Color32::WHITE,
                    );
                }
            }

            // Playhead
            let ph_x = origin.x + app.time.global_time as f32 * pps;
            painter.line_segment(
                [
                    egui::pos2(ph_x, origin.y),
                    egui::pos2(ph_x, origin.y + total_h),
                ],
                egui::Stroke::new(1.5, RED),
            );
            painter.circle_filled(egui::pos2(ph_x, origin.y), 4.0, RED);

            // Track click
            for (i, e) in app.world.entities.iter().enumerate() {
                if let Some(tl) = &e.components.timeline {
                    let y = tracks_y + i as f32 * (track_h + gap);
                    let x0 = origin.x + tl.start_time as f32 * pps;
                    let w = tl.duration as f32 * pps;
                    let r =
                        egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(w, track_h));

                    if ui.input(|inp| inp.pointer.any_click()) {
                        if let Some(pos) = ui.input(|inp| inp.pointer.hover_pos()) {
                            if r.contains(pos) {
                                app.selected = Some(i);
                            }
                        }
                    }
                }
            }
        });
}
