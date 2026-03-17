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
        match pane {
            EditorPane::Viewport => {
                ui.label("Viewport Placeholder");
            }
            EditorPane::EntityList => {
                ui.label("Entity List Placeholder");
            }
            EditorPane::Properties => {
                ui.label("Properties Placeholder");
            }
            EditorPane::Timeline => {
                ui.label("Timeline Placeholder");
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
        
        let left = tiles.insert_pane(EditorPane::EntityList);
        let right = tiles.insert_pane(EditorPane::Properties);
        let center_top = tiles.insert_pane(EditorPane::Viewport);
        let center_bottom = tiles.insert_pane(EditorPane::Timeline);

        let center = tiles.insert_vertical_tile(vec![center_top, center_bottom]);
        let right_split = tiles.insert_horizontal_tile(vec![center, right]);
        let root = tiles.insert_horizontal_tile(vec![left, right_split]);

        Self {
            tree: egui_tiles::Tree::new("workspace_tree", root, tiles),
        }
    }
}
