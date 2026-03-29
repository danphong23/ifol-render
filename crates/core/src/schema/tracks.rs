use serde::{Deserialize, Serialize};

/// Defines the absolute start and end time (in seconds).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Lifespan {
    pub start: f64,
    pub end: f64,
}

impl Default for Lifespan {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: f64::MAX,
        }
    }
}

impl Lifespan {
    pub fn contains(&self, time: f64) -> bool {
        time >= self.start && time < self.end
    }
}

// ══════════════════════════════════════
// Interpolation System
// ══════════════════════════════════════

/// Interpolation mode between keyframes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Interpolation {
    Hold,
    Linear,
    CubicBezier {
        #[serde(default = "default_cb_x1")] x1: f32,
        #[serde(default)] y1: f32,
        #[serde(default = "default_cb_x2")] x2: f32,
        #[serde(default = "default_cb_y2")] y2: f32,
    },
    Bezier {
        #[serde(default = "default_tangent")] out_x: f32,
        #[serde(default)] out_y: f32,
        #[serde(default = "default_tangent")] in_x: f32,
        #[serde(default)] in_y: f32,
    },
}

fn default_cb_x1() -> f32 { 0.42 }
fn default_cb_x2() -> f32 { 0.58 }
fn default_cb_y2() -> f32 { 1.0 }
fn default_tangent() -> f32 { 0.33 }

impl Default for Interpolation {
    fn default() -> Self { Interpolation::Linear }
}

fn cubic_bezier_eval(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let mut u = t;
    for _ in 0..8 {
        let bx = 3.0 * (1.0 - u).powi(2) * u * x1
               + 3.0 * (1.0 - u) * u.powi(2) * x2
               + u.powi(3);
        let dx = 3.0 * (1.0 - u).powi(2) * x1
               + 6.0 * (1.0 - u) * u * (x2 - x1)
               + 3.0 * u.powi(2) * (1.0 - x2);
        if dx.abs() < 1e-7 { break; }
        u -= (bx - t) / dx;
        u = u.clamp(0.0, 1.0);
    }
    3.0 * (1.0 - u).powi(2) * u * y1
    + 3.0 * (1.0 - u) * u.powi(2) * y2
    + u.powi(3)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Keyframe {
    pub time: f64,
    pub value: f32,
    #[serde(default)]
    pub interpolation: Interpolation,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FloatTrack {
    #[serde(default)]
    pub keyframes: Vec<Keyframe>,
}

impl FloatTrack {
    pub fn evaluate(&self, time: f64, default_val: f32) -> f32 {
        if self.keyframes.is_empty() { return default_val; }
        if time <= self.keyframes[0].time { return self.keyframes[0].value; }
        
        let last = self.keyframes.last().unwrap();
        if time >= last.time { return last.value; }
        
        for window in self.keyframes.windows(2) {
            let (a, b) = (&window[0], &window[1]);
            if time >= a.time && time < b.time {
                let t = ((time - a.time) / (b.time - a.time)) as f32;
                let eased_t = match a.interpolation {
                    Interpolation::Hold => 0.0,
                    Interpolation::Linear => t,
                    Interpolation::CubicBezier { x1, y1, x2, y2 } => cubic_bezier_eval(t, x1, y1, x2, y2),
                    Interpolation::Bezier { out_x, out_y, .. } => {
                        let (in_x, in_y) = match b.interpolation {
                            Interpolation::Bezier { in_x, in_y, .. } => (in_x, in_y),
                            _ => (0.67, 0.0),
                        };
                        cubic_bezier_eval(t, out_x, out_y, 1.0 - in_x, 1.0 - in_y)
                    }
                };
                return a.value + (b.value - a.value) * eased_t;
            }
        }
        default_val
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransformTrack {
    #[serde(default)] pub x: FloatTrack,
    #[serde(default)] pub y: FloatTrack,
    #[serde(default)] pub rotation: FloatTrack,
    #[serde(default)] pub anchor_x: FloatTrack,
    #[serde(default)] pub anchor_y: FloatTrack,
    #[serde(default)] pub scale_x: FloatTrack,
    #[serde(default)] pub scale_y: FloatTrack,
    #[serde(default, skip_serializing)] pub width: FloatTrack,
    #[serde(default, skip_serializing)] pub height: FloatTrack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringKeyframe {
    pub time: f64,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StringTrack {
    #[serde(default)]
    pub keyframes: Vec<StringKeyframe>,
}

impl StringTrack {
    pub fn evaluate<'a>(&'a self, time: f64, default_val: &'a str) -> &'a str {
        if self.keyframes.is_empty() || time < self.keyframes[0].time {
            return default_val;
        }
        let mut last_val: &str = &self.keyframes[0].value;
        for kf in &self.keyframes {
            if time >= kf.time { last_val = &kf.value; } else { break; }
        }
        last_val
    }
}
