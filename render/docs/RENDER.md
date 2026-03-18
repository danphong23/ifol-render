# ifol-render — Pure GPU Executor

## Role

**Pure GPU executor.** Nhận shader (WGSL string), compile, cache, chạy. **Không sở hữu bất kỳ shader hay rendering logic nào.**

```
Bên ngoài (core/user/plugin)           Render Tool
┌──────────────────────┐               ┌──────────────────────────┐
│ WGSL shader source   │ ─register──→  │ compile → cache pipeline │
│ Texture data (RGBA)  │ ─upload───→   │ upload → cache GPU       │
│ DrawCommand[]        │ ─dispatch──→  │ GPU execute → pixels     │
│ EffectConfig[]       │ ─effects───→  │ ping-pong → pixels       │
│ "resize 1920x1080"   │ ─resize───→   │ recreate output texture  │
│ "capabilities?"      │ ←─query────   │ GPU limits, VRAM, name   │
└──────────────────────┘               └──────────────────────────┘
```

## Render chỉ làm 3 việc

| Việc | Giải thích |
|------|-----------|
| **Compile** | Nhận WGSL string → tạo GPU pipeline + bind group layout |
| **Cache** | Pipeline/texture tạo 1 lần, dùng lại mỗi frame |
| **Execute** | Nhận DrawCommand → dispatch GPU → trả pixels |

## Render KHÔNG làm

- Không sở hữu shader (kể cả composite, blend, SDF)
- Không hard-code rendering logic
- Không biết "blur", "vignette", "rect" là gì
- Không quyết định vẽ gì, khi nào, ở đâu
- Không biết ECS, Entity, timeline, animation

---

## Architecture

```
render/
├── src/
│   ├── lib.rs              Public API: Renderer (thin wrapper)
│   ├── engine/
│   │   ├── mod.rs          GPU context: device, queue, adapter
│   │   ├── gpu.rs          Capabilities, limits detection
│   │   └── texture.rs      Texture upload, cache, eviction
│   ├── pipeline/
│   │   ├── mod.rs          PipelineExecutor: compile → cache → execute
│   │   ├── cache.rs        Pipeline cache (create once, reuse)
│   │   ├── draw.rs         Execute draw calls (quad geometry dispatch)
│   │   └── fullscreen.rs   Execute fullscreen passes (effect dispatch)
│   └── effects/
│       └── context.rs      Ping-pong textures for chaining
└── Cargo.toml

Không có shaders/ folder trong render.
```

---

## Public API

```rust
impl Renderer {
    // === Engine ===
    fn new(w: u32, h: u32) -> Self;
    fn resize(&mut self, w: u32, h: u32);
    fn capabilities(&self) -> GpuCapabilities;

    // === Texture (render cache, bên ngoài quyết định load gì) ===
    fn load_texture(&mut self, key: &str, data: &[u8], w: u32, h: u32);
    fn load_texture_from_file(&mut self, key: &str, path: &str) -> Result<()>;
    fn has_texture(&self, key: &str) -> bool;
    fn evict_texture(&mut self, key: &str);
    fn clear_textures(&mut self);

    // === Pipeline (shader do bên ngoài truyền vào) ===
    fn register_pipeline(&mut self, name: &str, wgsl: &str, config: PipelineConfig);
    fn register_effect(&mut self, name: &str, wgsl: &str, params: Vec<(String,f32)>, passes: u32);

    // === Draw ===
    fn render_frame(&mut self, commands: &[DrawCommand]) -> Vec<u8>;
    fn render_frame_with_effects(&mut self, commands: &[DrawCommand], effects: &[EffectConfig]) -> Vec<u8>;

    // === Export ===
    fn save_png(pixels: &[u8], w: u32, h: u32, path: &str) -> Result<()>;
}
```

### DrawCommand

```rust
pub struct DrawCommand {
    pub pipeline: String,        // tên pipeline đã register
    pub transform: [f32; 16],    // 4x4 matrix
    pub uniforms: Vec<f32>,      // shader-specific float params
    pub textures: Vec<String>,   // tên textures đã cache
}
```

### EffectConfig

```rust
pub struct EffectConfig {
    pub effect_type: String,          // tên effect đã register
    pub params: HashMap<String, f32>, // override default params
}
```

---

## Cache (thuộc render, quản lý nội bộ)

| Cache | Key | Value | Eviction |
|-------|-----|-------|----------|
| Texture | string key | GPU texture + view | Manual / LRU |
| Pipeline | pipeline name | RenderPipeline + bind group layout | Persistent |
| Sampler | per-pipeline | GPU sampler | Persistent |

Bên ngoài gọi `evict_texture()` hoặc `clear_textures()` khi cần.

---

## Hardware Detection

```rust
pub struct GpuCapabilities {
    pub gpu_name: String,
    pub max_texture_size: u32,
    pub max_buffer_size: u64,
    pub backend: String,           // "Vulkan", "DX12", "Metal", "WebGPU"
}
```

Render tự detect khi khởi tạo. Bên ngoài đọc qua `renderer.capabilities()`.
