use std::any::{Any, TypeId};
use std::collections::HashMap;

/// A dynamic type-map that strictly stores one instance per Rust TypeId.
/// Used for implementing the Sparse-Set ECS architecture where component columns
/// (`HashMap<EntityId, Component>`) are stored cleanly without static definitions.
#[derive(Default)]
pub struct TypeMap {
    data: HashMap<TypeId, Box<dyn Any>>,
}

impl TypeMap {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    /// Automatically injects a value of type T
    pub fn insert<T: 'static>(&mut self, val: T) {
        self.data.insert(TypeId::of::<T>(), Box::new(val));
    }
    
    /// Get a shared, strongly-typed reference to the storage column of type T
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.data.get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }
    
    /// Get a mutable, strongly-typed reference to the storage column of type T
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.data.get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut::<T>())
    }

    /// Helper to get a single component for an entity from a storage column `HashMap<String, T>`.
    pub fn get_component<T: 'static>(&self, entity_id: &str) -> Option<&T> {
        self.get::<HashMap<String, T>>()
            .and_then(|map| map.get(entity_id))
    }
    
    /// Helper to get a mutable single component for an entity from a storage column `HashMap<String, T>`.
    pub fn get_component_mut<T: 'static>(&mut self, entity_id: &str) -> Option<&mut T> {
        self.get_mut::<HashMap<String, T>>()
            .and_then(|map| map.get_mut(entity_id))
    }
}
