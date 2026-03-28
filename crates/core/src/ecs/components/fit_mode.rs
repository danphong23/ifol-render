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

    /// Computes the UV bounds `([offset_x, offset_y], [scale_x, scale_y])` based on FitMode constraints.
    pub fn calculate_uv(
        &self,
        display_width: f32,
        display_height: f32,
        intrinsic_width: f32,
        intrinsic_height: f32,
        align_x: f32,
        align_y: f32,
    ) -> ([f32; 2], [f32; 2]) {
        if intrinsic_width <= 0.0 || intrinsic_height <= 0.0 || *self == Self::Stretch {
            return ([0.0, 0.0], [1.0, 1.0]);
        }
        
        let display_aspect = display_width / display_height.max(0.001);
        let source_aspect = intrinsic_width / intrinsic_height.max(0.001);
        
        match self {
            Self::Contain => {
                if source_aspect > display_aspect {
                    let scale_y = display_aspect / source_aspect;
                    ([0.0, (1.0 - scale_y) * align_y], [1.0, scale_y])
                } else {
                    let scale_x = source_aspect / display_aspect;
                    ([(1.0 - scale_x) * align_x, 0.0], [scale_x, 1.0])
                }
            }
            Self::Cover => {
                if source_aspect > display_aspect {
                    let uv_w = display_aspect / source_aspect;
                    ([(1.0 - uv_w) * align_x, 0.0], [uv_w, 1.0])
                } else {
                    let uv_h = source_aspect / display_aspect;
                    ([0.0, (1.0 - uv_h) * align_y], [1.0, uv_h])
                }
            }
            _ => ([0.0, 0.0], [1.0, 1.0]),
        }
    }

    /// Computes the actual pixel-space content bounds within a rect.
    ///
    /// For `Contain`: content is smaller than rect → returns offset + smaller size.
    /// For `Cover`/`Stretch`: content fills rect → returns (0, 0, rect_w, rect_h).
    ///
    /// Returns `(offset_x, offset_y, content_w, content_h)` relative to the rect's top-left.
    pub fn calculate_rendered_bounds(
        &self,
        rect_w: f32,
        rect_h: f32,
        intrinsic_w: f32,
        intrinsic_h: f32,
        align_x: f32,
        align_y: f32,
    ) -> (f32, f32, f32, f32) {
        if intrinsic_w <= 0.0 || intrinsic_h <= 0.0 || *self != Self::Contain {
            return (0.0, 0.0, rect_w, rect_h);
        }

        let display_aspect = rect_w / rect_h.max(0.001);
        let source_aspect = intrinsic_w / intrinsic_h.max(0.001);

        if source_aspect > display_aspect {
            // Image wider than rect → fits width, letterboxed vertically
            let content_h = rect_w / source_aspect;
            let offset_y = (rect_h - content_h) * align_y;
            (0.0, offset_y, rect_w, content_h)
        } else {
            // Image taller than rect → fits height, pillarboxed horizontally
            let content_w = rect_h * source_aspect;
            let offset_x = (rect_w - content_w) * align_x;
            (offset_x, 0.0, content_w, rect_h)
        }
    }
}

