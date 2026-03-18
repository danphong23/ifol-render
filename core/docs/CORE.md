# ifol-render-core — ECS Engine & Scene API

## Role

CPU-side engine that owns all **business logic**. Zero GPU dependencies. Core là "đạo diễn" — quyết định cái gì, khi nào, ở đâu. Render chỉ thực thi vẽ.

```
Scene JSON → Core (ECS) → DrawCommand[] → Render Tool → pixels
                │
        Core quyết định:
        - Entity nào hiện tại thời điểm này?
        - Animation đang ở giá trị nào?
        - Transform matrix cuối cùng là gì?
        - Cắt entity nào ngoài khung?
        - Gửi danh sách DrawCommand cho render
```

## Core sở hữu gì

| Trách nhiệm | Giải thích |
|-------------|-----------|
| **ECS** | Entity, Component, System, World |
| **Timeline logic** | Ai hiện, ai ẩn, tại giây bao nhiêu |
| **Animation** | Keyframe interpolation, easing |
| **Transform** | Parent-child hierarchy, world matrix |
| **Culling** | Cắt entity ngoài viewport (giảm DrawCommand) |
| **Command/Undo** | AddEntity, RemoveEntity, SetProperty |
| **Scene I/O** | JSON serialize/deserialize |
| **Time management** | Frame index, global time, delta time |
| **Video export loop** | Điều phối vòng lặp frame (gọi render mỗi frame) |

## Core KHÔNG sở hữu

- GPU context, shader, texture GPU
- Cache texture/pipeline (thuộc render)
- Pixel processing, blend mode logic
- Hardware detection

---

## Architecture

```
core/
├── src/
│   ├── lib.rs              Re-exports
│   ├── ecs/
│   │   ├── mod.rs          Entity, Components, World, ResolvedState
│   │   ├── components/     ← Directory module (extensible)
│   │   │   ├── mod.rs      Re-exports all components
│   │   │   ├── sources.rs  VideoSource, ImageSource, TextSource, ColorSource
│   │   │   ├── timeline.rs Timeline
│   │   │   ├── transform.rs Transform
│   │   │   ├── appearance.rs BlendMode, ColorAdjust
│   │   │   ├── animation.rs Animation (keyframe interpolation)
│   │   │   ├── effects.rs  Effect, EffectStack (per-entity)
│   │   │   └── camera.rs   Camera
│   │   ├── systems/        ← Directory module (extensible)
│   │   │   ├── mod.rs      Re-exports all systems
│   │   │   ├── timeline.rs Visibility/time resolution
│   │   │   ├── animation.rs Keyframe interpolation
│   │   │   ├── transform.rs World matrix computation
│   │   │   └── effects.rs  Effect dispatch
│   │   ├── pipeline.rs     System execution order
│   │   └── draw.rs         ECS → DrawCommand bridge
│   ├── commands/
│   │   ├── mod.rs          Command trait, CommandHistory
│   │   ├── entity.rs       AddEntity, RemoveEntity
│   │   └── property.rs     SetProperty
│   ├── scene.rs            SceneDescription, RenderSettings, JSON I/O
│   ├── color.rs            Color4, ColorSpace, conversion
│   ├── types.rs            Vec2, Mat4, Keyframe, Easing
│   ├── time.rs             TimeState, EntityTime
│   └── export/
│       ├── mod.rs          ExportConfig, export_video()
│       └── ffmpeg.rs       FfmpegPipe
├── docs/
│   └── CORE.md             ← Bạn đang đây
└── Cargo.toml
```

### Extensibility

Thêm component mới = thêm 1 file `.rs` trong `components/` + `pub use` trong `mod.rs`
Thêm system mới = thêm 1 file `.rs` trong `systems/` + `pub use` trong `mod.rs`

---

## Per-Frame Pipeline

```
pipeline::render_frame(world, time, settings, renderer) → Vec<u8>

  Phase 1: timeline_system    → visibility, time resolution
  Phase 2: animation_system   → keyframe interpolation
  Phase 3: transform_system   → world matrix (parent × child)
  Phase 4: effects_system     → effect dispatch
  Phase 5: draw::build_draw_commands → filter, sort, convert → DrawCommand[]
  Phase 6: renderer.render_frame(commands) → pixels
```

### Culling (Core responsibility)

```rust
// Core cắt TRƯỚC KHI gửi cho render:
fn build_draw_commands(world, settings) -> Vec<DrawCommand> {
    for entity in world.sorted_by_layer() {
        // Chỉ gửi entity visible + trong viewport
        if !entity.resolved.visible { continue; }
        // Tương lai: viewport culling
        // if entity.position.x > viewport_right + margin { continue; }
        commands.push(DrawCommand { ... });
    }
}
```

---

## Components

| Category | Component | Fields |
|----------|-----------|--------|
| **Sources** | `VideoSource` | path, trim_start, trim_end, playback_rate |
| | `ImageSource` | path |
| | `TextSource` | content, font, font_size, color, bold, italic |
| | `ColorSource` | color |
| **Timeline** | `Timeline` | start_time, duration, layer, locked, muted, solo |
| **Transform** | `Transform` | position, scale, rotation, anchor, z_index |
| **Appearance** | `BlendMode` | Normal/Multiply/Screen/Overlay/SoftLight/Add/Difference |
| | `ColorAdjust` | brightness, contrast, saturation, hue, temperature |
| **Animation** | `Animation` | keyframes (property, time, value, easing) |
| **Effects** | `Effect` | effect_type, params HashMap |
| | `EffectStack` | ordered Vec<Effect> per entity |
| **Hierarchy** | parent, children | entity ID references |
| **Camera** | `Camera` | position, zoom, rotation |

---

## Core gọi Render như thế nào

Core import render như **Rust crate** (thư viện), gọi trực tiếp qua function call:

```toml
# core/Cargo.toml
[dependencies]
ifol-render = { path = "../render" }
```

```rust
// Core gọi render qua code, không qua CLI exe
use ifol_render::Renderer;

let mut renderer = Renderer::new(1920, 1080);
renderer.load_image("bg", "assets/bg.png");
let pixels = renderer.render_frame(&commands);
```

**Build chung** 1 binary: core + render compile thành 1 file exe.

---

## Video Export (Core điều phối)

```rust
// Core's export pipeline
fn export_video(world, settings, config, renderer) {
    let mut ffmpeg = FfmpegPipe::start(config);

    for frame in 0..total_frames {
        let time = frame as f64 / fps;

        // 1. Core tính toán: ai hiện, animation ở đâu
        pipeline::run(world, &TimeState::at(time));

        // 2. Core gom danh sách vẽ
        let commands = draw::build_draw_commands(world, settings);

        // 3. Render vẽ → pixels
        let pixels = renderer.render_frame(&commands);

        // 4. Gửi pixels cho FFmpeg encode
        ffmpeg.write_frame(&pixels);
    }

    ffmpeg.finish();
}
```

Core điều phối vòng lặp, render chỉ vẽ mỗi frame khi được gọi.

---

## Scene Format (JSON)

```json
{
  "version": "1.0",
  "settings": {
    "width": 1920, "height": 1080,
    "fps": 30, "duration": 10.0
  },
  "entities": [
    {
      "id": "bg",
      "components": {
        "colorSource": { "color": { "r": 0.1, "g": 0.1, "b": 0.15, "a": 1.0 } },
        "timeline": { "startTime": 0, "duration": 10, "layer": 0 },
        "effectStack": {
          "effects": [
            { "type": "vignette", "params": { "intensity": 0.5 } }
          ]
        }
      }
    }
  ]
}
```
