//! SetProperty command — change a single property on an entity.

use super::Command;
use crate::ecs::World;
use crate::color::Color4;
use crate::types::Vec2;

/// Identifies which property to change on an entity.
#[derive(Debug, Clone)]
pub enum PropertyValue {
    // Transform
    PositionX(f32),
    PositionY(f32),
    ScaleX(f32),
    ScaleY(f32),
    Rotation(f32),

    // Timeline
    StartTime(f64),
    Duration(f64),
    Layer(i32),

    // Appearance
    Opacity(f32),
    Color(Color4),

    // Identity
    EntityId(String),
}

/// Command that sets a single property on an entity, remembering the old value.
#[derive(Debug, Clone)]
pub struct SetProperty {
    pub entity_id: String,
    pub field: String,
    pub old_value: PropertyValue,
    pub new_value: PropertyValue,
}

impl SetProperty {
    pub fn new(entity_id: String, field: String, old_value: PropertyValue, new_value: PropertyValue) -> Self {
        Self { entity_id, field, old_value, new_value }
    }
}

impl Command for SetProperty {
    fn execute(&self, world: &mut World) {
        apply_property(world, &self.entity_id, &self.new_value);
    }

    fn undo(&self, world: &mut World) {
        apply_property(world, &self.entity_id, &self.old_value);
    }

    fn description(&self) -> String {
        format!("Set {} on '{}'", self.field, self.entity_id)
    }
}

/// Apply a PropertyValue to the matching entity in the world.
fn apply_property(world: &mut World, entity_id: &str, value: &PropertyValue) {
    let entity = match world.entities.iter_mut().find(|e| e.id == entity_id) {
        Some(e) => e,
        None => {
            log::warn!("SetProperty: entity '{}' not found", entity_id);
            return;
        }
    };

    match value {
        PropertyValue::PositionX(v) => {
            if let Some(ref mut tf) = entity.components.transform {
                tf.position.x = *v;
            }
        }
        PropertyValue::PositionY(v) => {
            if let Some(ref mut tf) = entity.components.transform {
                tf.position.y = *v;
            }
        }
        PropertyValue::ScaleX(v) => {
            if let Some(ref mut tf) = entity.components.transform {
                tf.scale.x = *v;
            }
        }
        PropertyValue::ScaleY(v) => {
            if let Some(ref mut tf) = entity.components.transform {
                tf.scale.y = *v;
            }
        }
        PropertyValue::Rotation(v) => {
            if let Some(ref mut tf) = entity.components.transform {
                tf.rotation = *v;
            }
        }
        PropertyValue::StartTime(v) => {
            if let Some(ref mut tl) = entity.components.timeline {
                tl.start_time = *v;
            }
        }
        PropertyValue::Duration(v) => {
            if let Some(ref mut tl) = entity.components.timeline {
                tl.duration = *v;
            }
        }
        PropertyValue::Layer(v) => {
            if let Some(ref mut tl) = entity.components.timeline {
                tl.layer = *v;
            }
        }
        PropertyValue::Opacity(v) => {
            entity.components.opacity = Some(*v);
        }
        PropertyValue::Color(c) => {
            if let Some(ref mut cs) = entity.components.color_source {
                cs.color = c.clone();
            }
        }
        PropertyValue::EntityId(new_id) => {
            entity.id = new_id.clone();
        }
    }
}
