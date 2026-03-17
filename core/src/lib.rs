//! # ifol-render-core
//!
//! Core module for the ifol-render engine. Provides:
//! - **ECS**: Entity-Component-System architecture
//! - **Components**: Transform, VideoSource, TextSource, Color, Timeline, Animation, etc.
//! - **Systems**: Timeline, Animation, Transform, Sort processing
//! - **Pipeline**: Full orchestration — ECS systems → DrawCommand build → GPU render
//! - **Color Management**: sRGB, Linear, ACEScg, Rec709 color space conversions
//! - **Datatypes**: Vec2, Vec3, Color4, Curve, TimeRange
//!
//! Core is the orchestrator: it knows how to use `ifol-render` (render tool) to produce pixels.
//! Consumers (editor/CLI) only need to import core.

pub mod color;
pub mod commands;
pub mod ecs;
pub mod scene;
pub mod time;
pub mod types;

// Re-export the Renderer so consumers can create and pass it to pipeline::render_frame()
// without needing a direct dependency on the render crate.
pub use ifol_render::Renderer;
