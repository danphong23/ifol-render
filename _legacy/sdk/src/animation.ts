// ══════════════════════════════════════════════════
// ifol-render-sdk — Animation Module (optional)
//
// Keyframe-based property animation for entities.
// Serializable, deterministic, SDK-level scene data.
//
// Procedural animation (particles, physics) stays
// in App Layer — it's not serializable.
// ══════════════════════════════════════════════════

import type { Entity } from './types.js';
import type { Scene } from './scene.js';

// ── Easing Functions ──

export type EasingFn = (t: number) => number;

export const Easing = {
  linear: (t: number) => t,
  easeIn: (t: number) => t * t,
  easeOut: (t: number) => t * (2 - t),
  easeInOut: (t: number) => t < 0.5 ? 2 * t * t : -1 + (4 - 2 * t) * t,
  easeInCubic: (t: number) => t * t * t,
  easeOutCubic: (t: number) => (--t) * t * t + 1,
  easeInOutCubic: (t: number) => t < 0.5 ? 4 * t * t * t : (t - 1) * (2 * t - 2) * (2 * t - 2) + 1,
  bounce: (t: number) => {
    if (t < 1 / 2.75) return 7.5625 * t * t;
    if (t < 2 / 2.75) return 7.5625 * (t -= 1.5 / 2.75) * t + 0.75;
    if (t < 2.5 / 2.75) return 7.5625 * (t -= 2.25 / 2.75) * t + 0.9375;
    return 7.5625 * (t -= 2.625 / 2.75) * t + 0.984375;
  },
  elastic: (t: number) => t === 0 || t === 1 ? t : -Math.pow(2, 10 * (t - 1)) * Math.sin((t - 1.1) * 5 * Math.PI),
} as const;

// ── Keyframe ──

export interface Keyframe {
  time: number;
  value: number;
  easing: EasingFn;
}

// ── KeyframeTrack ──

/**
 * KeyframeTrack — animates a single entity property over time.
 *
 * ```ts
 * const track = new KeyframeTrack('x');
 * track.addKeyframe(0, 100);           // x=100 at t=0
 * track.addKeyframe(2, 500, Easing.easeInOut); // x=500 at t=2
 *
 * const value = track.evaluate(1);     // interpolated x at t=1
 * ```
 */
export class KeyframeTrack {
  readonly property: string;
  private keyframes: Keyframe[] = [];

  constructor(property: string) {
    this.property = property;
  }

  addKeyframe(time: number, value: number, easing: EasingFn = Easing.linear): this {
    this.keyframes.push({ time, value, easing });
    this.keyframes.sort((a, b) => a.time - b.time);
    return this;
  }

  removeKeyframesAt(time: number, epsilon = 0.001): void {
    this.keyframes = this.keyframes.filter(k => Math.abs(k.time - time) > epsilon);
  }

  /** Evaluate the property value at the given time. */
  evaluate(time: number): number | undefined {
    const kfs = this.keyframes;
    if (kfs.length === 0) return undefined;
    if (time <= kfs[0].time) return kfs[0].value;
    if (time >= kfs[kfs.length - 1].time) return kfs[kfs.length - 1].value;

    // Find surrounding keyframes
    for (let i = 0; i < kfs.length - 1; i++) {
      if (time >= kfs[i].time && time <= kfs[i + 1].time) {
        const t = (time - kfs[i].time) / (kfs[i + 1].time - kfs[i].time);
        const eased = kfs[i + 1].easing(t);
        return kfs[i].value + (kfs[i + 1].value - kfs[i].value) * eased;
      }
    }
    return kfs[kfs.length - 1].value;
  }

  get isEmpty(): boolean { return this.keyframes.length === 0; }
  get keyframeCount(): number { return this.keyframes.length; }

  /** Serialize to plain object. */
  toJSON(): { property: string; keyframes: { time: number; value: number }[] } {
    return {
      property: this.property,
      keyframes: this.keyframes.map(k => ({ time: k.time, value: k.value })),
    };
  }
}

// ── AnimationManager ──

/**
 * AnimationManager — manages keyframe animations for entities.
 *
 * ```ts
 * const mgr = new AnimationManager();
 * const track = new KeyframeTrack('x')
 *   .addKeyframe(0, 0)
 *   .addKeyframe(2, 500, Easing.easeOut);
 * mgr.attach('entity1', track);
 *
 * // In render loop:
 * mgr.applyAll(scene, time);
 * ```
 */
export class AnimationManager {
  private tracks = new Map<string, KeyframeTrack[]>();

  /** Attach a track to an entity. Multiple tracks per entity allowed. */
  attach(entityId: string, track: KeyframeTrack): void {
    const list = this.tracks.get(entityId) ?? [];
    list.push(track);
    this.tracks.set(entityId, list);
  }

  /** Remove all tracks for an entity. */
  detach(entityId: string): void {
    this.tracks.delete(entityId);
  }

  /** Remove a specific track by property name. */
  detachProperty(entityId: string, property: string): void {
    const list = this.tracks.get(entityId);
    if (list) {
      const filtered = list.filter(t => t.property !== property);
      if (filtered.length > 0) this.tracks.set(entityId, filtered);
      else this.tracks.delete(entityId);
    }
  }

  /** Evaluate all tracks and apply to scene entities at the given time. */
  applyAll(scene: Scene, time: number): void {
    for (const [entityId, trackList] of this.tracks) {
      const patch: Partial<Entity> = {};
      let hasUpdate = false;
      for (const track of trackList) {
        const val = track.evaluate(time);
        if (val !== undefined) {
          (patch as any)[track.property] = val;
          hasUpdate = true;
        }
      }
      if (hasUpdate) scene.updateEntity(entityId, patch);
    }
  }

  /** Get all tracks for an entity. */
  getTracksFor(entityId: string): KeyframeTrack[] {
    return this.tracks.get(entityId) ?? [];
  }

  /** Get all animated entity IDs. */
  animatedIds(): string[] {
    return [...this.tracks.keys()];
  }

  /** Total number of animated entities. */
  get count(): number { return this.tracks.size; }

  /** Clear all animations. */
  clear(): void { this.tracks.clear(); }
}
