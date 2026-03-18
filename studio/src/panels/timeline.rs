use crate::app::{BORDER, EditorApp, RED, TEXT_DIM, TEXT_PRIMARY};
use egui::{Align, Color32, Layout, RichText, Stroke, Ui, Vec2};

const TRACK_HEADER_W: f32 = 120.0;

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    // Transport Controls — compact horizontal bar
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("TIMELINE")
                .color(TEXT_DIM)
                .strong()
                .size(10.0),
        );
        ui.add_space(8.0);

        if ui
            .small_button(RichText::new("⏮").size(11.0).color(TEXT_PRIMARY))
            .clicked()
        {
            app.time.seek(0.0);
            app.needs_render = true;
        }

        let play_color = if app.playing {
            RED
        } else {
            Color32::from_rgb(100, 220, 120)
        };
        let play_icon = if app.playing { "⏸" } else { "▶" };
        if ui
            .small_button(RichText::new(play_icon).size(11.0).color(play_color))
            .clicked()
        {
            app.playing = !app.playing;
        }

        if ui
            .small_button(RichText::new("⏭").size(11.0).color(TEXT_PRIMARY))
            .clicked()
        {
            app.time.seek(app.settings.duration);
            app.needs_render = true;
        }

        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "{:02}:{:04.1}",
                (app.time.global_time / 60.0) as u32,
                app.time.global_time % 60.0
            ))
            .color(Color32::WHITE)
            .size(12.0)
            .monospace()
            .strong(),
        );

        ui.add_space(8.0);

        // Scrub slider
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
            app.needs_render = true;
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add(
                egui::Slider::new(&mut app.zoom, 0.3..=4.0)
                    .show_value(false)
                    .logarithmic(true),
            );
            ui.label(RichText::new("Zoom").color(TEXT_DIM).size(9.0));
        });
    });

    ui.add_space(1.0);

    // NLE Track Area with headers
    let avail_w = ui.available_width();
    let dur = app.settings.duration;
    let track_area_w = avail_w - TRACK_HEADER_W;
    let pps = (track_area_w / dur as f32) * app.zoom;
    let track_h = 24.0f32;
    let gap = 1.0f32;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let ruler_h = 20.0;
            let total_tracks = app.world.entities.len();
            let total_h = ruler_h + total_tracks as f32 * (track_h + gap) + 8.0;
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(avail_w, total_h), egui::Sense::click_and_drag());

            let painter = ui.painter_at(rect);
            let origin = rect.min;
            let tracks_origin_x = origin.x + TRACK_HEADER_W;

            // ── Ruler background ──
            painter.rect_filled(
                egui::Rect::from_min_size(origin, egui::vec2(avail_w, ruler_h)),
                0.0,
                Color32::from_rgb(30, 31, 35),
            );

            // ── Click/Drag on ruler to seek ──
            let ruler_rect = egui::Rect::from_min_size(
                egui::pos2(tracks_origin_x, origin.y),
                egui::vec2(track_area_w, ruler_h),
            );
            if (response.dragged() || response.clicked())
                && let Some(pos) = response.interact_pointer_pos()
                && (ruler_rect.contains(pos) || response.dragged())
            {
                let clicked_time = ((pos.x - tracks_origin_x) / pps) as f64;
                let clamped = clicked_time.clamp(0.0, dur);
                app.time.seek(clamped);
                app.needs_render = true;
            }

            // ── Ruler ticks ──
            let step = if app.zoom > 2.0 {
                0.5
            } else if app.zoom > 1.0 {
                1.0
            } else {
                2.0
            };
            let mut tm = 0.0f64;
            while tm <= dur {
                let x = tracks_origin_x + tm as f32 * pps;
                painter.line_segment(
                    [
                        egui::pos2(x, origin.y + ruler_h - 6.0),
                        egui::pos2(x, origin.y + ruler_h),
                    ],
                    Stroke::new(1.0, TEXT_DIM),
                );
                painter.text(
                    egui::pos2(x + 2.0, origin.y + 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("{:.1}s", tm),
                    egui::FontId::monospace(9.0),
                    TEXT_DIM,
                );
                tm += step;
            }

            // ── Header/Track separator ──
            painter.line_segment(
                [
                    egui::pos2(tracks_origin_x - 1.0, origin.y),
                    egui::pos2(tracks_origin_x - 1.0, origin.y + total_h),
                ],
                Stroke::new(1.0, BORDER),
            );

            // ── Ruler bottom line ──
            painter.line_segment(
                [
                    egui::pos2(origin.x, origin.y + ruler_h),
                    egui::pos2(origin.x + avail_w, origin.y + ruler_h),
                ],
                Stroke::new(0.5, BORDER),
            );

            // ── Track headers + items ──
            let tracks_y = origin.y + ruler_h + 1.0;

            // Collect entity info to draw headers
            struct TrackInfo {
                name: String,
                visible: bool,
                locked: bool,
                muted: bool,
                is_sel: bool,
                color: Color32,
            }

            let infos: Vec<TrackInfo> = app
                .world
                .entities
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let color = match () {
                        _ if e.components.color_source.is_some() => Color32::from_rgb(147, 51, 234),
                        _ if e.components.image_source.is_some() => Color32::from_rgb(234, 88, 12),
                        _ if e.components.text_source.is_some() => Color32::from_rgb(22, 163, 74),
                        _ => Color32::from_rgb(88, 101, 242),
                    };
                    TrackInfo {
                        name: e.display_name().to_string(),
                        visible: e.components.visible,
                        locked: e.components.timeline.as_ref().is_some_and(|t| t.locked),
                        muted: e.components.timeline.as_ref().is_some_and(|t| t.muted),
                        is_sel: app.selected == Some(i) || app.selected_indices.contains(&i),
                        color,
                    }
                })
                .collect();

            for (i, info) in infos.iter().enumerate() {
                let y = tracks_y + i as f32 * (track_h + gap);

                // ── Track header ──
                let header_rect = egui::Rect::from_min_size(
                    egui::pos2(origin.x, y),
                    egui::vec2(TRACK_HEADER_W - 2.0, track_h),
                );

                // Header background
                let hdr_bg = if info.is_sel {
                    info.color.linear_multiply(0.2)
                } else {
                    Color32::from_rgb(28, 29, 33)
                };
                painter.rect_filled(header_rect, 2.0, hdr_bg);

                // Color indicator bar
                painter.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(origin.x, y), egui::vec2(3.0, track_h)),
                    0.0,
                    info.color,
                );

                // Track name
                let name_color = if info.muted { TEXT_DIM } else { TEXT_PRIMARY };
                painter.text(
                    egui::pos2(origin.x + 8.0, y + 5.0),
                    egui::Align2::LEFT_TOP,
                    &info.name,
                    egui::FontId::proportional(10.0),
                    name_color,
                );

                // Status icons in header (right side)
                let icons_x = origin.x + TRACK_HEADER_W - 38.0;

                // Eye icon
                let eye_color = if info.visible { TEXT_DIM } else { RED };
                painter.text(
                    egui::pos2(icons_x, y + 5.0),
                    egui::Align2::LEFT_TOP,
                    if info.visible { "👁" } else { "—" },
                    egui::FontId::proportional(10.0),
                    eye_color,
                );

                // Lock icon
                if info.locked {
                    painter.text(
                        egui::pos2(icons_x + 14.0, y + 5.0),
                        egui::Align2::LEFT_TOP,
                        "🔒",
                        egui::FontId::proportional(10.0),
                        Color32::from_rgb(255, 190, 60),
                    );
                }

                // ── Track lane (clip area) ──
                let lane_rect = egui::Rect::from_min_size(
                    egui::pos2(tracks_origin_x, y),
                    egui::vec2(track_area_w, track_h),
                );
                painter.rect_filled(lane_rect, 0.0, Color32::from_black_alpha(12));

                // Draw clip
                let e = &app.world.entities[i];
                if let Some(tl) = &e.components.timeline {
                    let x0 = tracks_origin_x + tl.start_time as f32 * pps;
                    let w = tl.duration as f32 * pps;

                    let clip_color = if info.muted {
                        info.color.linear_multiply(0.3)
                    } else {
                        info.color
                    };

                    let clip_rect = egui::Rect::from_min_size(
                        egui::pos2(x0, y + 1.0),
                        egui::vec2(w, track_h - 2.0),
                    );
                    painter.rect_filled(clip_rect, 3.0, clip_color);

                    if info.is_sel {
                        painter.rect_stroke(
                            clip_rect,
                            3.0,
                            Stroke::new(1.5, Color32::WHITE),
                            egui::StrokeKind::Inside,
                        );
                    }

                    // Clip label
                    let label_rect = clip_rect.shrink2(egui::vec2(6.0, 3.0));
                    painter.with_clip_rect(label_rect).text(
                        label_rect.min,
                        egui::Align2::LEFT_TOP,
                        &info.name,
                        egui::FontId::proportional(9.0),
                        Color32::WHITE,
                    );
                }
            }

            // ── Playhead ──
            let ph_x = tracks_origin_x + app.time.global_time as f32 * pps;
            painter.line_segment(
                [
                    egui::pos2(ph_x, origin.y),
                    egui::pos2(ph_x, origin.y + total_h),
                ],
                Stroke::new(1.5, RED),
            );
            // Playhead handle
            let handle_pts = vec![
                egui::pos2(ph_x - 5.0, origin.y),
                egui::pos2(ph_x + 5.0, origin.y),
                egui::pos2(ph_x + 5.0, origin.y + 8.0),
                egui::pos2(ph_x, origin.y + 14.0),
                egui::pos2(ph_x - 5.0, origin.y + 8.0),
            ];
            painter.add(egui::Shape::convex_polygon(handle_pts, RED, Stroke::NONE));

            // ── Click-to-select tracks ──
            if response.clicked()
                && let Some(pos) = response.interact_pointer_pos()
                && pos.y > origin.y + ruler_h
            {
                for (i, e) in app.world.entities.iter().enumerate() {
                    if let Some(tl) = &e.components.timeline {
                        let y = tracks_y + i as f32 * (track_h + gap);
                        let x0 = tracks_origin_x + tl.start_time as f32 * pps;
                        let w = tl.duration as f32 * pps;
                        let r =
                            egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(w, track_h));
                        if r.contains(pos) {
                            app.selected = Some(i);
                            app.selected_indices.clear();
                            app.selected_indices.insert(i);
                        }
                    }
                }
            }
        });
}
