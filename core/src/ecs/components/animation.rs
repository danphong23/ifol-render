//! Animation component — keyframe interpolation.

use crate::types::Keyframe;
use serde::{Deserialize, Serialize};

/// Animation component — collection of keyframes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Animation {
    pub keyframes: Vec<Keyframe>,
}

impl Animation {
    /// Evaluate a property at the given time (relative to entity start).
    pub fn evaluate(&self, property: &str, time: f64) -> Option<f64> {
        let relevant: Vec<&Keyframe> = self
            .keyframes
            .iter()
            .filter(|k| k.property == property)
            .collect();

        if relevant.is_empty() {
            return None;
        }

        // Before first keyframe
        if time <= relevant[0].time {
            return Some(relevant[0].value);
        }

        // After last keyframe
        if time >= relevant.last().unwrap().time {
            return Some(relevant.last().unwrap().value);
        }

        // Interpolate between keyframes
        for window in relevant.windows(2) {
            let (a, b) = (window[0], window[1]);
            if time >= a.time && time < b.time {
                let t = ((time - a.time) / (b.time - a.time)) as f32;
                let eased = a.easing.evaluate(t);
                return Some(a.value + (b.value - a.value) * eased as f64);
            }
        }

        None
    }
}
