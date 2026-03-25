use serde::{Deserialize, Serialize};

/// Defines how visual assets scale within their bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FitMode {
    Stretch,
    Contain,
    Cover,
}

impl Default for FitMode {
    fn default() -> Self {
        Self::Stretch
    }
}

impl FitMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "contain" => Self::Contain,
            "cover" => Self::Cover,
            _ => Self::Stretch,
        }
    }
    pub fn as_u32(&self) -> u32 {
        match self {
            Self::Stretch => 0,
            Self::Contain => 1,
            Self::Cover => 2,
        }
    }
}

