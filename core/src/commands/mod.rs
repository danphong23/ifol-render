//! Command system — undo/redo for all World mutations.
//!
//! Every mutation to the ECS World goes through a Command.
//! CommandHistory manages undo/redo stacks.

mod entity;
mod property;

pub use entity::{AddEntity, RemoveEntity};
pub use property::{PropertyValue, SetProperty};

use crate::ecs::World;

/// A reversible command that mutates the World.
pub trait Command: std::fmt::Debug {
    /// Apply this command to the world.
    fn execute(&self, world: &mut World);
    /// Reverse this command.
    fn undo(&self, world: &mut World);
    /// Human-readable description for UI.
    fn description(&self) -> String;
}

/// Manages undo/redo stacks of commands.
#[derive(Default)]
pub struct CommandHistory {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,
}

impl CommandHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute a command and push it onto the undo stack.
    /// Clears the redo stack (new action branches off).
    pub fn execute(&mut self, command: Box<dyn Command>, world: &mut World) {
        command.execute(world);
        self.undo_stack.push(command);
        self.redo_stack.clear();
    }

    /// Push a command that was already applied (value already set on entity).
    /// Just records it in the undo stack without calling execute().
    pub fn push_executed(&mut self, command: Box<dyn Command>) {
        self.undo_stack.push(command);
        self.redo_stack.clear();
    }

    /// Undo the last command. Returns the description if successful.
    pub fn undo(&mut self, world: &mut World) -> Option<String> {
        if let Some(cmd) = self.undo_stack.pop() {
            cmd.undo(world);
            let desc = cmd.description();
            self.redo_stack.push(cmd);
            Some(desc)
        } else {
            None
        }
    }

    /// Redo the last undone command. Returns the description if successful.
    pub fn redo(&mut self, world: &mut World) -> Option<String> {
        if let Some(cmd) = self.redo_stack.pop() {
            cmd.execute(world);
            let desc = cmd.description();
            self.undo_stack.push(cmd);
            Some(desc)
        } else {
            None
        }
    }

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Number of commands in undo stack.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of commands in redo stack.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }
}
