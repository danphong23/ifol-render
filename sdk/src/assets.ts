// ══════════════════════════════════════
// Asset Manager — prefetch + cache to WASM
// ══════════════════════════════════════

/** Minimal WASM renderer interface for asset caching */
export interface WasmAssetRenderer {
  cache_image(path: string, data: Uint8Array): void;
  cache_video_frame(path: string, timestamp: number, data: Uint8Array, width: number, height: number): void;
  clear_video_frames(): void;
}

/**
 * Asset manager — handles fetching, caching, and lifecycle of
 * images, fonts, and video frames for the WASM renderer.
 */
export class AssetManager {
  private renderer: WasmAssetRenderer;
  private baseUrl: string;
  private cachedAssets: Set<string> = new Set();
  private videoElements: Map<string, HTMLVideoElement> = new Map();

  constructor(renderer: WasmAssetRenderer, baseUrl: string = 'http://localhost:8000/asset?path=') {
    this.renderer = renderer;
    this.baseUrl = baseUrl;
  }

  /**
   * Fetch and cache a single asset (image or font) to WASM memory.
   * Skips if already cached.
   */
  async cacheAsset(path: string): Promise<void> {
    if (this.cachedAssets.has(path)) return;

    try {
      const res = await fetch(`${this.baseUrl}${encodeURIComponent(path)}`);
      if (!res.ok) {
        console.warn(`Asset 404: ${path}`);
        return;
      }
      const buf = await res.arrayBuffer();
      this.renderer.cache_image(path, new Uint8Array(buf));
      this.cachedAssets.add(path);
    } catch (e) {
      console.warn(`Failed to fetch asset ${path}:`, e);
    }
  }

  /**
   * Ensure a video element is loaded and ready for frame extraction.
   */
  async ensureVideo(path: string): Promise<HTMLVideoElement> {
    const existing = this.videoElements.get(path);
    if (existing && existing.readyState >= 2) return existing;

    const video = document.createElement('video');
    video.crossOrigin = 'anonymous';
    video.muted = true;
    video.preload = 'auto';
    video.src = `${this.baseUrl}${encodeURIComponent(path)}`;

    return new Promise((resolve, reject) => {
      video.addEventListener('loadeddata', () => {
        this.videoElements.set(path, video);
        resolve(video);
      }, { once: true });
      video.addEventListener('error', () => {
        reject(new Error(`Failed to load video: ${path}`));
      }, { once: true });
      video.load();
    });
  }

  /**
   * Extract a video frame at the given timestamp and cache to WASM.
   * Uses Canvas2D drawImage + getImageData.
   */
  async captureVideoFrame(
    path: string,
    timestamp: number,
    width?: number,
    height?: number,
  ): Promise<void> {
    const video = await this.ensureVideo(path);
    const w = width ?? video.videoWidth;
    const h = height ?? video.videoHeight;

    // Seek to timestamp
    video.currentTime = timestamp;
    await new Promise<void>(resolve => {
      video.addEventListener('seeked', () => resolve(), { once: true });
    });

    // Extract frame
    const canvas = document.createElement('canvas');
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext('2d')!;
    ctx.drawImage(video, 0, 0, w, h);
    const imageData = ctx.getImageData(0, 0, w, h);

    this.renderer.cache_video_frame(
      path, timestamp,
      new Uint8Array(imageData.data.buffer),
      w, h,
    );
  }

  /**
   * Get a video element for continuous playback (used during play mode).
   */
  getVideo(path: string): HTMLVideoElement | undefined {
    return this.videoElements.get(path);
  }

  /** Clear all cached video frames from WASM memory */
  clearVideoFrames(): void {
    if (typeof this.renderer.clear_video_frames === 'function') {
      this.renderer.clear_video_frames();
    }
  }

  /** Check if an asset is already cached */
  isCached(path: string): boolean {
    return this.cachedAssets.has(path);
  }
}
