# ifol-render — ECS Architecture V4

## Overview

ifol-render is a pure ECS (Entity-Component-System) video/motion graphics engine.
**Everything is a System** — including rendering.

```
┌─ Consumer (Web/Studio/CLI) ─────────────────────────────┐
│  Scene editing, asset loading, orchestration              │
├─ Core Engine (Rust/WASM) ───────────────────────────────┤
│  ECS World → Pipeline (7 systems) → GPU draw             │
├─ Render Tool (wgpu) ────────────────────────────────────┤
│  GPU execution: draw commands → pixels                    │
└───────────────────────────────────────────────────────────┘
```

**Data flow:** Consumer builds Scene JSON → Core loads into ECS World → Pipeline runs 7 systems → Pixels.

---

## Components

Components are pure data. No logic. Two fundamental categories:

### Context Components — define WHEN (not animatable)

Context properties define time and structure. They are resolved before animation
and **cannot be animated** (circular dependency with local_time).

| Component | Fields | Purpose |
|-----------|--------|---------|
| **Lifespan** | `start: f64, end: f64` | Entity visibility window. `visible = time ∈ [start, end)` |
| **Composition** | `speed: f32, trim_start: f64, trim_end: Option<f64>, duration: DurationMode, loop_mode: LoopMode` | Turns entity into group with internal timeline. Children see `content_time` |
| **Camera** | `resolution_width/height: u32, bg_color: [f32;4], fov: f32` | Defines viewport projection. At least one camera per scene |

### Value Components — define WHAT (animatable)

Static default values. AnimationComponent keyframes override these at runtime.

| Component | Fields | Defaults |
|-----------|--------|----------|
| **Transform** | `x, y, rotation, anchor_x, anchor_y, scale_x, scale_y` | 0, 0, 0°, 0, 0, 1, 1 |
| **Rect** | `width, height, fit_mode` | 0, 0, Stretch |
| **Visual** | `opacity, volume, blend_mode` | 1.0, 1.0, Normal |

### Source Components — what to draw

At least one required for the entity to produce draw calls.

| Component | Fields |
|-----------|--------|
| **ShapeSource** | `kind: ShapeKind, fill_color: [f32;4], stroke_color: Option, stroke_width: f32` |
| **ColorSource** | `r, g, b, a` (solid fill) |
| **ImageSource** | `asset_id, intrinsic_width, intrinsic_height` |
| **VideoSource** | `asset_id, trim_start, intrinsic_width/height, duration, fps` |
| **TextSource** | `content, font, font_size, color, bold, italic` |
| **AudioSource** | `asset_id, trim_start, duration` (no visual) |

### Animation Component — keyframe tracks

```rust
struct AnimationComponent {
    float_tracks: Vec<FloatAnimTrack>,    // {target, keyframes}
    string_tracks: Vec<StringAnimTrack>,  // {target, keyframes}
}
```

**AnimTarget** — compile-time safe animatable property targets:

| Target | Type | Source |
|--------|------|--------|
| `TransformX/Y/Rotation/AnchorX/AnchorY/ScaleX/ScaleY` | float | Transform |
| `RectWidth/RectHeight` | float | Rect |
| `Opacity/Volume` | float | Visual |
| `BlendMode` | string | Visual |
| `ColorR/G/B/A` | float | ColorSource |
| `FloatUniform(name)` | float | Material |
| `StringUniform(name)` | string | Material |

Entity **without** AnimationComponent = fully static. Zero evaluation cost.

### Runtime Component — draw output (not serialized)

| Component | Fields | Lifetime |
|-----------|--------|----------|
| **DrawComponent** | `draw_calls: Vec<DrawCall>, material_chain: Vec<Material>, texture_requests: Vec<TextureRequest>` | Created fresh each frame. Never serialized to scene JSON |

`DrawCall` is the universal render primitive:

```rust
struct DrawCall {
    kind: DrawKind,              // SolidRect, SolidEllipse, Texture, Text, Outline, Gizmo, CameraFrame
    x, y, width, height: f32,   // world space (post-hierarchy)
    rotation: f32,               // radians
    anchor_x, anchor_y: f32,
    opacity: f32,
    blend_mode: BlendMode,
    color: [f32; 4],
    texture_key: Option<String>,
    shader: String,
    params: Vec<f32>,
    fit_mode: FitMode,
    intrinsic_width, intrinsic_height: f32,
}
```

### Meta

| Field | Type | Description |
|-------|------|-------------|
| `parent_id` | `Option<String>` | Hierarchy parent. Position/rotation/scale relative to parent |
| `layer` | `Option<i32>` | Z-order (higher = on top) |
| `materials` | `Vec<Material>` | Shader effect chain (blur, color_grade, ...) |

---

## Pipeline — 3 Phases, 7 Systems

```
┌─── PHASE 1: TIME ──────────────────────────────────────────┐
│  ① time_sys                                                 │
│     Queries: Lifespan, Composition, parent_id               │
│     Output: visible, local_time, content_time               │
├─── PHASE 2: RESOLVE ───────────────────────────────────────┤
│  ② animation_sys                                            │
│     Queries: Transform, Rect, Visual, ColorSource,          │
│              AnimationComponent                              │
│     Output: resolved.x/y/rot/scale/w/h/opacity/color/...   │
│                                                              │
│  ③ rect_sys                                                  │
│     Queries: resolved state, VideoSource/ImageSource         │
│     Output: resolved.width/height (base × scale)            │
│                                                              │
│  ④ hierarchy_sys                                             │
│     Queries: resolved state, parent_id                       │
│     Output: cascaded world-space resolved state              │
├─── PHASE 3: RENDER ────────────────────────────────────────┤
│  ⑤ source_sys                                                │
│     Queries: Shape/Color/Video/Image/TextSource, resolved    │
│     Output: DrawComponent on each entity                     │
│                                                              │
│  ⑥ editor_sys  (OPTIONAL — skip during export)              │
│     Queries: Camera, selection_state                         │
│     Output: additional DrawCalls (selection, gizmo, camera)  │
│                                                              │
│  ⑦ render_sys                                                │
│     Queries: DrawComponent, Camera, materials                │
│     Output: GPU draw → pixels on screen                      │
└──────────────────────────────────────────────────────────────┘
```

### ① time_sys

Resolves `local_time` for all entities in the time hierarchy.

**Phase 1 — Roots** (no parent, or parent without Composition):
- `current_time = global_time`
- Check lifespan → `visible`, `local_time = current_time - start`

**Phase 2 — Composition cascade** (top-down, parents before children):
- `content_time = local_time × speed + trim_start` (apply loop)
- For each child: `current_time = parent.content_time`, re-check lifespan

### ② animation_sys

1. Copy component defaults → resolved state
2. If AnimationComponent exists: evaluate each track at `local_time` → override

### ③ rect_sys

Size fallback: Rect value > intrinsic (video/image) > default 200×200.
Final: `width = base × scale_x`, `height = base × scale_y`.

### ④ hierarchy_sys

Parent→child cascade in world space:
- Position: `child_world = parent_pos + rotate(child_local × parent_scale, parent_rot)`
- Rotation: `child_rot += parent_rot`
- Scale: `child_scale *= parent_scale`
- Opacity: `child_opacity *= parent_opacity`

### ⑤ source_sys

Reads source components + resolved state → creates `DrawComponent` with `DrawCall`s.
Each source type maps to a `DrawKind` (SolidRect, Texture, Text, etc.).

### ⑥ editor_sys (optional)

Adds editor-only DrawCalls: selection outlines, resize/rotate gizmos, camera frame overlays.
**Skipped during export** — render output is clean.

### ⑦ render_sys

1. Collects all DrawCalls from all entities, sorts by layer
2. For each view/viewport: projects world-space → screen-space via camera
3. Handles material chains: render to intermediate RT → apply shader chain → composite
4. Submits render passes to GPU

**Multi-view:** Same DrawComponent data renders to multiple viewports at different scales.
Texture/video frame cache shared across views.

---

## Coordinate System

- **World units** — infinite 2D plane. Origin (0,0). X+ right, Y+ down.
- Entity `(x, y)` = position in world. Position is **relative to parent** (hierarchy_sys cascades).
- **Pixels** — computed at render time by `render_sys` using camera projection.
- `screen_pos = (world_pos - cam_pos) × (screen_size / cam_view_size) + screen_center`

---

## Scene JSON

```json
{
  "assets": {
    "bg_img": { "type": "image", "url": "https://..." }
  },
  "entities": [
    {
      "id": "cam",
      "camera": { "resolutionWidth": 1920, "resolutionHeight": 1080, "bgColor": [0,0,0,1] },
      "transform": { "x": 0, "y": 0 }
    },
    {
      "id": "box",
      "shapeSource": { "kind": "rectangle", "fillColor": [1,0,0,1] },
      "transform": { "x": 100, "y": 50, "scaleX": 1, "scaleY": 1 },
      "rect": { "width": 200, "height": 150 },
      "visual": { "opacity": 1 },
      "lifespan": { "start": 0, "end": 10 },
      "layer": 1,
      "animation": {
        "floatTracks": [
          { "target": "transform_x", "keyframes": [{"time":0,"value":0}, {"time":3,"value":500}] },
          { "target": "opacity", "keyframes": [{"time":0,"value":0}, {"time":1,"value":1}] }
        ]
      }
    },
    {
      "id": "child",
      "shapeSource": { "kind": "ellipse", "fillColor": [0,1,0,1] },
      "transform": { "x": 50, "y": 30 },
      "rect": { "width": 80, "height": 80 },
      "parentId": "box",
      "layer": 2
    }
  ]
}
```

---

## WASM API

| Method | Description |
|--------|-------------|
| `load_scene(json)` | Parse scene JSON → create ECS World |
| `tick(time, mode, cam_id, view_config)` | Run full pipeline → render to GPU |
| `resize(w, h)` | Resize GPU surface |
| `set_selection(ids)` | Set selected entity IDs for editor overlays |
| `cache_image(url, rgba, w, h)` | Inject decoded image |
| `cache_video_frame(url, rgba, w, h)` | Inject video frame |

**Modes:**
- `Mode::Editor` — runs editor_sys (selection, gizmo, camera frame)
- `Mode::Export` — skips editor_sys, renders clean output
