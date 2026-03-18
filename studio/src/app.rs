//! Core editor application — a third-party consumer of ifol-render-core.
//!
//! Architecture:
//!   ifol-render       (Render tool: GPU shaders, composite, pixel output — passive)
//!   ifol-render-core  (ECS: datatypes, components, systems, pipeline orchestration)
//!   ifol-render-studio (THIS: GUI editor that USES core as library)
//!
//! The studio does NOT know about the render crate. It calls core's pipeline API.

use egui::{Color32, ColorImage, Key, Modifiers, TextureHandle, TextureOptions};
use ifol_render_core::commands::{CommandHistory, RemoveEntity};
use ifol_render_core::ecs::World;
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
    /// Whether the scene needs re-rendering (viewport update).
    pub needs_render: bool,
    pub status: String,
    pub zoom: f32,
    /// Persistent renderer — obtained through core's re-export.
    pub renderer: Option<ifol_render_core::Renderer>,
    /// Path to FFmpeg binary for export.
    pub ffmpeg_path: Option<String>,
    /// Command history for undo/redo.
    pub commands: CommandHistory,
    /// Advanced dockable workspace layout
    pub workspace: crate::panels::WorkspaceLayout,
    /// Pending workspace action from editor switcher/split menu
    pub pending_workspace_action: Option<crate::panels::workspace::WorkspaceAction>,
    /// Current scene file path (None = unsaved)
    pub scene_path: Option<std::path::PathBuf>,
    /// Show grid overlay in viewport
    pub show_grid: bool,
    /// Show safe zones in viewport
    pub show_safe_zones: bool,
    /// Multi-selection: set of selected entity indices
    pub selected_indices: std::collections::HashSet<usize>,
    /// Viewport camera (pan/zoom)
    pub camera: ifol_render_core::ecs::components::Camera,
    /// Collapsed property sections
    pub collapsed_sections: std::collections::HashSet<String>,
    /// Expanded entities in hierarchy tree
    pub expanded_entities: std::collections::HashSet<String>,
}

impl Default for EditorApp {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorApp {
    pub fn new() -> Self {
        let world = World::new();

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
            dirty: false,
            needs_render: true,
            status: "Ready — New Scene".into(),
            zoom: 1.0,
            renderer: None,
            ffmpeg_path: None,
            commands: CommandHistory::new(),
            workspace: crate::panels::WorkspaceLayout::new(),
            pending_workspace_action: None,
            scene_path: None,
            show_grid: false,
            show_safe_zones: false,
            selected_indices: std::collections::HashSet::new(),
            camera: ifol_render_core::ecs::components::Camera::default(),
            collapsed_sections: std::collections::HashSet::new(),
            expanded_entities: std::collections::HashSet::new(),
        }
    }

    fn ensure_renderer(&mut self) {
        if self.renderer.is_none() {
            let mut r = ifol_render_core::Renderer::new(self.settings.width, self.settings.height);
            // Load images for entities that have image_source
            for entity in &self.world.entities {
                if let Some(ref img) = entity.components.image_source
                    && let Err(e) = r.load_image(&entity.id, &img.path)
                {
                    log::warn!("Failed to load image for '{}': {}", entity.id, e);
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
        self.needs_render = false;
    }

    fn invalidate_renderer(&mut self) {
        self.renderer = None;
        self.needs_render = true;
        self.dirty = true;
    }

    /// Split a tab tile into two side-by-side (or stacked) panels.
    /// Creates a new pane of `pane_type`, wraps it in a tab tile,
    /// then replaces `target_id` in its parent with a Linear container
    /// holding both the original and the new tab tile.
    fn split_tile(
        tree: &mut egui_tiles::Tree<crate::panels::EditorPane>,
        target_id: egui_tiles::TileId,
        pane_type: crate::panels::EditorPane,
        dir: egui_tiles::LinearDir,
    ) {
        // 1. Create the new pane wrapped in a tab
        let new_pane = tree.tiles.insert_pane(pane_type);
        let new_tab = tree.tiles.insert_tab_tile(vec![new_pane]);

        // 2. Create a linear container with [original, new]
        let linear_container = egui_tiles::Container::new_linear(dir, vec![target_id, new_tab]);
        let linear_id = tree
            .tiles
            .insert_new(egui_tiles::Tile::Container(linear_container));

        // 3. Find parent and replace target_id with linear_id
        if let Some(parent_id) = tree.tiles.parent_of(target_id) {
            if let Some(egui_tiles::Tile::Container(parent)) = tree.tiles.get_mut(parent_id) {
                // Replace the child: remove old, insert new at same position
                let children = parent.children_vec();
                let mut new_children = Vec::with_capacity(children.len());
                for child in children {
                    if child == target_id {
                        new_children.push(linear_id);
                    } else {
                        new_children.push(child);
                    }
                }
                // Rebuild the container with new children
                match parent {
                    egui_tiles::Container::Linear(linear) => {
                        linear.children = new_children;
                    }
                    egui_tiles::Container::Tabs(tabs) => {
                        tabs.children = new_children;
                    }
                    egui_tiles::Container::Grid(_grid) => {
                        // Grid: use container-level remove/add
                        parent.remove_child(target_id);
                        parent.add_child(linear_id);
                    }
                }
            }
        } else {
            // target_id is the root — replace root
            tree.root = Some(linear_id);
        }
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
            self.needs_render = true;
            ctx.request_repaint();
        }

        // ── Keyboard shortcuts ──
        ctx.input_mut(|input| {
            // Space = play/pause
            if input.consume_key(Modifiers::NONE, Key::Space) {
                self.playing = !self.playing;
            }
            // Ctrl+Z = undo
            if input.consume_key(Modifiers::CTRL, Key::Z)
                && let Some(desc) = self.commands.undo(&mut self.world)
            {
                self.status = format!("↩ Undo: {}", desc);
                self.needs_render = true;
                self.dirty = true;
            }
            // Ctrl+Y = redo
            if input.consume_key(Modifiers::CTRL, Key::Y)
                && let Some(desc) = self.commands.redo(&mut self.world)
            {
                self.status = format!("↪ Redo: {}", desc);
                self.needs_render = true;
                self.dirty = true;
            }
            // Ctrl+S = save
            if input.consume_key(Modifiers::CTRL, Key::S) {
                // Will be handled after panels
            }
            // Delete = remove selected entity
            if input.consume_key(Modifiers::NONE, Key::Delete)
                && let Some(i) = self.selected
                && i < self.world.entities.len()
            {
                let eid = self.world.entities[i].id.clone();
                self.commands
                    .execute(Box::new(RemoveEntity::new(eid)), &mut self.world);
                self.selected = None;
                self.invalidate_renderer();
                self.status = "Deleted entity".into();
            }
        });

        if self.needs_render {
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

        // ── Top bar ── (must render BEFORE CentralPanel to reserve space)
        egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::new()
                    .fill(BG_APP)
                    .inner_margin(egui::Margin::symmetric(10, 4)),
            )
            .exact_height(40.0)
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
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                // Take the tree out of self so we can pass `self` mutably to behavior
                let mut tree = std::mem::replace(
                    &mut self.workspace.tree,
                    egui_tiles::Tree::empty("ifol_workspace"),
                );

                let mut behavior = crate::panels::workspace::WorkspaceBehavior { app: self };
                tree.ui(&mut behavior, ui);

                // Put the tree back
                self.workspace.tree = tree;

                // Process pending workspace actions (add tab, split)
                if let Some(action) = self.pending_workspace_action.take() {
                    use crate::panels::workspace::WorkspaceAction;
                    match action {
                        WorkspaceAction::AddTab(tab_tile_id, pane_type) => {
                            let new_pane_id = self.workspace.tree.tiles.insert_pane(pane_type);
                            if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(
                                tabs,
                            ))) = self.workspace.tree.tiles.get_mut(tab_tile_id)
                            {
                                tabs.add_child(new_pane_id);
                                tabs.set_active(new_pane_id);
                            }
                        }
                        WorkspaceAction::SplitH(tab_tile_id, pane_type) => {
                            Self::split_tile(
                                &mut self.workspace.tree,
                                tab_tile_id,
                                pane_type,
                                egui_tiles::LinearDir::Horizontal,
                            );
                        }
                        WorkspaceAction::SplitV(tab_tile_id, pane_type) => {
                            Self::split_tile(
                                &mut self.workspace.tree,
                                tab_tile_id,
                                pane_type,
                                egui_tiles::LinearDir::Vertical,
                            );
                        }
                    }
                }
            });
    }
}
