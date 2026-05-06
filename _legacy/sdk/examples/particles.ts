/**
 * Particles Stress Test — ifol-render SDK
 *
 * Spawns N particles with deterministic bounce animation.
 * Tests rendering performance with many entities.
 *
 * Usage:
 *   <canvas id="viewport" style="width:100%;height:100%"></canvas>
 *   <script type="module" src="particles.ts"></script>
 */

import { IfolRenderer } from '../src/index.js';

const PARTICLE_COUNT = 500;
const SCENE_W = 1920, SCENE_H = 1080;

async function main() {
  const renderer = await IfolRenderer.create({
    canvas: document.getElementById('viewport') as HTMLCanvasElement,
    scene: { ppu: 1, fps: 60, duration: 30 },
  });

  // Spawn particles
  for (let i = 0; i < PARTICLE_COUNT; i++) {
    const t = i / PARTICLE_COUNT;
    renderer.addShape(`p${i}`, 'rect', {
      x: SCENE_W / 2 + Math.cos(t * Math.PI * 2) * 400,
      y: SCENE_H / 2 + Math.sin(t * Math.PI * 2) * 400,
      width: 6 + Math.random() * 20,
      height: 6 + Math.random() * 20,
      color: [
        0.3 + Math.random() * 0.7,
        0.3 + Math.random() * 0.7,
        0.3 + Math.random() * 0.7,
        0.4 + Math.random() * 0.6,
      ],
    });
  }

  // Center viewport
  renderer.setViewport({
    centerX: SCENE_W / 2,
    centerY: SCENE_H / 2,
    zoom: 1,
    renderScale: 1,
  });

  // FPS counter
  let frames = 0, lastTick = performance.now();
  renderer.onFrameCallback(() => {
    frames++;
    const now = performance.now();
    if (now - lastTick >= 1000) {
      console.log(`FPS: ${frames}, Entities: ${PARTICLE_COUNT}`);
      frames = 0;
      lastTick = now;
    }
  });

  renderer.play();
  renderer.startLoop();
}

main().catch(console.error);
