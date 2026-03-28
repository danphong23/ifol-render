use std::collections::HashMap;
use crate::ecs::World;
use serde::de::DeserializeOwned;

/// A function capable of parsing an unknown JSON structure and injecting 
/// its typed data directly into the World column corresponding to the Entity.
pub type ComponentLoaderFn = fn(&mut World, &str, &serde_json::Value) -> Result<(), String>;

/// Manages dynamic zero-touch component deserialization.
pub struct ComponentRegistry {
    pub loaders: HashMap<String, ComponentLoaderFn>,
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        let mut reg = Self { loaders: HashMap::new() };
        
        reg.register::<crate::ecs::components::ShapeSource>("shapeSource");
        reg.register::<crate::ecs::components::VideoSource>("videoSource");
        reg.register::<crate::ecs::components::ImageSource>("imageSource");
        reg.register::<crate::ecs::components::TextSource>("textSource");
        reg.register::<crate::ecs::components::ColorSource>("colorSource");
        reg.register::<crate::ecs::components::AudioSource>("audioSource");
        
        reg.register::<crate::ecs::components::CameraComponent>("camera");
        reg.register::<crate::ecs::components::Transform>("transform");
        reg.register::<crate::ecs::components::Rect>("rect");
        reg.register::<crate::ecs::components::Visual>("visual");
        reg.register::<crate::ecs::components::AnimationComponent>("animation");
        reg.register::<crate::ecs::components::Composition>("composition");
        reg.register::<crate::scene::Lifespan>("lifespan");
        
        reg.register::<crate::ecs::components::meta::ParentId>("parentId");
        reg.register::<crate::ecs::components::meta::MaskId>("maskId");
        reg.register::<crate::ecs::components::meta::Layer>("layer");
        reg.register::<crate::ecs::components::meta::Materials>("materials");
        reg.register::<crate::ecs::components::meta::FloatUniforms>("floatUniforms");
        reg.register::<crate::ecs::components::meta::StringUniforms>("stringUniforms");
        
        reg
    }
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a strong-typed component parser onto a raw JSON property name.
    /// Example: `registry.register::<ShapeSource>("shapeSource");`
    /// Once registered, ANY shapeSource defined in an entity JSON payload
    /// automatically compiles into `HashMap<EntityId, ShapeSource>`.
    pub fn register<T: DeserializeOwned + 'static>(&mut self, json_key: &str) {
        self.loaders.insert(json_key.to_string(), |world, entity_id, value| {
            let comp: T = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            world.add_component(entity_id, comp);
            Ok(())
        });
    }
}
