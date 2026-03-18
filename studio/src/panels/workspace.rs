use egui::{Ui, WidgetText};
use std::fmt;

/// Defines the different types of editors/panels available in the workspace.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditorPane {
    Viewport,
    EntityList,
    Properties,
    Timeline,
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

pub struct WorkspaceBehavior<'a> {
    pub app: &'a mut crate::app::EditorApp,
}

impl<'a> egui_tiles::Behavior<EditorPane> for WorkspaceBehavior<'a> {
    fn pane_ui(&mut self, ui: &mut Ui, _pane_id: egui_tiles::TileId, pane: &mut EditorPane) -> egui_tiles::UiResponse {
        // Render a small dropdown in the top-right corner to allow switching pane types
        let mut changed_pane = None;
        ui.allocate_ui_at_rect(
            egui::Rect::from_min_size(ui.max_rect().right_top() - egui::vec2(24.0, 0.0), egui::vec2(24.0, 24.0)),
            |ui| {
                ui.menu_button("☰", |ui| {
                    if ui.button("Viewport").clicked() { changed_pane = Some(EditorPane::Viewport); ui.close_menu(); }
                    if ui.button("Entities").clicked() { changed_pane = Some(EditorPane::EntityList); ui.close_menu(); }
                    if ui.button("Properties").clicked() { changed_pane = Some(EditorPane::Properties); ui.close_menu(); }
                    if ui.button("Timeline").clicked() { changed_pane = Some(EditorPane::Timeline); ui.close_menu(); }
                });
            }
        );

        if let Some(new_pane) = changed_pane {
            *pane = new_pane;
        }

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
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &EditorPane) -> WidgetText {
        format!("{}", pane).into()
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
    pub fn new() -> Self {
        let mut tiles = egui_tiles::Tiles::default();
        
        let pane_left = tiles.insert_pane(EditorPane::EntityList);
        let left = tiles.insert_tab_tile(vec![pane_left]);
        
        let pane_right = tiles.insert_pane(EditorPane::Properties);
        let right = tiles.insert_tab_tile(vec![pane_right]);
        
        let pane_center_top = tiles.insert_pane(EditorPane::Viewport);
        let center_top = tiles.insert_tab_tile(vec![pane_center_top]);
        
        let pane_center_bottom = tiles.insert_pane(EditorPane::Timeline);
        let center_bottom = tiles.insert_tab_tile(vec![pane_center_bottom]);

        let mut center = tiles.insert_vertical_tile(vec![center_top, center_bottom]);
        let mut right_split = tiles.insert_horizontal_tile(vec![center, right]);
        let mut root = tiles.insert_horizontal_tile(vec![left, right_split]);

        // Adjust shares to make proportions look professional out of the box
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) = tiles.get_mut(center) {
            linear.shares.set_share(center_top, 3.0);
            linear.shares.set_share(center_bottom, 1.0);
        }
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) = tiles.get_mut(right_split) {
            linear.shares.set_share(center, 3.0);
            linear.shares.set_share(right, 1.0);
        }
        if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) = tiles.get_mut(root) {
            linear.shares.set_share(left, 1.0);
            linear.shares.set_share(right_split, 4.0);
        }

        Self {
            tree: egui_tiles::Tree::new("workspace_tree", root, tiles),
        }
    }
}
