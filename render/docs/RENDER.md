# ifol-render — GPU Rendering Tool

## Role

**Independent, passive GPU rendering tool.** Receives `DrawCommand[]`, outputs pixels. Does NOT know about ECS, Entity, Component, World, or any business logic.

> Render = "họa sĩ câm" — chỉ biết vẽ, không biết vẽ cho ai.

```
Bất kỳ caller nào               Render Tool
┌───────────────┐               ┌──────────────────────────┐
│ DrawCommand[] │ ─────────────>│ render_frame()           │
│               │               │  → GPU composite         │
│ EffectConfig[]│ ─────────────>│ render_frame_with_effects│
│               │               │  → effect pipeline       │
│ "load_image"  │ ─────────────>│ load_image() → cache GPU │
│ "evict"       │ ─────────────>│ evict_texture() → xóa    │
│ "resize"      │ ─────────────>│ resize() → đổi output    │
│ "capabilities"│ <─────────────│ capabilities() → limits  │
└───────────────┘               └──────────────────────────┘
```

## Render sở hữu gì

| Trách nhiệm | Giải thích |
|-------------|-----------|
| **GPU context** | Tạo, quản lý wgpu device/queue |
| **Shader pipeline** | Load WGSL, tạo pipeline, cache |
| **Texture cache** | Load file → upload GPU → cache trong VRAM |
| **Effect pipeline** | Pipeline cache, ping-pong context, generic dispatch |
| **Export** | PNG, video frames (render pixel → file) |
| **Hardware detection** | adapter.limits(), VRAM, GPU name |
| **Layer cache** (tương lai) | Cache kết quả layer không đổi |

## Render KHÔNG biết

- Entity, Component, ECS là gì
- Frame thứ mấy, thời gian bao nhiêu
- Animation, keyframe
- Tại sao entity này ẩn/hiện
- Ai đang drag cái gì trên editor

---

## Architecture

```
render/
├── src/
│   ├── lib.rs                  Main API: Renderer, DrawCommand, BlendMode
│   ├── engine.rs               GpuEngine: wgpu device, queue, output texture
│   ├── effects/
│   │   ├── mod.rs              EffectEntry, EffectRegistry, EffectConfig
│   │   ├── context.rs          EffectContext: ping-pong textures
│   │   └── pipeline_cache.rs   PipelineCache: create once, reuse per frame
│   ├── passes/
│   │   ├── mod.rs
│   │   └── composite.rs        CompositePipeline: quad shader + blend modes
│   ├── render_graph.rs         RenderGraph DAG (tương lai)
│   ├── resource_manager.rs     Resource caching (tương lai)
│   └── vertex.rs               Vertex struct for quad geometry
├── docs/
│   └── RENDER.md               ← Bạn đang đây
└── Cargo.toml

shaders/
├── composite.wgsl              Quad rendering + 7 blend modes
└── effects/
    ├── blur.wgsl               9-tap separable Gaussian (2-pass)
    ├── color_grade.wgsl        Brightness/contrast/saturation
    ├── vignette.wgsl           Smoothstep edge darkening
    └── chromatic_aberration.wgsl  Radial RGB channel offset
```

---

## Public API

### `DrawCommand`

```rust
pub struct DrawCommand {
    pub transform: [f32; 16],   // 4x4 column-major matrix
    pub opacity: f32,           // 0.0–1.0
    pub source: DrawSource,     // Color([f32;4]) or Texture(String)
    pub blend_mode: BlendMode,  // 7 modes
}

pub enum BlendMode {
    Normal, Multiply, Screen, Overlay, SoftLight, Add, Difference
}
```

### `Renderer`

```rust
impl Renderer {
    // Khởi tạo
    fn new(width: u32, height: u32) -> Self;

    // Texture management
    fn load_image(&mut self, key: &str, path: &str);
    fn load_rgba(&mut self, key: &str, data, w, h);

    // Render
    fn render_frame(&mut self, commands: &[DrawCommand]) -> Vec<u8>;
    fn render_frame_with_effects(&mut self, commands, effects) -> Vec<u8>;

    // Effects
    fn effect_registry(&self) -> &EffectRegistry;
    fn register_effect(&mut self, name, wgsl_source, params, passes);

    // Export
    fn save_png(pixels, width, height, path);
}
```

### `EffectConfig`

```rust
pub struct EffectConfig {
    pub effect_type: String,          // "blur", "color_grade", "vignette",...
    pub params: HashMap<String, f32>, // shader uniform values
}
```

---

## Cơ chế Cache

### Texture Cache
- **Load 1 lần**: file → decode → upload GPU VRAM → cache theo key
- **Dùng lại**: DrawCommand chỉ chứa `texture_key`, render tự tìm trong cache
- **Evict**: `evict_texture(key)`, `evict_unused(seconds)`, `clear_cache()` (tương lai)
- **LRU**: texture không dùng quá N giây → tự xóa (tương lai)

### Pipeline Cache
- **Tạo 1 lần**: shader source → GPU pipeline + bind group layout + sampler
- **Dùng lại**: mỗi frame chỉ tạo bind group + uniform buffer
- **Keyed by**: shader name string

### Layer Cache (tương lai)
- Cache kết quả vẽ của layer không đổi
- So sánh bằng CPU (hash metadata: position, opacity, texture_key,...)
- Chỉ vẽ lại layer thay đổi → nhanh hơn 5-10x khi drag edit

---

## Effect System

### Convention

Mọi effect shader đều tuân theo cùng layout:

```wgsl
@group(0) @binding(0) var<uniform> params: YourStruct;  // float params
@group(0) @binding(1) var t_input: texture_2d<f32>;      // ping-pong input
@group(0) @binding(2) var t_sampler: sampler;             // linear clamp
@vertex fn vs_fullscreen(...)   // fullscreen triangle (no VBO)
@fragment fn fs_main(...)       // effect logic
```

### Thêm effect mới

1. Tạo file `.wgsl` trong `shaders/effects/`
2. Register trong `EffectRegistry::register_builtins()`
3. Xong — không cần viết Rust code

### Custom shader (runtime)

```rust
let wgsl = std::fs::read_to_string("my_effect.wgsl").unwrap();
renderer.register_effect("my_effect", wgsl, vec![
    ("intensity".into(), 0.5),
    ("radius".into(), 3.0),
], 1);
```

---

## Blend Modes

7 per-pixel blend modes trong `composite.wgsl`:

| Mode | Công thức |
|------|-----------|
| Normal | `src` (alpha over) |
| Multiply | `src × dst` |
| Screen | `1 - (1-src)(1-dst)` |
| Overlay | `if dst<0.5: 2×src×dst else: 1-2(1-src)(1-dst)` |
| SoftLight | W3C formula |
| Add | `src + dst` (clamp) |
| Difference | `abs(src - dst)` |

---

## Hardware Detection

```rust
let caps = renderer.capabilities();  // (tương lai)
caps.max_texture_size    // e.g. 16384
caps.max_render_size     // e.g. 8192×8192
caps.vram_available      // e.g. 4GB
caps.gpu_name            // "NVIDIA RTX 4060"
```

Render tự điều chỉnh khi GPU yếu (giảm resolution, tắt effect nặng).

---

## Parallel Rendering (tương lai)

### Preview: GPU pipeline parallel
```
CPU gửi: frame 10 → frame 11 → frame 12
GPU:     [render 10] [render 11] [render 12]   ← overlap
```

### Export: batch render
```rust
renderer.render_batch(&[commands_f0, commands_f1, commands_f2]);
```

---

## Nguyên tắc thiết kế

1. **Contract ổn định**: `render_frame(commands) → Vec<u8>` không bao giờ thay đổi
2. **Core/Studio không cần sửa** khi render được nâng cấp
3. **Zero business logic**: render không biết scene, timeline, animation
4. **GPU-first**: mọi thứ nặng chạy trên GPU, CPU chỉ dispatch
5. **Cache aggressive**: texture, pipeline, layer — minimize GPU work
