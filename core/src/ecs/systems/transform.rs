//! Transform system — compute world transform matrices.

use crate::ecs::World;
use crate::time::TimeState;
use crate::types::Mat4;

/// Compute final world transform matrices.
/// Resolves parent-child hierarchy: child_world = parent_world * child_local
pub fn transform_system(world: &mut World, _time: &TimeState) {
    // Pass 1: Compute local transform matrices for all visible entities
    for entity in &mut world.entities {
        if !entity.resolved.visible {
            continue;
        }
        if let Some(tf) = &entity.components.transform {
            entity.resolved.world_matrix =
                Mat4::from_2d(tf.position, tf.scale, tf.rotation, tf.anchor);
        } else {
            entity.resolved.world_matrix = Mat4::identity();
        }
    }

    // Pass 2: Resolve parent-child hierarchy
    // Clone IDs + parent refs to avoid borrow issues
    let hierarchy: Vec<(String, Option<String>)> = world
        .entities
        .iter()
        .map(|e| (e.id.clone(), e.components.parent.clone()))
        .collect();

    for (id, parent_id) in &hierarchy {
        if let Some(parent) = parent_id {
            // Find parent's world matrix
            let parent_matrix = world
                .entities
                .iter()
                .find(|e| e.id == *parent)
                .map(|e| e.resolved.world_matrix)
                .unwrap_or(Mat4::identity());

            // Apply: child_world = parent_world * child_local
            if let Some(entity) = world.entities.iter_mut().find(|e| e.id == *id) {
                entity.resolved.world_matrix = parent_matrix.mul(&entity.resolved.world_matrix);
            }
        }
    }
}
