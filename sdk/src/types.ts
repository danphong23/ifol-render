// ══════════════════════════════════════════════════
// ifol-render-sdk — Core Types
//
// All spatial values are in WORLD UNITS.
// Pixels only appear in FlatEntity (after flatten).
// FPS is NOT in SDK — it's an App Layer concern.
// ══════════════════════════════════════════════════

// ── Scene Settings ──

export interface SceneSettings {
  /** Pixels-per-unit: converts media pixel dims → unit dims on import. */
  ppu: number;
  /** Total duration in seconds. */
  duration: number;
  // NOTE: fps intentionally omitted — App Layer decides frame rate
}

// ── Entity ──

export type EntityType = 'rect' | 'circle' | 'image' | 'video' | 'text' | 'group';
export type BlendMode = 'normal' | 'multiply' | 'screen' | 'overlay';

export interface Entity {
  id: string;
  type: EntityType;
  label?: string;

  // Transform (world units)
  x: number;
  y: number;
  width: number;
  height: number;
  rotation: number;    // radians
  opacity: number;     // 0–1

  // Rotation anchor point (0–1, relative to entity bounds)
  // Defaults to center (0.5, 0.5). SDK converts to correct
  // coordinates for Core rendering.
  anchorX: number;     // 0 = left edge, 0.5 = center, 1 = right edge
  anchorY: number;     // 0 = top edge, 0.5 = center, 1 = bottom edge

  // Appearance
  color: [number, number, number, number]; // RGBA 0–1
  blendMode: BlendMode;
  shader: string;
  params: number[];

  // Media reference (key in AssetManager)
  source?: string;
  // Actual filesystem path — used for CLI export (backend loads from disk)
  // On web, source is the cache key; sourcePath is the original file path.
  sourcePath?: string;

  // Timeline
  layer: number;
  startTime: number;   // seconds
  duration: number;     // seconds
}

// ── World Region ──

export interface WorldRegion {
  left: number;        // units
  top: number;         // units
  width: number;       // units
  height: number;      // units
  rotation: number;    // radians — camera rotation
}

// ── Camera Interface ──

export interface Camera {
  readonly type: string;
  /** Compute the visible world region at time t. */
  getRegion(time: number): WorldRegion;
}

// ── Viewport Config (for FreeCamera) ──

export interface ViewportConfig {
  /** CSS display width in pixels. */
  screenWidth: number;
  /** CSS display height in pixels. */
  screenHeight: number;
  /** Center of viewport in world units. */
  centerX: number;
  centerY: number;
  /** Zoom multiplier: 1 = default, 2 = zoomed in 2×. */
  zoom: number;
}

// ── Media Assets ──

export interface ImageAsset {
  key: string;
  url: string;
  pixelWidth: number;
  pixelHeight: number;
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

// ── Flatten Output (pixel-space) ──

export interface FlatEntity {
  id: number;
  x: number;           // pixels
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

export interface RenderPass {
  output: string;
  pass_type: {
    Entities?: { entities: FlatEntity[]; clear_color: number[] };
    Output?: { input: string };
  };
}

export interface TextureUpdate {
  LoadImage?: { key: string; path: string };
  UploadRgba?: { key: string; data: number[]; width: number; height: number };
  DecodeVideoFrame?: { key: string; path: string; timestamp_secs: number; width?: number; height?: number };
  Evict?: { key: string };
}

export interface Frame {
  passes: RenderPass[];
  texture_updates: TextureUpdate[];
}

// ── Audio ──

export interface AudioEntity {
  id: string;
  /** Label for UI */
  label?: string;
  /** Actual filesystem path — used for CLI export and web proxy */
  source: string;
  
  // Timeline
  /** When this audio clip starts playing in the scene (seconds) */
  startTime: number;
  /** Duration to play (seconds). If undefined, plays to end of media. */
  duration?: number;
  /** Start playing the media from this specific offset (seconds) */
  offset: number;
  
  // Properties
  /** Volume multiplier (0.0 to 1.0+) */
  volume: number;
  /** Fade in duration (seconds) */
  fadeIn: number;
  /** Fade out duration (seconds) */
  fadeOut: number;
  
  /** Tracks help group related audio clips together (e.g. 'bgm', 'sfx', 'video-1') */
  trackId: string;
}
