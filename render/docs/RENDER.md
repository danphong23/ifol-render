# ifol-render — Pure GPU Executor

## Tổng quan

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

### Render chỉ làm 3 việc

| Việc | Giải thích |
|------|-----------| 
| **Compile** | Nhận WGSL string → tạo GPU pipeline + bind group layout |
| **Cache** | Pipeline/texture tạo 1 lần, dùng lại mỗi frame |
| **Execute** | Nhận DrawCommand → dispatch GPU → trả pixels |

### Render KHÔNG làm

- Không sở hữu shader (kể cả composite, blend, SDF)
- Không hard-code rendering logic
- Không biết "blur", "vignette", "rect" là gì
- Không quyết định vẽ gì, khi nào, ở đâu
- Không biết ECS, Entity, timeline, animation, video

---

## Architecture

```
render/
├── src/
│   ├── lib.rs              Renderer: public API + uniform ring buffer
│   ├── engine/
│   │   ├── mod.rs          GpuEngine: device, queue, adapter, resize, readback
│   │   └── gpu.rs          GpuCapabilities: hardware detection
│   ├── effects/
│   │   └── context.rs      Ping-pong textures for effect chaining
│   └── vertex.rs           Quad geometry (shared by all pipelines)
└── Cargo.toml

shaders/                    ← Thuộc core, KHÔNG thuộc render
├── composite.wgsl          Layer compositing + 7 blend modes
├── shapes.wgsl             SDF shapes (rect, circle, ellipse, line)
├── gradient.wgsl           Linear, radial, conic gradients
├── mask.wgsl               Alpha clip masks
├── effects/
│   ├── blur.wgsl           Gaussian blur (2-pass)
│   ├── vignette.wgsl       Vignette overlay
│   ├── color_grade.wgsl    Color grading
│   └── chromatic_aberration.wgsl
└── ...
```

---

## Public API

### Khởi tạo & Engine

```rust
// Tạo renderer headless (không cần window)
let mut renderer = Renderer::new(1920, 1080);

// Thay đổi kích thước output
renderer.resize(3840, 2160);

// Truy vấn GPU
let caps = renderer.capabilities();
println!("GPU: {} ({})", caps.gpu_name, caps.backend);
println!("Max texture: {}x{}", caps.max_texture_size, caps.max_texture_size);
```

### Đăng ký Pipeline (shader từ bên ngoài)

```rust
// Đăng ký pipeline từ WGSL string
renderer.register_pipeline(
    "composite",                        // tên tùy ý
    include_str!("shaders/composite.wgsl"),  // WGSL source
    PipelineConfig::quad(),             // quad rendering + alpha blend
);

// Kiểm tra pipeline đã đăng ký chưa
assert!(renderer.has_pipeline("composite"));

// Xem danh sách pipelines
let pipelines = renderer.available_pipelines(); // ["composite", "shapes", ...]
```

### Đăng ký Effect (post-processing)

```rust
renderer.register_effect(
    "blur",
    include_str!("shaders/effects/blur.wgsl"),
    vec![
        ("radius".into(), 5.0),
        ("direction_x".into(), 0.0),
        ("direction_y".into(), 0.0),
        ("texel_size".into(), 0.0),
    ],
    2,  // 2 passes: horizontal + vertical
);
```

### Texture Cache

```rust
// Load ảnh từ file (PNG/JPEG)
renderer.load_image("background", "assets/bg.png")?;

// Load raw RGBA pixels (video frame, generated data, ...)
renderer.load_rgba("video_frame", &rgba_pixels, 1920, 1080);

// Kiểm tra texture tồn tại
if renderer.has_texture("background") { ... }

// Xóa texture khỏi GPU
renderer.evict_texture("old_frame");

// Xóa tất cả textures
renderer.clear_textures();

// Set giới hạn VRAM cache (auto-evict LRU khi vượt)
renderer.set_max_cache_size(256 * 1024 * 1024); // 256MB

// Xem thống kê cache
let stats = renderer.texture_cache_stats();
println!("{} textures, {} KB", stats.count, stats.total_bytes / 1024);
```

### Render Frame

```rust
// Tạo draw commands
let commands = vec![
    DrawCommand {
        pipeline: "composite".into(),
        uniforms: vec![/* transform + shader params */],
        textures: vec!["background".into()],
    },
];

// Render → trả RGBA pixels
let pixels: Vec<u8> = renderer.render_frame(&commands);

// Render với effects (post-processing)
let effects = vec![
    EffectConfig {
        effect_type: "blur".into(),
        params: HashMap::from([("radius".into(), 8.0)]),
    },
];
let pixels = renderer.render_frame_with_effects(&commands, &effects);

// Lưu PNG
Renderer::save_png(&pixels, 1920, 1080, "output.png")?;
```

### VRAM Stats

```rust
let vram = renderer.vram_usage();
println!("Texture cache: {} textures, {} MB", 
    vram.texture_count, 
    vram.texture_cache_bytes / 1024 / 1024);
println!("Uniform ring: {} KB", vram.uniform_buffer_bytes / 1024);
```

---

## DrawCommand — Cấu trúc lệnh vẽ

```rust
pub struct DrawCommand {
    pub pipeline: String,        // tên pipeline đã register
    pub uniforms: Vec<f32>,      // raw float data, layout phải match shader struct
    pub textures: Vec<String>,   // tên textures đã cache (binding 1, 2, ...)
}
```

### Uniform layout quy ước

Mỗi shader tự định nghĩa struct `Uniforms`. Core pack đúng layout khi tạo `DrawCommand`.

**Composite shader** (24 floats):
```
[0..15]   transform: mat4x4f     // clip-space transform
[16..19]  color: vec4f            // RGBA tint
[20]      opacity: f32
[21]      use_texture: f32        // 0.0 = solid color, 1.0 = textured
[22]      blend_mode: f32         // 0=Normal, 1=Multiply, 2=Screen, ...
[23]      _pad: f32
```

**Shapes shader** (28 floats):
```
[0..15]   transform: mat4x4f
[16..19]  color: vec4f
[20]      shape_type: f32         // 0=rect, 1=rounded_rect, 2=circle, 3=ellipse, 4=line
[21]      fill_mode: f32          // 0=filled, 1=stroke
[22]      param1: f32             // corner_radius / stroke_width / line params
[23]      param2: f32
[24..25]  size: vec2f             // width, height in UV space
[26..27]  extra: vec2f
```

**Gradient shader** (28 floats):
```
[0..15]   transform: mat4x4f
[16..19]  color_start: vec4f
[20..23]  color_end: vec4f
[24]      grad_type: f32          // 0=linear, 1=radial, 2=conic
[25]      angle: f32              // rotation (radians)
[26]      center_x: f32           // offset from center
[27]      center_y: f32
```

**Mask shader** (24 floats):
```
[0..15]   transform: mat4x4f
[16..19]  color: vec4f            // tint (usually white)
[20]      opacity: f32
[21]      mask_shape: f32         // 0=rect, 1=circle, 2=rounded_rect, 3=feathered
[22]      param1: f32             // corner_radius / feather_amount
[23]      _pad: f32
```

---

## Performance & Tối ưu hóa

| Feature | Mô tả |
|---------|-------|
| **Uniform Ring Buffer** | 2MB pre-allocated, zero alloc per draw. ~8000 draws/frame |
| **Dynamic Offsets** | Bind group dùng dynamic offset vào ring buffer |
| **Pipeline Switch Tracking** | Chỉ `set_pipeline()` khi pipeline thay đổi |
| **Single Command Encoder** | Toàn bộ frame = 1 encoder + 1 `queue.submit()` |
| **Texture LRU Cache** | Auto-evict texture ít dùng nhất khi vượt budget |
| **VRAM Monitoring** | Real-time tracking qua `vram_usage()` |

### Benchmark (Intel Iris Xe, dev build)
```
500 draw commands:
  Build commands:  0.18ms
  Render frame:    28.96ms
  VRAM ring:       2048 KB
```

---

## Cache System

| Cache | Key | Lifecycle | Eviction |
|-------|-----|-----------|----------|
| Pipeline | `string name` | Đăng ký 1 lần, live forever | Không evict |
| Texture | `string key` | Load khi cần | Manual hoặc LRU tự động |
| Sampler | Per-pipeline | Tạo cùng pipeline | Không evict |
| Uniform | Ring buffer | Reset mỗi frame | Tự động |

### LRU Texture Eviction

```rust
// Set budget 512MB
renderer.set_max_cache_size(512 * 1024 * 1024);

// Khi load texture mới vượt budget:
// → tự xóa texture ít dùng nhất (oldest last_used_frame)
// → log::info!("Evicting texture 'old_key' (LRU frame 42)")
```

---

## Hardware Detection

```rust
pub struct GpuCapabilities {
    pub gpu_name: String,        // "NVIDIA GeForce RTX 4090"
    pub backend: String,         // "Vulkan", "DX12", "Metal"
    pub max_texture_size: u32,   // 16384
    pub max_buffer_size: u64,    // bytes
}
```

---

## CLI Test

```bash
# Chạy test (output vào tests/output/)
cargo run -p ifol-render-cli -- render-test --test basic --output tests/output/basic.png

# Các test có sẵn
#   basic       6 colored quads
#   blend       7 blend modes
#   shapes      SDF shapes (filled + stroke)
#   gradients   Linear, radial, conic gradients
#   resize      Dynamic resize (800x600 → 1200x900)
#   masking     Alpha clip masks over textures
#   text        CPU text rasterization → GPU
#   effects     Vignette post-processing
#   perf        500 draws + timing benchmark
```

---

## Ai làm gì?

| Trách nhiệm | Thuộc về |
|-------------|---------|
| Compile WGSL, cache pipeline | **Render** |
| Cache texture GPU, evict LRU | **Render** |
| Execute draw commands | **Render** |
| Readback pixels | **Render** |
| Detect GPU capabilities | **Render** |
| Sở hữu shader files | **Core** |
| Đăng ký pipelines/effects | **Core** |
| Tạo DrawCommand (pack uniforms) | **Core** |
| Quyết định draw order | **Core** |
| Layer logic (visibility, blend) | **Core** |
| Load video frame → RGBA | **Core** + FFmpeg |
| Text rasterization (ab_glyph) | **Core** |
| Scene/timeline/animation | **Core** |
