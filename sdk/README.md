# ifol-render-sdk

GPU-accelerated 2D rendering SDK for web. Built on WebGPU via WASM.

## Features

- **Unit coordinate system** — all positions/sizes in abstract units, pixels only at render time
- **PPU (Pixels-Per-Unit)** — configurable import scale for media assets
- **Multi-viewport** — edit viewport + camera view, or N viewports simultaneously
- **Resolution scaling** — render at lower quality for performance, display at full size
- **Image & video** — automatic decode pipeline (fetch → decode → GPU texture)
- **Batch rendering** — push pre-computed frames for playback streaming
- **Export** — render camera view to video via CLI backend

## Quick Start

```ts
import { IfolRenderer } from 'ifol-render-sdk';

const renderer = await IfolRenderer.create({
  canvas: document.getElementById('viewport'),
  scene: { ppu: 1, fps: 30, duration: 10 },
});

// Add shapes (position & size in world units)
renderer.addShape('rect1', 'rect', {
  x: 100, y: 50, width: 400, height: 300,
  color: [0.2, 0.6, 1.0, 1.0],
});

// Add images (auto-sized from pixel dimensions / PPU)
await renderer.addImage('bg', '/photos/background.jpg');

// Add videos
await renderer.addVideo('clip', '/videos/intro.mp4');

// Control viewport
renderer.setViewport({ centerX: 960, centerY: 540, zoom: 1.5, renderScale: 0.75 });

// Playback
renderer.play();
renderer.startLoop(); // starts requestAnimationFrame loop
```

## Architecture

```
Developer API
  │
  ├─ IfolRenderer — single entry point
  │    ├─ addShape / addImage / addVideo / removeEntity
  │    ├─ setViewport / setCamera
  │    ├─ play / pause / seekTo
  │    └─ startLoop → tick() → flatten → render
  │
  ├─ Scene — entity model in unit space
  │    ├─ flattenForViewport() → pixel-space Frame
  │    ├─ flattenForCamera() → pixel-space Frame
  │    ├─ canvasToWorld() / worldToCanvas()
  │    └─ hitTest() / hitTestBorder()
  │
  ├─ AssetManager — media decode pipeline
  │    ├─ loadImage() → fetch → RGBA → Core cache
  │    ├─ loadVideo() → HTML5 <video> → metadata
  │    ├─ extractVideoFrame() → canvas → RGBA → Core cache
  │    └─ cache lifecycle (evict, clear, destroy)
  │
  └─ Core WASM — GPU rendering engine
       ├─ render_frame(Frame JSON)
       ├─ cache_image / cache_video_frame
       ├─ resize / clear_frames
       └─ WebGPU pipeline → canvas pixels
```

## API Reference

### `IfolRenderer`

| Method | Description |
|--------|-------------|
| `create(opts)` | Initialize renderer with canvas + settings |
| `addShape(id, type, opts)` | Add rect or circle entity |
| `addImage(id, url, opts?)` | Load image + create entity |
| `addVideo(id, url, opts?)` | Load video + create entity |
| `removeEntity(id)` | Remove entity + cleanup assets |
| `updateEntity(id, patch)` | Update entity properties |
| `setViewport(patch)` | Update viewport center/zoom/renderScale |
| `setCamera(patch)` | Update camera position/size |
| `play() / pause() / stop()` | Playback control |
| `seekTo(time)` | Seek to timestamp |
| `startLoop() / stopLoop()` | Animation frame loop |
| `canvasToWorld(x,y)` | CSS pixel → world unit |
| `worldToCanvas(x,y)` | World unit → CSS pixel |
| `destroy()` | Cleanup all resources |

### `Scene`

| Method | Description |
|--------|-------------|
| `flattenForViewport(time, vp, excludeIds?)` | Unit → pixel frame for viewport |
| `flattenForCamera(time, cam, w, h, scale, excludeIds?)` | Unit → pixel frame for camera |
| `flattenForExport(time, cam, exportW, exportH)` | Unit → pixel frame for export |
| `canvasToWorld(cx, cy, vp)` | Canvas pixel → world unit |
| `worldToCanvas(wx, wy, vp)` | World unit → canvas pixel |
| `hitTest(wx, wy, excludeIds?)` | Find topmost entity at position |
| `hitTestBorder(wx, wy, entity, margin)` | Test if point is on entity border |

### `AssetManager`

| Method | Description |
|--------|-------------|
| `loadImage(key, url)` | Fetch + decode + cache in Core |
| `loadVideo(key, url)` | Create video element + get metadata |
| `extractVideoFrame(key, timestamp)` | Decode frame → RGBA → Core |
| `removeImage(key)` | Remove from registry |
| `removeVideo(key)` | Stop + remove video element |
| `clearVideoFrames()` | Clear all cached frames from WASM |
| `destroy()` | Release all resources |

## Coordinate System

See [UNIT_SYSTEM.md](../docs/UNIT_SYSTEM.md) for full specification.

**Key concepts:**
- All entity positions/sizes are in **world units**
- PPU converts media pixels → units on import: `unitSize = pixelSize / PPU`
- Viewport determines visible region: `visibleW = screenW / (PPU × zoom)`
- Flatten converts units → pixels for a specific render target
- renderScale controls GPU quality: `backingSize = cssSize × renderScale`

## Building

```bash
# Build WASM
cd crates/wasm
wasm-pack build --target web --release

# Build SDK TypeScript
cd sdk
npm run build
```

## License

MIT
