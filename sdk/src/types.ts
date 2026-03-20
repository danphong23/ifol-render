// ══════════════════════════════════════
// ifol-render-sdk — Type Definitions
// ══════════════════════════════════════

// ── Render Settings ──

export interface RenderSettings {
  width: number;
  height: number;
  fps: number;
  background?: [number, number, number, number];
}

// ── Entity (rich, frontend-owned) ──

export interface Entity {
  id: string;
  type: 'rect' | 'image' | 'video' | 'text' | 'shape' | 'custom';
  transform: Transform;
  timeline: Timeline;
  style?: EntityStyle;
  /** Text content (for type: 'text') */
  content?: string;
  /** Asset source path (for type: 'image' | 'video') */
  source?: string;
  /** Custom shader name (for type: 'custom') */
  shader?: string;
  /** Extra shader params */
  params?: number[];
  /** Child entity IDs (hierarchy) */
  children?: string[];
}

export interface Transform {
  /** Position in scene units (origin: top-left of scene) */
  x: number;
  y: number;
  /** Size in scene units */
  width: number;
  height: number;
  /** Rotation in radians */
  rotation?: number;
  /** Anchor point 0-1 (default: 0.5, 0.5 = center) */
  anchorX?: number;
  anchorY?: number;
}

export interface Timeline {
  /** Start time in seconds */
  start: number;
  /** Duration in seconds */
  duration: number;
  /** Layer index (0 = bottom) */
  layer?: number;
  /** Z-index within layer */
  zIndex?: number;
}

export interface EntityStyle {
  opacity?: number;
  blendMode?: BlendMode;
  color?: [number, number, number, number];
  fontSize?: number;
  fontKey?: string;
  maxWidth?: number;
  lineHeight?: number;
  textAlign?: 'left' | 'center' | 'right';
  /** Border radius for shapes */
  borderRadius?: number;
}

export type BlendMode =
  | 'normal'
  | 'multiply'
  | 'screen'
  | 'overlay'
  | 'softLight'
  | 'add'
  | 'difference';

// ── Viewport ──

export interface Viewport {
  /** Zoom level: 1.0 = 100% (fit scene to viewport) */
  zoom: number;
  /** Pan offset X in scene units */
  panX: number;
  /** Pan offset Y in scene units */
  panY: number;
}

// ── Flat Frame (Core input) ──

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

export interface TextureUpdate {
  LoadImage?: { key: string; path: string };
  UploadRgba?: { key: string; data: number[]; width: number; height: number };
  LoadFont?: { key: string; path: string };
  RasterizeText?: {
    key: string;
    content: string;
    font_size: number;
    color: [number, number, number, number];
    font_key?: string;
    max_width?: number;
    line_height?: number;
    alignment?: number;
  };
  DecodeVideoFrame?: {
    key: string;
    path: string;
    timestamp_secs: number;
    width?: number;
    height?: number;
  };
  Evict?: { key: string };
}

export interface RenderPass {
  output: string;
  pass_type: PassType;
}

export type PassType =
  | { Entities: { entities: FlatEntity[]; clear_color: [number, number, number, number] } }
  | { Effect: { shader: string; inputs: string[]; params: number[] } }
  | { Output: { input: string } };

export interface Frame {
  passes: RenderPass[];
  texture_updates: TextureUpdate[];
}

// ── SDK Config ──

export interface SDKConfig {
  /** Canvas element to render to */
  canvas: HTMLCanvasElement;
  /** Render settings */
  settings: RenderSettings;
  /** Asset base URL (default: 'http://localhost:8000/asset?path=') */
  assetBaseUrl?: string;
}
