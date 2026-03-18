# Architecture — ifol-render

## Tổng quan

ifol-render là hệ thống rendering modular, chia thành các tool độc lập:

```
┌─────────────────────────────────────────────────────────────┐
│  Consumers (chọn 1 hoặc nhiều)                              │
│  ┌─────────┐  ┌──────────┐  ┌──────┐  ┌──────────────┐    │
│  │ Studio  │  │   CLI    │  │ WASM │  │  Your App    │    │
│  │  (GUI)  │  │ (export) │  │ (web)│  │ (Rust crate) │    │
│  └────┬────┘  └────┬─────┘  └──┬───┘  └──────┬───────┘    │
│       │            │           │              │             │
│  ┌────┴────────────┴───────────┴──────────────┴──────┐     │
│  │  core (ECS, optional — tiện lợi, không bắt buộc)  │     │
│  │  Scene JSON → Entity/Component → DrawCommand[]     │     │
│  └───────────────────────┬────────────────────────────┘     │
│                          │ hoặc gửi DrawCommand[] trực tiếp │
│  ┌───────────────────────▼────────────────────────────┐     │
│  │  render (GPU, độc lập 100%)                         │     │
│  │  DrawCommand[] → GPU → pixels                       │     │
│  └────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

## Nguyên tắc tách biệt

### Render = Thực thi GPU

| Render sở hữu | Render KHÔNG biết |
|---------------|-------------------|
| GPU context (wgpu) | Entity, Component |
| Shader pipeline | Frame thứ mấy |
| Texture cache (VRAM) | Animation, keyframe |
| Effect pipeline | Timeline logic |
| Blend modes | Tại sao entity ẩn/hiện |
| Export (PNG, video frames) | Scene JSON format |
| Hardware detection | Ai đang drag cái gì |
| Pipeline cache | |

### Core = Quyết định logic

| Core sở hữu | Core KHÔNG biết |
|-------------|-----------------|
| ECS (Entity, Component, System) | GPU, shader, VRAM |
| Timeline (visibility) | Pipeline cache |
| Animation (keyframe eval) | Texture upload |
| Transform (world matrix) | Blend mode formula |
| Culling (cắt ngoài viewport) | Hardware limits |
| Command/Undo | Pixel processing |
| Scene JSON I/O | |
| Video export loop | |

### Quy tắc đơn giản

```
Core gọi Render: render_frame(), resize(), evict_texture(), register_effect()
Render trả Core: pixels, capabilities(), cache_stats()
Core KHÔNG chạm: GPU, shader, VRAM
Render KHÔNG biết: entity, timeline, animation
```

---

## Crate Structure

```
ifol-render/
├── render/         ifol-render         GPU rendering library
├── core/           ifol-render-core    ECS engine (depends on render)
├── studio/         ifol-render-studio  GUI editor (depends on core + render)
├── crates/
│   ├── cli/        ifol-render-cli     Headless CLI (depends on core + render)
│   └── wasm/       ifol-render-wasm    WebAssembly target
├── shaders/
│   ├── composite.wgsl                  Quad rendering + blend modes
│   └── effects/                        Effect shaders (drop .wgsl = new effect)
└── docs/           Architecture docs
```

### Build output

```
cargo build
├── render (lib)   ← compile thành thư viện
├── core (lib)     ← compile + link với render
├── cli (bin)      ← 1 file exe chứa tất cả
└── studio (bin)   ← 1 file exe chứa tất cả
```

---

## Data Flow

### Preview (real-time)

```
User thao tác → Core tính ECS → DrawCommand[] → Render vẽ → hiển thị
     ↑                                                          │
     └──────────────────── 60fps loop ──────────────────────────┘
```

### Export video

```
Core loop: for frame in 0..N
  → Core tính ECS tại time=frame/fps
  → Core gom DrawCommand[]
  → Render vẽ → pixels
  → FFmpeg encode → video file
```

### Web (WASM)

```
JavaScript UI → WASM (core+render) → WebGPU → Canvas
               gọi như thư viện, zero overhead
```

---

## Cache Architecture (thuộc Render)

```
┌─────────────────────────────────────────┐
│  Render's Cache System                  │
│                                         │
│  ┌─ Texture Cache (VRAM)               │
│  │  key → GPU texture (load 1 lần)     │
│  │  evict: LRU / manual / clear_all    │
│  │                                      │
│  ├─ Pipeline Cache                      │
│  │  shader_name → GPU pipeline          │
│  │  tạo 1 lần, dùng mãi               │
│  │                                      │
│  └─ Layer Cache (tương lai)             │
│     layer_hash → GPU texture result     │
│     so sánh CPU (hash metadata)         │
│     chỉ vẽ lại layer thay đổi          │
└─────────────────────────────────────────┘

Core có thể gọi:
  renderer.evict_texture("cat.png")   // xóa 1 texture
  renderer.clear_cache()              // xóa hết
  renderer.capabilities()             // đọc giới hạn GPU
```

---

## Effect System

### Ownership

```
Core (ECS):                    Render (GPU):
EffectStack component          EffectRegistry
  effects: [                     shader WGSL files
    { type: "blur",              pipeline cache
      params: {radius: 5} },    ping-pong textures
    { type: "vignette",          generic dispatch engine
      params: {intensity: 0.5} }
  ]
       ↓ convert to EffectConfig[]
       ↓ gửi cho render
       → render_frame_with_effects(commands, effects) → pixels
```

### Thêm effect mới

```
Built-in: thêm .wgsl file + register trong EffectRegistry
Runtime:  renderer.register_effect("name", wgsl_source, params, passes)
Tương lai: auto-scan thư mục shaders/effects/
```
