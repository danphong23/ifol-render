// ══════════════════════════════════════════════════
// ifol-render-sdk — Frame Builder
//
// Builder classes for constructing Frame JSON.
// Dev with ANY framework can use these to build
// exactly what Core WASM expects.
//
// This is the PRIMARY API for SDK toolkit users.
// ══════════════════════════════════════════════════

/**
 * A drawable element in pixel space — exactly what Core WASM consumes.
 *
 * This class represents a single rendered rectangle with:
 * - Position & size in PIXELS (not world units)
 * - A shader pipeline name (what draws it)
 * - Texture references (what it looks like)
 * - Ordering info (layer, z_index)
 *
 * Dev can create these however they want — from their own ECS,
 * from a scene graph, from raw data, etc.
 *
 * ```ts
 * // Simple colored rectangle
 * const rect = new DrawableEntity(0, 100, 200, 300, 150)
 *   .setShader('shapes')
 *   .setColor(1, 0, 0, 1);
 *
 * // Image with texture
 * const img = new DrawableEntity(1, 50, 80, 400, 300)
 *   .setShader('composite')
 *   .addTexture('my_image.png')
 *   .setOpacity(0.8)
 *   .setRotation(Math.PI / 4);
 *
 * // Circle (shapes shader with param 1.0)
 * const circle = new DrawableEntity(2, 500, 300, 100, 100)
 *   .setShader('shapes')
 *   .setParams([1.0]);
 * ```
 */
export class DrawableEntity {
  /** Unique numeric ID (for Core's dirty tracking) */
  id: number;
  /** Top-left X in pixels */
  x: number;
  /** Top-left Y in pixels */
  y: number;
  /** Width in pixels */
  width: number;
  /** Height in pixels */
  height: number;
  /** Rotation in radians (around center of entity) */
  rotation: number = 0;
  /** Opacity: 0.0 (invisible) – 1.0 (fully opaque) */
  opacity: number = 1;
  /** Blend mode: 0=normal, 1=multiply, 2=screen, 3=overlay, 4=soft_light, 5=add, 6=difference */
  blend_mode: number = 0;
  /** RGBA color tint [0–1]. Default: white (no tint) */
  color: [number, number, number, number] = [1, 1, 1, 1];
  /** GPU shader/pipeline name. Built-in: 'composite' (texture), 'shapes' (colored) */
  shader: string = 'composite';
  /** Texture cache keys to bind to this entity */
  textures: string[] = [];
  /** Extra shader uniform parameters */
  params: number[] = [];
  /** Draw order: layer (coarse sort, ascending) */
  layer: number = 0;
  /** Draw order: z_index within same layer (fine sort, ascending) */
  z_index: number = 0;

  constructor(id: number, x: number, y: number, width: number, height: number) {
    this.id = id;
    this.x = x;
    this.y = y;
    this.width = width;
    this.height = height;
  }

  // ── Chainable setters ──

  setRotation(radians: number): this { this.rotation = radians; return this; }
  setOpacity(opacity: number): this { this.opacity = opacity; return this; }
  setBlendMode(mode: number): this { this.blend_mode = mode; return this; }
  setColor(r: number, g: number, b: number, a: number = 1): this { this.color = [r, g, b, a]; return this; }
  setShader(name: string): this { this.shader = name; return this; }
  addTexture(key: string): this { this.textures.push(key); return this; }
  setTextures(keys: string[]): this { this.textures = keys; return this; }
  setParams(params: number[]): this { this.params = params; return this; }
  setLayer(layer: number, zIndex: number = 0): this { this.layer = layer; this.z_index = zIndex; return this; }

  /** Convert to plain JSON object for Core WASM */
  toJSON(): object {
    return {
      id: this.id,
      x: this.x, y: this.y, width: this.width, height: this.height,
      rotation: this.rotation,
      opacity: this.opacity,
      blend_mode: this.blend_mode,
      color: this.color,
      shader: this.shader,
      textures: this.textures,
      params: this.params,
      layer: this.layer,
      z_index: this.z_index,
    };
  }
}

// ══════════════════════════════════════════════════
// Texture Update builders
// ══════════════════════════════════════════════════

/** Instructions for Core to load/update textures before rendering */
export type TextureUpdateData =
  | { LoadImage: { key: string; path: string } }
  | { UploadRgba: { key: string; data: number[] | Uint8Array; width: number; height: number } }
  | { DecodeVideoFrame: { key: string; path: string; timestamp_secs: number; width?: number; height?: number } }
  | { RasterizeText: { key: string; content: string; font_size: number; color: [number, number, number, number]; font_key?: string; max_width?: number; alignment?: number } }
  | { Evict: { key: string } };

export const TextureUpdates = {
  /** Load image from file path (cached — skips if already loaded) */
  loadImage(key: string, path: string): TextureUpdateData {
    return { LoadImage: { key, path } };
  },
  /** Decode a video frame at timestamp (uses FFmpeg on CLI, WebCodecs on web) */
  decodeVideoFrame(key: string, path: string, timestampSecs: number, width?: number, height?: number): TextureUpdateData {
    return { DecodeVideoFrame: { key, path, timestamp_secs: timestampSecs, width, height } };
  },
  /** Upload raw RGBA pixels directly (video frames, procedural textures) */
  uploadRgba(key: string, data: number[] | Uint8Array, width: number, height: number): TextureUpdateData {
    return { UploadRgba: { key, data: Array.from(data), width, height } };
  },
  /** Rasterize text to a texture */
  rasterizeText(key: string, content: string, fontSize: number, color: [number, number, number, number], opts?: { fontKey?: string; maxWidth?: number; alignment?: number }): TextureUpdateData {
    return { RasterizeText: { key, content, font_size: fontSize, color, font_key: opts?.fontKey, max_width: opts?.maxWidth, alignment: opts?.alignment ?? 0 } };
  },
  /** Remove a texture from GPU cache */
  evict(key: string): TextureUpdateData {
    return { Evict: { key } };
  },
};

// ══════════════════════════════════════════════════
// Frame Builder
// ══════════════════════════════════════════════════

/**
 * Build a complete Frame for Core WASM rendering.
 *
 * A Frame = render passes (what to draw) + texture updates (what to load).
 *
 * ```ts
 * // SIMPLE: one pass, a few entities
 * const frame = new FrameBuilder()
 *   .addEntity(new DrawableEntity(0, 100, 200, 300, 150).setShader('shapes').setColor(1,0,0,1))
 *   .addEntity(new DrawableEntity(1, 50, 80, 400, 300).setShader('composite').addTexture('bg.png'))
 *   .addTextureUpdate(TextureUpdates.loadImage('bg.png', '/assets/bg.png'))
 *   .build();
 *
 * core.render_frame(JSON.stringify(frame));
 *
 * // ADVANCED: multi-pass rendering (post-processing)
 * const frame = new FrameBuilder()
 *   .beginPass('scene')
 *     .addEntity(bg)
 *     .addEntity(character)
 *   .beginPass('bloom', 'Effect', { shader: 'bloom', inputs: ['scene'], params: [1.5] })
 *   .setOutput('bloom')
 *   .build();
 * ```
 */
export class FrameBuilder {
  private passes: Array<{ output: string; pass_type: object }> = [];
  private textureUpdates: TextureUpdateData[] = [];
  private currentEntities: DrawableEntity[] = [];
  private currentPassName: string = 'main';
  private currentClearColor: [number, number, number, number] = [0, 0, 0, 1];
  private outputPass: string = 'main';

  /** Set background clear color for current entity pass */
  setClearColor(r: number, g: number, b: number, a: number = 1): this {
    this.currentClearColor = [r, g, b, a];
    return this;
  }

  /** Add a drawable entity to the current pass */
  addEntity(entity: DrawableEntity): this {
    this.currentEntities.push(entity);
    return this;
  }

  /** Add multiple entities at once */
  addEntities(entities: DrawableEntity[]): this {
    this.currentEntities.push(...entities);
    return this;
  }

  /** Add a texture update instruction */
  addTextureUpdate(update: TextureUpdateData): this {
    this.textureUpdates.push(update);
    return this;
  }

  /** Add multiple texture updates */
  addTextureUpdates(updates: TextureUpdateData[]): this {
    this.textureUpdates.push(...updates);
    return this;
  }

  /**
   * Start a new render pass. Flushes current entities into a pass.
   * @param name - Output texture key for this pass
   * @param type - 'Entities' (default) or 'Effect'
   * @param effectOpts - For Effect passes: { shader, inputs, params }
   */
  beginPass(name: string, type: 'Entities' | 'Effect' = 'Entities', effectOpts?: { shader: string; inputs: string[]; params?: number[] }): this {
    // Flush current entities as a pass
    this.flushCurrentPass();
    this.currentPassName = name;
    if (type === 'Effect' && effectOpts) {
      this.passes.push({
        output: name,
        pass_type: { Effect: { shader: effectOpts.shader, inputs: effectOpts.inputs, params: effectOpts.params ?? [] } },
      });
    }
    return this;
  }

  /** Set which pass is the final output (default: 'main') */
  setOutput(passName: string): this {
    this.outputPass = passName;
    return this;
  }

  /** Build the final Frame JSON object */
  build(): object {
    this.flushCurrentPass();
    // Add output pass
    this.passes.push({ output: 'screen', pass_type: { Output: { input: this.outputPass } } });
    return {
      passes: this.passes,
      texture_updates: this.textureUpdates,
    };
  }

  private flushCurrentPass(): void {
    if (this.currentEntities.length > 0) {
      this.passes.push({
        output: this.currentPassName,
        pass_type: {
          Entities: {
            entities: this.currentEntities.map(e => e.toJSON()),
            clear_color: [...this.currentClearColor],
          },
        },
      });
      this.currentEntities = [];
    }
  }
}

// ══════════════════════════════════════════════════
// Export Payload Builder
// ══════════════════════════════════════════════════

/** Export configuration for the CLI backend */
export interface ExportConfig {
  /** Output file path */
  output: string;
  /** Video width in pixels */
  width: number;
  /** Video height in pixels */
  height: number;
  /** Frames per second */
  fps: number;
  /** FFmpeg executable path (optional — uses system PATH by default) */
  ffmpeg?: string;
  /** Video codec (default: 'libx264') */
  codec?: string;
  /** Constant Rate Factor (default: 23, lower = better quality) */
  crf?: number;
  /** Encoding preset (default: 'medium') */
  preset?: string;
  /** Pixel format (default: 'yuv420p') */
  pixelFormat?: string;
  /** Background color RGBA (default: black) */
  background?: [number, number, number, number];
}

/**
 * Build a complete export payload for CLI/backend.
 *
 * ```ts
 * import { AudioScene } from './audio.js';
 *
 * // Without audio
 * const payload = buildExportPayload(config, frames);
 *
 * // With audio (use AudioScene)
 * const audio = new AudioScene();
 * audio.addClip({ source: 'music.mp3', volume: 0.8 });
 * const payload = buildExportPayload(config, frames, audio.flattenForExport());
 *
 * await fetch('/export', { body: JSON.stringify(payload) });
 * ```
 */
import type { FlatAudioClip } from './audio.js';

export function buildExportPayload(
  config: ExportConfig,
  frames: object[],
  audioClips?: FlatAudioClip[],
): object {
  return {
    settings: {
      width: config.width,
      height: config.height,
      fps: config.fps,
      background: config.background ?? [0, 0, 0, 1],
    },
    frames,
    output: config.output,
    ffmpeg: config.ffmpeg,
    codec: config.codec,
    crf: config.crf,
    preset: config.preset,
    pixel_format: config.pixelFormat,
    audio_clips: audioClips,
  };
}
