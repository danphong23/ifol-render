//! Effect pass system — extensible post-processing pipeline.
//!
//! Each effect is a single file implementing `EffectPass`.
//! Effects are chained: scene → Effect1 → Effect2 → ... → output.

pub mod blur;
pub mod color_grade;
pub mod context;

/// An effect pass that reads from an input texture and writes to an output texture.
///
/// To add a new effect:
/// 1. Create a new file in `effects/` (e.g., `effects/glow.rs`)
/// 2. Implement `EffectPass` for your struct
/// 3. Register it in the `EffectRegistry`
pub trait EffectPass: Send + Sync {
    /// Human-readable name for this effect.
    fn name(&self) -> &str;

    /// Execute the effect pass.
    /// Reads from `ctx.input_view()`, writes to `ctx.output_view()`.
    fn execute(&self, device: &wgpu::Device, queue: &wgpu::Queue, ctx: &mut context::EffectContext);
}

/// Configuration for an effect instance (driven by ECS components).
#[derive(Debug, Clone)]
pub struct EffectConfig {
    pub effect_type: String,
    pub params: std::collections::HashMap<String, f32>,
}

/// Registry of available effects.
pub struct EffectRegistry {
    effects: std::collections::HashMap<String, Box<dyn EffectPass>>,
}

impl EffectRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            effects: std::collections::HashMap::new(),
        };
        // Register built-in effects
        registry.register(Box::new(blur::BlurEffect));
        registry.register(Box::new(color_grade::ColorGradeEffect));
        registry
    }

    pub fn register(&mut self, effect: Box<dyn EffectPass>) {
        self.effects.insert(effect.name().to_string(), effect);
    }

    pub fn get(&self, name: &str) -> Option<&dyn EffectPass> {
        self.effects.get(name).map(|b| b.as_ref())
    }

    pub fn available(&self) -> Vec<&str> {
        self.effects.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for EffectRegistry {
    fn default() -> Self {
        Self::new()
    }
}
