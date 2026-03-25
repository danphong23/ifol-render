# ifol-render — Rebuild Roadmap V4

## Strategy

Bottom-up rebuild. Pure ECS — everything is a system, including rendering.
Each level is fully tested before proceeding.

**Pipeline:** `time_sys → animation_sys → rect_sys → hierarchy_sys → source_sys → [editor_sys] → render_sys`

---

## Level 0: Foundation ✅

**Goal:** Keyframe engine.

- FloatTrack::evaluate — linear, hold, cubic_bezier, bezier
- StringTrack::evaluate — step interpolation
- Lifespan::contains — time range check

**Status:** 23 unit tests passing.

---

## Level 1: Component Structs

**Goal:** New pure-data component types.

**Deliverables:**

| Component | Type | Details |
|-----------|------|---------|
| `Transform` | Value | 7 × f32 fields (no FloatTrack) |
| `Rect` | Value | width/height f32, fit_mode enum |
| `Visual` | Value | opacity/volume f32, blend_mode enum |
| `ShapeSource` | Source | kind (Rectangle/Ellipse), fill/stroke color |
| `AnimationComponent` | Animation | Vec\<FloatAnimTrack\>, Vec\<StringAnimTrack\> |
| `AnimTarget` | Enum | 16 fixed targets + 2 extensible |
| `DrawComponent` | Runtime | Vec\<DrawCall\> — not serialized |
| `DrawCall` | Runtime | Universal render primitive (kind, transform, visual, content) |

**Test criteria:** Struct creation, default values, serde round-trip.

---

## Level 2: time_sys

**Goal:** Unified time resolution (replaces composition_sys + timeline_sys + speed_sys).

**Logic:**
1. Root entities: `current_time = global_time` → lifespan check
2. Composition cascade top-down: `content_time → children's current_time`

**Test criteria:**
- Root entity ± lifespan → correct visibility and local_time
- Composition child → local_time from content_time
- Nested 3-level composition → correct cascade
- Loop / PingPong / Once modes
- Entity without lifespan → always visible

---

## Level 3: animation_sys

**Goal:** Centralized keyframe evaluation (replaces transform_sys + visual_sys).

**Logic:**
1. Copy component defaults → resolved
2. If AnimationComponent → evaluate tracks → override

**Test criteria:**
- Static entity (no animation) → defaults only
- Animated transform x/y → values change with time
- Animated opacity → fade
- No AnimationComponent → zero cost path

---

## Level 4: rect_sys

**Goal:** Size resolution + scale multiply.

**Logic:** Rect > intrinsic > 200×200, then `× scale`.

**Test criteria:**
- Entity with Rect → uses rect
- Entity with ImageSource, no Rect → uses intrinsic
- Entity with nothing → 200×200
- Scale multiply correct
- Fit mode (stretch/contain/cover)

---

## Level 5: hierarchy_sys

**Goal:** Parent→child spatial cascade with scale propagation.

**Logic:**
- `child_world_pos = parent_pos + rotate(child_local × parent_scale, parent_rot)`
- `child_rot += parent_rot`
- `child_scale *= parent_scale` **(BUG FIX)**
- `child_opacity *= parent_opacity`

**Test criteria:**
- Parent move → child follows
- Parent rotate → child orbits
- Parent scale → child scales (position + visual)
- Opacity cascade
- 3+ level hierarchy

---

## Level 6: source_sys + render_sys (Shape only)

**Goal:** Core render loop working. Shape entities render on screen.

**Deliverables:**
- `source_sys`: ShapeSource → DrawComponent with DrawCall
- `render_sys`: collect DrawCalls → camera projection → GPU render
- Multi-view support (editor viewport + potential preview)

**Test criteria:**
- ShapeSource rectangle renders as colored rect
- ShapeSource ellipse renders as colored ellipse
- Multiple entities layer correctly
- Camera at offset → entities shift
- WASM build + web test page works

> **🎯 MILESTONE: Core ECS loop rendering shapes on web.**

---

## Level 7: editor_sys

**Goal:** Selection, gizmo, camera overlays.

**Deliverables:**
- Selection outline as DrawCall::Outline
- Resize/rotate gizmo as DrawCall::Gizmo
- Camera boundary as DrawCall::CameraFrame
- Hit-test for click selection

**Test criteria:**
- Click → correct entity selected
- Selection outline renders
- Gizmo handles visible
- Editor mode vs export mode (skip editor_sys)

---

## Level 8: Asset Sources

**Goal:** Image, video, text, audio.

**Deliverables:**
- image_sys: ImageSource → DrawCall::Texture
- video_sys: VideoSource → DrawCall::Texture + frame extraction request
- text_sys: TextSource → DrawCall::Text
- Audio playback sync

**Test criteria:**
- Image loads + displays at correct size/position
- Video scrub → correct frame
- Text renders with font/size/color
- Audio in sync

---

## Level 9: Materials

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
L0 Foundation       ← keyframe engine
L1 Components       ← needs L0 for Keyframe types
L2 time_sys         ← needs L1 for Lifespan/Composition
L3 animation_sys    ← needs L1 + L2 for AnimationComponent + local_time
L4 rect_sys         ← needs L3 for resolved base size
L5 hierarchy_sys    ← needs L3+L4 for resolved spatial state
L6 source+render    ← needs L5 for world-space resolved → DrawComponent → GPU
L7 editor_sys       ← needs L6 for render loop
L8 Asset sources    ← needs L6 for render loop + asset management
L9 Materials        ← needs L6 for intermediate render targets
```

---

## Known Bug Fixes

| Bug | Level | Fix |
|-----|-------|-----|
| Hierarchy lacks scale propagation | L5 | `child.scale *= parent.scale` + scaled position offset |
| Rotation-aware drag broken | L7 | Inverse-rotate mouse delta in editor_sys |
| Color animation workaround | L3 | AnimTarget::ColorR/G/B/A replaces float_uniforms hack |
| Camera mode inconsistency | L6 | render_sys uses ViewConfig for editor vs export |
