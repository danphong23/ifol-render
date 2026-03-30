use crate::scene::Keyframe;
use serde::{Deserialize, Serialize};

/// Timeline control for Video/Audio sources, enabling fast-forward, rewind, or freeze frames.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaybackTrack {
    pub time_keyframes: Vec<Keyframe>,
}
