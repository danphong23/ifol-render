use serde::{Deserialize, Serialize};
use crate::scene::{FloatTrack, StringTrack};

/// Main animation component housing keyframe tracks.
///
/// This component decouples animation timing from the baseline properties.
/// If an entity lacks this component, no animation evaluation happens.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationComponent {
    #[serde(default)]
    pub float_tracks: Vec<FloatAnimTrack>,
    #[serde(default)]
    pub string_tracks: Vec<StringAnimTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FloatAnimTrack {
    pub target: AnimTarget,
    pub track: FloatTrack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StringAnimTrack {
    pub target: AnimTarget,
    pub track: StringTrack,
}

/// Compile-time safe list of supported targets for animation tracks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimTarget {
    // Transform
    TransformX,
    TransformY,
    TransformRotation,
    TransformAnchorX,
    TransformAnchorY,
    TransformScaleX,
    TransformScaleY,
    
    // Rect
    RectWidth,
    RectHeight,
    
    // Visual
    Opacity,
    Volume,
    PlaybackTime,
    BlendMode,
    
    // ColorSource
    ColorR,
    ColorG,
    ColorB,
    ColorA,
    
    // Material Uniforms (Extensible)
    FloatUniform(String),
    StringUniform(String),
}
