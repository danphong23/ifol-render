//! # ifol-render-core
//!
//! Core module for the ifol-render engine. Provides:
//! - **ECS**: Entity-Component-System architecture
//! - **Components**: Transform, VideoSource, TextSource, Color, Timeline, Animation, etc.
//! - **Systems**: Timeline, Animation, Transform, Sort processing
//! - **Color Management**: sRGB, Linear, ACEScg, Rec709 color space conversions
//! - **Datatypes**: Vec2, Vec3, Color4, Curve, TimeRange

pub mod schema;
pub mod assets;
pub mod color;
pub mod ecs;
pub mod frame;
pub mod scene;
pub mod time;
pub mod types;

pub use assets::{AssetManager, AssetCommand};
