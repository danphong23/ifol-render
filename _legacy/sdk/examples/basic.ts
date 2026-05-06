/**
 * Basic Example — ifol-render SDK
 *
 * Minimal setup: create renderer, add shapes, start render loop.
 *
 * Usage:
 *   <canvas id="viewport" style="width:800px;height:600px"></canvas>
 *   <script type="module" src="basic.ts"></script>
 */

import { IfolRenderer } from '../src/index.js';

async function main() {
  const renderer = await IfolRenderer.create({
    canvas: document.getElementById('viewport') as HTMLCanvasElement,
    scene: { ppu: 1, fps: 30, duration: 10 },
  });

  // Add colored rectangles
  renderer.addShape('red', 'rect', {
    x: 100, y: 100, width: 300, height: 200,
    color: [0.9, 0.2, 0.3, 1.0],
  });

  renderer.addShape('blue', 'rect', {
    x: 500, y: 300, width: 200, height: 200,
    color: [0.2, 0.4, 0.9, 0.8],
  });

  renderer.addShape('circle1', 'circle', {
    x: 800, y: 200, width: 150, height: 150,
    color: [0.1, 0.8, 0.5, 1.0],
  });

  // Set viewport to center on the scene
  renderer.setViewport({
    centerX: 600, centerY: 400,
    zoom: 1, renderScale: 1,
  });

  // Start render loop
  renderer.startLoop();

  console.log('Basic example running. Open DevTools for logs.');
}

main().catch(console.error);
