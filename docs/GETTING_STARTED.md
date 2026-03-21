# Getting Started with ifol-render

## Installation

### Web (NPM)
```bash
npm install @danphong23/ifol-render-wasm ifol-render-sdk
```

### Desktop (GitHub Releases)
Download the latest `.zip` from [Releases](https://github.com/danphong23/ifol-render/releases):
- `ifol-render.exe` — CLI tool for headless rendering and export
- `ifol-render-studio.exe` — GUI editor with real-time preview

> **Note:** FFmpeg is required for video export. Install it from [ffmpeg.org](https://ffmpeg.org/download.html) or via `winget install ffmpeg`.

---

## Quick Start — Web

### 1. Initialize the WASM engine
```html
<canvas id="canvas" width="1920" height="1080"></canvas>
<script type="module">
  import init, { IfolRenderWeb } from '@danphong23/ifol-render-wasm';
  import { DrawableEntity, FrameBuilder, TextureUpdates } from 'ifol-render-sdk';

  await init();
  const canvas = document.getElementById('canvas');
  const core = await new IfolRenderWeb(canvas, 1920, 1080, 30);
  core.setup_builtins();
</script>
```

### 2. Draw your first frame
```javascript
// Create a blue background
const bg = new DrawableEntity(0, 0, 0, 1920, 1080)
  .setShader('shapes')
  .setColor(0.1, 0.1, 0.3, 1)
  .setLayer(0);

// Create a green circle
const circle = new DrawableEntity(1, 800, 400, 200, 200)
  .setShader('shapes')
  .setParams([1.0])  // 1.0 = circle
  .setColor(0.2, 0.9, 0.4, 0.9)
  .setLayer(1);

// Build and render
const frame = new FrameBuilder()
  .setClearColor(0, 0, 0, 1)
  .addEntity(bg)
  .addEntity(circle)
  .build();

core.render_frame(JSON.stringify(frame));
```

### 3. Load images
```javascript
import { AssetManager } from 'ifol-render-sdk';

const assets = new AssetManager({
  ppu: 100,
  urlResolver: (path) => `/assets/${path}`,
  coreCache: (key, data) => core.cache_image(key, data),
});

await assets.loadImage('hero.png');

const img = new DrawableEntity(2, 100, 100, 400, 300)
  .setShader('composite')
  .addTexture('hero.png')
  .setLayer(2);
```

### 4. Add audio
```javascript
import { AudioScene } from 'ifol-render-sdk';

const audio = new AudioScene();
audio.addClip({
  source: 'bgm.mp3',
  startTime: 0,
  volume: 0.8,
  fadeIn: 1.0,
  fadeOut: 2.0,
}, 'bgm');
```

### 5. Export
```javascript
import { buildExportPayload } from 'ifol-render-sdk';

const frames = []; // collect frames for each timestep
const payload = buildExportPayload(
  { output: 'video.mp4', width: 1920, height: 1080, fps: 30 },
  frames,
  audio.flattenForExport()
);

await fetch('/export', {
  method: 'POST',
  body: JSON.stringify(payload),
});
```

---

## SDK Modules

| Module | Purpose |
|--------|---------|
| `DrawableEntity` | Pixel-space drawable element |
| `FrameBuilder` | Composable frame assembly |
| `AudioScene` | Audio track & clip management |
| `AssetManager` | Image/video decode & cache |
| `Scene` | Optional entity CRUD helper |
| `Timeline` | Optional playback state |
| `BoundCamera` / `FreeCamera` | Viewport math |
| `AnimationManager` | Optional keyframe interpolation |

## Architecture
```
Your App (any framework)
  ↓ builds DrawableEntity / FrameBuilder
SDK Toolkit (produces Frame JSON)
  ↓ Frame JSON string
Core WASM (GPU rendering)
  ↓ pixels on canvas
```
