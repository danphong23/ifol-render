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
   * Seeks the video to the timestamp, draws to canvas, and caches in Core.
   */
  async extractVideoFrame(key: string, timestamp: number): Promise<void> {
    const asset = this.videos.get(key);
    if (!asset?.element) throw new Error(`Video '${key}' not loaded`);

    const video = asset.element;
    const w = asset.pixelWidth;
    const h = asset.pixelHeight;

    // Seek to timestamp
    video.currentTime = Math.max(0, Math.min(timestamp, asset.duration));

    await new Promise<void>((resolve) => {
      video.onseeked = () => resolve();
      // If already at correct time, seeked won't fire
      if (Math.abs(video.currentTime - timestamp) < 0.01) resolve();
    });

    // Draw to canvas and extract RGBA
    const canvas = new OffscreenCanvas(w, h);
    const ctx = canvas.getContext('2d')!;
    ctx.drawImage(video, 0, 0, w, h);
    const rgba = new Uint8Array(ctx.getImageData(0, 0, w, h).data.buffer);

    // Cache in Core
    this.coreVideoCache?.(key, timestamp, rgba, w, h);
  }

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
