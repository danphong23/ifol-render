// ══════════════════════════════════════
// ifol-render-sdk — Public API
// ══════════════════════════════════════

export type { Entity, Transform, Timeline, EntityStyle, BlendMode } from './types.js';
export type { Viewport, RenderSettings, SDKConfig } from './types.js';
export type { Frame, FlatEntity, RenderPass, TextureUpdate, PassType } from './types.js';
export { Scene } from './scene.js';
export { Flattener } from './flattener.js';
export { RingBuffer } from './ring-buffer.js';
export type { WasmBatchRenderer } from './ring-buffer.js';
export { AssetManager } from './assets.js';
export type { WasmAssetRenderer } from './assets.js';

import type { Entity, Viewport, SDKConfig } from './types.js';
import { Scene } from './scene.js';
import { Flattener } from './flattener.js';
import { RingBuffer } from './ring-buffer.js';
import { AssetManager } from './assets.js';

/**
 * IfolRenderer — high-level SDK for ifol-render.
 *
 * Wraps the WASM Core engine with a scene model, batch streaming,
 * viewport management, and asset caching.
 *
 * ## Lifecycle
 *
 * 1 IfolRenderer = 1 WASM CoreEngine = 1 Canvas = 1 WebGPU device.
 *
 * For multiple viewports (e.g. main preview + thumbnail), create
 * multiple IfolRenderer instances, each bound to its own canvas.
 * They share the same GPU adapter but have independent state.
 *
 * ## Usage
 *
 * ```ts
 * const renderer = await IfolRenderer.create({
 *   canvas: document.getElementById('viewport'),
 *   settings: { width: 1280, height: 720, fps: 30 },
 * });
 *
 * renderer.addEntity({
 *   id: 'bg', type: 'video',
 *   source: 'background.mp4',
 *   transform: { x: 0, y: 0, width: 1920, height: 1080 },
 *   timeline: { start: 0, duration: 30 },
 * });
 *
 * renderer.play();
 * ```
 */
export class IfolRenderer {
  /** WASM renderer instance */
  private wasm: any; // IfolRenderWeb from wasm-bindgen
  readonly scene: Scene;
  readonly flattener: Flattener;
  readonly ringBuffer: RingBuffer;
  readonly assets: AssetManager;

  private viewport: Viewport = { zoom: 1, panX: 0, panY: 0 };
  private playing = false;
  private currentFrame = 0;
  private playbackStartTime = 0;
  private playbackStartFrame = 0;
  private animFrameId: number | null = null;
  private renderWidth: number;
  private renderHeight: number;

  private constructor(
    wasm: any,
    scene: Scene,
    width: number,
    height: number,
    assetBaseUrl: string,
  ) {
    this.wasm = wasm;
    this.scene = scene;
    this.flattener = new Flattener(scene);
    this.ringBuffer = new RingBuffer(wasm);
    this.assets = new AssetManager(wasm, assetBaseUrl);
    this.renderWidth = width;
    this.renderHeight = height;
  }

  /**
   * Create a new IfolRenderer bound to a canvas.
   *
   * This initializes the WASM module, creates a WebGPU device,
   * and sets up the rendering pipeline.
   *
   * @param config - Canvas, render settings, and optional asset URL
   * @param wasmInit - WASM initialization function (from ifol-render-wasm package)
   * @returns Promise<IfolRenderer>
   */
  static async create(
    config: SDKConfig,
    wasmInit: () => Promise<any>,
  ): Promise<IfolRenderer> {
    // Initialize WASM module
    const wasmModule = await wasmInit();
    const { IfolRenderWeb } = wasmModule;

    const { width, height, fps } = config.settings;
    const wasm = await new IfolRenderWeb(config.canvas, width, height, fps);
    wasm.setup_builtins();

    const scene = new Scene({
      width, height, fps,
      duration: 0, // Will be set by user
    });

    const baseUrl = config.assetBaseUrl ?? 'http://localhost:8000/asset?path=';

    return new IfolRenderer(wasm, scene, width, height, baseUrl);
  }

  // ── Entity Operations ──

  addEntity(entity: Entity): void {
    this.scene.addEntity(entity);
    this.invalidateBuffer();
  }

  updateEntity(id: string, patch: Partial<Entity>): void {
    this.scene.updateEntity(id, patch);
    this.invalidateBuffer();
  }

  removeEntity(id: string): void {
    this.scene.removeEntity(id);
    this.invalidateBuffer();
  }

  // ── Viewport ──

  setViewport(viewport: Partial<Viewport>): void {
    this.viewport = { ...this.viewport, ...viewport };
    this.invalidateBuffer();
  }

  getViewport(): Viewport {
    return { ...this.viewport };
  }

  // ── Resize ──

  resize(width: number, height: number): void {
    this.renderWidth = width;
    this.renderHeight = height;
    this.wasm.resize(width, height); // auto-clears WASM buffer
    this.invalidateBuffer();
  }

  // ── Playback ──

  /** Seek to a specific time and render that frame */
  async seekTo(time: number): Promise<void> {
    const fps = this.scene.fps;
    this.currentFrame = Math.floor(time * fps);

    // Flatten single frame
    const frame = this.flattener.flattenSingle(
      time, this.renderWidth, this.renderHeight, this.viewport,
    );

    // TODO: preload assets for this frame
    const json = JSON.stringify(frame);
    this.wasm.render_frame(json);
  }

  /** Start continuous playback from current position */
  play(): void {
    if (this.playing) return;
    this.playing = true;
    this.playbackStartTime = performance.now();
    this.playbackStartFrame = this.currentFrame;

    // Pre-fill buffer
    this.fillBuffer();

    // Start render loop
    this.animFrameId = requestAnimationFrame((ts) => this.playLoop(ts));
  }

  /** Pause playback */
  pause(): void {
    this.playing = false;
    if (this.animFrameId !== null) {
      cancelAnimationFrame(this.animFrameId);
      this.animFrameId = null;
    }
  }

  get isPlaying(): boolean { return this.playing; }
  get currentTime(): number { return this.currentFrame / this.scene.fps; }

  // ── Internal ──

  private playLoop(timestamp: number): void {
    if (!this.playing) return;

    const fps = this.scene.fps;
    const elapsed = (timestamp - this.playbackStartTime) / 1000.0;
    const targetFrame = this.playbackStartFrame + Math.floor(elapsed * fps);

    // Render from buffer
    if (targetFrame !== this.currentFrame) {
      this.currentFrame = targetFrame;
      if (!this.ringBuffer.render(this.currentFrame)) {
        // Frame not in buffer — flatten single frame fallback
        const time = this.currentFrame / fps;
        const frame = this.flattener.flattenSingle(
          time, this.renderWidth, this.renderHeight, this.viewport,
        );
        this.wasm.render_frame(JSON.stringify(frame));
      }
    }

    // Prefetch if buffer running low
    if (this.ringBuffer.needsPrefetch(this.currentFrame)) {
      this.fillBuffer();
    }

    this.animFrameId = requestAnimationFrame((ts) => this.playLoop(ts));
  }

  private fillBuffer(): void {
    const fps = this.scene.fps;
    const startFrame = this.ringBuffer.endFrame || this.currentFrame;
    const startTime = startFrame / fps;

    // Flatten batch (time-budgeted: 16ms)
    const batch = this.flattener.flattenBatch(
      startTime, 90, // up to 3 seconds
      this.renderWidth, this.renderHeight,
      this.viewport,
      16, // budget: 16ms
    );

    if (batch.length > 0) {
      this.ringBuffer.push(batch, startFrame);
    }
  }

  private invalidateBuffer(): void {
    this.ringBuffer.clear();
    this.scene.markClean();
  }

  /** Direct access to WASM renderer (escape hatch) */
  get wasmRenderer(): any {
    return this.wasm;
  }
}
