# Ifol-Render Web Editor Integration Guide (WASM)

This document is intended for web developers and UI engineers integrating the `ifol-render-wasm` module into fully featured web applications (such as a Video Editor or Compositor UI).

## 1. Engine Initialization

The WASM backend is designed to run completely implicitly on a single WebGPU context, while allowing the DOM to handle the presentation logic via the `Canvas 2D` API.

**Step 1:** Create two canvas elements (one hidden for WebGPU processing, one for DOM display).
```html
<!-- Hidden WebGPU Render Target -->
<canvas id="gpuCanvas" style="display:none;" width="1280" height="720"></canvas>

<!-- Visible Editor Viewport -->
<div id="viewportArea" style="width: 100%; height: 100%;">
  <canvas id="canvasMain" style="width: 100%; height: 100%; object-fit: contain;"></canvas>
</div>
```

**Step 2:** Instantiate the Engine.
```javascript
import init, { IfolRenderWeb } from './pkg/ifol_render_wasm.js';

await init(); // Initialize WASM
const engine = await new IfolRenderWeb(document.getElementById('gpuCanvas'), 1280, 720, 60);
engine.setup_builtins(); // Loads internal shaders before rendering!
```

## 2. Dynamic Viewports & Resolution Scaling (Quality %)

To simulate high-end software like *Adobe Premiere Pro*, the engine natively supports changing the rendering resolution decoupled from the DOM layout.

**Setting Playback Quality (e.g. 50%):**
```javascript
const qualityScale = 0.5;
const domWidth = 1000;
const domHeight = 500;

const renderWidth = Math.floor(domWidth * qualityScale);
const renderHeight = Math.floor(domHeight * qualityScale);

// Update backing buffers
document.getElementById('canvasMain').width = renderWidth;
document.getElementById('canvasMain').height = renderHeight;
document.getElementById('gpuCanvas').width = renderWidth;
document.getElementById('gpuCanvas').height = renderHeight;

// Notify the WASM Engine
engine.resize(renderWidth, renderHeight);
```

## 3. VRAM Profiling & LRU Resource Management

Ifol-Render intelligently caches `Image` and `<video>` textures within WebGPU to maximize FPS, but VRAM is finite.

**Limit Set & Clear APIs:**
```javascript
// Force the engine to clear stale textures if cache exceeds 128 MB (Soft Limit)
engine.set_vram_limit_mb(128);

// Completely destroy all cached textures immediately (returns VRAM to 0)
// This requires the Editor to re-feed `engine.cache_image()` afterwards!
engine.clear_textures();
```

**Read Real-time Metrics Profile:**
```javascript
const metricsJsonStr = engine.render_frame_v2(...);
const metrics = JSON.parse(metricsJsonStr);

console.log(`VRAM Used: ${(metrics.vram_bytes / 1024 / 1024).toFixed(1)} MB`);
console.log(`Active Textures: ${metrics.vram_count}`);
```

## 4. Keyframe Manipulation / Transient Dragging (Native Support)

Ifol-render natively supports dual-mode editing when dragging elements on the canvas via `engine.drag_entity_v2(id, dx, dy, ...)`:

1. **Static Entity (No Keyframes):** The core ECS Engine will permanently apply `dx` and `dy` to the `Transform` block. The position is saved for the duration of the web session.
2. **Keyframed Entity:** The core engine detects that the active track (e.g., `TransformX`) is driven by keyframes. Instead of mutating the static component, it automatically registers a **Transient Override** for that specific entity on the current frame.

**Workflow for Web UI:**
- **On Drag (`mousemove`):** Just call `engine.drag_entity_v2(...)`; the engine applies it natively. The entity moves smoothly without fighting the keyframe interpolation.
- **On Time Scrub / Play:** When the global `time_sec` passed to `render_frame_v2` changes, **the engine automatically clears all Transient Overrides**. The entity snaps back to its rigid Keyframe curve.
- **On Commit (e.g., "Add Keyframe"):** Because dragging only overrides the render state, if you want to permanently commit the dragged position, your Frontend JS must read the final Canvas coordinate and manually insert a new keyframe into `currentScene.entities[...].transform.keyframes` and invoke `applyJson()`!
