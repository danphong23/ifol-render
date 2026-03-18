# ifol-render-studio — Professional GUI Editor

## Role

Third-party consumer of `ifol-render-core`. The studio **does NOT know about the render crate** — it calls core's pipeline API which internally uses the GPU renderer. All GUI, user interaction, and workspace management lives here.

```
User ──> Studio (egui GUI) ──> Core (ECS API) ──> Render (GPU)
                 │
          ┌──────┴──────┐
          │  EditorApp   │
          │  ├── World   │
          │  ├── Time    │
          │  ├── Camera  │
          │  ├── History │
          │  └── Workspace│
          └──────────────┘
```

## Architecture

```
studio/
├── src/
│   ├── main.rs                 Entry point (eframe)
│   ├── app.rs                  EditorApp state, theme colors, update loop
│   └── panels/
│       ├── mod.rs              Panel registry, WorkspaceLayout
│       ├── workspace.rs        egui_tiles workspace (split/tab/close)
│       ├── viewport.rs         Real-time preview, grid, safe zones
│       ├── timeline.rs         NLE timeline with track headers
│       ├── entity_list.rs      Hierarchy tree (add/delete/select)
│       ├── properties.rs       Property inspector (collapsible sections)
│       ├── top_bar.rs          3-zone flex top bar (brand, info, actions)
│       └── status_bar.rs       Status bar (entity count, FPS)
├── docs/
│   └── STUDIO.md               ← You are here
└── Cargo.toml
```

## EditorApp (Central State)

| Field | Purpose |
|-------|---------|
| `world` | ECS World (all entities) |
| `settings` | RenderSettings (resolution, FPS, duration) |
| `time` | TimeState (current time, playing state) |
| `camera` | Viewport Camera (pan, zoom, rotation) |
| `commands` | CommandHistory (undo/redo stack) |
| `workspace` | WorkspaceLayout (egui_tiles split/tab layout) |
| `selected` | Currently selected entity index |
| `selected_indices` | Multi-selection set |
| `dirty` | Unsaved changes indicator |
| `needs_render` | Viewport needs re-render |
| `scene_path` | Current file path |
| `ffmpeg_path` | FFmpeg binary location |
| `collapsed_sections` | Collapsed property panels |
| `expanded_entities` | Expanded hierarchy nodes |

## Panels

### Viewport (`viewport.rs`)
- Renders `pixels` as egui texture
- Grid overlay (rule-of-thirds)
- Safe zones (action 90%, title 80%)
- Resolution badge
- **Planned**: Camera pan/zoom, selection gizmos

### Hierarchy / Entity List (`entity_list.rs`)
- Tree view with depth indentation
- Type icons: 🎨 Color, 🖼 Image, 📝 Text, ▶ Video
- Expand/collapse arrows ▶/▼ for parent entities
- Visibility 👁, Lock 🔒, Mute 🔇 toggle icons
- Add menu: Color Solid, Image Layer, Text Layer
- Multi-select: Ctrl+Click (toggle), Shift+Click (range)
- Batch delete

### Properties (`properties.rs`)
- Collapsible sections: Transform, Appearance, Timeline, Source
- Section headers: click ▶/▼ to expand/collapse (full-width hitbox)
- Transform: Position X/Y, Scale X/Y, Rotation, Z-Index
- Appearance: Opacity slider, BlendMode dropdown (7 modes), Visible checkbox, Color picker
- Timeline: Start time, Duration, Layer, Locked, Muted checkboxes
- Source: File path display (read-only)
- Entity name editor in header
- All edits create undo entries via SetProperty command

### Timeline (`timeline.rs`)
- Transport controls: ⏮ ⏸/▶ ⏭, time display, scrub slider
- 120px track header area:
  - Color indicator bar (3px left edge)
  - Entity display name
  - Eye 👁 and Lock 🔒 status icons
- Track lanes with clip rectangles:
  - Color-coded by entity type
  - Muted clips rendered at 30% opacity
  - Selected clip: white border stroke
  - Clip label inside rectangle
- Playhead: red line + pentagon handle
- Click ruler to seek, drag to scrub
- Click clip to select entity
- Zoom slider (0.3×–4.0× logarithmic)

### Top Bar (`top_bar.rs`)
- 3-zone layout (like professional editors):
  - Left: Brand + scene name
  - Center: Resolution badge + "Compositing" label
  - Right: Play button + actions (undo/redo/save/workspace)
- File menu: New Scene, Open, Save, Quick Save
- Overflow menu (⋮): Export Video, FFmpeg path editor + Browse
- Ctrl+S shortcut
- Dirty indicator (● in title)

### Workspace (`workspace.rs`)
- egui_tiles-based dockable layout
- 5 editor types: Viewport, Timeline, EntityList, Properties, Empty
- Split horizontally/vertically via context menu
- Close panel via ✕ button
- Editor type switcher dropdown

## Theme

Dark professional theme matching video compositing tools:

| Token | Hex | Usage |
|-------|-----|-------|
| `BG_APP` | `#18191C` | Window background |
| `BG_PANEL` | `#242529` | Panel backgrounds |
| `BG_SURFACE` | `#2A2C32` | Elevated surfaces |
| `BG_HOVER` | `#373A42` | Hover state |
| `BORDER` | `#303031` | Borders, dividers |
| `TEXT_PRIMARY` | `#E0E0E0` | Main text |
| `TEXT_DIM` | `#828796` | Labels, secondary text |
| `ACCENT` | `#5865F2` | Selection, active state |
| `RED` | `#ED4245` | Playhead, delete, errors |

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Space` | Play / Pause |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `Ctrl+S` | Save |
| `Delete` | Delete selected entity |

## Update Loop

```
EditorApp::update() — called every frame by eframe

  1. Handle keyboard shortcuts (Space, Ctrl+Z/Y/S, Delete)
  2. If playing: advance time, set needs_render
  3. Draw top bar (TopBottomPanel::top)
  4. Draw status bar (TopBottomPanel::bottom)
  5. Draw workspace (CentralPanel → egui_tiles)
     ├── Each tile calls panel-specific ui() function
     └── Panels read/write EditorApp state
  6. Process pending workspace actions (split, close, switch type)
  7. If needs_render:
     ├── ensure_renderer() — create if needed
     ├── render_scene() — pipeline::render_frame() → pixels
     └── Upload pixels as egui texture
```
