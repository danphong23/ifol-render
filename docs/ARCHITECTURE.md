# Architecture — ifol-render

## Tổng quan

```
┌──────────────────────────────────────────────────────────┐
│  Consumers                                                │
│  ┌─────────┐  ┌──────────┐  ┌──────┐  ┌──────────────┐  │
│  │ Studio  │  │   CLI    │  │ WASM │  │  Your App    │  │
│  └────┬────┘  └────┬─────┘  └──┬───┘  └──────┬───────┘  │
│       │            │           │              │           │
│  ┌────┴────────────┴───────────┴──────────────┴────────┐ │
│  │  core (owns shaders + ECS + logic)                   │ │
│  │  → register shaders vào render                       │ │
│  │  → build DrawCommand[] từ ECS                        │ │
│  └───────────────────────┬──────────────────────────────┘ │
│                          │ register_pipeline + render_frame│
│  ┌───────────────────────▼──────────────────────────────┐ │
│  │  render (pure GPU executor)                           │ │
│  │  → compile shader, cache pipeline, execute, trả pixels│ │
│  └──────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

---

## Nguyên tắc tách biệt

### Render = Pure GPU Executor

| Render LÀM | Render KHÔNG LÀM |
|-------------|-------------------|
| Compile WGSL → pipeline | Sở hữu/quản lý shader |
| Cache pipeline + texture | Biết "blur", "composite" là gì |
| Execute draw commands | Quyết định vẽ gì |
| Readback pixels | Biết ECS, Entity, timeline |
| Detect GPU capabilities | Hard-code rendering logic |

### Core = Quyết định + Cung cấp

| Core LÀM | Core KHÔNG LÀM |
|-----------|-----------------|
| Sở hữu shader files | Biết GPU, VRAM |
| Register shaders vào render | Compile shader |
| Build DrawCommand[] từ ECS | Quản lý pipeline cache |
| Culling (cắt ngoài viewport) | Tạo GPU texture |
| Điều phối video export loop | Readback pixels |

### Quy tắc đơn giản

```
Core:   "dùng shader này, vẽ cái này"  →  Render
Render: "đây pixels"                    →  Core
```

---

## Shader Ownership

```
shaders/                     ← Root workspace, core sở hữu
├── composite.wgsl              Core register khi init
├── shapes/
│   ├── rect.wgsl
│   └── circle.wgsl
└── effects/
    ├── blur.wgsl
    ├── color_grade.wgsl
    ├── vignette.wgsl
    └── chromatic_aberration.wgsl

render/                     ← KHÔNG có shader files
├── src/
│   ├── engine/             GPU context
│   ├── pipeline/           Compile + cache + execute
│   └── lib.rs              API
```

| Loại shader | Ai sở hữu | Ai chạy |
|-------------|-----------|---------|
| Composite (quad) | Core/root | Render |
| SDF shapes | Core/root | Render |
| Built-in effects | Core/root | Render |
| Custom effects | User/plugin | Render |
| Custom draw | User/plugin | Render |

**Render không import, embed, hay biết tên bất kỳ shader nào.**

---

## Data Flow

### Init

```
Core/CLI:
  renderer = Renderer::new(1920, 1080);
  renderer.register_pipeline("composite", COMPOSITE_WGSL, config);
  renderer.register_effect("blur", BLUR_WGSL, params, 2);
  renderer.load_texture("bg", "assets/bg.png");
```

### Per-Frame (preview)

```
Core: ECS systems run → DrawCommand[] → renderer.render_frame() → pixels → display
```

### Export (video)

```
Core: for frame in 0..N → ECS systems → DrawCommand[] → render_frame() → ffmpeg
```

---

## Cache Architecture (render nội bộ)

| Cache | Ai tạo | Ai xóa | Nằm đâu |
|-------|--------|--------|---------|
| Texture | Bên ngoài gọi `load_texture()` | Bên ngoài gọi `evict_texture()` | GPU VRAM |
| Pipeline | Render tự tạo khi `register_pipeline()` | Persistent | GPU |
| Layer cache (tương lai) | Render tự tạo | Render tự quản lý LRU | GPU VRAM |

---

## Build Output

```
cargo build
├── render (lib)   ← compile thành thư viện (pure executor)
├── core (lib)     ← compile + link render (owns shaders + logic)
├── cli (bin)      ← 1 file exe (core + render + CLI)
└── studio (bin)   ← 1 file exe (core + render + GUI)
```
