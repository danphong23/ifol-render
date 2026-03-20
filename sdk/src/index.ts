// ══════════════════════════════════════════════════
// ifol-render-sdk — Public Exports
//
// SDK = Scene + Camera + RenderView + AssetManager + Timeline
// Produces Frame JSON. Does NOT touch GPU, DOM, or backend.
// ══════════════════════════════════════════════════

// Types
export type {
  Entity, EntityType, BlendMode, SceneSettings,
  Camera, ViewportConfig, WorldRegion,
  ImageAsset, VideoAsset,
  FlatEntity, Frame, RenderPass, TextureUpdate,
} from './types.js';

// Scene — entity model
export { Scene } from './scene.js';

// Camera — view definitions
export { BoundCamera, FreeCamera } from './camera.js';

// RenderView — Scene × Camera → Frame JSON
export { RenderView } from './render-view.js';

// Flatten — pure function (advanced use)
export { flatten } from './flatten.js';

// AssetManager — media decode + cache
export { AssetManager } from './assets.js';

// Timeline — playback state
export { Timeline } from './timeline.js';

// Animation — keyframe tracks (optional)
export { KeyframeTrack, AnimationManager, Easing } from './animation.js';
export type { EasingFn, Keyframe } from './animation.js';
