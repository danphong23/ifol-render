use egui::{Ui, Vec2, RichText, Color32, Stroke, Rect, pos2};
use crate::app::{EditorApp, TEXT_DIM, ACCENT};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    let avail = ui.available_size();

    if let Some(tex) = &app.viewport_tex {
        let aspect = app.settings.width as f32 / app.settings.height as f32;
        let (w, h) = if avail.x / avail.y > aspect {
            (avail.y * aspect, avail.y)
        } else {
            (avail.x, avail.x / aspect)
        };

        // Center the viewport
        let offset_x = (avail.x - w) / 2.0;
        let offset_y = (avail.y - h) / 2.0;
        let min = ui.min_rect().min;
        let viewport_rect = Rect::from_min_size(
            pos2(min.x + offset_x, min.y + offset_y),
            Vec2::new(w, h),
        );

        // Draw the rendered image
        let img = egui::Image::new(egui::load::SizedTexture::new(tex.id(), Vec2::new(w, h)));
        ui.put(viewport_rect, img);

        // Draw grid overlay if enabled
        if app.show_grid {
            draw_grid_overlay(ui, viewport_rect);
        }

        // Draw safe zones if enabled
        if app.show_safe_zones {
            draw_safe_zones(ui, viewport_rect);
        }

        // Resolution badge
        let badge = format!("{}x{}", app.settings.width, app.settings.height);
        let badge_pos = pos2(viewport_rect.right() - 80.0, viewport_rect.bottom() - 18.0);
        ui.painter().text(
            badge_pos,
            egui::Align2::LEFT_CENTER,
            badge,
            egui::FontId::monospace(9.0),
            Color32::from_rgba_premultiplied(180, 180, 180, 120),
        );

        // Click to select entity (basic hit-test)
        if ui.input(|i| i.pointer.primary_clicked()) {
            if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                if viewport_rect.contains(pos) {
                    // Normalize click to 0..1 canvas coords
                    let nx = (pos.x - viewport_rect.left()) / viewport_rect.width();
                    let ny = (pos.y - viewport_rect.top()) / viewport_rect.height();
                    app.status = format!("Click: ({:.2}, {:.2})", nx, ny);
                }
            }
        }
    } else {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No output").color(TEXT_DIM));
        });
    }

    // Viewport toolbar (bottom overlay)
    let toolbar_rect = Rect::from_min_size(
        pos2(ui.min_rect().left() + 4.0, ui.min_rect().bottom() - 22.0),
        Vec2::new(200.0, 20.0),
    );
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(toolbar_rect), |ui| {
        ui.horizontal(|ui| {
            ui.style_mut().spacing.item_spacing.x = 2.0;
            // Grid toggle
            let grid_label = if app.show_grid { "▦" } else { "▥" };
            let grid_color = if app.show_grid { ACCENT } else { TEXT_DIM };
            if ui.button(RichText::new(grid_label).color(grid_color).size(12.0)).clicked() {
                app.show_grid = !app.show_grid;
            }
            // Safe zone toggle  
            let sz_label = if app.show_safe_zones { "◻" } else { "◻" };
            let sz_color = if app.show_safe_zones { ACCENT } else { TEXT_DIM };
            if ui.button(RichText::new(sz_label).color(sz_color).size(12.0)).clicked() {
                app.show_safe_zones = !app.show_safe_zones;
            }
        });
    });
}

/// Draw a rule-of-thirds grid over the viewport.
fn draw_grid_overlay(ui: &Ui, rect: Rect) {
    let painter = ui.painter();
    let stroke = Stroke::new(0.5, Color32::from_rgba_premultiplied(255, 255, 255, 40));

    // Thirds lines
    for i in 1..3 {
        let frac = i as f32 / 3.0;
        // Vertical
        let x = rect.left() + rect.width() * frac;
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke);
        // Horizontal
        let y = rect.top() + rect.height() * frac;
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
    }

    // Center crosshair
    let cx = rect.center().x;
    let cy = rect.center().y;
    let cross_stroke = Stroke::new(0.5, Color32::from_rgba_premultiplied(255, 255, 255, 25));
    painter.line_segment([pos2(cx, rect.top()), pos2(cx, rect.bottom())], cross_stroke);
    painter.line_segment([pos2(rect.left(), cy), pos2(rect.right(), cy)], cross_stroke);
}

/// Draw broadcast safe zones (title-safe 80% and action-safe 90%).
fn draw_safe_zones(ui: &Ui, rect: Rect) {
    let painter = ui.painter();

    // Action safe: 90% of frame
    let action_inset = 0.05;
    let action_rect = Rect::from_min_max(
        pos2(
            rect.left() + rect.width() * action_inset,
            rect.top() + rect.height() * action_inset,
        ),
        pos2(
            rect.right() - rect.width() * action_inset,
            rect.bottom() - rect.height() * action_inset,
        ),
    );
    painter.rect_stroke(action_rect, 0.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(100, 200, 255, 60)), egui::StrokeKind::Inside);

    // Title safe: 80% of frame
    let title_inset = 0.10;
    let title_rect = Rect::from_min_max(
        pos2(
            rect.left() + rect.width() * title_inset,
            rect.top() + rect.height() * title_inset,
        ),
        pos2(
            rect.right() - rect.width() * title_inset,
            rect.bottom() - rect.height() * title_inset,
        ),
    );
    painter.rect_stroke(title_rect, 0.0, Stroke::new(1.0, Color32::from_rgba_premultiplied(255, 200, 100, 50)), egui::StrokeKind::Inside);
}
