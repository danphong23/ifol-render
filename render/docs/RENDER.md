# ifol-render — GPU Rendering Tool

## Role

Passive GPU rendering tool. Receives `DrawCommand`s, outputs pixels. **Does NOT know about ECS, Entity, Component, World, or any business logic.** It only knows how to draw textured/colored quads with transforms and opacity.

```
Caller (core/ECS)                   Render Tool
┌───────────────┐                  ┌───────────────────┐
│ Build          │   DrawCommand[]  │                   │
│ DrawCommands   │ ────────────────>│  render_frame()   │
│ from entities  │                  │                   │
└───────────────┘                  │  → GPU composite   │
                                   │  → readback pixels │
                                   │                   │
                                   │  Vec<u8> (RGBA)   │
                                   └───────────────────┘
```

## Current State (v0.1 — Proof of Concept)

### What it does
- Creates a headless wgpu context (Vulkan/DX12/Metal auto-select)
- Accepts `DrawCommand[]` — each command = 1 colored/textured quad
- Renders all quads in a single composite pass
- Reads pixels back from GPU → returns `Vec<u8>` RGBA

### What it does NOT do (yet)
- Shader-based blend modes (BlendMode enum exists but is not implemented in shaders)
- Multi-pass effects (blur, glow, color grade)
- Render-to-texture (nested compositions)
- Instanced/batched drawing (currently 1 draw call per entity)
- Async readback (currently blocks with `poll(Wait)`)
- GPU text rendering
- Masking / clipping paths
- Hardware video decode

## Architecture

```
render/
├── src/
│   ├── lib.rs              Main API: Renderer, DrawCommand, DrawSource, BlendMode
│   ├── engine.rs           GpuEngine: wgpu device, queue, output texture
│   ├── passes/
│   │   ├── mod.rs
│   │   └── composite.rs    CompositePipeline: shader, vertex buffer, bind group layout
│   ├── render_graph.rs     RenderGraph DAG (scaffolding — not yet wired)
│   ├── resource_manager.rs Resource caching (scaffolding)
│   └── vertex.rs           Vertex struct for quad geometry
├── docs/
│   └── RENDER.md           ← You are here
└── Cargo.toml
```

## Public API

### `DrawCommand`
```rust
pub struct DrawCommand {
    pub transform: [f32; 16],   // 4x4 column-major matrix
    pub opacity: f32,           // 0.0–1.0
    pub source: DrawSource,     // Color([f32;4]) or Texture(String)
    pub blend_mode: BlendMode,  // Normal | Additive | Multiply
}
```

### `Renderer`
```rust
impl Renderer {
    fn new(width: u32, height: u32) -> Self;          // headless context
    fn load_image(&mut self, key: &str, path: &str);  // cache texture
    fn load_rgba(&mut self, key: &str, data, w, h);   // cache raw RGBA
    fn render_frame(&mut self, commands: &[DrawCommand]) -> Vec<u8>;  // → RGBA pixels
    fn save_png(pixels, width, height, path);          // utility
}
```

### Pipeline per frame
```
1. For each DrawCommand:
   ├── Create CompositeUniforms (transform, opacity, color, use_texture)
   ├── Create uniform buffer (GPU upload)
   ├── Pick texture view (cached texture or white 1x1 fallback)
   └── Create bind group (uniform + texture + sampler)

2. Begin render pass (clear to dark background)
   ├── Set pipeline (vertex shader + fragment shader)
   ├── Set vertex/index buffers (fullscreen quad)
   └── For each draw call:
       ├── Set bind group
       └── draw_indexed(6 indices = 2 triangles = 1 quad)

3. Copy output texture → staging buffer
4. Map staging buffer → read pixels → return Vec<u8>
```

## Shaders

### Vertex Shader (`composite.wgsl`)
- Takes quad vertices (positions + UVs)
- Multiplies by 4x4 transform matrix from uniforms
- Outputs clip-space position + UV

### Fragment Shader (`composite.wgsl`)
- Reads `use_texture` flag from uniforms
- If texture: sample texture at UV, multiply by opacity
- If color: use solid color from uniforms, multiply by opacity
- **No blend mode logic** — all quads are drawn with default alpha-over

## Known Limitations

| Issue | Impact | Solution |
|-------|--------|----------|
| 1 draw call per entity | Slow at 100+ entities | Instanced drawing |
| No shader blend modes | BlendMode enum is decorative | Per-mode fragment shaders |
| Synchronous readback | Blocks main thread | Async map with ring buffer |
| No effects | Can't blur/glow/grade | Compute shader passes |
| No render-to-texture | Can't nest compositions | Multi-target render graph |
| No masking | Can't clip entities | Stencil buffer or alpha mask pass |
| Fixed output size | Must recreate renderer for resize | Dynamic resize |

## Upgrade Roadmap

### Phase 1: Shader Blend Modes
- Implement per-mode fragment shaders (or branching in single shader)
- Read `blend_mode` from uniform, compute blend per-pixel
- Requires reading destination color → need 2 render targets or ping-pong

### Phase 2: Effect Pipeline
- Add `EffectPass` trait: input texture → output texture
- Implement: GaussianBlur, ColorGrade, Glow
- Each effect = 1 compute shader dispatch
- Chain effects via temporary textures

### Phase 3: Render Graph
- Wire up existing `render_graph.rs` DAG
- Each node = an effect pass or composite pass
- Automatic dependency resolution and texture allocation
- Support nested compositions (render group → texture → composite)

### Phase 4: Performance
- Instanced drawing (batch same-type entities)
- Async readback with ring buffer
- Texture atlas for small images
- GPU-side compositing (accumulate in compute shader)

## How It Connects

```
                    ┌─────────────┐
                    │   Studio    │  (GUI — knows nothing about GPU)
                    └──────┬──────┘
                           │ calls core API
                    ┌──────▼──────┐
                    │    Core     │  (ECS, systems, pipeline)
                    │             │  builds DrawCommand[]
                    └──────┬──────┘
                           │ passes DrawCommand[]
                    ┌──────▼──────┐
                    │   Render    │  (GPU — draws quads, returns pixels)
                    └─────────────┘
```

**The render tool is designed to be upgraded without changing core or studio.** As long as `render_frame(commands) → Vec<u8>` contract is maintained, all consumers work unchanged.
