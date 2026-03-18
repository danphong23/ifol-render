# Architecture

## Overview

ifol-render is a modular GPU rendering engine organized as a Rust workspace with multiple crates. The architecture follows the **Entity-Component-System (ECS)** pattern for scene management and a **pipeline-based** approach for rendering and export.

```
┌─────────────────────────────────────────────────────────┐
│                      Consumers                          │
│   ┌─────────┐  ┌──────────┐  ┌──────┐  ┌──────────┐   │
│   │ Studio  │  │   CLI    │  │ WASM │  │ Your App │   │
│   └────┬────┘  └─────┬────┘  └──┬───┘  └─────┬────┘   │
│        │             │          │             │         │
├────────┴─────────────┴──────────┴─────────────┴────────┤
│                    ifol-render-core                      │
│  ┌──────────┐  ┌────────────┐  ┌──────────────────┐    │
│  │   ECS    │  │  Commands  │  │    Scene I/O     │    │
│  │ World    │  │  History   │  │ SceneDescription │    │
│  │ Entity   │  │  Undo/Redo │  │ JSON serialize   │    │
│  │ Systems  │  │            │  │                  │    │
│  └──────────┘  └────────────┘  └──────────────────┘    │
│  ┌──────────┐  ┌────────────┐  ┌──────────────────┐    │
│  │  Color   │  │  Animation │  │   Export (FFmpeg) │    │
│  │  Spaces  │  │  Keyframes │  │   H264/VP9/ProRes│    │
│  │  Convert │  │  Easing    │  │   Progress CB    │    │
│  └──────────┘  └────────────┘  └──────────────────┘    │
├─────────────────────────────────────────────────────────┤
│                    ifol-render (GPU)                     │
│  ┌──────────────┐  ┌───────────┐  ┌───────────────┐    │
│  │ Render Graph │  │  Passes   │  │   Shaders     │    │
│  │ DAG executor │  │  Composite│  │   WGSL files  │    │
│  │ Auto-deps    │  │  Effects  │  │   Runtime load│    │
│  └──────────────┘  └───────────┘  └───────────────┘    │
└─────────────────────────────────────────────────────────┘
```

## Crate Structure

### `core/` — ifol-render-core

The heart of the engine. Zero GPU dependencies — all CPU-side logic.

| Module | Purpose |
|--------|---------|
| `ecs/` | Entity, Components, World, Systems, Pipeline |
| `ecs/components.rs` | All component types (Transform, Timeline, ColorSource, etc.) |
| `ecs/systems.rs` | Per-frame systems (visibility, animation, transform, opacity) |
| `ecs/pipeline.rs` | Frame rendering pipeline orchestrator |
| `ecs/draw.rs` | Software rasterizer for compositing |
| `commands/` | Command pattern for undo/redo (AddEntity, RemoveEntity, SetProperty) |
| `scene.rs` | SceneDescription + RenderSettings (JSON ↔ World round-trip) |
| `color.rs` | Color4, ColorSpace, conversion matrices |
| `types.rs` | Vec2, Mat4, Keyframe, Easing |
| `time.rs` | TimeState, EntityTime |
| `export/` | FFmpeg pipe, ExportConfig, video export with progress |

### `render/` — ifol-render

GPU rendering engine built on wgpu.

| Module | Purpose |
|--------|---------|
| `render_graph.rs` | DAG of render passes with dependency tracking |
| `passes/` | Individual render passes (composite, effects) |
| `shaders/` | WGSL shader loading and compilation |

### `studio/` — ifol-render-studio

Professional GUI editor built with egui + egui_tiles.

| Module | Purpose |
|--------|---------|
| `app.rs` | Main application state (EditorApp) |
| `panels/viewport.rs` | Real-time viewport with grid, safe zones |
| `panels/timeline.rs` | NLE-style timeline with track lanes |
| `panels/entity_list.rs` | Entity browser with multi-select |
| `panels/properties.rs` | Property inspector with undo support |
| `panels/top_bar.rs` | 3-zone flex top bar (brand, workspace, actions) |
| `panels/status_bar.rs` | Status bar with entity count |
| `panels/workspace.rs` | egui_tiles workspace with split/tab support |

### `crates/cli/` — ifol-render-cli

Headless CLI tool for rendering and export.

| Subcommand | Purpose |
|------------|---------|
| `info` | Display scene metadata |
| `preview` | Render single frame to PNG |
| `export` | Export video via FFmpeg |

### `crates/wasm/` — ifol-render-wasm

WebAssembly target for browser-based preview.

---

## ECS Pipeline

Each frame follows this pipeline:

```
1. Visibility System     → determines which entities are active at current time
2. Animation System      → evaluates keyframes, applies animated values
3. Transform System      → computes world matrices (with parent-child hierarchy)
4. Opacity System        → resolves final opacity per entity
5. Draw/Composite        → software rasterizer composites visible layers
```

### Parent-Child Hierarchy

Entities can reference a parent via the `parent` component field. The transform system resolves the hierarchy using matrix multiplication (`Mat4::mul`), ensuring children inherit parent transforms.

### Animation & Easing

Keyframes support multiple easing functions:
- `linear` — constant rate
- `easeIn` / `easeOut` / `easeInOut` — cubic bezier presets
- `cubicBezier: [x1, y1, x2, y2]` — custom cubic bezier (Newton-Raphson solver)

---

## Command System

All mutations go through the Command pattern for undo/redo:

```
User Action → Command::execute() → World mutation
                                  → History push
Ctrl+Z      → Command::undo()   → Reverse mutation
Ctrl+Y      → Command::redo()   → Re-apply mutation
```

Commands: `AddEntity`, `RemoveEntity`, `SetProperty`

---

## Export Pipeline

```
SceneDescription → render_frame() loop → RGBA pixels → FFmpeg stdin pipe → video file
                     ↑                                        ↓
               progress callback                    codec (H264/VP9/ProRes)
```

The export system supports configurable FFmpeg path (`--ffmpeg /path/to/ffmpeg`).
