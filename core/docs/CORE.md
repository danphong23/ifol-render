# ifol-render-core — ECS Engine & Scene API

## Role

CPU-side engine: owns business logic, **provides shaders to render**.

```
Core sở hữu:
  - ECS (Entity, Component, System)
  - Shader files (composite, shapes, effects — truyền cho render)
  - Timeline, animation, transform logic
  - Culling (cắt entity ngoài viewport)
  - Video export loop

Core gọi render:
  renderer.register_pipeline("composite", COMPOSITE_WGSL, config);
  renderer.register_effect("blur", BLUR_WGSL, params, 2);
  renderer.render_frame(&draw_commands);
```

---

## Architecture

```
core/
├── src/
│   ├── lib.rs              Re-exports
│   ├── ecs/
│   │   ├── mod.rs          Entity, Components, World, ResolvedState
│   │   ├── components/     ← Directory module (extensible)
│   │   │   ├── mod.rs      Re-exports all
│   │   │   ├── sources.rs  VideoSource, ImageSource, TextSource, ColorSource
│   │   │   ├── timeline.rs Timeline
│   │   │   ├── transform.rs Transform
│   │   │   ├── appearance.rs BlendMode, ColorAdjust
│   │   │   ├── animation.rs Animation
│   │   │   ├── effects.rs  Effect, EffectStack
│   │   │   └── camera.rs   Camera
│   │   ├── systems/        ← Directory module (extensible)
│   │   │   ├── mod.rs      Re-exports all
│   │   │   ├── timeline.rs Visibility/time
│   │   │   ├── animation.rs Keyframe interpolation
│   │   │   ├── transform.rs World matrix
│   │   │   └── effects.rs  Effect dispatch
│   │   ├── pipeline.rs     System execution order
│   │   └── draw.rs         ECS → DrawCommand bridge
│   ├── shaders/            ← Core SỞ HỮU shader files
│   │   ├── composite.wgsl     Quad rendering + blend modes
│   │   ├── shapes/            SDF primitives
│   │   └── effects/           Built-in effects
│   ├── commands/           Undo/redo
│   ├── scene.rs            JSON I/O
│   ├── color.rs            Color spaces
│   ├── types.rs            Vec2, Mat4, Keyframe, Easing
│   ├── time.rs             TimeState
│   └── export/             FFmpeg pipe
└── Cargo.toml
```

---

## Core cung cấp shader cho render

```rust
// Core init — register shaders vào render
fn setup_renderer(renderer: &mut Renderer) {
    // Core sở hữu shader, truyền source cho render compile
    renderer.register_pipeline(
        "composite",
        include_str!("../shaders/composite.wgsl"),
        PipelineConfig::quad(),
    );
    renderer.register_effect(
        "blur",
        include_str!("../shaders/effects/blur.wgsl"),
        vec![("radius".into(), 4.0), ...],
        2,
    );
}
```

**Shader files nằm trong core (hoặc root `shaders/`), không nằm trong render.**

---

## Per-Frame Pipeline

```
Phase 1: timeline_system    → visibility, time
Phase 2: animation_system   → keyframe interpolation
Phase 3: transform_system   → world matrix (parent × child)
Phase 4: effects_system     → effect dispatch
Phase 5: build_draw_commands → filter, sort, culling → DrawCommand[]
Phase 6: renderer.render_frame(commands) → pixels
```

### Culling (Core cắt trước khi gửi render)

Core chỉ gửi entity visible + trong viewport cho render.
→ Giảm DrawCommand = render nhẹ hơn.

---

## Video Export (Core điều phối)

```rust
for frame in 0..total_frames {
    let time = frame as f64 / fps;
    pipeline::run(world, &TimeState::at(time));
    let commands = draw::build_draw_commands(world, settings);
    let pixels = renderer.render_frame(&commands);
    ffmpeg.write_frame(&pixels);
}
```

Core điều phối loop, render chỉ vẽ khi được gọi.
