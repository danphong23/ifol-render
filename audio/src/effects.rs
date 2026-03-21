//! Audio effects system — extensible pipeline (like shaders for video).
//!
//! ## Architecture
//!
//! Each effect implements the `AudioEffect` trait, which processes PCM samples in-place.
//! Effects are chained per-clip and applied after decoding, before mixing.
//!
//! ```text
//! Source → decode → [EQ] → [Reverb] → volume/fade → mix buffer
//!                   ^^^^^^^^^^^^^^^ per-clip effect chain
//!
//! Mix buffer → [Limiter] → output
//!              ^^^^^^^^^ master effect chain
//! ```
//!
//! ## Extensibility
//!
//! Developers register custom effects into `EffectRegistry`:
//! ```rust,ignore
//! registry.register("my_effect", |params| Box::new(MyEffect::from_params(params)));
//! ```

use crate::clip::AudioConfig;
use std::collections::HashMap;

/// Trait for audio effects — process PCM samples in-place.
///
/// Analogous to shaders in the render pipeline.
pub trait AudioEffect: Send + Sync {
    /// Effect name (e.g., "eq", "reverb", "compressor").
    fn name(&self) -> &str;

    /// Process samples in-place.
    ///
    /// `samples` is interleaved f32 PCM at `config.sample_rate` and `config.channels`.
    fn process(&self, samples: &mut [f32], config: &AudioConfig);
}

/// An effect instance from JSON: type name + parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EffectInstance {
    /// Effect type name (matches registered name).
    #[serde(rename = "type")]
    pub effect_type: String,
    /// Effect parameters (effect-specific).
    #[serde(default)]
    pub params: HashMap<String, f64>,
}

/// Effect factory function type.
type EffectFactory = Box<dyn Fn(&HashMap<String, f64>) -> Box<dyn AudioEffect> + Send + Sync>;

/// Registry of available audio effects.
///
/// Like the shader registry in the render pipeline.
pub struct EffectRegistry {
    factories: HashMap<String, EffectFactory>,
}

impl EffectRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Create a registry with built-in effects.
    pub fn with_builtins() -> Self {
        let mut reg = Self::new();
        // Built-in effects will be registered here as we implement them:
        // reg.register("eq", |params| Box::new(EqEffect::from_params(params)));
        // reg.register("reverb", |params| Box::new(ReverbEffect::from_params(params)));
        // reg.register("compressor", |params| Box::new(CompressorEffect::from_params(params)));
        // reg.register("limiter", |params| Box::new(LimiterEffect::from_params(params)));
        let _ = &mut reg; // suppress unused warning for now
        reg
    }

    /// Register a custom effect.
    pub fn register<F>(&mut self, name: &str, factory: F)
    where
        F: Fn(&HashMap<String, f64>) -> Box<dyn AudioEffect> + Send + Sync + 'static,
    {
        self.factories.insert(name.to_string(), Box::new(factory));
    }

    /// Create an effect instance from type name and parameters.
    pub fn create(&self, effect_type: &str, params: &HashMap<String, f64>) -> Option<Box<dyn AudioEffect>> {
        self.factories.get(effect_type).map(|factory| factory(params))
    }

    /// Apply a chain of effects to samples.
    pub fn apply_chain(&self, effects: &[EffectInstance], samples: &mut [f32], config: &AudioConfig) {
        for inst in effects {
            if let Some(effect) = self.create(&inst.effect_type, &inst.params) {
                effect.process(samples, config);
            } else {
                log::warn!("Unknown audio effect: '{}' — skipping", inst.effect_type);
            }
        }
    }
}

impl Default for EffectRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}
