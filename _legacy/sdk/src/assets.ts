// ══════════════════════════════════════════════════
// ifol-render-sdk — Asset Manager
// Handles image loading, video frame extraction, and
// cache lifecycle. SDK layer — no direct GPU access.
// ══════════════════════════════════════════════════

import type { ImageAsset, VideoAsset } from './types.js';

/**
 * AssetManager — loads and decodes media for Core.
 *
 * Images: fetch → createImageBitmap → OffscreenCanvas → RGBA bytes
 *   → cache_image(key, bytes) once → Core decodes + GPU texture
 *
 * Videos: fetch → <video> → Canvas2D extract per frame
 *   → cache_video_frame(key, t, rgba, w, h) each tick
 */
export class AssetManager {
  private images = new Map<string, ImageAsset>();
  private videos = new Map<string, VideoAsset>();

  /** Reference to core's cache_image function (injected). */
  private coreCache?: (key: string, data: Uint8Array) => void;
  /** Reference to core's cache_video_frame function (injected). */
  private coreVideoCache?: (key: string, t: number, data: Uint8Array, w: number, h: number) => void;
  /** Reference to core's clear_video_frames function (injected). */
  private coreClearVideo?: () => void;
  /** URL resolver — transforms paths for fetch (e.g. asset server proxy). */
  private urlResolver: (path: string) => string;
  /** PPU for computing unit sizes. */
  private ppu: number;

  constructor(opts: {
    ppu: number;
    urlResolver?: (path: string) => string;
    coreCache?: (key: string, data: Uint8Array) => void;
    coreVideoCache?: (key: string, t: number, data: Uint8Array, w: number, h: number) => void;
    coreClearVideo?: () => void;
  }) {
    this.ppu = opts.ppu;
    this.urlResolver = opts.urlResolver ?? ((p) => p);
    this.coreCache = opts.coreCache;
    this.coreVideoCache = opts.coreVideoCache;
    this.coreClearVideo = opts.coreClearVideo;
  }

  // ── Image ──

  /**
   * Load an image from URL/path.
   * Fetches, decodes to RGBA, caches in Core, and returns asset metadata.
   */
  async loadImage(key: string, urlOrPath: string): Promise<ImageAsset> {
    // Already loaded?
    const existing = this.images.get(key);
    if (existing?.cached) return existing;

    const url = this.urlResolver(urlOrPath);
    const response = await fetch(url);
    if (!response.ok) throw new Error(`Failed to fetch image: ${url} (${response.status})`);

    const blob = await response.blob();
    const bitmap = await createImageBitmap(blob);

    const w = bitmap.width;
    const h = bitmap.height;

    // Extract RGBA bytes
    const canvas = new OffscreenCanvas(w, h);
    const ctx = canvas.getContext('2d')!;
    ctx.drawImage(bitmap, 0, 0);
    const rgba = ctx.getImageData(0, 0, w, h).data;
    bitmap.close();

    // Inject raw bytes into Core cache (Core will decode on first LoadImage)
    // We send the original blob bytes so Core's image::load_from_memory can decode
    const rawBytes = new Uint8Array(await blob.arrayBuffer());
    this.coreCache?.(key, rawBytes);

    const asset: ImageAsset = {
      key,
      url: urlOrPath,
      pixelWidth: w,
      pixelHeight: h,
      unitWidth: w / this.ppu,
      unitHeight: h / this.ppu,
      cached: true,
    };
    this.images.set(key, asset);
    return asset;
  }

  getImage(key: string): ImageAsset | undefined {
    return this.images.get(key);
  }

  // ── Video ──

  /**
   * Load a video and prepare for frame extraction.
   * Creates a hidden <video> element, loads metadata to get dimensions/duration.
   */
  async loadVideo(key: string, urlOrPath: string): Promise<VideoAsset> {
    const existing = this.videos.get(key);
    if (existing?.element) return existing;

    const url = this.urlResolver(urlOrPath);

    return new Promise<VideoAsset>((resolve, reject) => {
      const video = document.createElement('video');
      video.crossOrigin = 'anonymous';
      video.preload = 'auto';
      video.muted = true;
      video.playsInline = true;
      video.style.display = 'none';
      document.body.appendChild(video);

      video.onloadedmetadata = () => {
        const asset: VideoAsset = {
          key,
          url: urlOrPath,
          pixelWidth: video.videoWidth,
          pixelHeight: video.videoHeight,
          unitWidth: video.videoWidth / this.ppu,
          unitHeight: video.videoHeight / this.ppu,
          duration: video.duration,
          element: video,
        };
        this.videos.set(key, asset);
        resolve(asset);
      };

      video.onerror = () => reject(new Error(`Failed to load video: ${url}`));
      video.src = url;
    });
  }

  /**
   * Extract a single video frame as RGBA at the given timestamp.
   * Seeks the video, draws to canvas, extracts raw RGBA pixels,
   * and pushes through coreVideoCache → Core's cache_video_frame().
   *
   * This matches the proven pipeline from web/main.js (DecodeVideoFrame path).
   */
  async extractVideoFrame(key: string, timestamp: number): Promise<void> {
    const asset = this.videos.get(key);
    if (!asset?.element) throw new Error(`Video '${key}' not loaded`);

    const video = asset.element;

    // Reduced resolution for performance (max 640px width)
    const maxW = 640;
    const scale = Math.min(1, maxW / asset.pixelWidth);
    const w = Math.round(asset.pixelWidth * scale);
    const h = Math.round(asset.pixelHeight * scale);

    // Only seek if timestamp changed significantly (seeking is the slow part)
    const target = Math.max(0, Math.min(timestamp, asset.duration));
    if (Math.abs(video.currentTime - target) > 0.05) {
      video.currentTime = target;
      await new Promise<void>((resolve) => {
        const onSeeked = () => { video.removeEventListener('seeked', onSeeked); resolve(); };
        video.addEventListener('seeked', onSeeked);
        setTimeout(resolve, 100);
      });
    }

    // Reuse canvas for performance
    if (!this.videoCanvas || this.videoCanvas.width !== w || this.videoCanvas.height !== h) {
      this.videoCanvas = new OffscreenCanvas(w, h);
      this.videoCtx = this.videoCanvas.getContext('2d', { willReadFrequently: true })!;
    }

    this.videoCtx!.drawImage(video, 0, 0, w, h);

    // Extract raw RGBA pixels (same as working main.js approach)
    const imageData = this.videoCtx!.getImageData(0, 0, w, h);
    const rgba = new Uint8Array(imageData.data.buffer);

    // Push RGBA through coreVideoCache → Core's cache_video_frame(key, t, rgba, w, h)
    // This lands in backend.video_frames[key@t] which decode_video_frame() reads from.
    this.coreVideoCache?.(key, timestamp, rgba, w, h);
  }

  /**
   * Prepare all video frames needed for the current time.
   * Call this once per render tick — SDK handles all video entities automatically.
   *
   * ```ts
   * // In your render loop:
   * await assets.prepareVideoFrames(scene.allEntities(), time);
   * const frame = view.flattenAt(time);
   * core.render_frame(JSON.stringify(frame));
   * ```
   */
  async prepareVideoFrames(entities: Iterable<{ type: string; source?: string; startTime: number }>, time: number): Promise<void> {
    // Clear old video frames to prevent memory bloat
    this.coreClearVideo?.();

    for (const e of entities) {
      if (e.type === 'video' && e.source) {
        const va = this.videos.get(e.source);
        if (va?.element && va.element.readyState >= 2) {
          const localTime = Math.max(0, Math.min(time - e.startTime, va.duration));
          try { await this.extractVideoFrame(e.source, localTime); } catch (_) {}
        }
      }
    }
  }

  private videoCanvas: OffscreenCanvas | null = null;
  private videoCtx: OffscreenCanvasRenderingContext2D | null = null;

  getVideo(key: string): VideoAsset | undefined {
    return this.videos.get(key);
  }

  // ── Cache Lifecycle ──

  /** Get all image keys currently in use. */
  imageKeys(): string[] {
    return [...this.images.keys()];
  }

  /** Get all video keys currently in use. */
  videoKeys(): string[] {
    return [...this.videos.keys()];
  }

  /** Remove an image from the asset registry. Does NOT evict Core texture. */
  removeImage(key: string): void {
    this.images.delete(key);
  }

  /** Remove a video: stop element, remove from DOM, clear from registry. */
  removeVideo(key: string): void {
    const asset = this.videos.get(key);
    if (asset?.element) {
      asset.element.pause();
      asset.element.src = '';
      asset.element.remove();
    }
    this.videos.delete(key);
  }

  /** Clear all cached video frames from Core WASM memory. */
  clearVideoFrames(): void {
    this.coreClearVideo?.();
  }

  /** Destroy all assets and release resources. */
  destroy(): void {
    for (const key of this.videos.keys()) this.removeVideo(key);
    this.images.clear();
    this.videos.clear();
  }
}
