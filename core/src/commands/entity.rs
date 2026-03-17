//! AddEntity / RemoveEntity commands.

use super::Command;
use crate::ecs::{Entity, World};

/// Command that adds an entity to the world.
#[derive(Debug)]
pub struct AddEntity {
    pub entity: Entity,
}

impl AddEntity {
    pub fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

impl Command for AddEntity {
    fn execute(&self, world: &mut World) {
        world.add_entity(self.entity.clone());
    }

    fn undo(&self, world: &mut World) {
        if let Some(idx) = world.entities.iter().position(|e| e.id == self.entity.id) {
            world.entities.remove(idx);
            world.rebuild_index();
        }
    }

    fn description(&self) -> String {
        format!("Add entity '{}'", self.entity.id)
    }
}

/// Command that removes an entity from the world (stores a full clone for undo).
#[derive(Debug)]
pub struct RemoveEntity {
    pub entity_id: String,
    /// Stored on first execute for undo.
    snapshot: std::cell::RefCell<Option<(usize, Entity)>>,
}

impl RemoveEntity {
    pub fn new(entity_id: String) -> Self {
        Self {
            entity_id,
            snapshot: std::cell::RefCell::new(None),
        }
    }
}

impl Command for RemoveEntity {
    fn execute(&self, world: &mut World) {
        if let Some(idx) = world.entities.iter().position(|e| e.id == self.entity_id) {
            let entity = world.entities.remove(idx);
            world.rebuild_index();
            *self.snapshot.borrow_mut() = Some((idx, entity));
        }
    }

    fn undo(&self, world: &mut World) {
        if let Some((idx, entity)) = self.snapshot.borrow().clone() {
            let insert_at = idx.min(world.entities.len());
            world.entities.insert(insert_at, entity);
            world.rebuild_index();
        }
    }

    fn description(&self) -> String {
        format!("Remove entity '{}'", self.entity_id)
    }
}
