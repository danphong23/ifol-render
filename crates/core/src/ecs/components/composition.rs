use serde::{Deserialize, Serialize};

/// Composition component — turns an entity into a "group" with its own internal timeline.
///
/// Children of a Composition entity live in the composition's internal time scope.
/// Speed, trim, and loop control how internal time maps to the parent timeline.
///
/// This is equivalent to After Effects' "Pre-composition" or DaVinci's "Compound Clip".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Composition {
    /// Playback speed multiplier (1.0 = normal, 2.0 = 2x fast, 0.5 = half speed).
    #[serde(default = "default_speed")]
    pub speed: f32,

    /// Start offset into internal content (seconds). Equivalent to "trim head".
    #[serde(default)]
    pub trim_start: f64,

    /// End offset into internal content (seconds). None = auto (= duration).
    #[serde(default)]
    pub trim_end: Option<f64>,

    /// Internal duration mode.
    #[serde(default)]
    pub duration: DurationMode,

    /// How content behaves when playback exceeds duration.
    #[serde(default)]
    pub loop_mode: LoopMode,

    /// If true, materials on this entity apply to the entire group output.
    /// (Renders children to intermediate texture, then applies effects.)
    #[serde(default = "default_true")]
    pub material_cascade: bool,
}

fn default_speed() -> f32 { 1.0 }
fn default_true() -> bool { true }

impl Default for Composition {
    fn default() -> Self {
        Self {
            speed: 1.0,
            trim_start: 0.0,
            trim_end: None,
            duration: DurationMode::default(),
            loop_mode: LoopMode::default(),
            material_cascade: true,
        }
    }
}

/// How the composition's internal duration is determined.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DurationMode {
    /// Auto-calculate from max(children.lifespan.end).
    Auto,
    /// User-specified fixed duration in seconds.
    Manual(f64),
}

impl Default for DurationMode {
    fn default() -> Self { DurationMode::Auto }
}

/// How content behaves when playback exceeds the composition's duration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoopMode {
    /// Play once, hold last frame.
    Once,
    /// Loop back to start.
    Loop,
    /// Alternate forward/backward.
    PingPong,
}

impl Default for LoopMode {
    fn default() -> Self { LoopMode::Once }
}
