# ifol-render-sdk

GPU-accelerated 2D rendering SDK for web. Built on WebGPU via WASM.

**SDK produces Frame JSON. App Layer pumps it to GPU.**

## Architecture

```
App Layer
  ● core.render_frame(frameJSON) → GPU
  ● UI events (pan, zoom, drag)
  ● Export I/O, procedural animation
  ● FPS control (requestAnimationFrame)

SDK ──── produces Frame JSON ────────────
  Scene         — entity CRUD in world units
  Camera        — BoundCamera, FreeCamera (extensible)
  RenderView    — Scene × Camera → Frame JSON
  AssetManager  — image/video decode + shared cache
  Timeline      — playback state (no FPS)
  Animation     — keyframe tracks + easing (optional)

Core WASM ── consumes Frame JSON ────────
  render_frame(json) → GPU pixels
```

## Quick Start

```ts
import { Scene, FreeCamera, BoundCamera, RenderView, Timeline, AssetManager } from 'ifol-render-sdk';

// Shared scene (unit-space)
const scene = new Scene({ ppu: 1, duration: 10 });

// Cameras
const editCam = new FreeCamera({ centerX: 960, centerY: 540, zoom: 1, screenWidth: 800, screenHeight: 600, ppu: 1 });
const exportCam = new BoundCamera(0, 0, 1920, 1080);

// Render views (multi-view: same scene, different cameras)
const editView = new RenderView(scene, editCam, { renderScale: 1 });
const camView = new RenderView(scene, exportCam, { renderScale: 1 });

// Add entities (world units)
scene.addEntity({ id: 'bg', type: 'rect', x: 0, y: 0, width: 1920, height: 1080,
  color: [0.1, 0.1, 0.2, 1], opacity: 1, rotation: 0,
  blendMode: 'normal', shader: 'shapes', params: [], layer: 0, startTime: 0, duration: 10 });

// Timeline
const tl = new Timeline(10);
tl.play();

// App render loop (App controls FPS)
function loop(ts) {
  const time = tl.tick(ts);
  const editFrame = editView.flattenAt(time);
  const camFrame = camView.flattenAt(time);
  editCore.render_frame(JSON.stringify(editFrame));
  camCore.render_frame(JSON.stringify(camFrame));
  requestAnimationFrame(loop);
}
requestAnimationFrame(loop);
```

## Modules

| Module | Class | Purpose |
|--------|-------|---------|
| `scene.ts` | `Scene` | Entity CRUD, visibleAt, hitTest |
| `camera.ts` | `BoundCamera` | Fixed region (export, preview) |
| `camera.ts` | `FreeCamera` | Center+zoom viewport, top-left anchor |
| `render-view.ts` | `RenderView` | Scene × Camera → Frame JSON |
| `flatten.ts` | `flatten()` | Pure function: entities + region → pixels |
| `timeline.ts` | `Timeline` | Playback: play/pause/seek/tick |
| `animation.ts` | `KeyframeTrack` | Property keyframes + easing |
| `animation.ts` | `AnimationManager` | Attach tracks, applyAll(scene, t) |
| `assets.ts` | `AssetManager` | Image/video decode + cache |

## Key Design Decisions

- **FPS is NOT in SDK** — App Layer controls frame rate
- **Camera is NOT an entity** — it's a view definition (BoundCamera, FreeCamera)
- **Multi-view**: N RenderViews share 1 Scene, produce independent Frame JSON
- **Unit coordinate system**: all spatial values in world units, pixels only after flatten
- **renderScale**: GPU renders at fraction of display size for performance
- **Viewport resize anchors top-left** (not center)
- **Keyframe animation** is SDK-level (serializable), procedural animation is App-level

## Building

```bash
cd sdk && npm run build
```

## License

MIT
