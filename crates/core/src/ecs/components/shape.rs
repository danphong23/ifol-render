use serde::{Deserialize, Serialize};

/// Basic shape source component.
///
/// Draws a fundamental vector shape (Rectangle or Ellipse) directly
/// without needing any asset. This is primarily used for testing,
/// placeholders, and simple motion graphics elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeSource {
    #[serde(default)]
    pub kind: ShapeKind,
    
    /// Fill color in RGBA format [r, g, b, a].
    #[serde(default = "default_white")]
    pub fill_color: [f32; 4],
    
    /// Optional stroke color.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_color: Option<[f32; 4]>,
    
    /// Stroke width (only applied if stroke_color exists).
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

impl Default for ShapeKind {
    fn default() -> Self {
        Self::Rectangle
    }
}

fn default_white() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_stroke_width() -> f32 {
    1.0
}
