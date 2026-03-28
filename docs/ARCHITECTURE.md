# ifol-render — ECS Architecture V4

## Overview

ifol-render is a pure ECS (Entity-Component-System) video/motion graphics engine.
**Everything is a System** — including rendering.

```
┌─ Consumer (Web/Studio/CLI) ─────────────────────────────┐
│  Scene editing, asset loading, orchestration              │
├─ Core Engine (Rust/WASM) ───────────────────────────────┤
│  ECS World → Pipeline (6 systems) → Frame → GPU draw     │
├─ Render Backend (wgpu) ─────────────────────────────────┤
│  GPU execution: FlatEntity list → pixels                  │
└───────────────────────────────────────────────────────────┘
```

**Data flow:** Consumer builds Scene JSON → Core loads into ECS World → Pipeline runs 6 systems → `render_sys` compiles Frame → Backend renders pixels.

---

## Coordinate System & Units

- **World units** — infinite 2D plane. Origin (0,0). X+ right, Y+ down.
- All positions, sizes, and offsets are in **world units** throughout the ECS.
- **Rotation** is in **radians** everywhere. JSON values, `resolved.rotation`, DrawCall.rotation — all radians.
  - Example: `"rotation": 0.5` means 0.5 radians ≈ 28.6°
  - A full rotation = `6.2832` (2π)
- **Anchor** `(anchor_x, anchor_y)` is normalized 0–1. `(0,0)` = top-left, `(0.5,0.5)` = center.
- Entity `(x, y)` = the **anchor point position** in parent-local space (cascaded by `hierarchy_sys`).
- **Pixels** are only computed at render time by `render_sys`:
  ```
  screen_pos = (world_pos - cam_pos) × (screen_pixels / cam_view_size)
  ```

---

## Components

Components are pure data. No logic. Three categories:

### Context Components — define WHEN (not animatable)

| Component | Fields | Purpose |
|-----------|--------|---------|
| **Lifespan** | `start: f64, end: f64` | Entity visibility window. `visible = time ∈ [start, end)` |
| **Composition** | `speed, trim_start, duration: DurationMode, loop_mode: LoopMode` | Turns entity into group with internal timeline |
| **Camera** | `post_effects: Vec` | Defines viewport projection. At least one camera per scene |

### Value Components — define WHAT (animatable)

Static default values. AnimationComponent keyframes override these at runtime.

| Component | Fields | Defaults |
|-----------|--------|----------|
| **Transform** | `x, y, rotation, anchor_x, anchor_y, scale_x, scale_y` | 0, 0, 0, 0, 0, 1, 1 |
| **Rect** | `width, height, fit_mode` | 0, 0, Stretch |
| **Visual** | `opacity, volume, blend_mode` | 1.0, 1.0, Normal |

### Source Components — what to draw

At least one required for the entity to produce draw calls.

| Component | Fields |
|-----------|--------|
| **ShapeSource** | `kind: ShapeKind (Rectangle/Ellipse), fill_color: [f32;4], stroke_color: Option, stroke_width` |
| **ColorSource** | `r, g, b, a` (solid fill) |
| **ImageSource** | `asset_id, intrinsic_width, intrinsic_height` |
| **VideoSource** | `asset_id, trim_start, intrinsic_width/height, duration, fps` |
| **TextSource** | `content, font, font_size, color, bold, italic` |
| **AudioSource** | `asset_id, trim_start, duration` (no visual) |

### Animation Component — keyframe tracks

```rust
struct AnimationComponent {
    float_tracks: Vec<FloatAnimTrack>,    // {target: AnimTarget, track: FloatTrack}
    string_tracks: Vec<StringAnimTrack>,  // {target: AnimTarget, track: StringTrack}
}
```

**AnimTarget** — compile-time safe animatable property targets:

| Target | Type | Source |
|--------|------|--------|
| `TransformX/Y/Rotation/AnchorX/AnchorY/ScaleX/ScaleY` | float | Transform |
| `RectWidth/RectHeight` | float | Rect |
| `Opacity/Volume` | float | Visual |
| `PlaybackTime` | float | VideoSource/AudioSource |
| `BlendMode` | string | Visual |
| `ColorR/G/B/A` | float | ColorSource |
| `FloatUniform(name)` | float | Material |
| `StringUniform(name)` | string | Material |

**Keyframe Interpolation** — 4 core primitives only. All presets (ease-in, ease-out, spring, steps) are defined at app/frontend level by mapping to `cubic_bezier` control points:

| Type | Description | JSON |
|------|-------------|------|
| `linear` | Straight line (default) | `{"type": "linear"}` |
| `hold` | Constant until next keyframe | `{"type": "hold"}` |
| `cubic_bezier` | Universal timing curve | `{"type": "cubic_bezier", "x1": 0.42, "y1": 0, "x2": 0.58, "y2": 1}` |
| `bezier` | AE-style tangent handles | `{"type": "bezier", "outX": 0.33, "outY": 0, "inX": 0.33, "inY": 0}` |

> **Design:** Core does NOT include named presets. `cubic_bezier` covers all standard curves:
> ease-in = `(0.42, 0, 1, 1)`, ease-out = `(0, 0, 0.58, 1)`, etc.
> Complex curves (steps, spring) are achieved by generating multiple keyframes at app level.

Entity **without** AnimationComponent = fully static. Zero evaluation cost.

### Runtime Component — draw output (not serialized)

| Component | Fields | Lifetime |
|-----------|--------|----------|
| **DrawComponent** | `draw_calls: Vec<DrawCall>, texture_requests: Vec<TextureRequest>` | Created fresh each frame by `source_sys`. Never serialized |

`DrawCall` is the universal render primitive:

```rust
struct DrawCall {
    kind: DrawKind,              // SolidRect, SolidEllipse, Texture, Text, Outline, Gizmo, CameraFrame
    x, y, width, height: f32,   // world units
    rotation: f32,               // radians (raw from resolved, NOT converted)
    anchor_x, anchor_y: f32,
    opacity: f32,
    blend_mode: String,
    color: [f32; 4],
    texture_key: Option<String>,
    params: Vec<f32>,            // [shape_type, corner_radius, border_width, pad]
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

## Pipeline — 3 Phases, 6 Systems

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
│     Output: resolved.width/height = base × scale            │
│                                                              │
│  ④ hierarchy_sys                                             │
│     Queries: resolved state, parent_id                       │
│     Output: cascaded world-space resolved state              │
│             (position, rotation, scale, width/height, opacity)│
├─── PHASE 3: RENDER ────────────────────────────────────────┤
│  ⑤ source_sys                                                │
│     Queries: Shape/Color/Video/Image/TextSource, resolved    │
│     Output: DrawComponent on each entity                     │
│                                                              │
│  ⑥ render_sys                                                │
│     Queries: DrawComponent, Camera, editor state             │
│     Output: Frame (FlatEntity list) → GPU pipeline           │
│     Also: injects camera wireframe + selection overlays      │
│            when in editor mode                               │
└──────────────────────────────────────────────────────────────┘
```

### ① time_sys

Resolves `local_time` for all entities in the time hierarchy.

**Phase 1 — Roots** (no parent, or parent without Composition):
- `current_time = global_time`
- Check lifespan → `visible`, `local_time = current_time - start`
- *(Note: `time_sys` DOES NOT compute media `playback_time`. It defaults to `0.0` here).*

**Phase 2 — Composition cascade** (top-down, parents before children):
- `content_time = local_time × speed + trim_start` (apply loop)
- For each child: `current_time = parent.content_time`, re-check lifespan

### ② animation_sys

1. Copy component defaults → resolved state
2. If AnimationComponent exists: evaluate each track at `local_time` → override resolved
   - **Important:** Media `playback_time` is driven strictly by an `AnimationComponent` targeting `PlaybackTime`. Evaluated linear curves automatically spin the media, keeping Core fully data-driven.

### ③ rect_sys

Size fallback: Rect value > intrinsic (video/image) > default 200×200.
Final: `width = base × scale_x`, `height = base × scale_y`.

### ④ hierarchy_sys

Parent→child cascade in world space (requires topological order):
- Position: `child_world = parent_pos + rotate(child_local × parent_scale, parent_rot)`
- Rotation: `child_rot += parent_rot` (additive, in radians)
- Scale: `child_scale *= parent_scale`
- **Width/Height: recomputed** after scale propagation (`width = base_w × composite_scale`)
- Opacity: `child_opacity *= parent_opacity`
- Layer: `child_layer += parent_layer`

### ⑤ source_sys

Reads source components + resolved state → creates `DrawComponent` with `DrawCall`s.
Each source type maps to a `DrawKind` (SolidRect, SolidEllipse, Texture, Text).

**FitMode** (`apply_fit_mode`) — applied to Texture DrawCalls (image/video):
- **Stretch** (default): texture fills DrawCall w/h exactly, may distort
- **Contain**: shrinks DrawCall w/h to fit image proportionally within Rect. Surplus area = transparent (no black fill)
- **Cover**: keeps Rect size, image scaled to cover entirely, edges cropped (UV crop in shader)

**Important:** `DrawCall.rotation` is copied directly from `resolved.rotation` — NO `.to_radians()` conversion. The entire pipeline uses radians.

Shape params layout: `[shape_type, corner_radius, border_width, pad]`
- shape_type: 0=rectangle, 3=ellipse

### ⑥ render_sys

1. Collects all DrawCalls from all entities, sorted by layer
2. **Editor mode overlays** (injected inline, not a separate system):
   - Camera entities → Magenta wireframe DrawCall (border-only via SDF shader)
   - Selected entities → Cyan wireframe DrawCall (shape-aware: rect or ellipse)
3. Projects world units → screen pixels via camera
4. Outputs `Frame` with `FlatEntity` list for GPU backend

**Camera projection:**
```
sx = screen_width / cam_view_width
sy = screen_height / cam_view_height
center_x = (entity.x - cam_x) × sx + anchor_offset_rotated
flat_x = center_x - pixel_width / 2
```

---

## Composition Scoped Playback

Compositions act as sub-timelines with **independent local time**:

```
World time → Composition(speed, trimStart, loop, duration) → content_time
  → children use content_time as their current_time
```

When an editor "enters" a composition:
1. `set_render_scope(entity_id)` — only render this composition's children
2. `set_scope_time(local_time)` — directly control composition's internal time (bypasses speed/loop/trim)
3. Children are evaluated using `scope_time_override` instead of `content_time`

**Composition properties:**
| Field | Type | Description |
|-------|------|-------------|
| `speed` | f64 | Playback speed multiplier (1.0 = normal) |
| `trim_start` | f64 | Start offset within composition |
| `duration` | DurationMode | `Fixed(secs)` or `Auto` |
| `loop_mode` | LoopMode | `None`, `Loop`, `PingPong` |

**Nested compositions:** Full 3-level support — compositions can contain other compositions, each with independent speed/loop/trim.

---

## Media Playback Architecture

Video and Audio are treated as strictly data-driven entities. The Core engine does **not** rely on implicit "magic" formulas to play videos. 

**The Drop-In Workflow:**
When a Video or Audio asset is imported, the App/SDK generates an Entity with:
1. `VideoSource`/`AudioSource` Component.
2. `Lifespan`: `[0, duration]`.
3. `AnimationComponent`: A `FloatTrack` targeting `PlaybackTime` with two linear keyframes from `(time: 0.0, value: 0.0)` to `(time: duration, value: duration)`.

**Composition Wrapping (Null Objects):**
To keep the timeline UX clean, media entities are typically nested inside a parent `Composition` entity. 
- The Parent Composition acts as a **Null Object**: It possesses a `Transform` and `Lifespan` to dictate spatial layout and overall visibility, but it does **not** possess a `Rect` (no width/height) or a `VideoSource`. 
- By splitting `Rect` from `Transform`, the parent Composition can translate, scale, and rotate its children locally without incurring the overhead of `rect_sys` bounding calculations or rendering actual pixels.
- If a user wishes to trim or loop the video clip in the UI, they simply interact with the parent `Composition` wrapper, and `time_sys` automatically ripples the modified `content_time` down to the child video track.

---

## Scene JSON

```json
{
  "assets": {
    "bg_img": { "image": { "url": "./hero.png" } },
    "clip1":  { "video": { "url": "asset://intro.mp4" } }
  },
  "entities": [
    {
      "id": "main_cam",
      "camera": { "postEffects": [] },
      "rect": { "width": 1280, "height": 720 },
      "transform": { "x": 0, "y": 0, "rotation": 0, "scaleX": 1, "scaleY": 1, "anchorX": 0, "anchorY": 0 },
      "lifespan": { "start": 0, "end": 100 }
    },
    {
      "id": "photo",
      "imageSource": { "assetId": "bg_img", "intrinsicWidth": 800, "intrinsicHeight": 600 },
      "rect": { "width": 400, "height": 300, "fitMode": "contain" },
      "transform": { "x": 640, "y": 360, "rotation": 0, "scaleX": 1, "scaleY": 1, "anchorX": 0.5, "anchorY": 0.5 },
      "lifespan": { "start": 0, "end": 10 },
      "layer": 1
    },
    {
      "id": "box",
      "shapeSource": { "kind": "rectangle", "fillColor": [1, 0, 0, 1] },
      "transform": { "x": 100, "y": 50, "rotation": 0.5, "scaleX": 1, "scaleY": 1, "anchorX": 0.5, "anchorY": 0.5 },
      "rect": { "width": 200, "height": 150 },
      "lifespan": { "start": 0, "end": 10 },
      "layer": 2,
      "animation": {
        "floatTracks": [
          { "target": "transformX", "track": { "keyframes": [
            { "time": 0, "value": 0 },
            { "time": 3, "value": 500, "interpolation": { "type": "cubic_bezier", "x1": 0.42, "y1": 0, "x2": 0.58, "y2": 1 } }
          ]}}
        ]
      }
    }
  ]
}
```

> **Note:** `rotation` values in JSON are in **radians**. `0.5` = 0.5 rad ≈ 28.6°.
> **Assets:** `url` is an abstract identifier resolved by the app layer. Core does not fetch.

---

## WASM API

### Core

| Method | Description |
|--------|-------------|
| `new(canvas, width, height, fps)` | Create engine instance bound to a canvas element |
| `load_scene_v2(json)` | Parse scene JSON → create ECS World |
| `render_frame_v2(time, cam_id, editor, cam_x?, cam_y?, cam_w?, cam_h?)` | Run full pipeline → render to GPU |
| `set_render_scope(entity_id)` | Scope render to a composition entity (show only its children) |
| `set_scope_time(time?)` | Override local time for scoped composition (bypasses speed/loop/trim) |

### Editor Interaction

| Method | Description |
|--------|-------------|
| `pick_entity_v2(screen_x, screen_y, cam_id, cam_x?, cam_y?, cam_w?, cam_h?)` | Hit-test: returns entity ID at screen position |
| `drag_entity_v2(entity_id, screen_dx, screen_dy, cam_id, cam_w?, cam_h?)` | Translate entity by screen delta |
| `select_entity_v2(entity_id)` | Set selection state for editor overlays |

### Asset Cache

| Method | Description |
|--------|-------------|
| `cache_image(url, rgba_data, width, height)` | Inject pre-decoded RGBA pixels for an image asset. App decodes, Core receives raw pixels |
| `cache_video_frame(url, timestamp, rgba_data, w, h)` | Inject decoded video frame RGBA |
| `clear_video_frames()` | Clear all cached video frames from WASM memory |
| `evict_texture(key)` | Remove a specific texture from GPU cache |

### Asset Pipeline

```
Scene JSON: assets."img1".image.url = "./hero.png"
         ↓ asset_id
App Resolver (environment-specific):
  Web:    fetch(url) → ImageBitmap → RGBA → cache_image(url, rgba, w, h)
  Server: fs.read(path) → image::decode → RGBA → cache_image(url, rgba, w, h)
         ↓ RGBA bytes
Core (agnostic):
  render_frame_v2 → has_texture(url)? skip : load_rgba(url, rgba, w, h)
  GPU texture cached → reused across ALL frames. Zero per-frame cost.
```

**Key design:** Core NEVER fetches or decodes assets. App layer is the sole resolver.
`url` in assets map is an abstract identifier — app decides what it means.

### Web Integration Notes

- **Screen coordinates** passed to `pick_entity_v2` must be in **canvas pixel space**, not CSS pixels
- Convert: `canvasX = cssX × (canvas.width / canvas.getBoundingClientRect().width)`
- **Special characters in URLs:** `#` must be encoded as `%23` for HTTP fetch
- **Mouse mapping:** Left-click = select/drag entity, Right-click = pan viewport, Scroll = zoom

---

## Editor Mode

When `editor=true` in `tick_v2`, `render_sys` injects additional overlays:

### Camera Wireframe
- Every camera entity gets a Magenta border-only DrawCall
- SDF shader params: `[0, 0, 0.012, 0]` (rect, no corner radius, thin border)
- Rendered at layer 9999 (always on top)

### Selection Highlight
- Selected entities get a Cyan border-only DrawCall
- Shape-aware: matches the entity's `ShapeKind` (Rectangle → rect outline, Ellipse → ellipse outline)
- SDF shader params: `[shape_type, 0, 0.015, 0]`

### Hit-Testing Algorithm
```
1. Convert screen coords → world coords via camera
2. For each entity (top layer first):
   a. Compute visual center = anchor_pos + rotate((0.5-ax)*w, (0.5-ay)*h)
   b. Inverse-rotate test point around center
   c. Check |local_x| ≤ w/2 && |local_y| ≤ h/2
3. Return first hit
```

### Drag Algorithm
```
1. Convert screen delta → world delta via camera scale
2. If entity has parent:
   a. Read parent.resolved (already contains full ancestor chain)
   b. Inverse-rotate world delta by parent rotation
   c. Divide by parent scale
3. Apply local delta to entity.components.transform.x/y
```
