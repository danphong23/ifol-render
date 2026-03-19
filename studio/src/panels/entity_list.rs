use crate::app::{ACCENT, BG_HOVER, EditorApp, RED, TEXT_DIM, TEXT_PRIMARY};
use egui::{Color32, Frame, Margin, RichText, Sense, Ui};
use ifol_render_core::commands::{AddEntity, RemoveEntity};
use ifol_render_core::ecs::{Entity, components};

pub fn ui(app: &mut EditorApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("HIERARCHY")
                .color(TEXT_DIM)
                .strong()
                .size(10.0),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button(RichText::new("+ Add").color(ACCENT).size(11.0), |ui| {
                if ui.button("🎨 Color Solid").clicked() {
                    let n = app.world.entities.len();
                    let mut e = Entity {
                        id: format!("color_{}", n),
                        components: Default::default(),
                        resolved: Default::default(),
                    };
                    e.components.color_source = Some(components::ColorSource {
                        color: ifol_render_core::color::Color4::new(0.5, 0.5, 0.5, 1.0),
                        size: None,
                    });
                    e.components.timeline = Some(components::Timeline {
                        start_time: app.time.global_time,
                        duration: 3.0,
                        layer: n as i32,
                        locked: false,
                        muted: false,
                        solo: false,
                    });
                    e.components.transform = Some(components::Transform::default());
                    app.commands
                        .execute(Box::new(AddEntity::new(e)), &mut app.world);
                    app.selected = Some(n);
                    app.renderer = None;
                    app.needs_render = true;
                    app.dirty = true;
                    ui.close_menu();
                }
                if ui.button("🖼 Image Layer").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                        .pick_file()
                    {
                        let n = app.world.entities.len();
                        let mut e = Entity {
                            id: format!("img_{}", n),
                            components: Default::default(),
                            resolved: Default::default(),
                        };
                        e.components.image_source = Some(components::ImageSource {
                            path: path.to_string_lossy().to_string(),
                            pixel_size: None,
                        });
                        e.components.timeline = Some(components::Timeline {
                            start_time: app.time.global_time,
                            duration: 5.0,
                            layer: n as i32,
                            locked: false,
                            muted: false,
                            solo: false,
                        });
                        e.components.transform = Some(components::Transform::default());
                        app.commands
                            .execute(Box::new(AddEntity::new(e)), &mut app.world);
                        app.selected = Some(n);
                        app.renderer = None;
                        app.needs_render = true;
                        app.dirty = true;
                    }
                    ui.close_menu();
                }
                if ui.button("📝 Text Layer").clicked() {
                    let n = app.world.entities.len();
                    let mut e = Entity {
                        id: format!("text_{}", n),
                        components: Default::default(),
                        resolved: Default::default(),
                    };
                    e.components.text_source = Some(components::TextSource {
                        content: "Hello".into(),
                        font: "NotoSans".into(),
                        font_size: 48.0,
                        color: ifol_render_core::color::Color4::white(),
                        bold: false,
                        italic: false,
                        pixel_size: None,
                    });
                    e.components.timeline = Some(components::Timeline {
                        start_time: app.time.global_time,
                        duration: 3.0,
                        layer: n as i32,
                        locked: false,
                        muted: false,
                        solo: false,
                    });
                    e.components.transform = Some(components::Transform::default());
                    app.commands
                        .execute(Box::new(AddEntity::new(e)), &mut app.world);
                    app.selected = Some(n);
                    app.renderer = None;
                    app.needs_render = true;
                    app.dirty = true;
                    ui.close_menu();
                }
            });
        });
    });

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(2.0);

    // ── Hierarchy Tree ──
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Build flat render list with depth for indentation
            let entity_count = app.world.entities.len();
            if entity_count == 0 {
                ui.label(
                    RichText::new("Empty scene — use + Add")
                        .color(TEXT_DIM)
                        .size(11.0)
                        .italics(),
                );
                return;
            }

            // Collect info to avoid borrow issues
            let items: Vec<(usize, String, String, bool, bool, bool, bool)> = app
                .world
                .entities
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let display = e.display_name().to_string();
                    let _parent = e.components.parent.clone().unwrap_or_default();
                    let has_children = !e.components.children.is_empty();
                    let visible = e.components.visible;
                    let locked = e.components.timeline.as_ref().is_some_and(|t| t.locked);
                    let muted = e.components.timeline.as_ref().is_some_and(|t| t.muted);
                    (
                        i,
                        e.id.clone(),
                        display,
                        has_children,
                        visible,
                        locked,
                        muted,
                    )
                })
                .collect();

            // Compute depth for each entity
            let depths: Vec<u8> = items
                .iter()
                .map(|(_, id, _, _, _, _, _)| {
                    let mut depth = 0u8;
                    let mut current = id.clone();
                    loop {
                        let parent = items
                            .iter()
                            .find(|(_, eid, _, _, _, _, _)| *eid == current)
                            .and_then(|(idx, _, _, _, _, _, _)| {
                                app.world.entities[*idx].components.parent.clone()
                            });
                        match parent {
                            Some(pid) if !pid.is_empty() => {
                                depth += 1;
                                current = pid;
                                if depth > 10 {
                                    break;
                                } // safety
                            }
                            _ => break,
                        }
                    }
                    depth
                })
                .collect();

            for (idx, (i, _eid, display, has_children, visible, locked, muted)) in
                items.iter().enumerate()
            {
                let depth = depths[idx];
                let is_sel = app.selected == Some(*i) || app.selected_indices.contains(i);

                // Entity type icon
                let e = &app.world.entities[*i];
                let (icon, color) = match () {
                    _ if e.components.color_source.is_some() => {
                        ("🎨", Color32::from_rgb(147, 51, 234))
                    }
                    _ if e.components.image_source.is_some() => {
                        ("🖼", Color32::from_rgb(234, 88, 12))
                    }
                    _ if e.components.text_source.is_some() => {
                        ("📝", Color32::from_rgb(22, 163, 74))
                    }
                    _ if e.components.video_source.is_some() => {
                        ("▶", Color32::from_rgb(234, 88, 12))
                    }
                    _ => ("◻", TEXT_DIM),
                };

                let bg = if is_sel {
                    ACCENT.linear_multiply(0.25)
                } else {
                    Color32::TRANSPARENT
                };

                let indent = depth as f32 * 16.0;

                let resp = Frame::NONE
                    .fill(bg)
                    .inner_margin(Margin::symmetric(4, 2))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.add_space(indent);

                            // Expand/collapse for entities with children
                            if *has_children {
                                let expanded = app.expanded_entities.contains(&_eid.clone());
                                let arrow = if expanded { "▼" } else { "▶" };
                                if ui
                                    .small_button(RichText::new(arrow).color(TEXT_DIM).size(9.0))
                                    .clicked()
                                {
                                    if expanded {
                                        app.expanded_entities.remove(&_eid.clone());
                                    } else {
                                        app.expanded_entities.insert(_eid.clone());
                                    }
                                }
                            } else {
                                ui.add_space(16.0);
                            }

                            // Type icon
                            ui.label(RichText::new(icon).color(color).size(11.0));
                            ui.add_space(4.0);

                            // Name
                            let tc = if is_sel { Color32::WHITE } else { TEXT_PRIMARY };
                            ui.label(RichText::new(display).color(tc).size(11.0));

                            // Status icons on the right
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Visibility toggle
                                    let vis_icon = if *visible { "👁" } else { "👁‍🗨" };
                                    let vis_color = if *visible { TEXT_DIM } else { RED };
                                    if ui
                                        .small_button(
                                            RichText::new(vis_icon).color(vis_color).size(10.0),
                                        )
                                        .clicked()
                                    {
                                        app.world.entities[*i].components.visible = !visible;
                                        app.needs_render = true;
                                        app.dirty = true;
                                    }

                                    // Lock toggle
                                    if *locked {
                                        ui.label(
                                            RichText::new("🔒")
                                                .color(Color32::from_rgb(255, 190, 60))
                                                .size(10.0),
                                        );
                                    }

                                    // Muted indicator
                                    if *muted {
                                        ui.label(RichText::new("🔇").color(TEXT_DIM).size(10.0));
                                    }
                                },
                            );
                        });
                    })
                    .response
                    .interact(Sense::click());

                // Handle hover
                if resp.hovered() {
                    ui.painter()
                        .rect_filled(resp.rect, 0.0, BG_HOVER.linear_multiply(0.3));
                }

                // Selection handling
                if resp.clicked() {
                    let modifiers = ui.input(|inp| inp.modifiers);
                    if modifiers.ctrl || modifiers.command {
                        if app.selected_indices.contains(i) {
                            app.selected_indices.remove(i);
                            if app.selected == Some(*i) {
                                app.selected = app.selected_indices.iter().next().copied();
                            }
                        } else {
                            app.selected_indices.insert(*i);
                            app.selected = Some(*i);
                        }
                    } else if modifiers.shift {
                        if let Some(anchor) = app.selected {
                            let (lo, hi) = if anchor <= *i {
                                (anchor, *i)
                            } else {
                                (*i, anchor)
                            };
                            for j in lo..=hi {
                                app.selected_indices.insert(j);
                            }
                        }
                        app.selected = Some(*i);
                    } else {
                        app.selected_indices.clear();
                        app.selected_indices.insert(*i);
                        app.selected = Some(*i);
                    }
                }
            }

            // Delete button
            let sel_count = app.selected_indices.len();
            if sel_count > 0 {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                let label = if sel_count == 1 {
                    "🗑 Delete".to_string()
                } else {
                    format!("🗑 Delete {}", sel_count)
                };
                if ui
                    .button(RichText::new(label).color(RED).size(11.0))
                    .clicked()
                {
                    let mut indices: Vec<usize> = app.selected_indices.iter().copied().collect();
                    indices.sort_unstable();
                    indices.reverse();
                    for idx in indices {
                        if idx < app.world.entities.len() {
                            let eid = app.world.entities[idx].id.clone();
                            app.commands
                                .execute(Box::new(RemoveEntity::new(eid)), &mut app.world);
                        }
                    }
                    app.selected = None;
                    app.selected_indices.clear();
                    app.renderer = None;
                    app.needs_render = true;
                    app.dirty = true;
                }
            }
        });
}
