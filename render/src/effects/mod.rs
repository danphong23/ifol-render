//! Effect pass system — extensible post-processing pipeline.
//!
//! ## Architecture
//!
//! **Render owns shaders.** Core provides component data (effect type + params).
//! The render tool loads shaders, creates pipelines, and executes GPU work.
//!
//! ## Adding a new effect
//!
//! 1. Create a `.wgsl` file in `shaders/effects/` following the convention:
//!    - Vertex entry: `vs_fullscreen` (fullscreen triangle, no VBO)
//!    - Fragment entry: `fs_main`
//!    - Binding 0: `var<uniform>` with your params (any struct, ≤256 bytes)
//!    - Binding 1: `var t_input: texture_2d<f32>`
//!    - Binding 2: `var t_sampler: sampler`
//! 2. Register the shader name in `EffectRegistry` — that's it.
//!
//! No Rust code needed per-effect. The `GenericEffect` handles pipeline
//! creation, bind groups, and execution for any conforming shader.
//!
//! ## Future: external shaders
//!
//! Users will be able to load custom `.wgsl` files at runtime.
//! The same convention applies — any shader following the binding layout
//! will work automatically.

pub mod context;
pub mod pipeline_cache;

use std::collections::HashMap;

/// Configuration for an effect instance (driven by ECS components).
#[derive(Debug, Clone)]
pub struct EffectConfig {
    /// Effect type name — maps to a shader in `shaders/effects/`.
    pub effect_type: String,
    /// Float parameters passed as uniforms to the shader.
    /// The generic engine packs these into a uniform buffer in order.
    pub params: HashMap<String, f32>,
}

/// An effect entry in the registry.
pub struct EffectEntry {
    /// Human-readable name.
    pub name: String,
    /// WGSL shader source.
    pub shader_source: String,
    /// Default parameter values (defines the param order for the uniform struct).
    pub default_params: Vec<(String, f32)>,
    /// Number of passes (e.g., blur = 2 for horizontal + vertical).
    pub pass_count: u32,
}

/// Registry of available effects.
pub struct EffectRegistry {
    effects: HashMap<String, EffectEntry>,
}

impl EffectRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            effects: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    /// Register all built-in effects from embedded shaders.
    fn register_builtins(&mut self) {
        // Blur — 2-pass separable Gaussian
        self.register(EffectEntry {
            name: "blur".into(),
            shader_source: include_str!("../../../shaders/effects/blur.wgsl").into(),
            default_params: vec![
                ("direction_x".into(), 1.0),
                ("direction_y".into(), 0.0),
                ("radius".into(), 4.0),
                ("texel_size".into(), 0.001),
            ],
            pass_count: 2, // horizontal + vertical
        });

        // Color Grade — brightness/contrast/saturation
        self.register(EffectEntry {
            name: "color_grade".into(),
            shader_source: include_str!("../../../shaders/effects/color_grade.wgsl").into(),
            default_params: vec![
                ("brightness".into(), 0.0),
                ("contrast".into(), 1.0),
                ("saturation".into(), 1.0),
                ("_pad".into(), 0.0),
            ],
            pass_count: 1,
        });

        // Vignette — darkened edges
        self.register(EffectEntry {
            name: "vignette".into(),
            shader_source: include_str!("../../../shaders/effects/vignette.wgsl").into(),
            default_params: vec![
                ("intensity".into(), 0.5),
                ("smoothness".into(), 0.5),
                ("_pad0".into(), 0.0),
                ("_pad1".into(), 0.0),
            ],
            pass_count: 1,
        });

        // Chromatic Aberration — RGB channel offset
        self.register(EffectEntry {
            name: "chromatic_aberration".into(),
            shader_source: include_str!("../../../shaders/effects/chromatic_aberration.wgsl")
                .into(),
            default_params: vec![
                ("intensity".into(), 0.005),
                ("_pad0".into(), 0.0),
                ("_pad1".into(), 0.0),
                ("_pad2".into(), 0.0),
            ],
            pass_count: 1,
        });
    }

    /// Register an effect entry.
    pub fn register(&mut self, entry: EffectEntry) {
        self.effects.insert(entry.name.clone(), entry);
    }

    /// Register an external shader (loaded from file at runtime).
    pub fn register_external(
        &mut self,
        name: &str,
        shader_source: String,
        default_params: Vec<(String, f32)>,
        pass_count: u32,
    ) {
        self.effects.insert(
            name.to_string(),
            EffectEntry {
                name: name.to_string(),
                shader_source,
                default_params,
                pass_count,
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<&EffectEntry> {
        self.effects.get(name)
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
