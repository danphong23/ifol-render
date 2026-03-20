// ══════════════════════════════════════════════════════
// ifol-render-sdk — Public API
//
// IfolRenderer is the single entry-point for developers.
// It orchestrates Scene (units), AssetManager (media),
// and Core WASM (GPU). Developers never touch WASM directly.
//
// See docs/UNIT_SYSTEM.md for coordinate system spec.
// ══════════════════════════════════════════════════════

import type {
  Entity, EntityType, SceneSettings, Viewport, Camera,
  ExportSettings, ImageAsset, VideoAsset, BlendMode,
} from './types.js';
import { Scene, viewportRegion } from './scene.js';
import { AssetManager } from './assets.js';

export type { Entity, EntityType, SceneSettings, Viewport, Camera, ExportSettings, ImageAsset, VideoAsset, BlendMode };
export { Scene, viewportRegion, AssetManager };

/** Options for creating an IfolRenderer instance. */
export interface RendererOptions {
  /** Canvas element to render to. */
  canvas: HTMLCanvasElement;
  /** Scene settings. */
  scene?: Partial<SceneSettings>;
  /**
   * URL resolver for asset loading.
   * Default: identity (pass path as-is to fetch).
   * Example for asset server: (p) => `http://localhost:8000/asset?path=${encodeURIComponent(p)}`
   */
  urlResolver?: (path: string) => string;
}

/**
 * IfolRenderer — the main SDK class.
 *
 * ```ts
 * const r = await IfolRenderer.create({ canvas: myCanvas });
 * await r.addImage('bg', '/photo.jpg');
 * r.addEntity({ id: 'bg', type: 'image', source: 'bg', x: 0, y: 0, width: 100, height: 75, ... });
 * r.play();
 * ```
 *
 * One IfolRenderer = one canvas = one Core instance = one GPU device.
 * For multiple viewports, create multiple IfolRenderer instances.
 */
export class IfolRenderer {
  readonly scene: Scene;
  readonly assets: AssetManager;
  private core: any; // IfolRenderWeb from WASM
  private canvas: HTMLCanvasElement;

  // Viewport state
  private viewport: Viewport;
  private _camera: Camera = { x: 0, y: 0, width: 1920, height: 1080 };

  // Playback
  private playing = false;
  private currentTime = 0;
  private playStartWall = 0;
  private playStartTime = 0;
  private animFrameId: number | null = null;

  // Callbacks
  private onFrame?: (time: number) => void;
  private onReady?: () => void;

  private constructor(
    canvas: HTMLCanvasElement,
    core: any,
    scene: Scene,
    assets: AssetManager,
  ) {
    this.canvas = canvas;
    this.core = core;
    this.scene = scene;
    this.assets = assets;
    this.viewport = {
      screenWidth: canvas.clientWidth || canvas.width,
      screenHeight: canvas.clientHeight || canvas.height,
      centerX: 960,
      centerY: 540,
      zoom: 1,
      renderScale: 1,
    };
  }

  /**
   * Create a new renderer.
   * Initializes WASM Core, attaches to canvas, and sets up builtins.
   */
  static async create(opts: RendererOptions): Promise<IfolRenderer> {
    // Dynamic import of WASM — allows SDK to work without bundling WASM
    const wasm = await import('../../crates/wasm/pkg/ifol_render_wasm.js');
    await wasm.default();

    const canvas = opts.canvas;
    const w = canvas.clientWidth || canvas.width || 800;
    const h = canvas.clientHeight || canvas.height || 600;
    canvas.width = w;
    canvas.height = h;

    const settings: SceneSettings = {
      ppu: opts.scene?.ppu ?? 1,
      fps: opts.scene?.fps ?? 30,
      duration: opts.scene?.duration ?? 10,
    };

    const core = await new wasm.IfolRenderWeb(canvas, w, h, settings.fps);
    core.setup_builtins();

    const assets = new AssetManager({
      ppu: settings.ppu,
      urlResolver: opts.urlResolver,
      coreCache: (key, data) => core.cache_image(key, data),
      coreVideoCache: (key, t, data, w, h) => core.cache_video_frame(key, t, data, w, h),
      coreClearVideo: () => core.clear_video_frames(),
    });

    const scene = new Scene(settings);

    return new IfolRenderer(canvas, core, scene, assets);
  }

  // ── Entity API (convenience wrappers around scene) ──

  /**
   * Add a shape entity (rect or circle).
   * Position and size are in world units.
   */
  addShape(id: string, type: 'rect' | 'circle', opts: {
    x: number; y: number; width: number; height: number;
    color?: [number, number, number, number];
    opacity?: number;
    rotation?: number;
    layer?: number;
    startTime?: number;
    duration?: number;
  }): void {
    this.scene.addEntity({
      id, type,
      x: opts.x, y: opts.y,
      width: opts.width, height: opts.height,
      color: opts.color ?? [1, 1, 1, 1],
      opacity: opts.opacity ?? 1,
      rotation: opts.rotation ?? 0,
      blendMode: 'normal',
      shader: 'shapes',
      params: type === 'circle' ? [1.0] : [],
      layer: opts.layer ?? this.scene.entityCount(),
      startTime: opts.startTime ?? 0,
      duration: opts.duration ?? this.scene.duration,
      source: undefined,
    });
  }

  /**
   * Add an image entity. Fetches and decodes the image, then creates entity
   * with size computed from image pixel dimensions / PPU.
   */
  async addImage(id: string, urlOrPath: string, opts?: {
    x?: number; y?: number;
    width?: number; height?: number;
    opacity?: number;
    layer?: number;
    startTime?: number;
    duration?: number;
  }): Promise<ImageAsset> {
    const asset = await this.assets.loadImage(id, urlOrPath);
    this.scene.addEntity({
      id, type: 'image',
      x: opts?.x ?? 0,
      y: opts?.y ?? 0,
      width: opts?.width ?? asset.unitWidth,
      height: opts?.height ?? asset.unitHeight,
      color: [1, 1, 1, 1],
      opacity: opts?.opacity ?? 1,
      rotation: 0,
      blendMode: 'normal',
      source: id, // asset key = entity id for simplicity
      shader: 'composite',
      params: [],
      layer: opts?.layer ?? this.scene.entityCount(),
      startTime: opts?.startTime ?? 0,
      duration: opts?.duration ?? this.scene.duration,
    });
    return asset;
  }

  /**
   * Add a video entity. Loads video metadata, creates entity with
   * size computed from video dimensions / PPU.
   */
  async addVideo(id: string, urlOrPath: string, opts?: {
    x?: number; y?: number;
    width?: number; height?: number;
    opacity?: number;
    layer?: number;
    startTime?: number;
    duration?: number;
  }): Promise<VideoAsset> {
    const asset = await this.assets.loadVideo(id, urlOrPath);
    this.scene.addEntity({
      id, type: 'video',
      x: opts?.x ?? 0,
      y: opts?.y ?? 0,
      width: opts?.width ?? asset.unitWidth,
      height: opts?.height ?? asset.unitHeight,
      color: [1, 1, 1, 1],
      opacity: opts?.opacity ?? 1,
      rotation: 0,
      blendMode: 'normal',
      source: id,
      shader: 'composite',
      params: [],
      layer: opts?.layer ?? this.scene.entityCount(),
      startTime: opts?.startTime ?? 0,
      duration: opts?.duration ?? asset.duration,
    });
    return asset;
  }

  /** Remove an entity and clean up its assets if no other entity uses them. */
  removeEntity(id: string): void {
    const entity = this.scene.removeEntity(id);
    if (!entity) return;

    // Check if any remaining entity uses the same source
    if (entity.source) {
      const stillUsed = this.scene.allEntities().some(e => e.source === entity.source);
      if (!stillUsed) {
        this.assets.removeImage(entity.source);
        this.assets.removeVideo(entity.source);
        // Evict Core texture
        this.renderFrame({ passes: [], texture_updates: [{ Evict: { key: entity.source } }] });
      }
    }
  }

  /** Update entity properties. */
  updateEntity(id: string, patch: Partial<Entity>): void {
    this.scene.updateEntity(id, patch);
  }

  // ── Viewport ──

  /** Update viewport settings. */
  setViewport(patch: Partial<Viewport>): void {
    Object.assign(this.viewport, patch);
    this.syncCanvasSize();
  }

  getViewport(): Readonly<Viewport> {
    return { ...this.viewport };
  }

  /** Resize viewport to match current canvas container size. */
  syncCanvasSize(): void {
    const w = this.canvas.clientWidth || this.canvas.width;
    const h = this.canvas.clientHeight || this.canvas.height;
    this.viewport.screenWidth = w;
    this.viewport.screenHeight = h;

    const renderW = Math.round(w * this.viewport.renderScale);
    const renderH = Math.round(h * this.viewport.renderScale);

    if (this.canvas.width !== renderW || this.canvas.height !== renderH) {
      this.canvas.width = renderW;
      this.canvas.height = renderH;
      this.core.resize(renderW, renderH);
    }
  }

  // ── Camera ──

  setCamera(cam: Partial<Camera>): void {
    Object.assign(this._camera, cam);
  }

  getCamera(): Readonly<Camera> {
    return { ...this._camera };
  }

  // ── Playback ──

  get isPlaying(): boolean { return this.playing; }
  get time(): number { return this.currentTime; }

  play(): void {
    if (this.playing) return;
    this.playing = true;
    this.playStartWall = performance.now();
    this.playStartTime = this.currentTime;
  }

  pause(): void {
    this.playing = false;
  }

  stop(): void {
    this.playing = false;
    this.currentTime = 0;
  }

  seekTo(time: number): void {
    this.currentTime = Math.max(0, Math.min(time, this.scene.duration));
    if (this.playing) {
      this.playStartWall = performance.now();
      this.playStartTime = this.currentTime;
    }
    this.assets.clearVideoFrames();
  }

  /** Set callback invoked each animation frame with current time. */
  onFrameCallback(cb: (time: number) => void): void {
    this.onFrame = cb;
  }

  // ── Render Loop ──

  /** Start the animation frame loop. Call once after setup. */
  startLoop(): void {
    if (this.animFrameId !== null) return;
    const tick = () => {
      this.animFrameId = requestAnimationFrame(tick);
      this.tick();
    };
    tick();
  }

  /** Stop the animation frame loop. */
  stopLoop(): void {
    if (this.animFrameId !== null) {
      cancelAnimationFrame(this.animFrameId);
      this.animFrameId = null;
    }
  }

  /** Single tick: update time, extract video frames, flatten, render. */
  async tick(): Promise<void> {
    // Update time
    if (this.playing) {
      this.currentTime = this.playStartTime +
        (performance.now() - this.playStartWall) / 1000;
      if (this.currentTime >= this.scene.duration) {
        this.currentTime = 0;
        this.playStartTime = 0;
        this.playStartWall = performance.now();
      }
    }

    // Extract video frames for current time
    await this.extractVideoFrames(this.currentTime);

    // Flatten + render
    const frame = this.scene.flattenForViewport(this.currentTime, this.viewport);
    this.renderFrame(frame);

    this.onFrame?.(this.currentTime);
  }

  /** Render a single frame to the canvas. */
  renderFrame(frame: { passes: any[]; texture_updates: any[] }): void {
    try {
      this.core.render_frame(JSON.stringify(frame));
    } catch (e) {
      console.warn('[IfolRenderer] render error:', e);
    }
  }

  /** Extract video frames needed for the given time. */
  private async extractVideoFrames(time: number): Promise<void> {
    const entities = this.scene.visibleAt(time);
    for (const e of entities) {
      if (e.type === 'video' && e.source) {
        const localTime = time - e.startTime;
        try {
          await this.assets.extractVideoFrame(e.source, localTime);
        } catch {
          // Video not yet loaded or seek failed — skip this frame
        }
      }
    }
  }

  // ── Coordinate conversion (delegated to scene) ──

  canvasToWorld(canvasX: number, canvasY: number): { x: number; y: number } {
    return this.scene.canvasToWorld(canvasX, canvasY, this.viewport);
  }

  worldToCanvas(worldX: number, worldY: number): { x: number; y: number } {
    return this.scene.worldToCanvas(worldX, worldY, this.viewport);
  }

  // ── Cleanup ──

  destroy(): void {
    this.stopLoop();
    this.assets.destroy();
  }
}
