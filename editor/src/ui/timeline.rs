//! Timeline panel UI.

use ifol_render_core::ecs::World;
use ifol_render_core::time::TimeState;

pub fn draw(ui: &mut egui::Ui, world: &World, time: &mut TimeState, playing: &mut bool) {
    ui.horizontal(|ui| {
        // Playback controls
        if ui.button(if *playing { "⏸" } else { "▶" }).clicked() {
            *playing = !*playing;
        }
        if ui.button("⏮").clicked() {
            time.seek(0.0);
            *playing = false;
        }

        ui.separator();
        ui.label(format!(
            "{:.2}s  |  Frame {}  |  {:.0} fps",
            time.global_time, time.frame_index, time.fps
        ));
    });

    ui.separator();

    // Timeline tracks
    let available = ui.available_size();
    let (_response, painter) =
        ui.allocate_painter(egui::Vec2::new(available.x, 60.0), egui::Sense::click());

    let rect = painter.clip_rect();
    let total_duration = 30.0_f64; // TODO: from scene settings

    // Draw track background
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(40, 40, 50));

    // Draw entity clips
    let track_height = 20.0;
    for (i, entity) in world.entities.iter().enumerate() {
        if let Some(ref tl) = entity.components.timeline {
            let x_start = rect.left() + (tl.start_time / total_duration) as f32 * rect.width();
            let x_end = rect.left()
                + ((tl.start_time + tl.duration) / total_duration) as f32 * rect.width();
            let y = rect.top() + 4.0 + i as f32 * (track_height + 2.0);

            let clip_rect = egui::Rect::from_min_max(
                egui::Pos2::new(x_start, y),
                egui::Pos2::new(x_end, y + track_height),
            );

            let color = match tl.layer % 4 {
                0 => egui::Color32::from_rgb(80, 140, 200),
                1 => egui::Color32::from_rgb(200, 120, 80),
                2 => egui::Color32::from_rgb(100, 180, 100),
                _ => egui::Color32::from_rgb(180, 100, 180),
            };

            painter.rect_filled(clip_rect, 3.0, color);
            painter.text(
                clip_rect.left_center() + egui::Vec2::new(4.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &entity.id,
                egui::FontId::proportional(11.0),
                egui::Color32::WHITE,
            );
        }
    }

    // Playhead
    let playhead_x = rect.left() + (time.global_time / total_duration) as f32 * rect.width();
    painter.line_segment(
        [
            egui::Pos2::new(playhead_x, rect.top()),
            egui::Pos2::new(playhead_x, rect.bottom()),
        ],
        egui::Stroke::new(2.0, egui::Color32::RED),
    );
}
