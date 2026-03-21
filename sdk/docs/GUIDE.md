# ifol-render SDK — Developer Guide

## What is the SDK?

A **toolkit** (not framework) that helps you produce Frame JSON for the Core WASM renderer.

```
YOUR APP (you decide everything)
  ● Scene management, UI, undo/redo
  ● Render loop timing (requestAnimationFrame)
  ● Timeline & seek control
  ↓ builds Frame JSON using SDK tools

SDK TOOLKIT (pure data transforms)
  ● DrawableEntity — pixel-space element
  ● FrameBuilder — composable frame assembly
  ● AudioClipBuilder — audio clip data for export
  ● flatten() — Entity[] + Camera → Frame
  ● AssetManager — image/video load & cache
  ↓ Frame JSON

CORE WASM (GPU rendering)
  ● render_frame(json) → pixels on canvas
  ● export_video() → mp4 (CLI only)
```

---

## Quick Start — 3 Lines to First Frame

```js
const core = await new IfolRenderWeb(canvas, 800, 600, 60);
core.setup_builtins();

// Manual Frame JSON — you don't even need the SDK!
const frame = {
  passes: [{
    output: 'main',
    pass_type: { Entities: {
      entities: [{ id: 0, x: 100, y: 100, width: 200, height: 150,
                   rotation: 0, opacity: 1, blend_mode: 0,
                   color: [0.3, 0.9, 0.6, 1],
                   shader: 'shapes', textures: [], params: [],
                   layer: 0, z_index: 0 }],
      clear_color: [0, 0, 0, 1],
    }},
  }, { output: 'screen', pass_type: { Output: { input: 'main' } } }],
  texture_updates: [],
};
core.render_frame(JSON.stringify(frame));
```

---

## Using SDK Builders (recommended)

```ts
import { DrawableEntity, FrameBuilder, TextureUpdates } from 'ifol-render-sdk';

// Create entities
const bg = new DrawableEntity(0, 0, 0, 800, 600)
  .setShader('shapes').setColor(0.05, 0.05, 0.15, 1);

const hero = new DrawableEntity(1, 200, 100, 400, 300)
  .setShader('composite')
  .addTexture('hero.png')
  .setRotation(0.1)
  .setOpacity(0.9)
  .setLayer(1);

// Build frame
const frame = new FrameBuilder()
  .setClearColor(0, 0, 0, 1)
  .addEntity(bg)
  .addEntity(hero)
  .addTextureUpdate(TextureUpdates.loadImage('hero.png', '/assets/hero.png'))
  .build();

core.render_frame(JSON.stringify(frame));
```

---

## Render Loop (APP controls timing)

```js
// APP decides the frame rate — SDK has NO render loop
function loop(timestamp) {
  // App: update time
  time += 1/60;

  // App: resolve your entities however you want
  const myEntities = getSceneAt(time);

  // SDK: flatten (if using world-space Entity interface)
  const frame = flatten(myEntities, camera.getRegion(time), w, h, time);

  // Core: render
  core.render_frame(JSON.stringify(frame));

  requestAnimationFrame(loop);
}
requestAnimationFrame(loop);
```

**Why App controls timing:**
- Some apps want 30fps, others 60fps, others unlimited
- Some apps pause rendering when tab is hidden
- Some apps sync to audio or external clock
- SDK can't know your timing requirements

---

## Seek (Timeline Scrubbing)

```js
// App handles seek — SDK doesn't track playback state
slider.oninput = (ev) => {
  const t = parseFloat(ev.target.value);
  // Just render at that time — same flatten(), different time
  const frame = flatten(entities, cam.getRegion(t), w, h, t);
  core.render_frame(JSON.stringify(frame));
};
```

---

## Export (Batch Frames → Backend)

```js
async function doExport() {
  const frames = [];
  const total = Math.round(duration * fps);

  // App: loop through all time steps
  for (let i = 0; i < total; i++) {
    const t = i / fps;
    frames.push(buildFrameAt(t));   // Your function
  }

  // Send to backend
  const payload = {
    settings: { width: 1920, height: 1080, fps: 30 },
    frames,
    output: 'my_video.mp4',
    audio_clips: [
      { path: 'C:/music/bgm.mp3', start_time: 0, volume: 0.8, fade_out: 2.0 }
    ],
  };
  await fetch('/export', { body: JSON.stringify(payload) });
}
```

---

## Audio

### Export Audio (Core handles via FFmpeg)
```ts
import { AudioClipBuilder, buildExportPayload } from 'ifol-render-sdk';

const bgm = new AudioClipBuilder('C:/music/bgm.mp3')
  .setVolume(0.8).setFadeIn(1.0).setFadeOut(2.0);

const sfx = new AudioClipBuilder('C:/sfx/boom.wav')
  .setStartTime(3.5).setDuration(1.0);

const payload = buildExportPayload(config, frames, [bgm, sfx]);
```

### Preview Audio (App handles via Web Audio API)
```js
// Audio preview is App-layer — Core only renders pixels
const audioCtx = new AudioContext();
const buffer = await fetch('bgm.mp3').then(r => r.arrayBuffer());
const audioBuffer = await audioCtx.decodeAudioData(buffer);

const source = audioCtx.createBufferSource();
source.buffer = audioBuffer;
source.connect(audioCtx.destination);
source.start(0, currentTime);  // Sync with your timeline
```

---

## Loading Images

```js
// Step 1: Fetch + cache in Core
const res = await fetch(ASSET_SERVER + encodeURIComponent(path));
const bytes = new Uint8Array(await res.arrayBuffer());
core.cache_image(path, bytes);  // Key = actual file path

// Step 2: Reference in entity
const img = new DrawableEntity(id, x, y, w, h)
  .setShader('composite')
  .addTexture(path);  // Same key as cache_image

// Step 3: Frame includes LoadImage instruction
frame.addTextureUpdate(TextureUpdates.loadImage(path, path));
```

**IMPORTANT**: Use the actual file path as key. This way:
- Web: `cache_image(path)` → `LoadImage { path }` → found in memory ✅
- CLI: `LoadImage { path }` → found on disk ✅

---

## Multi-Viewport

```js
// App creates N canvases + N Core instances
const editCore = await new IfolRenderWeb(editCanvas, ...);
const previewCore = await new IfolRenderWeb(previewCanvas, ...);

// Each render: same entities, different cameras
const editFrame = flatten(entities, editCam.getRegion(t), editW, editH, t);
const previewFrame = flatten(entities, previewCam.getRegion(t), prevW, prevH, t);
editCore.render_frame(JSON.stringify(editFrame));
previewCore.render_frame(JSON.stringify(previewFrame));
```

---

## Entity Fields Reference

| Field | Type | Description |
|-------|------|-------------|
| `id` | number | Unique ID (Core dirty tracking) |
| `x`, `y` | number | Top-left position in pixels |
| `width`, `height` | number | Size in pixels |
| `rotation` | number | Radians, around entity center |
| `opacity` | number | 0.0–1.0 |
| `blend_mode` | number | 0=normal, 1=multiply, 2=screen, ... |
| `color` | [r,g,b,a] | RGBA 0–1, shader tint |
| `shader` | string | 'shapes' (flat color), 'composite' (texture) |
| `textures` | string[] | Texture cache keys to bind |
| `params` | number[] | Extra shader uniforms |
| `layer` | number | Coarse draw order (ascending) |
| `z_index` | number | Fine draw order within layer |

---

## Examples

See `web/examples/`:
- `01-minimal.html` — 3 shapes, animation, zero SDK imports
- `02-image.html` — Load + display image from asset server
- `03-export.html` — Batch frame export to MP4
