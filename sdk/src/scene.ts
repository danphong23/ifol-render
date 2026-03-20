// ══════════════════════════════════════════════════
// ifol-render-sdk — Scene Model
// Manages entities in UNIT space. Flatten converts
// unit coords → pixel coords for a given viewport/camera.
// ══════════════════════════════════════════════════

import type {
  Entity, EntityType, BlendMode, SceneSettings, Viewport, Camera,
  WorldRegion, FlatEntity, Frame, RenderPass, TextureUpdate,
} from './types.js';

/** Compute the visible world region from a viewport. */
export function viewportRegion(vp: Viewport, ppu: number): WorldRegion {
  const w = vp.screenWidth / (ppu * vp.zoom);
  const h = vp.screenHeight / (ppu * vp.zoom);
  return {
    left: vp.centerX - w / 2,
    top: vp.centerY - h / 2,
    width: w,
    height: h,
  };
}

/** Compute the world region from a camera entity. */
export function cameraRegion(cam: Camera): WorldRegion {
  return { left: cam.x, top: cam.y, width: cam.width, height: cam.height };
}

/**
 * Scene — the unit-space entity model.
 *
 * All positions and sizes are in world units. The Scene knows nothing
 * about pixels, GPU, or rendering. It provides `flatten()` to convert
 * unit-space entities into pixel-space Frame data for Core.
 */
export class Scene {
  readonly settings: SceneSettings;
  private entities = new Map<string, Entity>();
  private _dirty = true;

  constructor(settings: SceneSettings) {
    this.settings = { ...settings };
  }

  get ppu(): number { return this.settings.ppu; }
  get fps(): number { return this.settings.fps; }
  get duration(): number { return this.settings.duration; }
  get dirty(): boolean { return this._dirty; }

  // ── CRUD ──

  addEntity(entity: Entity): void {
    this.entities.set(entity.id, { ...entity });
    this._dirty = true;
  }

  updateEntity(id: string, patch: Partial<Entity>): void {
    const e = this.entities.get(id);
    if (e) { Object.assign(e, patch); this._dirty = true; }
  }

  removeEntity(id: string): Entity | undefined {
    const e = this.entities.get(id);
    if (e) { this.entities.delete(id); this._dirty = true; }
    return e;
  }

  getEntity(id: string): Entity | undefined {
    return this.entities.get(id);
  }

  allEntities(): Entity[] {
    return [...this.entities.values()];
  }

  entityCount(): number {
    return this.entities.size;
  }

  markClean(): void { this._dirty = false; }

  // ── Flatten ──

  /**
   * Convert unit-space entities → pixel-space Frame for rendering.
   *
   * @param time - Current time in seconds
   * @param region - World region to render (from viewport or camera)
   * @param renderW - Render target width in pixels
   * @param renderH - Render target height in pixels
   * @param excludeIds - Entity IDs to exclude (e.g. camera entity in camera view)
   * @returns Frame ready for Core WASM
   */
  flatten(
    time: number,
    region: WorldRegion,
    renderW: number,
    renderH: number,
    excludeIds?: Set<string>,
  ): Frame {
    // Uniform scale: fit region into render target preserving aspect ratio
    const scaleX = renderW / region.width;
    const scaleY = renderH / region.height;
    const scale = Math.min(scaleX, scaleY);

    // Center offset for letterbox
    const offsetX = (renderW - region.width * scale) / 2;
    const offsetY = (renderH - region.height * scale) / 2;

    const flatEntities: FlatEntity[] = [];
    const texUpdates: TextureUpdate[] = [];
    let idx = 0;

    // Sort by layer for correct draw order
    const sorted = this.visibleAt(time, excludeIds)
      .sort((a, b) => a.layer - b.layer);

    for (const e of sorted) {
      const flat: FlatEntity = {
        id: idx++,
        x: (e.x - region.left) * scale + offsetX,
        y: (e.y - region.top) * scale + offsetY,
        width: e.width * scale,
        height: e.height * scale,
        rotation: e.rotation,
        opacity: e.opacity,
        blend_mode: blendModeToInt(e.blendMode),
        color: e.color,
        shader: e.shader,
        textures: [],
        params: [...e.params],
        layer: e.layer,
        z_index: 0,
      };

      // Texture references
      if (e.source && (e.type === 'image' || e.type === 'video')) {
        flat.textures = [e.source];
        // Image: LoadImage (cached in Core after first load)
        if (e.type === 'image') {
          texUpdates.push({ LoadImage: { key: e.source, path: e.source } });
        }
        // Video: UploadRgba handled by AssetManager before flatten
      }

      flatEntities.push(flat);
    }

    const passes: RenderPass[] = [
      {
        output: 'main',
        pass_type: {
          Entities: {
            entities: flatEntities,
            clear_color: [0.04, 0.04, 0.08, 1.0],
          },
        },
      },
      { output: 'screen', pass_type: { Output: { input: 'main' } } },
    ];

    return { passes, texture_updates: texUpdates };
  }

  /**
   * Flatten a single frame for a Viewport.
   * Computes visible region from viewport settings, applies renderScale.
   */
  flattenForViewport(time: number, vp: Viewport, excludeIds?: Set<string>): Frame {
    const region = viewportRegion(vp, this.settings.ppu);
    const renderW = Math.round(vp.screenWidth * vp.renderScale);
    const renderH = Math.round(vp.screenHeight * vp.renderScale);
    return this.flatten(time, region, renderW, renderH, excludeIds);
  }

  /**
   * Flatten a single frame for a Camera preview.
   * Camera region is in units; renderW/H are display pixels × renderScale.
   */
  flattenForCamera(
    time: number,
    cam: Camera,
    displayW: number,
    displayH: number,
    renderScale: number,
    excludeIds?: Set<string>,
  ): Frame {
    const region = cameraRegion(cam);
    const renderW = Math.round(displayW * renderScale);
    const renderH = Math.round(displayH * renderScale);
    return this.flatten(time, region, renderW, renderH, excludeIds);
  }

  /**
   * Flatten for export: camera region → export pixel resolution.
   */
  flattenForExport(time: number, cam: Camera, exportW: number, exportH: number): Frame {
    return this.flatten(time, cameraRegion(cam), exportW, exportH);
  }

  // ── Query ──

  /** Get entities visible at a given time. */
  visibleAt(time: number, excludeIds?: Set<string>): Entity[] {
    const result: Entity[] = [];
    for (const e of this.entities.values()) {
      if (excludeIds?.has(e.id)) continue;
      if (time >= e.startTime && time < e.startTime + e.duration) {
        result.push(e);
      }
    }
    return result;
  }

  /** Hit test: find entity at world-unit position (topmost first). */
  hitTest(worldX: number, worldY: number, excludeIds?: Set<string>): Entity | null {
    const candidates = this.visibleAt(0, excludeIds) // time=0 for editing
      .sort((a, b) => b.layer - a.layer);
    for (const e of candidates) {
      if (worldX >= e.x && worldX <= e.x + e.width &&
          worldY >= e.y && worldY <= e.y + e.height) {
        return e;
      }
    }
    return null;
  }

  /** Hit test camera border only (within `margin` units of edge). */
  hitTestBorder(
    worldX: number, worldY: number,
    entity: Entity, margin: number,
  ): boolean {
    const inOuter = worldX >= entity.x - margin &&
                    worldX <= entity.x + entity.width + margin &&
                    worldY >= entity.y - margin &&
                    worldY <= entity.y + entity.height + margin;
    const inInner = worldX >= entity.x + margin &&
                    worldX <= entity.x + entity.width - margin &&
                    worldY >= entity.y + margin &&
                    worldY <= entity.y + entity.height - margin;
    return inOuter && !inInner;
  }

  // ── Coordinate conversion ──

  /** Convert canvas pixel coordinates → world units for a viewport. */
  canvasToWorld(canvasX: number, canvasY: number, vp: Viewport): { x: number; y: number } {
    const region = viewportRegion(vp, this.settings.ppu);
    const scaleX = vp.screenWidth / region.width;
    const scaleY = vp.screenHeight / region.height;
    const scale = Math.min(scaleX, scaleY);
    const offsetX = (vp.screenWidth - region.width * scale) / 2;
    const offsetY = (vp.screenHeight - region.height * scale) / 2;
    return {
      x: (canvasX - offsetX) / scale + region.left,
      y: (canvasY - offsetY) / scale + region.top,
    };
  }

  /** Convert world units → canvas pixel coordinates for a viewport. */
  worldToCanvas(worldX: number, worldY: number, vp: Viewport): { x: number; y: number } {
    const region = viewportRegion(vp, this.settings.ppu);
    const scaleX = vp.screenWidth / region.width;
    const scaleY = vp.screenHeight / region.height;
    const scale = Math.min(scaleX, scaleY);
    const offsetX = (vp.screenWidth - region.width * scale) / 2;
    const offsetY = (vp.screenHeight - region.height * scale) / 2;
    return {
      x: (worldX - region.left) * scale + offsetX,
      y: (worldY - region.top) * scale + offsetY,
    };
  }
}

function blendModeToInt(mode: BlendMode): number {
  switch (mode) {
    case 'normal': return 0;
    case 'multiply': return 1;
    case 'screen': return 2;
    case 'overlay': return 3;
    default: return 0;
  }
}
