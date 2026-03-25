# Getting Started with ifol-render (V2)

## Prerequisites

- **Rust 1.85+** with `wasm32-unknown-unknown` target
- **wasm-pack** (`cargo install wasm-pack`)
- **Node.js 18+** (for Vite dev server)
- **FFmpeg** (for video export only)
- **Browser with WebGPU** (Chrome/Edge 113+)

---

## Quick Start — Web

### 1. Build WASM
```bash
cd crates/wasm
wasm-pack build --target web --out-dir pkg
```

### 2. Copy to web directory
```bash
cp -r crates/wasm/pkg web/ifol-render-wasm
```

### 3. Start dev server
```bash
cd web
npx vite --port 5174
```

### 4. Open in browser
Navigate to `http://localhost:5174` — the test editor loads automatically.

---

## V2 API — Scene JSON

ifol-render V2 uses **ECS architecture**. You define entities with components in JSON:

```javascript
import init, { IfolRenderWeb } from './ifol-render-wasm/ifol_render_wasm.js';

// Initialize
await init();
const canvas = document.getElementById('canvas');
const engine = await new IfolRenderWeb(canvas, 1280, 720, 30);
engine.setup_builtins();

// Define scene
const scene = {
  assets: {},
  entities: [
    {
      id: "cam",
      lifespan: { start: 0, end: 99999 },
      camera: { resolutionWidth: 1280, resolutionHeight: 720 },
      transform: { x: { keyframes: [{ time: 0, value: 0 }] }, y: { keyframes: [{ time: 0, value: 0 }] } }
    },
    {
      id: "red_box",
      lifespan: { start: 0, end: 10 },
      transform: {
        x: { keyframes: [{ time: 0, value: -200 }, { time: 5, value: 200 }] },
        y: { keyframes: [{ time: 0, value: 0 }] },
        rotation: { keyframes: [{ time: 0, value: 0 }, { time: 10, value: 360 }] },
        anchor_x: { keyframes: [{ time: 0, value: 0.5 }] },
        anchor_y: { keyframes: [{ time: 0, value: 0.5 }] },
        scale_x: { keyframes: [{ time: 0, value: 1 }] },
        scale_y: { keyframes: [{ time: 0, value: 1 }] }
      },
      rect: { width: { keyframes: [{ time: 0, value: 200 }] }, height: { keyframes: [{ time: 0, value: 150 }] }, fitMode: "stretch" },
      colorSource: { color: { r: 0.8, g: 0.1, b: 0.1, a: 1 } },
      opacity: { keyframes: [{ time: 0, value: 1 }] },
      layer: 1
    }
  ]
};

// Load and render
engine.load_scene_v2(JSON.stringify(scene));
engine.render_frame_v2(0.0, "cam");  // Render at time=0
```

---

## Core Concepts

### Entity = ID + Components
An entity is a blank container. **Components define what it is**:
- `colorSource` → renders a solid color quad
- `videoSource` → renders video frames
- `camera` → defines a viewpoint
- `composition` → groups children into a nested timeline

### All Properties are Keyframe Tracks
Transform, opacity, rect size — everything is animatable:
```json
{ "keyframes": [
  { "time": 0, "value": 0, "interpolation": { "type": "linear" } },
  { "time": 5, "value": 400, "interpolation": { "type": "cubic_bezier", "x1": 0.42, "y1": 0, "x2": 0.58, "y2": 1 } }
]}
```

### Scene per Change
Every time you change an entity, rebuild and send the scene:
```javascript
scene.entities[1].transform.x.keyframes[0].value = newX;
engine.load_scene_v2(JSON.stringify(scene));
engine.render_frame_v2(currentTime, "cam");
```

---

## Render Modes

```javascript
// Camera mode: render at camera's native resolution
engine.render_frame_v2(time, "cam");

// Editor mode: render custom viewport region (pan/zoom)
engine.render_frame_v2(time, "cam", panX, panY, viewWidth, viewHeight);
```

---

## Loading Assets

### Images
```javascript
// Register in scene
scene.assets["my_image"] = { type: "image", url: "http://server/photo.png" };
entity.imageSource = { assetId: "my_image", intrinsicWidth: 1920, intrinsicHeight: 1080 };

// Fetch and inject
const resp = await fetch("http://server/photo.png");
const blob = await resp.blob();
const bitmap = await createImageBitmap(blob);
// ... extract RGBA and cache
engine.cache_image("http://server/photo.png", rgbaData);
```

### Video Frames
```javascript
scene.assets["my_video"] = { type: "video", url: "http://server/video.mp4" };

// Create hidden video element
const video = document.createElement("video");
video.src = "http://server/video.mp4";

// Each frame: seek, extract, inject
video.currentTime = playbackTime;
// ... draw to canvas, getImageData
engine.cache_video_frame("http://server/video.mp4", rgbaData, width, height);
```

---

## Further Reading

- [ARCHITECTURE.md](ARCHITECTURE.md) — Full system architecture, ECS pipeline, components
- [ROADMAP.md](ROADMAP.md) — Bottom-up build roadmap
- [CLI_GUIDE.md](CLI_GUIDE.md) — Server-side export commands
