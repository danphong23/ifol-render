use serde::{Deserialize, Serialize};

/// Defines how layers are composited together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Add,
    Subtract,
    Darken,
    Lighten,
    SoftLight,
    HardLight,
    Difference,
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

impl BlendMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "multiply" => Self::Multiply,
            "screen" => Self::Screen,
            "overlay" => Self::Overlay,
            "add" => Self::Add,
            "subtract" => Self::Subtract,
            "darken" => Self::Darken,
            "lighten" => Self::Lighten,
            "soft_light" => Self::SoftLight,
            "hard_light" => Self::HardLight,
            "difference" => Self::Difference,
            _ => Self::Normal,
        }
    }

    pub fn as_u32(&self) -> u32 {
        match self {
            Self::Normal => 0,
            Self::Multiply => 1,
            Self::Screen => 2,
            Self::Overlay => 3,
            Self::Add => 4,
            Self::Subtract => 5,
            Self::Darken => 6,
            Self::Lighten => 7,
            Self::SoftLight => 8,
            Self::HardLight => 9,
            Self::Difference => 10,
        }
    }
}
