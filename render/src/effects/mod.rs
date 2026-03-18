//! Effects — ping-pong context for effect chaining.
//!
//! Render does NOT own effect shaders. They are registered from outside
//! via `Renderer::register_effect()`. This module only provides the
//! ping-pong texture infrastructure.

pub mod context;
