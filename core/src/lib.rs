//! # ifol-render-core
//!
//! Thin render pipeline engine for 2D video composition.
//!
//! Core is a **stateless render machine**: receives flat frame data,
//! renders via GPU, returns pixels. No ECS, no timeline, no animation.
//!
//! ## Architecture
//!
//! ```text
//! Frontend (ECS, timeline, animation, camera)
//!     ↓ Frame (flat pixel-based entities)
//! Core (sort → pixel→clip → pack uniforms → GPU render)
//!     ↓ DrawCommands
//! Render (GPU execution → RGBA pixels)
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ifol_render_core::{CoreEngine, RenderSettings, Frame, FlatEntity};
//!
//! let mut engine = CoreEngine::new(RenderSettings::default());
//! engine.setup_builtins();
//!
//! let frame = Frame { passes: vec![...], texture_updates: vec![] };
//! let pixels = engine.render_frame(&frame);
//! ```

pub mod color;
pub mod draw;
pub mod engine;
pub mod export;
pub mod frame;
pub mod shaders;
pub mod text;
pub mod types;

// ── Legacy modules (for studio/frontend migration) ──
// These modules contain the old ECS architecture.
// They will be moved to studio/frontend in the next phase.
// New consumers should use CoreEngine + Frame API instead.
pub mod commands;
pub mod ecs;
pub mod scene;
pub mod time;

// ── Public re-exports (NEW API) ──
pub use engine::CoreEngine;
pub use export::{ExportConfig, ExportProgress, VideoCodec};
pub use frame::{FlatEntity, Frame, PassType, RenderPass, RenderSettings, TextureUpdate};

// Re-export render types for consumers that need them
pub use ifol_render::{DrawCommand, EffectConfig, GpuCapabilities, PipelineConfig, Renderer};
