//! Timeline placement component.

use serde::{Deserialize, Serialize};

/// Timeline placement of an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    /// Start time on the global timeline (seconds).
    pub start_time: f64,
    /// Duration (seconds).
    pub duration: f64,
    /// Layer/track index (higher = on top).
    #[serde(default)]
    pub layer: i32,
    /// Whether this track is locked (cannot be edited).
    #[serde(default)]
    pub locked: bool,
    /// Whether this track is muted (hidden during playback).
    #[serde(default)]
    pub muted: bool,
    /// Solo mode — only solo tracks are visible during playback.
    #[serde(default)]
    pub solo: bool,
}
