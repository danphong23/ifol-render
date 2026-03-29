# Getting Started with ifol-render

## Prerequisites

- **Rust 1.85+** with `wasm32-unknown-unknown` target
- **wasm-pack** (`cargo install wasm-pack`)
- **Browser with WebGPU** (Chrome/Edge 113+)
- **FFmpeg** (for video export via CLI only)

---

## Quick Start — Web

### 1. Build WASM
```bash
cd crates/wasm
wasm-pack build --target web
```

### 2. Start dev server
```bash
cd web
npx vite --port 5174
```

### 3. Open in browser
Navigate to `http://localhost:5174/v4-test.html` — test editor w/ timeline loads automatically.

---

## V4 API — Scene JSON (ECS)

ifol-render V4 uses **pure ECS architecture**. Entities have components; systems process them.

```javascript
import init, { IfolRenderWeb } from '../crates/wasm/pkg/ifol_render_wasm.js';

// Initialize
await init();
const canvas = document.getElementById('canvas');
const engine = await new IfolRenderWeb(canvas, 1280, 720, 30);
engine.setup_builtins();

// Define scene
const scene = {
  assets: {
    "photo": { image: { url: "./hero.png" } }
  },
  entities: [
    {
      id: "main_cam",
      camera: { resolutionWidth: 1280, resolutionHeight: 720 },
      rect: { width: 1280, height: 720 },
      transform: { x: 0, y: 0, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0, anchorY: 0 },
      lifespan: { start: 0, end: 100 }
    },
    {
      id: "red_box",
      shapeSource: { kind: "rectangle", fillColor: [0.8, 0.1, 0.1, 1.0] },
      rect: { width: 200, height: 150 },
      transform: { x: 100, y: 50, rotation: 0, scaleX: 1, scaleY: 1, anchorX: 0.5, anchorY: 0.5 },
      lifespan: { start: 0, end: 10 },
      layer: 1,
      animation: { floatTracks: [
        { target: "transformX", track: { keyframes: [
          { time: 0, value: -200 },
          { time: 5, value: 200, interpolation: { type: "cubic_bezier", x1: 0.42, y1: 0, x2: 0.58, y2: 1 } }
        ]}}
      ]}
    }
  ]
};

// Load assets, then scene
async function loadAndRender() {
  // Fetch + decode image → RGBA → cache
  const resp = await fetch("./hero.png");
  const blob = await resp.blob();
  const bitmap = await createImageBitmap(blob);
  const c = new OffscreenCanvas(bitmap.width, bitmap.height);
  const ctx = c.getContext('2d');
  ctx.drawImage(bitmap, 0, 0);
  const rgba = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
  engine.cache_image("./hero.png", rgba.data, bitmap.width, bitmap.height);
  bitmap.close();

  // Load scene + render
  engine.load_scene_v2(JSON.stringify(scene));
  engine.render_frame_v2(0.0, "main_cam", true, 0, 0, 1280, 720);
}
```

---

## Core Concepts

### Entity = ID + Components
An entity is a blank container. **Components define what it is**:
- `shapeSource` → renders a rectangle or ellipse
- `imageSource` → renders an image texture (requires asset loading)
- `videoSource` → renders video frames
- `camera` → defines a viewpoint
- `composition` → groups children into a nested sub-timeline

### Animation = Keyframe Tracks
Properties are animated via `floatTracks` in an `animation` component:
```json
{ "target": "transformX", "track": { "keyframes": [
  { "time": 0, "value": 0 },
  { "time": 5, "value": 400, "interpolation": { "type": "cubic_bezier", "x1": 0.42, "y1": 0, "x2": 0.58, "y2": 1 } }
]}}
```

### Asset Pipeline (App Decodes, Core Renders)
```
Scene JSON → assets: { "id": { image: { url: "..." } } }
App layer:   fetch(url) → decode → RGBA → engine.cache_image(url, rgba, w, h)
Core:        GPU upload once → reuse every frame. Zero per-frame cost.
```

### Render Modes
```javascript
// Editor mode: custom viewport (pan/zoom)
engine.render_frame_v2(time, "cam", true, panX, panY, viewW, viewH);

// Camera mode: render at camera's native resolution
engine.render_frame_v2(time, "cam", false);
```

---

## FitMode

When an image/video is inside a Rect, FitMode controls scaling:

| Mode | Behavior |
|------|----------|
| `stretch` (default) | Fill Rect exactly, may distort |
| `contain` | Fit proportionally, transparent surplus |
| `cover` | Fill Rect, crop edges |

```json
{ "rect": { "width": 400, "height": 300, "fitMode": "contain" } }
```

---

## Further Reading

- [ARCHITECTURE.md](ARCHITECTURE.md) — Full ECS pipeline, 6 systems, components, asset pipeline
- [ASSET_MANAGEMENT.md](ASSET_MANAGEMENT.md) — Quy cách và ranh giới trách nhiệm khi nạp Asset cho Frontend/Backend
- [INTEGRATION_GUIDE.md](INTEGRATION_GUIDE.md) — Hướng dẫn chi tiết setup WASM trong JS và gọi CLI trong NodeJS Backend
- [TEST_CASES.md](TEST_CASES.md) — Danh sách và giải nghĩa các tính năng lõi (TC1 - TC19) để test
- [ROADMAP.md](ROADMAP.md) — Bottom-up rebuild roadmap with completion status
- [CLI_GUIDE.md](CLI_GUIDE.md) — Server-side export commands
