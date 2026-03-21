// ══════════════════════════════════════════════════
// ifol-render-sdk — RenderView
//
// RenderView = Scene × Camera → Frame JSON
// Each RenderView produces independent frames.
// Multiple RenderViews can share the same Scene.
// ══════════════════════════════════════════════════

import type { Camera, Entity, Frame } from './types.js';
import type { Scene } from './scene.js';
import { flatten } from './flatten.js';

/**
 * RenderView — produces Frame JSON for one output.
 *
 * ```ts
 * const scene = new Scene({ ppu: 1, duration: 10 });
 * const cam = new FreeCamera({ ... });
 *
 * const editView = new RenderView(scene, cam);
 * const frame = editView.flatten(currentTime);
 * core.render_frame(JSON.stringify(frame));
 * ```
 *
 * Multi-view:
 * ```ts
 * const editView  = new RenderView(scene, freeCam);
 * const camView   = new RenderView(scene, boundCam);
 * // Both read from same scene, produce different Frame JSON
 * ```
 */
export class RenderView {
  readonly scene: Scene;
  camera: Camera;

  /** GPU render quality: 0.25–1.0. Affects backing resolution. */
  renderScale: number;
  /** CSS display width in pixels. */
  displayWidth: number;
  /** CSS display height in pixels. */
  displayHeight: number;

  /** Entity IDs to exclude from this view's output. */
  excludeIds: Set<string>;

  constructor(
    scene: Scene,
    camera: Camera,
    opts?: {
      renderScale?: number;
      displayWidth?: number;
      displayHeight?: number;
      excludeIds?: Set<string>;
    },
  ) {
    this.scene = scene;
    this.camera = camera;
    this.renderScale = opts?.renderScale ?? 1;
    this.displayWidth = opts?.displayWidth ?? 800;
    this.displayHeight = opts?.displayHeight ?? 600;
    this.excludeIds = opts?.excludeIds ?? new Set();
  }

  /**
   * Produce Frame JSON for this view at the given time.
   *
   * @param time - Current time in seconds
   * @returns Frame ready for Core WASM render_frame()
   */
  flattenAt(time: number): Frame {
    const region = this.camera.getRegion(time);
    const entities = this.scene.visibleAt(time, this.excludeIds);
    const renderW = Math.round(this.displayWidth * this.renderScale);
    const renderH = Math.round(this.displayHeight * this.renderScale);
    return flatten(entities, region, renderW, renderH, time);
  }

  /**
   * Flatten for export at a specific pixel resolution.
   * Ignores displayWidth/Height and renderScale — uses export dimensions directly.
   */
  flattenForExport(time: number, exportW: number, exportH: number): Frame {
    const region = this.camera.getRegion(time);
    const entities = this.scene.visibleAt(time, this.excludeIds);
    return flatten(entities, region, exportW, exportH, time);
  }

  /**
   * Generate all export frames as an iterator.
   * App Layer decides fps.
   *
   * ```ts
   * for (const frame of view.exportFrames(30, 1920, 1080)) {
   *   frames.push(frame);
   * }
   * ```
   */
  *exportFrames(fps: number, exportW: number, exportH: number): Generator<Frame> {
    const totalFrames = Math.ceil(this.scene.duration * fps);
    for (let i = 0; i < totalFrames; i++) {
      yield this.flattenForExport(i / fps, exportW, exportH);
    }
  }

  /**
   * Build a complete export scene JSON for the CLI/backend.
   *
   * Produces `{ settings, frames, audio_clips? }` — the exact format
   * that `ifol-render export --scene` expects.
   *
   * ```ts
   * const sceneJson = view.buildExportScene(30, 1920, 1080);
   * // POST to backend: fetch('/export', { body: JSON.stringify(sceneJson) })
   * ```
   */
  buildExportScene(
    fps: number,
    exportW: number,
    exportH: number,
    opts?: { audioClips?: Array<{ path: string; start_time: number; duration: number; volume?: number }>; background?: [number, number, number, number] },
  ): { settings: object; frames: Frame[]; audio_clips?: object[] } {
    const frames: Frame[] = [];
    for (const frame of this.exportFrames(fps, exportW, exportH)) {
      frames.push(frame);
    }
    const result: { settings: object; frames: Frame[]; audio_clips?: object[] } = {
      settings: {
        width: exportW,
        height: exportH,
        fps,
        background: opts?.background ?? [0, 0, 0, 1],
      },
      frames,
    };
    if (opts?.audioClips && opts.audioClips.length > 0) {
      result.audio_clips = opts.audioClips;
    }
    return result;
  }

  /** Update display size (triggers no recomputation — lazy). */
  resize(displayW: number, displayH: number): void {
    this.displayWidth = displayW;
    this.displayHeight = displayH;
  }
}
