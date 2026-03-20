// ══════════════════════════════════════
// Flattener — time-budgeted batch frame production
// ══════════════════════════════════════

import type { Frame, Viewport } from './types.js';
import { Scene } from './scene.js';

/**
 * Time-budgeted batch flattener.
 *
 * Converts scene model into flat Frame data in chunks, spending at most
 * `budgetMs` milliseconds per batch. This prevents UI thread blocking
 * when the scene has many entities or complex hierarchies.
 *
 * Usage:
 *   const batch = flattener.flattenBatch(startTime, 90);
 *   renderer.push_frames(JSON.stringify(batch));
 */
export class Flattener {
  private scene: Scene;

  constructor(scene: Scene) {
    this.scene = scene;
  }

  /**
   * Flatten frames starting from `startTime`, spending at most `maxFrames`
   * frames or `budgetMs` milliseconds — whichever limit is hit first.
   *
   * @param startTime - Start time in seconds
   * @param maxFrames - Maximum number of frames to flatten in this batch
   * @param targetWidth - Render target width in pixels
   * @param targetHeight - Render target height in pixels
   * @param viewport - Current viewport transform
   * @param budgetMs - Wall-clock time budget (default: 16ms = 1 rAF tick)
   * @returns Array of flattened Frame objects
   */
  flattenBatch(
    startTime: number,
    maxFrames: number,
    targetWidth: number,
    targetHeight: number,
    viewport: Viewport = { zoom: 1, panX: 0, panY: 0 },
    budgetMs: number = 16,
  ): Frame[] {
    const frames: Frame[] = [];
    const fps = this.scene.fps;
    const duration = this.scene.duration;
    const frameDuration = 1.0 / fps;
    const deadline = performance.now() + budgetMs;

    let time = startTime;

    for (let i = 0; i < maxFrames; i++) {
      // Stop if past scene end
      if (time >= duration) break;

      // Stop if time budget exceeded
      if (performance.now() >= deadline) break;

      frames.push(this.scene.flattenAt(time, targetWidth, targetHeight, viewport));
      time += frameDuration;
    }

    return frames;
  }

  /**
   * Flatten a single frame at the given time.
   * Use for scrubbing / seeking (non-playback).
   */
  flattenSingle(
    time: number,
    targetWidth: number,
    targetHeight: number,
    viewport?: Viewport,
  ): Frame {
    return this.scene.flattenAt(time, targetWidth, targetHeight, viewport);
  }
}
