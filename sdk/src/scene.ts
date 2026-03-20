// ══════════════════════════════════════════════════
// ifol-render-sdk — Scene Model
//
// Manages entities in UNIT space. Pure data model.
// Flatten logic moved to flatten.ts + RenderView.
// ══════════════════════════════════════════════════

import type { Entity, SceneSettings } from './types.js';

/**
 * Scene — the unit-space entity model.
 *
 * All positions and sizes are in world units. The Scene knows nothing
 * about pixels, GPU, cameras, or rendering. It's a shared data store
 * that multiple RenderViews read from.
 *
 * ```ts
 * const scene = new Scene({ ppu: 1, duration: 10 });
 * scene.addEntity({ id: 'bg', ... });
 * // RenderView reads from scene to produce Frame JSON
 * ```
 */
export class Scene {
  readonly settings: SceneSettings;
  private entities = new Map<string, Entity>();
  private _dirty = true;

  constructor(settings: SceneSettings) {
    this.settings = { ...settings };
  }

  get ppu(): number { return this.settings.ppu; }
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

  /** Hit test: find topmost entity at world-unit position. */
  hitTest(worldX: number, worldY: number, excludeIds?: Set<string>): Entity | null {
    const candidates = [...this.entities.values()]
      .filter(e => !excludeIds?.has(e.id))
      .sort((a, b) => b.layer - a.layer);
    for (const e of candidates) {
      if (worldX >= e.x && worldX <= e.x + e.width &&
          worldY >= e.y && worldY <= e.y + e.height) {
        return e;
      }
    }
    return null;
  }

  /** Hit test border only (within `margin` units of edge). */
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
}
