use egui::{Ui, RichText};
use ifol_render_core::commands::{SetProperty, PropertyValue};
use crate::app::{EditorApp, TEXT_DIM};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    ui.label(
        RichText::new("PROPERTIES")
            .color(TEXT_DIM)
            .size(10.0)
            .strong(),
    );
    ui.add_space(2.0);
    ui.separator();

    let i = match app.selected {
        Some(i) if i < app.world.entities.len() => i,
        _ => {
            ui.add_space(20.0);
            ui.label(RichText::new("Select an entity").color(TEXT_DIM).size(11.0));
            return;
        }
    };

    // Collect pending commands here — applied after entity borrow ends.
    let mut pending: Vec<Box<dyn ifol_render_core::commands::Command>> = Vec::new();
    let mut needs_dirty = false;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let e = &mut app.world.entities[i];

            // ID
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("ID").color(TEXT_DIM).size(10.0));
                ui.text_edit_singleline(&mut e.id);
            });

            // Transform
            if let Some(ref mut tf) = e.components.transform {
                ui.add_space(6.0);
                ui.label(RichText::new("TRANSFORM").color(TEXT_DIM).size(10.0).strong());
                let eid = e.id.clone();
                let (old_px, old_py) = (tf.position.x, tf.position.y);
                let (old_sx, old_sy) = (tf.scale.x, tf.scale.y);
                let old_rot = tf.rotation;

                ui.horizontal(|ui| {
                    ui.label(RichText::new("X").color(TEXT_DIM).size(10.0));
                    ui.add(egui::DragValue::new(&mut tf.position.x).speed(0.01));
                    ui.label(RichText::new("Y").color(TEXT_DIM).size(10.0));
                    ui.add(egui::DragValue::new(&mut tf.position.y).speed(0.01));
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("W").color(TEXT_DIM).size(10.0));
                    ui.add(egui::DragValue::new(&mut tf.scale.x).speed(0.01).range(0.0..=4.0));
                    ui.label(RichText::new("H").color(TEXT_DIM).size(10.0));
                    ui.add(egui::DragValue::new(&mut tf.scale.y).speed(0.01).range(0.0..=4.0));
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Rot").color(TEXT_DIM).size(10.0));
                    ui.add(egui::DragValue::new(&mut tf.rotation).speed(0.5).suffix("°"));
                });

                // Queue commands for any changed values
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

            // Opacity
            if let Some(ref mut op) = e.components.opacity {
                ui.add_space(6.0);
                let eid = e.id.clone();
                let old_op = *op;
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Opacity").color(TEXT_DIM).size(10.0));
                    ui.add(egui::Slider::new(op, 0.0..=1.0).show_value(true));
                });
                if *op != old_op {
                    pending.push(Box::new(SetProperty::new(eid, "opacity".into(), PropertyValue::Opacity(old_op), PropertyValue::Opacity(*op))));
                    needs_dirty = true;
                }
            }

            // Color
            if let Some(ref mut cs) = e.components.color_source {
                ui.add_space(6.0);
                ui.label(RichText::new("COLOR").color(TEXT_DIM).size(10.0).strong());
                let eid = e.id.clone();
                let old_color = cs.color.clone();
                let mut rgb = [cs.color.r, cs.color.g, cs.color.b];
                if ui.color_edit_button_rgb(&mut rgb).changed() {
                    cs.color = ifol_render_core::color::Color4::new(rgb[0], rgb[1], rgb[2], cs.color.a);
                    pending.push(Box::new(SetProperty::new(eid, "color".into(), PropertyValue::Color(old_color), PropertyValue::Color(cs.color.clone()))));
                    needs_dirty = true;
                }
            }

            // Image
            if let Some(ref img) = e.components.image_source {
                ui.add_space(6.0);
                ui.label(RichText::new("IMAGE").color(TEXT_DIM).size(10.0).strong());
                ui.label(RichText::new(&img.path).color(TEXT_DIM).size(9.0));
            }

            // Timeline
            if let Some(ref mut tl) = e.components.timeline {
                ui.add_space(6.0);
                ui.label(RichText::new("TIMELINE").color(TEXT_DIM).size(10.0).strong());
                let eid = e.id.clone();
                let (old_start, old_dur, old_layer) = (tl.start_time, tl.duration, tl.layer);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Start").color(TEXT_DIM).size(10.0));
                    ui.add(egui::DragValue::new(&mut tl.start_time).speed(0.1).suffix("s"));
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Dur").color(TEXT_DIM).size(10.0));
                    ui.add(egui::DragValue::new(&mut tl.duration).speed(0.1).suffix("s"));
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Layer").color(TEXT_DIM).size(10.0));
                    ui.add(egui::DragValue::new(&mut tl.layer));
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

    // Entity borrow is now released — push commands to history.
    for cmd in pending {
        app.commands.push_executed(cmd);
    }
    if needs_dirty {
        app.dirty = true;
    }
}
