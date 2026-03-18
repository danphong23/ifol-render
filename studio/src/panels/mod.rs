pub mod entity_list;
pub mod properties;
pub mod status_bar;
pub mod timeline;
pub mod top_bar;
pub mod viewport;
pub mod workspace;

// Re-export specific structs if needed
pub use workspace::{EditorPane, WorkspaceLayout};
