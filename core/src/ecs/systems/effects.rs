//! Effects system — dispatch per-entity effect stacks to the render tool.

use crate::ecs::World;
use crate::time::TimeState;

/// Process entity effect stacks and prepare effect configs for the render tool.
///
/// This system reads `EffectStack` components from entities and converts
/// them to render-compatible `EffectConfig` structs. The actual GPU
/// processing happens in the render tool.
pub fn effects_system(_world: &mut World, _time: &TimeState) {
    // Currently a no-op placeholder.
    // In the future, this system will:
    // 1. Read EffectStack components from visible entities
    // 2. Resolve animated effect parameters
    // 3. Build per-entity EffectConfig lists
    // 4. Store in resolved state for draw.rs to consume
}
