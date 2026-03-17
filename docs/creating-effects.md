# Creating Effects

This guide shows how to add a new visual effect to ifol-render.

## Step 1: Create the Shader

Create a new WGSL file in `shaders/`:

```wgsl
// shaders/my_effect.wgsl
// My custom effect shader

@group(0) @binding(0) var t_texture: texture_2d<f32>;
@group(0) @binding(1) var t_sampler: sampler;

struct Params {
    intensity: f32,
    // Add your parameters here
}
@group(0) @binding(2) var<uniform> params: Params;

// Time bindings (auto-injected by engine)
struct TimeUniforms {
    frame_time: f32,
    global_time: f32,
    normalized_time: f32,
    delta_time: f32,
}
@group(0) @binding(3) var<uniform> time: TimeUniforms;

@fragment
fn fs_main(@location(0) uv: vec2f) -> @location(0) vec4f {
    let color = textureSample(t_texture, t_sampler, uv);
    // Apply your effect here
    return color * params.intensity;
}
```

## Step 2: Create the Render Pass

Create `crates/gpu/src/passes/my_effect.rs`:

```rust
use super::super::render_graph::{RenderPass, PassContext};

pub struct MyEffectPass {
    pub intensity: f32,
}

impl RenderPass for MyEffectPass {
    fn name(&self) -> &str { "my_effect" }

    fn execute(&self, ctx: &mut PassContext) {
        // TODO: bind shader, set uniforms, draw
    }
}
```

## Step 3: Register the Pass

Add to `crates/gpu/src/passes/mod.rs`:

```rust
pub mod my_effect;
```

## Step 4: Use in a Scene

```json
{
    "id": "clip_01",
    "components": {
        "effects": [
            { "type": "my_effect", "params": { "intensity": 0.8 } }
        ]
    }
}
```

That's it! No core engine changes needed.
