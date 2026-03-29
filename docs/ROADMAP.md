# ifol-render — Rebuild Roadmap V4

## Strategy

Bottom-up rebuild. Pure ECS — everything is a system, including rendering.
Each level is fully tested before proceeding.

**Pipeline:** `time_sys → animation_sys → rect_sys → hierarchy_sys → source_sys → render_sys`

---

## Level 0: Foundation ✅

**Goal:** Keyframe engine.

- FloatTrack::evaluate — linear, hold, cubic_bezier, bezier
- StringTrack::evaluate — step interpolation
- Lifespan::contains — time range check

**Status:** 23 unit tests passing.

---

## Level 1: Component Structs ✅

**Goal:** New pure-data component types.

**Deliverables:**

| Component | Type | Details |
|-----------|------|---------|
| `Transform` | Value | 7 × f32 fields (x, y, rotation, anchor_x/y, scale_x/y) |
| `Rect` | Value | width/height f32, fit_mode enum |
| `Visual` | Value | opacity/volume f32, blend_mode enum |
| `ShapeSource` | Source | kind (Rectangle/Ellipse), fill/stroke color |
| `AnimationComponent` | Animation | Vec\<FloatAnimTrack\>, Vec\<StringAnimTrack\> |
| `AnimTarget` | Enum | 16 fixed targets + 2 extensible (FloatUniform, StringUniform) |
| `DrawComponent` | Runtime | Vec\<DrawCall\> — not serialized |

**Status:** All structs compile, serde round-trip verified.

---

## Level 2: time_sys ✅

**Goal:** Unified time resolution (replaces composition_sys + timeline_sys + speed_sys).

**Logic:**
1. Root entities: `current_time = global_time` → lifespan check
2. Composition cascade top-down: `content_time → children's current_time`

**Status:** Working. Lifespan, composition speed, loop/pingpong/once modes functional.

---

## Level 3: animation_sys ✅

**Goal:** Centralized keyframe evaluation (replaces transform_sys + visual_sys).

**Logic:**
1. Copy component defaults → resolved
2. If AnimationComponent → evaluate tracks → override

**Interpolation types:**
- `linear` (default), `hold`, `cubic_bezier`, `bezier`

**Status:** Working. Keyframes support all 4 interpolation modes.

---

## Level 4: rect_sys ✅

**Goal:** Size resolution + scale multiply.

**Logic:** Rect > intrinsic > 200×200, then `× scale`.

**Status:** Working. Handles Rect, intrinsic (video/image), camera, and default fallbacks.

---

## Level 5: hierarchy_sys ✅

**Goal:** Parent→child spatial cascade with scale propagation.

**Logic:**
- `child_world_pos = parent_pos + rotate(child_local × parent_scale, parent_rot)`
- `child_rot += parent_rot` (radians)
- `child_scale *= parent_scale`
- **Width/height recomputed** to include parent scale
- `child_opacity *= parent_opacity`
- `child_layer += parent_layer`

**Status:** Working. Width/height recomputation after scale propagation fixed.

---

## Level 6: source_sys + render_sys ✅

**Goal:** Core render loop working. Shape entities render on screen.

**Deliverables:**
- `source_sys`: ShapeSource/ColorSource/VideoSource/ImageSource/TextSource → DrawComponent
- `render_sys`: collect DrawCalls → camera projection → FlatEntity list → GPU
- SDF shader pipeline for shapes (param layout: `[shape_type, corner_radius, border_width, pad]`)

**Status:** Working. Shapes, hierarchy, animations all render correctly on web.

> **🎯 MILESTONE: Core ECS loop rendering shapes on web.** ✅

---

## Level 7: Editor Interaction ✅

**Goal:** Interactive viewport with selection, drag, and camera overlays.

**Deliverables:**
- ✅ Hit-testing — visual-center AABB with inverse rotation (CSS→canvas coord conversion)
- ✅ Entity drag — parent-local inverse rotation + scale correction
- ✅ Selection overlay — Cyan wireframe via SDF shader (shape-aware: rect or ellipse)
- ✅ Camera wireframe — Magenta thin border overlay (layer 9999)
- ✅ Editor vs Camera view mode toggle
- ✅ Mouse mapping: left=select/drag, right=pan, scroll=zoom

**Status:** Working. All editor interactions functional with correct coordinate transforms.

> **🎯 MILESTONE: Interactive editor viewport with selection and drag.** ✅

---

## Level 7.5: Interpolation Architecture ✅

**Goal:** Universal easing system — extensible via CubicBezier primitive.

**Core Primitives (4 only):**
- `Hold` — constant until next keyframe
- `Linear` — straight line
- `CubicBezier{x1,y1,x2,y2}` — universal curve (covers all standard easings)
- `Bezier{out_x,out_y,in_x,in_y}` — AE-style tangent handles

**Design:** Core intentionally does NOT include named presets (ease-in, steps, spring).
All presets are defined at the app/frontend level as CubicBezier control points.
Complex curves (steps, spring) are achieved by generating multiple keyframes.

**Status:** Working. TC11 demonstrates 10 frontend-defined presets (hold, linear, ease_in, ease_out, ease_in_out, ease_in_quad, ease_out_quad, ease_in_out_cubic, ease_in_back, ease_out_back).

---

## Level 8a: Image Sources — IN PROGRESS ⬜→✅

**Goal:** Image asset loading + rendering with FitMode.

**Architecture:**
- App layer: `fetch(url)` → decode → `cache_image(url, rgba, w, h)` — ONE-TIME cost
- Core: `render_frame_v2` → `has_texture(url)?` skip : `load_rgba()` — GPU cached
- `source_sys`: ImageSource → DrawCall::Texture with `texture_key = resolved URL`
- FitMode contain: `apply_fit_mode()` shrinks DrawCall quad proportionally (transparent surplus)

**Status:**
- ✅ `cache_image(url, rgba, w, h)` WASM API — accepts pre-decoded RGBA
- ✅ GPU texture cache — zero per-frame decode cost, no export bottleneck
- ✅ JS `loadAndCacheAssets()` — async parallel image loading
- ✅ Auto-fill intrinsicWidth/Height from decoded image
- ✅ FitMode Contain in source_sys
- ⬜ FitMode Cover (UV crop in shader)
- ✅ TC12 — 3 fit modes side-by-side with `#cmt_0.png`

---

## Level 8b: Video/Text/Audio Sources — DONE ✅

**Goal:** Video playback, text rendering, audio sync.

**Deliverables:**
- ✅ Video sync architecture (WasmMediaManager: Zero-Waste Entity caching, Scrubbing snap)
- ✅ Audio playback sync (WasmAudioManager: Sync tolerance, cleanup)
- ✅ TextSource → DrawCall::Text (font rendering with continuous rasterization toggle)

**Test criteria:**
- ✅ Video scrub → correct frame (Frame-perfect snapping, no memory leaks)
- ✅ Text renders natively via WGPU ab_glyph handling correct resolution bounds and FitModes
- ✅ Audio in sync (250ms drift tolerance)

---

## Level 9: Materials — TODO

**Goal:** Shader effect chains.

**Deliverables:**
- Material chain in render_sys: base → intermediate RT → shader passes → composite
- Animation of material uniforms via AnimTarget::FloatUniform

**Test criteria:**
- Blur effect renders
- Effect chain (blur → color_grade) works
- Animated uniform changes over time

---

## Dependency Graph

```
L0 Foundation       ← keyframe engine                         ✅
L1 Components       ← needs L0 for Keyframe types             ✅
L2 time_sys         ← needs L1 for Lifespan/Composition       ✅
L3 animation_sys    ← needs L1 + L2 for AnimationComponent    ✅
L4 rect_sys         ← needs L3 for resolved base size         ✅
L5 hierarchy_sys    ← needs L3+L4 for resolved spatial state  ✅
L6 source+render    ← needs L5 for DrawComponent → GPU        ✅
L7 editor_sys       ← needs L6 for interactive viewport       ✅
L7.5 interpolation  ← needs L3 for CubicBezier primitive      ✅
L8a Image sources   ← needs L6 for asset management           ✅ (partial)
L8b Video/Text/Audio ← needs L8a for asset pipeline           ⬜
L9 Materials        ← needs L6 for intermediate RT            ⬜
```

---

## Bug Fix History

| Bug | Level | Root Cause | Fix |
|-----|-------|------------|-----|
| Hierarchy lacks scale propagation | L5 | `child.scale *= parent.scale` not applied | Added scale multiply + position scaled offset |
| Hierarchy width/height stale after scale | L5 | `rect_sys` computes size before `hierarchy_sys`, scale not reflected | Recompute `width = (width/old_scale) × new_scale` in hierarchy_sys |
| Rotation double-converted | L5,L6 | `resolved.rotation` is radians, but `source_sys`/`hierarchy_sys` called `.to_radians()` | Removed all `.to_radians()` calls. Pipeline is 100% radians |
| Selection overlay rotation mismatch | L7 | Overlay used `.to_radians()` while source_sys didn't (after fix) | Both use raw radians |
| SDF shader params misaligned | L7 | `params` had 3 floats but WGSL expects 4 | Padded to 4 floats: `[type, radius, border, pad]` |
| Hit-test anchor math inverted | L7 | AABB test computed around anchor instead of visual center | Compute visual center, then inverse-rotate test point around center |
| Hit-test CSS vs canvas pixels | Web | `e.clientX - rect.left` returns CSS pixels, WASM expects canvas pixels | Scale by `canvas.width / rect.width` |
| Drag skewed for child entities | L7 | World-space delta applied directly to parent-local coordinates | Inverse-rotate delta through parent rotation, divide by parent scale |
| Drag parent chain double-counting | L7 | Walking ancestors added already-accumulated rotation | Use immediate parent's `resolved` state (includes all ancestors) |
| Color animation via uniforms | L3 | Float uniforms hack for color channels | Added AnimTarget::ColorR/G/B/A |
