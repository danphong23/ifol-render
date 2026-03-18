# ifol-render-core — ECS Engine & Scene API

## Role

CPU-side engine that owns all business logic. **Zero GPU dependencies.** Provides:
- Entity-Component-System (ECS) with hierarchy
- Command pattern (undo/redo)
- Scene serialization (JSON ↔ World)
- Animation with keyframe interpolation
- Color management
- Export pipeline (FFmpeg integration)
- Per-frame pipeline that converts ECS state → `DrawCommand[]` for the render tool

```
Scene JSON ─────> Core (ECS) ─────> DrawCommand[] ─────> Render Tool
                     │
              Systems run each frame:
              1. Timeline (visibility)
              2. Animation (keyframes)
              3. Transform (matrices)
              4. Draw (→ DrawCommand[])
```

## Architecture

```
core/
├── src/
│   ├── lib.rs              Re-exports: Renderer, DrawCommand, etc.
│   ├── ecs/
│   │   ├── mod.rs          Entity, Components, World (hierarchy ops)
│   │   ├── components.rs   All component types (30+ fields)
│   │   ├── systems.rs      Per-frame systems (4 systems)
│   │   ├── pipeline.rs     Frame pipeline orchestrator
│   │   └── draw.rs         ECS → DrawCommand[] conversion
│   ├── commands/
│   │   ├── mod.rs          Command trait, CommandHistory
│   │   ├── entity.rs       AddEntity, RemoveEntity
│   │   └── property.rs     SetProperty (15 property types)
│   ├── scene.rs            SceneDescription, RenderSettings, JSON I/O
│   ├── color.rs            Color4, ColorSpace (6 spaces), conversion
│   ├── types.rs            Vec2, Mat4, Keyframe, Easing
│   ├── time.rs             TimeState, EntityTime
│   └── export/
│       ├── mod.rs          ExportConfig, VideoCodec, export_video()
│       └── ffmpeg.rs       FfmpegPipe (stdin streaming)
├── docs/
│   └── CORE.md             ← You are here
└── Cargo.toml
```

## ECS Design

### Entity
```rust
struct Entity {
    id: String,
    components: Components,   // all data
    resolved: ResolvedState,  // computed each frame (not serialized)
}
```

### Components (data only — no behavior)
| Category | Components |
|----------|-----------|
| **Sources** | `VideoSource`, `ImageSource`, `TextSource`, `ColorSource` |
| **Layout** | `Timeline` (start, duration, layer, locked, muted, solo) |
| **Transform** | `Transform` (position, scale, rotation, anchor, z_index) |
| **Appearance** | `opacity`, `BlendMode` (7 modes), `visible`, `name` |
| **Effects** | `ColorAdjust` (brightness, contrast, saturation, hue, temperature) |
| **Animation** | `Animation` (keyframes with easing) |
| **Hierarchy** | `parent`, `children` |
| **FX** | `effects: Vec<Effect>` (type + params HashMap) |

### ResolvedState (computed per frame)
```rust
struct ResolvedState {
    visible: bool,          // is entity active at current time?
    world_matrix: Mat4,     // final transform (parent × local)
    opacity: f32,           // final opacity (animated)
    time: EntityTime,       // local/normalized/global time
    layer: i32,             // sort order
    z_index: f32,           // depth within layer
}
```

### World (entity container + hierarchy)
| Method | Purpose |
|--------|---------|
| `add_entity(e)` | Add entity, update index |
| `remove_entity(idx)` | Remove + orphan children + update parent |
| `reparent(id, parent_id)` | Move entity in hierarchy |
| `get_roots()` | Entities with no parent |
| `get_children(id)` | Direct children |
| `get(id)` / `get_mut(id)` | Lookup by ID |
| `sorted_by_layer()` | Visible entities sorted by layer + z_index |

## Per-Frame Pipeline

```
pipeline::render_frame(world, time, settings, renderer) → Vec<u8>

  1. timeline_system(world, time)
     ├── Check entity.visible flag
     ├── Check timeline range (start ≤ t < start+dur)
     ├── Apply mute/solo logic
     └── Set resolved.visible, resolved.layer, resolved.time

  2. animation_system(world, time)
     ├── Evaluate keyframes at local_time
     ├── Animate: opacity, position.x/y, scale.x/y, rotation
     └── Apply easing: Linear, EaseIn/Out, CubicBezier

  3. transform_system(world, time)
     ├── Pass 1: local matrix = Mat4::from_2d(pos, scale, rot, anchor)
     └── Pass 2: world matrix = parent_world × child_local

  4. draw::build_draw_commands(world, settings)
     ├── Filter visible entities
     ├── Sort by layer + z_index
     ├── For each: create DrawCommand with world_matrix + opacity
     └── Return Vec<DrawCommand>

  5. renderer.render_frame(commands) → Vec<u8>
```

## Command System

```
User action → Command::execute(world) → push to history
Ctrl+Z      → Command::undo(world)
Ctrl+Y      → Command::redo(world)
```

| Command | Fields |
|---------|--------|
| `AddEntity` | entity snapshot |
| `RemoveEntity` | entity_id, snapshot (for undo) |
| `SetProperty` | entity_id, field, old_value, new_value |

### PropertyValue variants
Position X/Y, Scale X/Y, Rotation, StartTime, Duration, Layer, Opacity, Color, EntityId

### Known limitation
No coalescing — dragging a slider creates 1 undo entry per frame. Planned: dual-mode coalescing (instant vs. accumulate) and transaction pattern for drag operations.

## Scene Format (JSON)

```json
{
  "version": "1.0",
  "settings": {
    "width": 1920, "height": 1080,
    "fps": 30, "duration": 10.0,
    "colorSpace": "linearSrgb",
    "outputColorSpace": "srgb"
  },
  "entities": [
    {
      "id": "bg",
      "components": {
        "colorSource": { "color": { "r": 0.1, "g": 0.1, "b": 0.15, "a": 1.0 } },
        "timeline": { "startTime": 0, "duration": 10, "layer": 0 },
        "transform": { "position": {"x":0,"y":0}, "scale": {"x":1,"y":1} }
      }
    }
  ]
}
```

## Export Pipeline

```
export_video(world, settings, config, renderer, on_progress)

  1. Start FFmpeg subprocess (stdin pipe)
  2. For each frame (0..total_frames):
     ├── time.seek(frame / fps)
     ├── render_frame() → RGBA pixels
     ├── ffmpeg.write_frame(pixels)
     └── on_progress(ExportProgress)
  3. ffmpeg.finish() → wait for encoding complete
```

Codecs: H264, H265, VP9, ProRes, PNG sequence. Configurable CRF, FPS, resolution, FFmpeg path.

## Color Management

6 color spaces: `Srgb`, `LinearSrgb`, `ACEScg`, `Rec709`, `Rec2020`, `DisplayP3`

Conversion via 3x3 matrices (RGB↔XYZ). Scene specifies working space (`colorSpace`) and output space (`outputColorSpace`). Currently CPU-side only — planned: GPU LUT.
