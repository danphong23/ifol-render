/**
 * Dual Viewport Example — ifol-render SDK
 *
 * Two renderers: edit viewport (with zoom/pan) + camera preview.
 * Demonstrates the unit coordinate system where resizing a viewport
 * changes the visible area, not the zoom level.
 *
 * Usage:
 *   <canvas id="editCanvas" style="width:60%;height:100%"></canvas>
 *   <canvas id="cameraCanvas" style="width:300px;height:169px"></canvas>
 *   <script type="module" src="dual-viewport.ts"></script>
 */

import { IfolRenderer, Scene, AssetManager, viewportRegion } from '../src/index.js';
import type { Viewport, Camera } from '../src/types.js';

async function main() {
  // Shared scene
  const scene = new Scene({ ppu: 1, fps: 30, duration: 10 });

  // Camera entity (unit-based)
  const camera: Camera = { x: 0, y: 0, width: 1920, height: 1080 };

  // Add entities in unit space
  scene.addEntity({
    id: 'bg', type: 'rect',
    x: 0, y: 0, width: 1920, height: 1080,
    color: [0.1, 0.1, 0.15, 1], opacity: 1, rotation: 0,
    blendMode: 'normal', shader: 'shapes', params: [],
    layer: 0, startTime: 0, duration: 10,
  });

  scene.addEntity({
    id: 'title', type: 'rect',
    x: 660, y: 400, width: 600, height: 80,
    color: [0.9, 0.3, 0.4, 1], opacity: 1, rotation: 0,
    blendMode: 'normal', shader: 'shapes', params: [],
    layer: 1, startTime: 0, duration: 10,
  });

  // Viewport state
  const viewport: Viewport = {
    screenWidth: 800, screenHeight: 600,
    centerX: 960, centerY: 540,
    zoom: 1, renderScale: 1,
  };

  // Render loop (manual — two canvases)
  function render(editCore: any, camCore: any) {
    const time = 0;

    // Edit viewport: flatten for current viewport
    const editFrame = scene.flattenForViewport(time, viewport);
    editCore.render_frame(JSON.stringify(editFrame));

    // Camera preview: flatten for camera region
    const camCanvas = document.getElementById('cameraCanvas') as HTMLCanvasElement;
    const camFrame = scene.flattenForCamera(time, camera,
      camCanvas.width, camCanvas.height, 1);
    camCore.render_frame(JSON.stringify(camFrame));

    requestAnimationFrame(() => render(editCore, camCore));
  }

  console.log('Dual viewport example — see README for setup.');
}

main().catch(console.error);
