// ══════════════════════════════════════
// Scene Model — entity registry + flatten
// ══════════════════════════════════════

import type {
  Entity, Viewport, Frame, FlatEntity, RenderPass, TextureUpdate, RenderSettings
} from './types.js';

const BLEND_MODE_MAP: Record<string, number> = {
  normal: 0, multiply: 1, screen: 2, overlay: 3,
  softLight: 4, add: 5, difference: 6,
};

/**
 * Scene model — manages entities and produces flat Frame data for Core.
 *
 * The scene stores entities in "scene units" (abstract units that map to the
 * export resolution). When flattening, entity positions are converted to
 * pixel coordinates based on the target render size and viewport transform.
 */
export class Scene {
  private entities: Map<string, Entity> = new Map();
  private dirty = true;
  /** Export/scene resolution (what entity coords are authored against) */
  readonly sceneWidth: number;
  readonly sceneHeight: number;
  readonly fps: number;
  readonly duration: number;

  constructor(settings: RenderSettings & { duration: number }) {
    this.sceneWidth = settings.width;
    this.sceneHeight = settings.height;
    this.fps = settings.fps;
    this.duration = settings.duration;
  }

  // ── Entity CRUD ──

  addEntity(entity: Entity): void {
    this.entities.set(entity.id, entity);
    this.dirty = true;
  }

  updateEntity(id: string, patch: Partial<Entity>): void {
    const existing = this.entities.get(id);
    if (!existing) throw new Error(`Entity not found: ${id}`);
    this.entities.set(id, { ...existing, ...patch });
    this.dirty = true;
  }

  removeEntity(id: string): void {
    this.entities.delete(id);
    this.dirty = true;
  }

  getEntity(id: string): Entity | undefined {
    return this.entities.get(id);
  }

  getAllEntities(): Entity[] {
    return Array.from(this.entities.values());
  }

  isDirty(): boolean { return this.dirty; }
  markClean(): void { this.dirty = false; }

  // ── Flatten ──

  /**
   * Flatten the scene at a specific time into a Core-ready Frame.
   *
   * Steps:
   * 1. Filter entities visible at `time` (timeline check)
   * 2. Convert scene-unit positions to pixel coords for `targetWidth × targetHeight`
   * 3. Apply viewport transform (zoom, pan)
   * 4. Build render passes + texture updates
   *
   * @param time - Current playback time in seconds
   * @param targetWidth - Render target width in pixels
   * @param targetHeight - Render target height in pixels
   * @param viewport - Viewport transform (zoom, pan) — default: { zoom: 1, panX: 0, panY: 0 }
   */
  flattenAt(
    time: number,
    targetWidth: number,
    targetHeight: number,
    viewport: Viewport = { zoom: 1, panX: 0, panY: 0 },
  ): Frame {
    // Scale factor: scene units → render pixels
    const sx = targetWidth / this.sceneWidth;
    const sy = targetHeight / this.sceneHeight;

    // Viewport transform
    const { zoom, panX, panY } = viewport;

    const visibleEntities: FlatEntity[] = [];
    const textureUpdates: TextureUpdate[] = [];

    let entityIndex = 0;

    for (const entity of this.entities.values()) {
      const tl = entity.timeline;

      // Visibility check
      if (time < tl.start || time >= tl.start + tl.duration) continue;

      // Convert scene-unit coords to pixel coords with viewport
      const t = entity.transform;
      const px = (t.x + panX) * sx * zoom;
      const py = (t.y + panY) * sy * zoom;
      const pw = t.width * sx * zoom;
      const ph = t.height * sy * zoom;

      // Determine shader
      const shader = entity.shader ?? this.defaultShader(entity.type);

      // Build texture updates for this entity
      const textures: string[] = [];

      if (entity.type === 'image' && entity.source) {
        const texKey = `img_${entity.id}`;
        textureUpdates.push({ LoadImage: { key: texKey, path: entity.source } });
        textures.push(texKey);
      } else if (entity.type === 'video' && entity.source) {
        const texKey = `vid_${entity.id}`;
        const videoTime = time - tl.start; // time relative to entity start
        textureUpdates.push({
          DecodeVideoFrame: {
            key: texKey,
            path: entity.source,
            timestamp_secs: videoTime,
          },
        });
        textures.push(texKey);
      } else if (entity.type === 'text' && entity.content) {
        const texKey = `txt_${entity.id}`;
        textureUpdates.push({
          RasterizeText: {
            key: texKey,
            content: entity.content,
            font_size: entity.style?.fontSize ?? 24,
            color: entity.style?.color ?? [1, 1, 1, 1],
            font_key: entity.style?.fontKey,
            max_width: entity.style?.maxWidth,
            line_height: entity.style?.lineHeight,
            alignment: entity.style?.textAlign === 'center' ? 1
              : entity.style?.textAlign === 'right' ? 2 : 0,
          },
        });
        textures.push(texKey);
      }

      const style = entity.style ?? {};

      visibleEntities.push({
        id: entityIndex++,
        x: px,
        y: py,
        width: pw,
        height: ph,
        rotation: t.rotation ?? 0,
        opacity: style.opacity ?? 1.0,
        blend_mode: BLEND_MODE_MAP[style.blendMode ?? 'normal'] ?? 0,
        color: style.color ?? [1, 1, 1, 1],
        shader,
        textures,
        params: entity.params ?? [],
        layer: tl.layer ?? 0,
        z_index: tl.zIndex ?? 0,
      });
    }

    // Build standard 2-pass pipeline: Entities → Output
    const passes: RenderPass[] = [
      {
        output: 'main',
        pass_type: {
          Entities: {
            entities: visibleEntities,
            clear_color: [0, 0, 0, 1],
          },
        },
      },
      {
        output: 'screen',
        pass_type: { Output: { input: 'main' } },
      },
    ];

    return { passes, texture_updates: textureUpdates };
  }

  private defaultShader(type: Entity['type']): string {
    switch (type) {
      case 'rect': return 'shapes';
      case 'shape': return 'shapes';
      case 'image': return 'composite';
      case 'video': return 'composite';
      case 'text': return 'composite';
      default: return 'composite';
    }
  }
}
