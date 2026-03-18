use crate::app::{ACCENT, BG_SURFACE, EditorApp, TEXT_DIM, TEXT_PRIMARY};
use egui::{Color32, Frame, Grid, Margin, RichText, Ui, vec2};
use ifol_render_core::commands::{PropertyValue, SetProperty};
use ifol_render_core::ecs::components::BlendMode;

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    let i = match app.selected {
        Some(i) if i < app.world.entities.len() => i,
        _ => {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Select an entity").color(TEXT_DIM));
            });
            return;
        }
    };

    let mut pending: Vec<Box<dyn ifol_render_core::commands::Command>> = Vec::new();
    let mut needs_dirty = false;

    let e = &mut app.world.entities[i];

    // Determine type
    let (header_color, type_name) = if e.components.image_source.is_some() {
        (Color32::from_rgb(234, 88, 12), "Image")
    } else if e.components.text_source.is_some() {
        (Color32::from_rgb(22, 163, 74), "Text")
    } else if e.components.video_source.is_some() {
        (Color32::from_rgb(234, 88, 12), "Video")
    } else {
        (Color32::from_rgb(147, 51, 234), "Color Solid")
    };

    // ── Header ──
    Frame::NONE
        .inner_margin(Margin::symmetric(10, 6))
        .fill(BG_SURFACE)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, header_color);

                ui.vertical(|ui| {
                    ui.label(RichText::new(type_name).color(TEXT_DIM).size(9.0));

                    // Display name / ID editor
                    let name = e.components.name.get_or_insert_with(|| e.id.clone());
                    ui.add(
                        egui::TextEdit::singleline(name)
                            .frame(false)
                            .font(egui::TextStyle::Body)
                            .desired_width(ui.available_width()),
                    );
                });
            });
        });

    ui.add_space(4.0);

    // Helper macro-like closure for collapsible sections
    let collapsed = &mut app.collapsed_sections;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let eid = app.world.entities[i].id.clone();

            // ── TRANSFORM ──
            {
                let section = "transform";
                let open = !collapsed.contains(section);
                if collapsible_header(ui, "TRANSFORM", header_color, open) {
                    if open {
                        collapsed.insert(section.into());
                    } else {
                        collapsed.remove(section);
                    }
                }
                if open {
                    if let Some(ref mut tf) = app.world.entities[i].components.transform {
                        let (old_px, old_py) = (tf.position.x, tf.position.y);
                        let (old_sx, old_sy) = (tf.scale.x, tf.scale.y);
                        let old_rot = tf.rotation;
                        let old_z = tf.z_index;

                        Grid::new("tf_grid")
                            .num_columns(2)
                            .spacing([16.0, 4.0])
                            .show(ui, |ui| {
                                prop_row(ui, "Pos X", &mut tf.position.x, 0.01, "");
                                prop_row(ui, "Pos Y", &mut tf.position.y, 0.01, "");
                                prop_row(ui, "Scale X", &mut tf.scale.x, 0.01, "");
                                prop_row(ui, "Scale Y", &mut tf.scale.y, 0.01, "");
                                prop_row(ui, "Rotation", &mut tf.rotation, 0.5, "°");
                                prop_row(ui, "Z-Index", &mut tf.z_index, 0.1, "");
                            });

                        if tf.position.x != old_px {
                            push_prop(
                                &mut pending,
                                &eid,
                                "position.x",
                                PropertyValue::PositionX(old_px),
                                PropertyValue::PositionX(tf.position.x),
                            );
                            needs_dirty = true;
                        }
                        if tf.position.y != old_py {
                            push_prop(
                                &mut pending,
                                &eid,
                                "position.y",
                                PropertyValue::PositionY(old_py),
                                PropertyValue::PositionY(tf.position.y),
                            );
                            needs_dirty = true;
                        }
                        if tf.scale.x != old_sx {
                            push_prop(
                                &mut pending,
                                &eid,
                                "scale.x",
                                PropertyValue::ScaleX(old_sx),
                                PropertyValue::ScaleX(tf.scale.x),
                            );
                            needs_dirty = true;
                        }
                        if tf.scale.y != old_sy {
                            push_prop(
                                &mut pending,
                                &eid,
                                "scale.y",
                                PropertyValue::ScaleY(old_sy),
                                PropertyValue::ScaleY(tf.scale.y),
                            );
                            needs_dirty = true;
                        }
                        if tf.rotation != old_rot {
                            push_prop(
                                &mut pending,
                                &eid,
                                "rotation",
                                PropertyValue::Rotation(old_rot),
                                PropertyValue::Rotation(tf.rotation),
                            );
                            needs_dirty = true;
                        }
                        if tf.z_index != old_z {
                            needs_dirty = true;
                        }
                    }
                    ui.add_space(4.0);
                }
            }

            // ── APPEARANCE ──
            {
                let section = "appearance";
                let open = !collapsed.contains(section);
                if collapsible_header(ui, "APPEARANCE", header_color, open) {
                    if open {
                        collapsed.insert(section.into());
                    } else {
                        collapsed.remove(section);
                    }
                }
                if open {
                    Grid::new("appear_grid")
                        .num_columns(2)
                        .spacing([16.0, 4.0])
                        .show(ui, |ui| {
                            // Opacity
                            let e = &mut app.world.entities[i];
                            let op = e.components.opacity.get_or_insert(1.0);
                            let old_op = *op;
                            ui.label(RichText::new("Opacity").color(TEXT_DIM).size(11.0));
                            ui.add(egui::Slider::new(op, 0.0..=1.0).show_value(true));
                            ui.end_row();

                            if *op != old_op {
                                push_prop(
                                    &mut pending,
                                    &eid,
                                    "opacity",
                                    PropertyValue::Opacity(old_op),
                                    PropertyValue::Opacity(*op),
                                );
                                needs_dirty = true;
                            }

                            // Blend Mode
                            let blend = e.components.blend_mode.get_or_insert(BlendMode::Normal);
                            ui.label(RichText::new("Blend").color(TEXT_DIM).size(11.0));
                            egui::ComboBox::from_id_salt("blend_mode")
                                .selected_text(blend.label())
                                .width(100.0)
                                .show_ui(ui, |ui| {
                                    for mode in BlendMode::ALL {
                                        ui.selectable_value(blend, *mode, mode.label());
                                    }
                                });
                            ui.end_row();

                            // Visible
                            ui.label(RichText::new("Visible").color(TEXT_DIM).size(11.0));
                            ui.checkbox(&mut e.components.visible, "");
                            ui.end_row();
                        });

                    // Color source
                    if let Some(ref mut cs) = app.world.entities[i].components.color_source {
                        let old_color = cs.color;
                        let mut rgb = [cs.color.r, cs.color.g, cs.color.b];
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Color").color(TEXT_DIM).size(11.0));
                            ui.add_space(20.0);
                            if ui.color_edit_button_rgb(&mut rgb).changed() {
                                cs.color = ifol_render_core::color::Color4::new(
                                    rgb[0], rgb[1], rgb[2], cs.color.a,
                                );
                                push_prop(
                                    &mut pending,
                                    &eid,
                                    "color",
                                    PropertyValue::Color(old_color),
                                    PropertyValue::Color(cs.color),
                                );
                                needs_dirty = true;
                            }
                        });
                    }

                    ui.add_space(4.0);
                }
            }

            // ── TIMELINE ──
            {
                let section = "timeline";
                let open = !collapsed.contains(section);
                if collapsible_header(ui, "TIMELINE", header_color, open) {
                    if open {
                        collapsed.insert(section.into());
                    } else {
                        collapsed.remove(section);
                    }
                }
                if open {
                    if let Some(ref mut tl) = app.world.entities[i].components.timeline {
                        let (old_start, old_dur, old_layer) =
                            (tl.start_time, tl.duration, tl.layer);

                        Grid::new("tl_grid")
                            .num_columns(2)
                            .spacing([16.0, 4.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Start").color(TEXT_DIM).size(11.0));
                                ui.add(
                                    egui::DragValue::new(&mut tl.start_time)
                                        .speed(0.1)
                                        .suffix("s"),
                                );
                                ui.end_row();

                                ui.label(RichText::new("Duration").color(TEXT_DIM).size(11.0));
                                ui.add(
                                    egui::DragValue::new(&mut tl.duration)
                                        .speed(0.1)
                                        .suffix("s"),
                                );
                                ui.end_row();

                                ui.label(RichText::new("Layer").color(TEXT_DIM).size(11.0));
                                ui.add(egui::DragValue::new(&mut tl.layer));
                                ui.end_row();

                                ui.label(RichText::new("Locked").color(TEXT_DIM).size(11.0));
                                ui.checkbox(&mut tl.locked, "");
                                ui.end_row();

                                ui.label(RichText::new("Muted").color(TEXT_DIM).size(11.0));
                                ui.checkbox(&mut tl.muted, "");
                                ui.end_row();
                            });

                        if tl.start_time != old_start {
                            push_prop(
                                &mut pending,
                                &eid,
                                "start_time",
                                PropertyValue::StartTime(old_start),
                                PropertyValue::StartTime(tl.start_time),
                            );
                            needs_dirty = true;
                        }
                        if tl.duration != old_dur {
                            push_prop(
                                &mut pending,
                                &eid,
                                "duration",
                                PropertyValue::Duration(old_dur),
                                PropertyValue::Duration(tl.duration),
                            );
                            needs_dirty = true;
                        }
                        if tl.layer != old_layer {
                            push_prop(
                                &mut pending,
                                &eid,
                                "layer",
                                PropertyValue::Layer(old_layer),
                                PropertyValue::Layer(tl.layer),
                            );
                            needs_dirty = true;
                        }
                    }
                    ui.add_space(4.0);
                }
            }

            // ── SOURCE INFO ──
            {
                let section = "source";
                let open = !collapsed.contains(section);
                let e = &app.world.entities[i];
                let has_source = e.components.image_source.is_some()
                    || e.components.video_source.is_some()
                    || e.components.text_source.is_some();

                if has_source {
                    if collapsible_header(ui, "SOURCE", header_color, open) {
                        if open {
                            collapsed.insert(section.into());
                        } else {
                            collapsed.remove(section);
                        }
                    }
                    if open {
                        if let Some(ref img) = e.components.image_source {
                            ui.label(RichText::new(&img.path).color(TEXT_PRIMARY).size(10.0));
                        }
                        if let Some(ref vid) = e.components.video_source {
                            ui.label(RichText::new(&vid.path).color(TEXT_PRIMARY).size(10.0));
                        }
                        if let Some(ref txt) = e.components.text_source {
                            ui.label(
                                RichText::new(format!("\"{}\"", txt.content))
                                    .color(TEXT_PRIMARY)
                                    .size(10.0),
                            );
                        }
                        ui.add_space(4.0);
                    }
                }
            }
        });

    for cmd in pending {
        app.commands.push_executed(cmd);
    }
    if needs_dirty {
        app.needs_render = true;
        app.dirty = true;
    }
}

/// Draw a collapsible section header. Returns true if clicked.
fn collapsible_header(ui: &mut Ui, label: &str, color: Color32, open: bool) -> bool {
    let arrow = if open { "▼" } else { "▶" };
    let resp = ui
        .horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(arrow).color(TEXT_DIM).size(9.0));
            ui.label(RichText::new(label).color(color).strong().size(10.0));
        })
        .response
        .interact(egui::Sense::click());

    if resp.hovered() {
        ui.painter()
            .rect_filled(resp.rect, 0.0, ACCENT.linear_multiply(0.08));
    }

    resp.clicked()
}

/// Draw a labeled drag-value row.
fn prop_row(ui: &mut Ui, label: &str, val: &mut f32, speed: f64, suffix: &str) {
    ui.label(RichText::new(label).color(TEXT_DIM).size(11.0));
    ui.add(egui::DragValue::new(val).speed(speed).suffix(suffix));
    ui.end_row();
}

/// Push a SetProperty command.
fn push_prop(
    pending: &mut Vec<Box<dyn ifol_render_core::commands::Command>>,
    eid: &str,
    field: &str,
    old: PropertyValue,
    new: PropertyValue,
) {
    pending.push(Box::new(SetProperty::new(
        eid.into(),
        field.into(),
        old,
        new,
    )));
}
