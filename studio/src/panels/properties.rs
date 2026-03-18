use egui::{Ui, RichText, Frame, Color32, Margin, vec2, Grid};
use ifol_render_core::commands::{SetProperty, PropertyValue};
use crate::app::{EditorApp, TEXT_DIM, TEXT_PRIMARY, BG_SURFACE};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    let i = match app.selected {
        Some(i) if i < app.world.entities.len() => i,
        _ => {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Select an entity to view properties").color(TEXT_DIM));
            });
            return;
        }
    };

    let mut pending: Vec<Box<dyn ifol_render_core::commands::Command>> = Vec::new();
    let mut needs_dirty = false;

    let e = &mut app.world.entities[i];
    let is_image = e.components.image_source.is_some();
    let header_color = if is_image { Color32::from_rgb(216, 67, 21) } else { Color32::from_rgb(123, 31, 162) }; // Orange or Purple
    let type_name = if is_image { "Image Source" } else { "Color Source" };

    // Beautiful Header
    Frame::NONE
        .inner_margin(Margin::symmetric(12, 8))
        .fill(BG_SURFACE)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _resp) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, header_color);
                
                ui.vertical(|ui| {
                    ui.label(RichText::new(type_name).color(TEXT_DIM).size(10.0));
                    ui.add(egui::TextEdit::singleline(&mut e.id).frame(false).font(egui::TextStyle::Body));
                });
            });
        });

    ui.add_space(8.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let eid = e.id.clone();
            
            // Transform Section
            if let Some(ref mut tf) = e.components.transform {
                ui.add_space(8.0);
                ui.label(RichText::new("TRANSFORM").color(header_color).strong().size(11.0));
                ui.add_space(4.0);
                
                let (old_px, old_py) = (tf.position.x, tf.position.y);
                let (old_sx, old_sy) = (tf.scale.x, tf.scale.y);
                let old_rot = tf.rotation;

                Grid::new("transform_grid")
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Position X").color(TEXT_DIM));
                        ui.add(egui::DragValue::new(&mut tf.position.x).speed(0.01));
                        ui.end_row();
                        
                        ui.label(RichText::new("Position Y").color(TEXT_DIM));
                        ui.add(egui::DragValue::new(&mut tf.position.y).speed(0.01));
                        ui.end_row();

                        ui.label(RichText::new("Scale X").color(TEXT_DIM));
                        ui.add(egui::DragValue::new(&mut tf.scale.x).speed(0.01));
                        ui.end_row();

                        ui.label(RichText::new("Scale Y").color(TEXT_DIM));
                        ui.add(egui::DragValue::new(&mut tf.scale.y).speed(0.01));
                        ui.end_row();

                        ui.label(RichText::new("Rotation").color(TEXT_DIM));
                        ui.add(egui::DragValue::new(&mut tf.rotation).speed(0.5).suffix("°"));
                        ui.end_row();
                    });

                if tf.position.x != old_px {
                    pending.push(Box::new(SetProperty::new(eid.clone(), "position.x".into(), PropertyValue::PositionX(old_px), PropertyValue::PositionX(tf.position.x))));
                    needs_dirty = true;
                }
                if tf.position.y != old_py {
                    pending.push(Box::new(SetProperty::new(eid.clone(), "position.y".into(), PropertyValue::PositionY(old_py), PropertyValue::PositionY(tf.position.y))));
                    needs_dirty = true;
                }
                if tf.scale.x != old_sx {
                    pending.push(Box::new(SetProperty::new(eid.clone(), "scale.x".into(), PropertyValue::ScaleX(old_sx), PropertyValue::ScaleX(tf.scale.x))));
                    needs_dirty = true;
                }
                if tf.scale.y != old_sy {
                    pending.push(Box::new(SetProperty::new(eid.clone(), "scale.y".into(), PropertyValue::ScaleY(old_sy), PropertyValue::ScaleY(tf.scale.y))));
                    needs_dirty = true;
                }
                if tf.rotation != old_rot {
                    pending.push(Box::new(SetProperty::new(eid.clone(), "rotation".into(), PropertyValue::Rotation(old_rot), PropertyValue::Rotation(tf.rotation))));
                    needs_dirty = true;
                }
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Opacity
            if let Some(ref mut op) = e.components.opacity {
                let old_op = *op;
                Grid::new("opacity_grid").num_columns(2).spacing([20.0, 8.0]).show(ui, |ui| {
                    ui.label(RichText::new("Opacity").color(TEXT_DIM));
                    ui.add(egui::Slider::new(op, 0.0..=1.0).show_value(true));
                    ui.end_row();
                });
                
                if *op != old_op {
                    pending.push(Box::new(SetProperty::new(eid.clone(), "opacity".into(), PropertyValue::Opacity(old_op), PropertyValue::Opacity(*op))));
                    needs_dirty = true;
                }
            }

            // Color
            if let Some(ref mut cs) = e.components.color_source {
                ui.add_space(8.0);
                ui.label(RichText::new("COLOR").color(header_color).strong().size(11.0));
                ui.add_space(4.0);

                let old_color = cs.color.clone();
                let mut rgb = [cs.color.r, cs.color.g, cs.color.b];
                
                Grid::new("color_grid").num_columns(2).spacing([20.0, 8.0]).show(ui, |ui| {
                    ui.label(RichText::new("Base Color").color(TEXT_DIM));
                    if ui.color_edit_button_rgb(&mut rgb).changed() {
                        cs.color = ifol_render_core::color::Color4::new(rgb[0], rgb[1], rgb[2], cs.color.a);
                        pending.push(Box::new(SetProperty::new(eid.clone(), "color".into(), PropertyValue::Color(old_color.clone()), PropertyValue::Color(cs.color.clone()))));
                        needs_dirty = true;
                    }
                    ui.end_row();
                });
            }

            // Image
            if let Some(ref img) = e.components.image_source {
                ui.add_space(8.0);
                ui.label(RichText::new("IMAGE").color(header_color).strong().size(11.0));
                ui.add_space(4.0);
                
                Grid::new("image_grid").num_columns(2).spacing([20.0, 8.0]).show(ui, |ui| {
                    ui.label(RichText::new("Source Path").color(TEXT_DIM));
                    ui.label(RichText::new(&img.path).color(TEXT_PRIMARY).size(10.0));
                    ui.end_row();
                });
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Timeline
            if let Some(ref mut tl) = e.components.timeline {
                ui.label(RichText::new("TIMELINE").color(header_color).strong().size(11.0));
                ui.add_space(4.0);
                
                let (old_start, old_dur, old_layer) = (tl.start_time, tl.duration, tl.layer);

                Grid::new("timeline_grid")
                    .num_columns(2)
                    .spacing([20.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Start Time").color(TEXT_DIM));
                        ui.add(egui::DragValue::new(&mut tl.start_time).speed(0.1).suffix("s"));
                        ui.end_row();
                
                        ui.label(RichText::new("Duration").color(TEXT_DIM));
                        ui.add(egui::DragValue::new(&mut tl.duration).speed(0.1).suffix("s"));
                        ui.end_row();
                
                        ui.label(RichText::new("Z-Layer").color(TEXT_DIM));
                        ui.add(egui::DragValue::new(&mut tl.layer));
                        ui.end_row();
                    });

                if tl.start_time != old_start {
                    pending.push(Box::new(SetProperty::new(eid.clone(), "start_time".into(), PropertyValue::StartTime(old_start), PropertyValue::StartTime(tl.start_time))));
                    needs_dirty = true;
                }
                if tl.duration != old_dur {
                    pending.push(Box::new(SetProperty::new(eid.clone(), "duration".into(), PropertyValue::Duration(old_dur), PropertyValue::Duration(tl.duration))));
                    needs_dirty = true;
                }
                if tl.layer != old_layer {
                    pending.push(Box::new(SetProperty::new(eid.clone(), "layer".into(), PropertyValue::Layer(old_layer), PropertyValue::Layer(tl.layer))));
                    needs_dirty = true;
                }
            }
        });

    for cmd in pending {
        app.commands.push_executed(cmd);
    }
    if needs_dirty {
        app.dirty = true;
    }
}
