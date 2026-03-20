// ══════════════════════════════════════════════════
// ifol-render-sdk — Type Definitions
// All spatial values are in WORLD UNITS unless noted.
// See docs/UNIT_SYSTEM.md for full specification.
// ══════════════════════════════════════════════════

// ── Scene ──

/** Scene-level settings. */
export interface SceneSettings {
  /** Pixels-per-unit. Media pixel dimensions / PPU = unit size. Default: 1 */
  ppu: number;
  /** Frames per second for playback and export. */
  fps: number;
  /** Total duration in seconds. */
  duration: number;
}

// ── Entity ──

export type EntityType = 'rect' | 'circle' | 'image' | 'video' | 'text';
export type BlendMode = 'normal' | 'multiply' | 'screen' | 'overlay';

/** An entity in world-unit space. */
export interface Entity {
  id: string;
  type: EntityType;
  /** Human-readable label. */
  label?: string;

  // ── Transform (units) ──
  x: number;
  y: number;
  width: number;
  height: number;
  rotation: number;

  // ── Style ──
  color: [number, number, number, number];
  opacity: number;
  blendMode: BlendMode;

  // ── Media ──
  /** Asset key (from addImage/addVideo). */
  source?: string;

  // ── Timeline ──
  startTime: number;
  duration: number;

  // ── Render ──
  /** Shader pipeline name. Auto-set by SDK based on type. */
  shader: string;
  /** Extra shader params (e.g. corner radius). */
  params: number[];
  /** Layer order (higher = drawn later = on top). */
  layer: number;
}

// ── Viewport ──

/** A view into the world. See docs/UNIT_SYSTEM.md §2. */
export interface Viewport {
  /** Display width in CSS pixels. */
  screenWidth: number;
  /** Display height in CSS pixels. */
  screenHeight: number;

  /** World X of viewport center (units). */
  centerX: number;
  /** World Y of viewport center (units). */
  centerY: number;
  /** Zoom factor. 1.0 = default, 2.0 = zoomed in 2×. */
  zoom: number;

  /**
   * Render resolution as fraction of screen size.
   * 1.0 = full quality, 0.5 = half (25% GPU work).
   */
  renderScale: number;
}

// ── Camera ──

/** Camera defines what the "output" shows. Position + size in units. */
export interface Camera {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Export settings for rendering camera output to file. */
export interface ExportSettings {
  /** Output pixel width. */
  width: number;
  /** Output pixel height. */
  height: number;
  fps: number;
  output: string;
}

// ── Visible Region ──

/** Computed visible rectangle in world units. */
export interface WorldRegion {
  left: number;
  top: number;
  width: number;
  height: number;
}

// ── Core Frame Types (pixel-based, sent to WASM) ──

/** Flat entity with pixel coordinates for Core rendering. */
export interface FlatEntity {
  id: number;
  x: number;
  y: number;
  width: number;
  height: number;
  rotation: number;
  opacity: number;
  blend_mode: number;
  color: [number, number, number, number];
  shader: string;
  textures: string[];
  params: number[];
  layer: number;
  z_index: number;
}

export type PassType =
  | { Entities: { entities: FlatEntity[]; clear_color: [number, number, number, number] } }
  | { Output: { input: string } };

export interface RenderPass {
  output: string;
  pass_type: PassType;
}

export interface TextureUpdate {
  LoadImage?: { key: string; path: string };
  UploadRgba?: { key: string; data: number[]; width: number; height: number };
  Evict?: { key: string };
}

/** A single render frame — pixel coordinates, ready for Core WASM. */
export interface Frame {
  passes: RenderPass[];
  texture_updates: TextureUpdate[];
}

// ── Asset Tracking ──

export interface ImageAsset {
  key: string;
  url: string;
  pixelWidth: number;
  pixelHeight: number;
  /** Size in world units (pixelW/PPU, pixelH/PPU). */
  unitWidth: number;
  unitHeight: number;
  cached: boolean;
}

export interface VideoAsset {
  key: string;
  url: string;
  pixelWidth: number;
  pixelHeight: number;
  unitWidth: number;
  unitHeight: number;
  duration: number;
  element?: HTMLVideoElement;
}
