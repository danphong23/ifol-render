use egui::{Ui, RichText, Color32, Vec2, vec2, Frame, Margin, Stroke, Align, Layout};
use crate::app::{EditorApp, TEXT_DIM, TEXT_PRIMARY, RED, BORDER, BG_SURFACE};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    // Top Controls Header
    Frame::NONE
        .inner_margin(Margin::symmetric(12, 8))
        .fill(BG_SURFACE)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("TIMELINE").color(TEXT_DIM).strong().size(11.0));
                ui.add_space(16.0);

                if ui.small_button(RichText::new("⏮").size(12.0).color(TEXT_PRIMARY)).clicked() {
                    app.time.seek(0.0);
                    app.dirty = true;
                }

                let play_color = if app.playing { RED } else { Color32::from_rgb(100, 220, 120) };
                let play_icon = if app.playing { "⏸" } else { "▶" };
                
                if ui.small_button(RichText::new(play_icon).size(12.0).color(play_color)).clicked() {
                    app.playing = !app.playing;
                }

                if ui.small_button(RichText::new("⏭").size(12.0).color(TEXT_PRIMARY)).clicked() {
                    app.time.seek(app.settings.duration);
                    app.dirty = true;
                }

                ui.add_space(16.0);

                ui.label(
                    RichText::new(format!(
                        "{:02}:{:04.1}",
                        (app.time.global_time / 60.0) as u32,
                        app.time.global_time % 60.0
                    ))
                    .color(Color32::WHITE)
                    .size(13.0)
                    .monospace()
                    .strong(),
                );

                ui.add_space(16.0);

                // Scrub bar
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

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(8.0);
                    ui.add(
                        egui::Slider::new(&mut app.zoom, 0.3..=4.0)
                            .show_value(false)
                            .logarithmic(true),
                    );
                    ui.label(RichText::new("Zoom").color(TEXT_DIM).size(10.0));
                });
            });
        });

    ui.add_space(2.0);

    let avail_w = ui.available_width();
    let dur = app.settings.duration;
    let pps = (avail_w / dur as f32) * app.zoom;
    let track_h = 24.0f32;
    let gap = 4.0f32;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let ruler_h = 20.0;
            let total_tracks = app.world.entities.len();
            let total_h = ruler_h + total_tracks as f32 * (track_h + gap) + 16.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, total_h), egui::Sense::click());

            let painter = ui.painter_at(rect);
            let origin = rect.min;

            // Ruler Base
            painter.rect_filled(
                egui::Rect::from_min_size(origin, vec2(avail_w, ruler_h)),
                0.0,
                BG_SURFACE
            );
            painter.line_segment(
                [egui::pos2(origin.x, origin.y + ruler_h), egui::pos2(origin.x + avail_w, origin.y + ruler_h)],
                Stroke::new(1.0, BORDER)
            );

            // Ruler Ticks
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
                    [egui::pos2(x, origin.y + ruler_h - 6.0), egui::pos2(x, origin.y + ruler_h)],
                    Stroke::new(1.0, TEXT_DIM),
                );
                painter.text(
                    egui::pos2(x + 3.0, origin.y + 4.0),
                    egui::Align2::LEFT_TOP,
                    format!("{:.1}s", tm),
                    egui::FontId::monospace(10.0),
                    TEXT_DIM,
                );
                tm += step;
            }

            // Tracks Background Grid
            let tracks_y = origin.y + ruler_h + 8.0;
            
            // Draw track lanes
            for i in 0..total_tracks {
                let y = tracks_y + i as f32 * (track_h + gap);
                painter.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(origin.x, y), egui::vec2(avail_w, track_h)),
                    2.0,
                    Color32::from_black_alpha(20)
                );
            }

            // Draw Track Items
            for (i, e) in app.world.entities.iter().enumerate() {
                if let Some(tl) = &e.components.timeline {
                    let y = tracks_y + i as f32 * (track_h + gap);
                    let x0 = origin.x + tl.start_time as f32 * pps;
                    let w = tl.duration as f32 * pps;

                    let base_color = match () {
                        _ if e.components.color_source.is_some() => Color32::from_rgb(123, 31, 162), // Purple
                        _ if e.components.image_source.is_some() => Color32::from_rgb(216, 67, 21),  // Orange
                        _ if e.components.text_source.is_some()  => Color32::from_rgb(46, 125, 50),  // Green
                        _ => Color32::from_rgb(88, 101, 242) // Default Blue
                    };

                    let is_sel = app.selected == Some(i);
                    let stroke = if is_sel { Stroke::new(1.5, Color32::WHITE) } else { Stroke::NONE };
                    let r = egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(w, track_h));
                    
                    painter.rect_filled(r, 4.0, base_color);
                    if is_sel {
                        painter.rect_stroke(r, 4.0, stroke, egui::StrokeKind::Inside);
                    }
                    
                    let mut text_rect = r.shrink(4.0);
                    text_rect.min.x += 4.0;
                    painter.with_clip_rect(text_rect).text(
                        text_rect.min,
                        egui::Align2::LEFT_TOP,
                        &e.id,
                        egui::FontId::proportional(11.0),
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
                Stroke::new(1.5, RED),
            );
            
            // Playhead Handle
            let handle_pts = vec![
                egui::pos2(ph_x - 6.0, origin.y),
                egui::pos2(ph_x + 6.0, origin.y),
                egui::pos2(ph_x + 6.0, origin.y + 10.0),
                egui::pos2(ph_x, origin.y + 16.0),
                egui::pos2(ph_x - 6.0, origin.y + 10.0),
            ];
            painter.add(egui::Shape::convex_polygon(handle_pts, RED, Stroke::NONE));

            // Interaction
            for (i, e) in app.world.entities.iter().enumerate() {
                if let Some(tl) = &e.components.timeline {
                    let y = tracks_y + i as f32 * (track_h + gap);
                    let x0 = origin.x + tl.start_time as f32 * pps;
                    let w = tl.duration as f32 * pps;
                    let r = egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(w, track_h));

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
