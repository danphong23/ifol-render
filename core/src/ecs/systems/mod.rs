//! ECS systems — one file per system.
//!
//! Systems run each frame to process entity state.
//! To add a new system, create a new file and re-export from here.

mod animation;
mod effects;
mod timeline;
mod transform;

pub use animation::animation_system;
pub use effects::effects_system;
pub use timeline::timeline_system;
pub use transform::transform_system;
