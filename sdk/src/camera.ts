// ══════════════════════════════════════════════════
// ifol-render-sdk — Camera System
//
// Camera defines WHAT part of the world is visible.
// Each Camera type computes a WorldRegion differently.
// Camera is NOT an entity — it's a view definition.
// ══════════════════════════════════════════════════

import type { Camera, WorldRegion, ViewportConfig } from './types.js';

/**
 * BoundCamera — fixed region in world space.
 *
 * Shows a specific rectangle (x, y, w, h) with optional rotation.
 * Used for: camera preview, export, cinematic shots.
 *
 * ```ts
 * const cam = new BoundCamera(0, 0, 1920, 1080);
 * cam.rotation = Math.PI / 4; // 45° rotation
 * ```
 */
export class BoundCamera implements Camera {
  readonly type = 'bound';
  x: number;
  y: number;
  width: number;
  height: number;
  rotation: number;

  constructor(x = 0, y = 0, width = 1920, height = 1080, rotation = 0) {
    this.x = x;
    this.y = y;
    this.width = width;
    this.height = height;
    this.rotation = rotation;
  }

  getRegion(_time: number): WorldRegion {
    return {
      left: this.x,
      top: this.y,
      width: this.width,
      height: this.height,
      rotation: this.rotation,
    };
  }

  /** Set position + size in one call. */
  set(x: number, y: number, width: number, height: number): void {
    this.x = x; this.y = y;
    this.width = width; this.height = height;
  }

  /** Copy from an entity's transform. */
  setFromEntity(e: { x: number; y: number; width: number; height: number; rotation?: number }): void {
    this.x = e.x; this.y = e.y;
    this.width = e.width; this.height = e.height;
    this.rotation = e.rotation ?? 0;
  }
}

/**
 * FreeCamera — viewport-style camera controlled by center + zoom.
 *
 * Visible region depends on screen size + zoom level.
 * Used for: edit viewports, interactive panning/zooming.
 *
 * ```ts
 * const cam = new FreeCamera(960, 540, 1);
 * cam.zoom = 2;  // zoom in 2×
 * ```
 */
export class FreeCamera implements Camera {
  readonly type = 'free';
  centerX: number;
  centerY: number;
  zoom: number;
  /** CSS pixel width of the display area. */
  screenWidth: number;
  /** CSS pixel height of the display area. */
  screenHeight: number;
  /** Pixels-per-unit (from scene). */
  private ppu: number;

  constructor(config: ViewportConfig & { ppu: number }) {
    this.centerX = config.centerX;
    this.centerY = config.centerY;
    this.zoom = config.zoom;
    this.screenWidth = config.screenWidth;
    this.screenHeight = config.screenHeight;
    this.ppu = config.ppu;
  }

  getRegion(_time: number): WorldRegion {
    const w = this.screenWidth / (this.ppu * this.zoom);
    const h = this.screenHeight / (this.ppu * this.zoom);
    return {
      left: this.centerX - w / 2,
      top: this.centerY - h / 2,
      width: w,
      height: h,
      rotation: 0,
    };
  }

  /**
   * Adjust center to preserve top-left anchor when screen size changes.
   * Call this before updating screenWidth/screenHeight.
   */
  anchorTopLeft(newScreenW: number, newScreenH: number): void {
    const oldW = this.screenWidth / (this.ppu * this.zoom);
    const oldH = this.screenHeight / (this.ppu * this.zoom);
    const leftEdge = this.centerX - oldW / 2;
    const topEdge = this.centerY - oldH / 2;

    const newW = newScreenW / (this.ppu * this.zoom);
    const newH = newScreenH / (this.ppu * this.zoom);

    this.centerX = leftEdge + newW / 2;
    this.centerY = topEdge + newH / 2;
    this.screenWidth = newScreenW;
    this.screenHeight = newScreenH;
  }

  /** Convert canvas CSS pixels → world units. */
  canvasToWorld(canvasX: number, canvasY: number): { x: number; y: number } {
    const region = this.getRegion(0);
    const scale = this.ppu * this.zoom;
    return {
      x: region.left + canvasX / scale,
      y: region.top + canvasY / scale,
    };
  }

  /** Convert world units → canvas CSS pixels. */
  worldToCanvas(worldX: number, worldY: number): { x: number; y: number } {
    const region = this.getRegion(0);
    const scale = this.ppu * this.zoom;
    return {
      x: (worldX - region.left) * scale,
      y: (worldY - region.top) * scale,
    };
  }
}
