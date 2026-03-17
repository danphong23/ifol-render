//! Color space management.
//!
//! Supports: sRGB, Linear sRGB, ACEScg, Rec.709, Rec.2020, Display P3.
//! Internal working space is Linear sRGB by default.

use serde::{Deserialize, Serialize};

/// Supported color spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorSpace {
    /// sRGB with gamma 2.2 (web standard, input default).
    #[default]
    Srgb,
    /// Linear sRGB (internal working space).
    LinearSrgb,
    /// ACEScg — wide gamut linear (film industry).
    AcesCg,
    /// Rec.709 — broadcast TV.
    Rec709,
    /// Rec.2020 — HDR / wide color.
    Rec2020,
    /// Display P3 — Apple/modern displays.
    DisplayP3,
}

/// RGBA color with associated color space.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Color4 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    #[serde(default)]
    pub space: ColorSpace,
}

impl Default for Color4 {
    fn default() -> Self {
        Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0, space: ColorSpace::Srgb }
    }
}

impl Color4 {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a, space: ColorSpace::Srgb }
    }

    pub fn white() -> Self {
        Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0, space: ColorSpace::Srgb }
    }

    /// Convert this color to the target color space.
    pub fn to_space(&self, target: ColorSpace) -> Self {
        if self.space == target {
            return *self;
        }

        // Step 1: Convert to Linear sRGB (common intermediate)
        let linear = self.to_linear();

        // Step 2: Convert from Linear sRGB to target
        linear.from_linear(target)
    }

    /// Convert to Linear sRGB.
    fn to_linear(&self) -> Self {
        match self.space {
            ColorSpace::LinearSrgb => *self,
            ColorSpace::Srgb | ColorSpace::Rec709 => {
                Self {
                    r: srgb_to_linear(self.r),
                    g: srgb_to_linear(self.g),
                    b: srgb_to_linear(self.b),
                    a: self.a,
                    space: ColorSpace::LinearSrgb,
                }
            }
            // TODO: implement full matrix transforms for ACEScg, Rec2020, DisplayP3
            _ => Self { space: ColorSpace::LinearSrgb, ..*self },
        }
    }

    /// Convert from Linear sRGB to target space.
    fn from_linear(&self, target: ColorSpace) -> Self {
        match target {
            ColorSpace::LinearSrgb => *self,
            ColorSpace::Srgb | ColorSpace::Rec709 => {
                Self {
                    r: linear_to_srgb(self.r),
                    g: linear_to_srgb(self.g),
                    b: linear_to_srgb(self.b),
                    a: self.a,
                    space: target,
                }
            }
            // TODO: implement full matrix transforms
            _ => Self { space: target, ..*self },
        }
    }
}

/// sRGB gamma to linear.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear to sRGB gamma.
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
