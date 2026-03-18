use egui::{Ui, WidgetText, Color32, RichText, Stroke, Response};
use std::fmt;
use crate::app::{BG_PANEL, BG_SURFACE, BORDER, TEXT_DIM, TEXT_PRIMARY, ACCENT};

/// Defines the different types of editors/panels available in the workspace.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditorPane {
    Viewport,
    EntityList,
    Properties,
    Timeline,
}

impl EditorPane {
    fn icon(&self) -> &'static str {
        match self {
            EditorPane::Viewport => "🖥",
            EditorPane::EntityList => "📋",
            EditorPane::Properties => "⚙",
            EditorPane::Timeline => "🎬",
        }
    }
}

impl fmt::Display for EditorPane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorPane::Viewport => write!(f, "Viewport"),
            EditorPane::EntityList => write!(f, "Entities"),
            EditorPane::Properties => write!(f, "Properties"),
            EditorPane::Timeline => write!(f, "Timeline"),
        }
    }
}

/// Deferred workspace action (requested from immutable callback, processed in update loop)
pub enum WorkspaceAction {
    AddTab(egui_tiles::TileId, EditorPane),
    SplitH(egui_tiles::TileId, EditorPane),
    SplitV(egui_tiles::TileId, EditorPane),
}

pub struct WorkspaceBehavior<'a> {
    pub app: &'a mut crate::app::EditorApp,
}

impl<'a> WorkspaceBehavior<'a> {
    /// Find the active pane type in a Tabs container
    fn get_active_pane_in_tab(&self, tiles: &egui_tiles::Tiles<EditorPane>, tab_tile_id: egui_tiles::TileId) -> EditorPane {
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) = tiles.get(tab_tile_id) {
            if let Some(active_id) = tabs.active {
                if let Some(egui_tiles::Tile::Pane(pane)) = tiles.get(active_id) {
                    return *pane;
                }
            }
            // Fallback: first child
            for child in tabs.children.iter() {
                if let Some(egui_tiles::Tile::Pane(pane)) = tiles.get(*child) {
                    return *pane;
                }
            }
        }
        EditorPane::Viewport // default fallback
    }
}

impl<'a> egui_tiles::Behavior<EditorPane> for WorkspaceBehavior<'a> {
    fn pane_ui(&mut self, ui: &mut Ui, _tile_id: egui_tiles::TileId, pane: &mut EditorPane) -> egui_tiles::UiResponse {
        let frame = egui::Frame::new()
            .fill(BG_PANEL)
            .inner_margin(egui::Margin::same(2));
        
        frame.show(ui, |ui| {
            match pane {
                EditorPane::Viewport => {
                    crate::panels::viewport::ui(self.app, ui);
                }
                EditorPane::EntityList => {
                    crate::panels::entity_list::ui(self.app, ui);
                }
                EditorPane::Properties => {
                    crate::panels::properties::ui(self.app, ui);
                }
                EditorPane::Timeline => {
                    crate::panels::timeline::ui(self.app, ui);
                }
            }
        });
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &EditorPane) -> WidgetText {
        RichText::new(format!("{} {}", pane.icon(), pane))
            .color(TEXT_PRIMARY)
            .size(11.0)
            .into()
    }

    // ── Editor switcher dropdown in each tab bar's right area ──
    fn top_bar_right_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<EditorPane>,
        ui: &mut Ui,
        tile_id: egui_tiles::TileId,
        _tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        // Store tile_id so we can use it later to add a pane
        let resp = ui.menu_button(RichText::new("☰").color(TEXT_DIM).size(11.0), |ui| {
            ui.label(RichText::new("Add Editor Tab").color(TEXT_DIM).size(10.0));
            ui.separator();
            let options = [
                (EditorPane::Viewport, "🖥 Viewport"),
                (EditorPane::EntityList, "📋 Entities"),
                (EditorPane::Properties, "⚙ Properties"),
                (EditorPane::Timeline, "🎬 Timeline"),
            ];
            let mut action: Option<WorkspaceAction> = None;
            for (pane_type, label) in options {
                if ui.button(label).clicked() {
                    action = Some(WorkspaceAction::AddTab(tile_id, pane_type));
                    ui.close_menu();
                }
            }

            ui.add_space(4.0);
            ui.separator();
            ui.label(RichText::new("Split Panel").color(TEXT_DIM).size(10.0));
            ui.separator();

            if ui.button("◧ Split Left/Right").clicked() {
                // Get current active pane type to duplicate
                let current = self.get_active_pane_in_tab(tiles, tile_id);
                action = Some(WorkspaceAction::SplitH(tile_id, current));
                ui.close_menu();
            }
            if ui.button("⬒ Split Top/Bottom").clicked() {
                let current = self.get_active_pane_in_tab(tiles, tile_id);
                action = Some(WorkspaceAction::SplitV(tile_id, current));
                ui.close_menu();
            }

            action
        });

        if let Some(inner) = resp.inner {
            if let Some(action) = inner {
                self.app.pending_workspace_action = Some(action);
            }
        }
    }

    // Dark-themed tab bar
    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> Color32 {
        BG_SURFACE
    }

    fn tab_bg_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<EditorPane>,
        _tile_id: egui_tiles::TileId,
        state: &egui_tiles::TabState,
    ) -> Color32 {
        if state.active { BG_PANEL } else { BG_SURFACE }
    }

    fn tab_outline_stroke(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<EditorPane>,
        _tile_id: egui_tiles::TileId,
        state: &egui_tiles::TabState,
    ) -> Stroke {
        if state.active { Stroke::new(1.0, ACCENT) } else { Stroke::NONE }
    }

    fn tab_text_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<EditorPane>,
        _tile_id: egui_tiles::TileId,
        state: &egui_tiles::TabState,
    ) -> Color32 {
        if state.active { Color32::WHITE } else { TEXT_DIM }
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        3.0  // wider gap for visible panel separation
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        22.0
    }

    // Allow closing tabs (split panels can be closed)
    fn is_tab_closable(&self, _tiles: &egui_tiles::Tiles<EditorPane>, _tile_id: egui_tiles::TileId) -> bool {
        true
    }

    // Visible divider stroke between panels
    fn resize_stroke(&self, _style: &egui::Style, resize_state: egui_tiles::ResizeState) -> Stroke {
        match resize_state {
            egui_tiles::ResizeState::Idle => Stroke::new(3.0, BORDER),
            egui_tiles::ResizeState::Hovering => Stroke::new(3.0, ACCENT.linear_multiply(0.6)),
            egui_tiles::ResizeState::Dragging => Stroke::new(3.0, ACCENT),
        }
    }

    // Draw a subtle border around each tile for visual separation
    fn paint_on_top_of_tile(
        &self,
        painter: &egui::Painter,
        _style: &egui::Style,
        _tile_id: egui_tiles::TileId,
        rect: egui::Rect,
    ) {
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
    }

    // Keep all tab bars visible so the ☰ editor switcher and tab titles are always shown
    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            prune_single_child_tabs: false,
            ..egui_tiles::SimplificationOptions::default()
        }
    }

    fn on_tab_button(
        &mut self,
        _tiles: &egui_tiles::Tiles<EditorPane>,
        _tile_id: egui_tiles::TileId,
        button_response: Response,
    ) -> Response {
        button_response
    }
}

/// The workspace holds the active tiles tree.
pub struct WorkspaceLayout {
    pub tree: egui_tiles::Tree<EditorPane>,
}

impl Default for WorkspaceLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceLayout {
    /// Layout:
    /// ```text
    /// ┌─────────┬──────────────────┬──────────┐
    /// │Entities │    Viewport      │Properties│
    /// │         │                  │          │
    /// ├─────────┴──────────────────┴──────────┤
    /// │              Timeline                 │
    /// └───────────────────────────────────────┘
    /// ```
    pub fn new() -> Self {
        let mut tiles = egui_tiles::Tiles::default();
        
        // Top row: Entities | Viewport | Properties
        let pane_entities = tiles.insert_pane(EditorPane::EntityList);
        let tab_entities = tiles.insert_tab_tile(vec![pane_entities]);
        
        let pane_viewport = tiles.insert_pane(EditorPane::Viewport);
        let tab_viewport = tiles.insert_tab_tile(vec![pane_viewport]);
        
        let pane_properties = tiles.insert_pane(EditorPane::Properties);
        let tab_properties = tiles.insert_tab_tile(vec![pane_properties]);
        
        let top_row = tiles.insert_horizontal_tile(vec![tab_entities, tab_viewport, tab_properties]);
        
        // Bottom: Timeline (full width)
        let pane_timeline = tiles.insert_pane(EditorPane::Timeline);
        let tab_timeline = tiles.insert_tab_tile(vec![pane_timeline]);
        
        // Root: top_row / timeline (vertical split)
        let root = tiles.insert_vertical_tile(vec![top_row, tab_timeline]);

        // Proportions: top row columns
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) = tiles.get_mut(top_row) {
            linear.shares.set_share(tab_entities, 1.0);
            linear.shares.set_share(tab_viewport, 3.0);
            linear.shares.set_share(tab_properties, 1.2);
        }
        // Proportions: vertical (top area 3:1 timeline)
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) = tiles.get_mut(root) {
            linear.shares.set_share(top_row, 3.0);
            linear.shares.set_share(tab_timeline, 1.2);
        }

        Self {
            tree: egui_tiles::Tree::new("workspace_tree", root, tiles),
        }
    }
}
