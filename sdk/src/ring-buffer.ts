// ══════════════════════════════════════
// Ring Buffer Manager — wraps WASM batch API
// ══════════════════════════════════════

import type { Frame } from './types.js';

/** Minimal interface for the WASM renderer's batch API */
export interface WasmBatchRenderer {
  push_frames(frames_json: string): number;
  push_frames_scaled(frames_json: string, scene_width: number, scene_height: number): number;
  render_at(index: number): boolean;
  clear_frames(): void;
  buffered_count(): number;
  resize(width: number, height: number): void;
}

/**
 * Ring buffer manager — wraps WASM push/render/clear with
 * prefetch logic and playhead tracking.
 *
 * The buffer stores frames starting from `bufferStartFrame`.
 * `render(globalFrame)` translates to `render_at(globalFrame - bufferStartFrame)`.
 */
export class RingBuffer {
  private renderer: WasmBatchRenderer;
  /** Global frame index of the first frame in the buffer */
  private bufferStartFrame: number = 0;
  /** Number of frames currently buffered */
  private _bufferedCount: number = 0;
  /** Prefetch threshold: request more frames when remaining < this */
  readonly prefetchThreshold: number;

  constructor(renderer: WasmBatchRenderer, prefetchThreshold: number = 30) {
    this.renderer = renderer;
    this.prefetchThreshold = prefetchThreshold;
  }

  /**
   * Push a batch of frames, starting at the given global frame index.
   * If this is the first push after a clear, sets the buffer start position.
   */
  push(frames: Frame[], startFrame: number): void {
    if (this._bufferedCount === 0) {
      this.bufferStartFrame = startFrame;
    }

    const json = JSON.stringify(frames);
    this._bufferedCount = this.renderer.push_frames(json);
  }

  /**
   * Render a frame at the given global frame index.
   * Returns false if the frame is not in the buffer.
   */
  render(globalFrame: number): boolean {
    const localIndex = globalFrame - this.bufferStartFrame;
    if (localIndex < 0 || localIndex >= this._bufferedCount) {
      return false;
    }
    return this.renderer.render_at(localIndex);
  }

  /**
   * Clear all frames. Call when viewport/entity changes invalidate the buffer.
   */
  clear(): void {
    this.renderer.clear_frames();
    this._bufferedCount = 0;
    this.bufferStartFrame = 0;
  }

  /** Number of frames currently buffered */
  get bufferedCount(): number { return this._bufferedCount; }

  /** Global frame index of the first buffered frame */
  get startFrame(): number { return this.bufferStartFrame; }

  /** Global frame index AFTER the last buffered frame */
  get endFrame(): number { return this.bufferStartFrame + this._bufferedCount; }

  /**
   * Check if buffer is running low relative to the current playhead.
   * Returns true if fewer than `prefetchThreshold` frames remain ahead of `currentFrame`.
   */
  needsPrefetch(currentFrame: number): boolean {
    const remaining = this.endFrame - currentFrame;
    return remaining < this.prefetchThreshold;
  }

  /**
   * Check if a specific global frame is available in the buffer.
   */
  has(globalFrame: number): boolean {
    const local = globalFrame - this.bufferStartFrame;
    return local >= 0 && local < this._bufferedCount;
  }
}
