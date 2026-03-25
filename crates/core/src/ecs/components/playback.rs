use serde::{Deserialize, Serialize};
use crate::scene::Keyframe;

/// Timeline control for Video/Audio sources, enabling fast-forward, rewind, or freeze frames.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaybackTrack {
    pub time_keyframes: Vec<Keyframe>,
}

