//! Core editor application — a third-party consumer of ifol-render-core.
//!
//! Architecture:
//!   ifol-render       (Render tool: GPU shaders, composite, pixel output — passive)
//!   ifol-render-core  (ECS: datatypes, components, systems, pipeline orchestration)
//!   ifol-render-studio (THIS: GUI editor that USES core as library)
//!
//! The studio does NOT know about the render crate. It calls core's pipeline API.

use egui::{Color32, ColorImage, Key, Modifiers, RichText, TextureHandle, TextureOptions, Vec2};
use ifol_render_core::commands::{CommandHistory, AddEntity, RemoveEntity, SetProperty, PropertyValue};
use ifol_render_core::ecs::{components, Entity, World};
use ifol_render_core::scene::RenderSettings;
use ifol_render_core::time::TimeState;

// ── Theme (matching VideoComposer: #18191C bg, #242529 panels, #303031 borders) ──
pub const BG_APP: Color32 = Color32::from_rgb(24, 25, 28); // #18191C
pub const BG_PANEL: Color32 = Color32::from_rgb(36, 37, 41); // #242529
pub const BG_SURFACE: Color32 = Color32::from_rgb(42, 44, 50); // #2a2c32
pub const BG_HOVER: Color32 = Color32::from_rgb(55, 58, 66); // #373a42
pub const BORDER: Color32 = Color32::from_rgb(48, 48, 49); // #303031
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(224, 224, 224); // #E0E0E0
pub const TEXT_DIM: Color32 = Color32::from_rgb(130, 135, 150); // #828796
pub const ACCENT: Color32 = Color32::from_rgb(88, 101, 242); // #5865f2
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(87, 242, 135); // #57f287
pub const RED: Color32 = Color32::from_rgb(237, 66, 69); // #ed4245
pub const TRACK_BG: Color32 = Color32::from_rgb(54, 57, 75); // #36394b
pub const TRACK_SEL: Color32 = Color32::from_rgb(88, 101, 242); // #5865f2

/// Editor application state.
pub struct EditorApp {
    pub world: World,
    pub settings: RenderSettings,
    pub time: TimeState,
    pub playing: bool,
    pub selected: Option<usize>,
    pub viewport_tex: Option<TextureHandle>,
    pub pixels: Vec<u8>,
    pub dirty: bool,
    pub status: String,
    pub zoom: f32,
    /// Persistent renderer — obtained through core's re-export.
    pub renderer: Option<ifol_render_core::Renderer>,
    /// Command history for undo/redo.
    pub commands: CommandHistory,
    /// Advanced dockable workspace layout
    pub workspace: crate::panels::WorkspaceLayout,
}

impl EditorApp {
    pub fn new() -> Self {
        let mut world = World::new();

        // Default scene
        Self::add_color_entity(
            &mut world,
            "background",
            [0.12, 0.13, 0.16, 1.0],
            [0.0, 0.0],
            [1.0, 1.0],
            0.0,
            10.0,
            0,
            1.0,
        );
        Self::add_color_entity(
            &mut world,
            "red_box",
            [0.9, 0.2, 0.2, 1.0],
            [-0.3, 0.2],
            [0.3, 0.3],
            0.0,
            10.0,
            1,
            0.8,
        );
        Self::add_color_entity(
            &mut world,
            "green_box",
            [0.2, 0.85, 0.35, 1.0],
            [0.3, -0.2],
            [0.25, 0.25],
            0.5,
            8.0,
            2,
            1.0,
        );
        Self::add_color_entity(
            &mut world,
            "blue_box",
            [0.3, 0.5, 0.95, 1.0],
            [0.0, 0.0],
            [0.2, 0.3],
            0.0,
            10.0,
            3,
            0.6,
        );

        let settings = RenderSettings {
            width: 640,
            height: 360,
            fps: 30.0,
            duration: 10.0,
            color_space: ifol_render_core::color::ColorSpace::LinearSrgb,
            output_color_space: ifol_render_core::color::ColorSpace::Srgb,
        };

        Self {
            world,
            settings,
            time: TimeState::new(30.0),
            playing: false,
            selected: None,
            viewport_tex: None,
            pixels: Vec::new(),
            dirty: true,
            status: "Ready".into(),
            zoom: 1.0,
            renderer: None,
            commands: CommandHistory::new(),
            workspace: crate::panels::WorkspaceLayout::new(),
        }
    }

    fn add_color_entity(
        world: &mut World,
        id: &str,
        rgba: [f32; 4],
        pos: [f32; 2],
        scale: [f32; 2],
        start: f64,
        dur: f64,
        layer: i32,
        opacity: f32,
    ) {
        let mut e = ifol_render_core::ecs::Entity {
            id: id.to_string(),
            components: Default::default(),
            resolved: Default::default(),
        };
        e.components.color_source = Some(components::ColorSource {
            color: ifol_render_core::color::Color4::new(rgba[0], rgba[1], rgba[2], rgba[3]),
        });
        e.components.timeline = Some(components::Timeline {
            start_time: start,
            duration: dur,
            layer,
        });
        e.components.transform = Some(components::Transform {
            position: ifol_render_core::types::Vec2 {
                x: pos[0],
                y: pos[1],
            },
            scale: ifol_render_core::types::Vec2 {
                x: scale[0],
                y: scale[1],
            },
            ..Default::default()
        });
        if (opacity - 1.0).abs() > 0.001 {
            e.components.opacity = Some(opacity);
        }
        world.add_entity(e);
    }

    fn ensure_renderer(&mut self) {
        if self.renderer.is_none() {
            let mut r = ifol_render_core::Renderer::new(self.settings.width, self.settings.height);
            // Load images for entities that have image_source
            for entity in &self.world.entities {
                if let Some(ref img) = entity.components.image_source {
                    if let Err(e) = r.load_image(&entity.id, &img.path) {
                        log::warn!("Failed to load image for '{}': {}", entity.id, e);
                    }
                }
            }
            self.renderer = Some(r);
        }
    }

    fn render_scene(&mut self) {
        self.ensure_renderer();
        if let Some(ref mut r) = self.renderer {
            self.pixels = ifol_render_core::ecs::pipeline::render_frame(
                &mut self.world,
                &self.time,
                &self.settings,
                r,
            );
        }
        self.dirty = false;
    }

    fn invalidate_renderer(&mut self) {
        self.renderer = None;
        self.dirty = true;
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let v = &mut style.visuals;

    v.panel_fill = BG_PANEL;
    v.window_fill = BG_SURFACE;
    v.extreme_bg_color = BG_APP;
    v.faint_bg_color = BG_SURFACE;

    v.widgets.noninteractive.bg_fill = BG_SURFACE;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_DIM);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(0.5, BORDER);

    v.widgets.inactive.bg_fill = BG_SURFACE;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(0.5, BORDER);

    v.widgets.hovered.bg_fill = BG_HOVER;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT);

    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::WHITE);

    v.selection.bg_fill = ACCENT.linear_multiply(0.4);
    v.selection.stroke = egui::Stroke::new(1.0, ACCENT);

    v.window_shadow = egui::epaint::Shadow::NONE;

    style.spacing.item_spacing = egui::vec2(6.0, 3.0);
    style.spacing.button_padding = egui::vec2(8.0, 3.0);
    ctx.set_style(style);
}

impl eframe::App for EditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_theme(ctx);

        if self.playing {
            let dt = ctx.input(|i| i.unstable_dt) as f64;
            self.time.seek(self.time.global_time + dt);
            if self.time.global_time >= self.settings.duration {
                self.time.seek(0.0);
            }
            self.dirty = true;
            ctx.request_repaint();
        }

        // ── Keyboard shortcuts ──
        ctx.input_mut(|input| {
            // Space = play/pause
            if input.consume_key(Modifiers::NONE, Key::Space) {
                self.playing = !self.playing;
            }
            // Ctrl+Z = undo
            if input.consume_key(Modifiers::CTRL, Key::Z) {
                if let Some(desc) = self.commands.undo(&mut self.world) {
                    self.status = format!("↩ Undo: {}", desc);
                    self.dirty = true;
                }
            }
            // Ctrl+Y = redo
            if input.consume_key(Modifiers::CTRL, Key::Y) {
                if let Some(desc) = self.commands.redo(&mut self.world) {
                    self.status = format!("↪ Redo: {}", desc);
                    self.dirty = true;
                }
            }
            // Delete = remove selected entity
            if input.consume_key(Modifiers::NONE, Key::Delete) {
                if let Some(i) = self.selected {
                    if i < self.world.entities.len() {
                        let eid = self.world.entities[i].id.clone();
                        self.commands.execute(
                            Box::new(RemoveEntity::new(eid)),
                            &mut self.world,
                        );
                        self.selected = None;
                        self.invalidate_renderer();
                        self.status = "Deleted entity".into();
                    }
                }
            }
        });

        if self.dirty {
            self.render_scene();
        }

        if !self.pixels.is_empty() {
            let img = ColorImage::from_rgba_unmultiplied(
                [self.settings.width as usize, self.settings.height as usize],
                &self.pixels,
            );
            let tex = ctx.load_texture("viewport", img, TextureOptions::LINEAR);
            self.viewport_tex = Some(tex);
        }

        // ── Top bar ──
        egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::new()
                    .fill(BG_APP)
                    .inner_margin(egui::Margin::symmetric(10, 5)),
            )
            .exact_height(34.0)
            .show(ctx, |ui| {
                crate::panels::top_bar::ui(self, ui);
            });

        // ── Status bar ──
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(BG_APP)
                    .inner_margin(egui::Margin::symmetric(10, 3)),
            )
            .exact_height(22.0)
            .show(ctx, |ui| {
                crate::panels::status_bar::ui(self, ui);
            });

        // ── Workspace Split System ──
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            // Take the tree out of self so we can pass `self` mutably to behavior
            let mut tree = std::mem::replace(&mut self.workspace.tree, egui_tiles::Tree::empty("ifol_workspace"));
            
            let mut behavior = crate::panels::workspace::WorkspaceBehavior { app: self };
            tree.ui(&mut behavior, ui);
            
            // Put the tree back
            self.workspace.tree = tree;
        });
    }
}
