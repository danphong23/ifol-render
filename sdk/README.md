# ifol-render-sdk

**Render Pipeline Toolkit** — produces Frame JSON for Core WASM.

SDK = toolkit, NOT framework. Dev chooses how to manage their app.

## Architecture

```
App Layer (dev decides everything)
  ● Any scene management (Array, ECS, Redux, ...)
  ● UI, interaction, undo/redo
  ● FPS control, playback
  ● Bones, particles, VFX, ...
  ↓ builds DrawableEntity / FrameBuilder

SDK Toolkit (produces Frame JSON)
  ● DrawableEntity — pixel-space drawable element class
  ● FrameBuilder — composable frame assembly
  ● flatten() — convenience Entity[] → Frame
  ● Camera, AssetManager
  ↓ Frame JSON

Core WASM (GPU rendering)
  ● render_frame(json) → pixels
  ● export_video() → mp4
```

## Usage — Builder API (Primary)

Dev with **any framework** can use builders to construct Frame JSON:

```ts
import { DrawableEntity, FrameBuilder, TextureUpdates } from 'ifol-render-sdk';

// 1. Create drawable entities (pixel-space)
const bg = new DrawableEntity(0, 0, 0, 1920, 1080)
  .setShader('shapes')
  .setColor(0.1, 0.1, 0.2, 1)
  .setLayer(0);

const img = new DrawableEntity(1, 100, 200, 400, 300)
  .setShader('composite')
  .addTexture('hero.png')
  .setRotation(Math.PI / 6)
  .setOpacity(0.9)
  .setLayer(1);

const circle = new DrawableEntity(2, 500, 300, 100, 100)
  .setShader('shapes')
  .setParams([1.0])  // shapes shader: 1.0 = circle
  .setColor(0, 1, 0.5, 0.8)
  .setLayer(2);

// 2. Build frame
const frame = new FrameBuilder()
  .setClearColor(0, 0, 0, 1)
  .addEntity(bg)
  .addEntity(img)
  .addEntity(circle)
  .addTextureUpdate(TextureUpdates.loadImage('hero.png', '/assets/hero.png'))
  .build();

// 3. Render
core.render_frame(JSON.stringify(frame));
```

## Usage — flatten() Convenience

If you manage entities in world units with the `Entity` interface:

```ts
import { flatten, BoundCamera } from 'ifol-render-sdk';

const cam = new BoundCamera(0, 0, 1920, 1080);
const frame = flatten(myEntities, cam.getRegion(time), 1920, 1080, time);
core.render_frame(JSON.stringify(frame));
```

## Export

```ts
import { buildExportPayload, FrameBuilder } from 'ifol-render-sdk';

// Build frames for each time step
const frames = [];
for (let t = 0; t < duration; t += 1/fps) {
  frames.push(new FrameBuilder().addEntities(getEntitiesAt(t)).build());
}

// Build export payload with all settings
const payload = buildExportPayload({
  output: 'my_video.mp4',
  width: 1920, height: 1080, fps: 30,
  codec: 'h264', crf: 23, preset: 'medium',
  ffmpeg: '/usr/bin/ffmpeg',  // optional
}, frames);

await fetch('/export', { body: JSON.stringify(payload) });
```

## Modules

| Module | Purpose |
|--------|---------|
| `builders.ts` | **DrawableEntity**, **FrameBuilder**, **TextureUpdates** — primary API |
| `flatten.ts` | `flatten()` — Entity[] + Camera → Frame |
| `camera.ts` | BoundCamera, FreeCamera — viewport math |
| `assets.ts` | AssetManager — image/video decode + cache |
| `scene.ts` | Scene — optional entity CRUD helper |
| `render-view.ts` | RenderView — optional Scene × Camera wrapper |
| `timeline.ts` | Timeline — optional playback state |
| `animation.ts` | AnimationManager — optional keyframes |

## Building

```bash
cd sdk && npm run build
```
