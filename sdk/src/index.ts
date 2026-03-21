// ══════════════════════════════════════════════════
// ifol-render-sdk — Public Exports
//
// SDK = Render Pipeline Toolkit
// Provides tools to produce Frame JSON for Core.
// Does NOT touch GPU, DOM, or backend.
// ══════════════════════════════════════════════════

// ══════════════════════════════════════════════════
// CORE TOOLKIT — the primary API
// ══════════════════════════════════════════════════

// Builder classes — construct Frame JSON for Core
// ANY framework can use these (ECS, scene graph, Redux, raw arrays...)
export {
  DrawableEntity,
  FrameBuilder,
  TextureUpdates,
  AudioClipBuilder,
  buildExportPayload,
} from './builders.js';
export type { ExportConfig, TextureUpdateData } from './builders.js';

// flatten() — convenience: Entity[] → Frame JSON (handles camera, world→pixel)
export { flatten } from './flatten.js';

// Camera — viewport math (world region computation)
export { BoundCamera, FreeCamera } from './camera.js';

// AssetManager — web texture pipeline (video frame extraction, image caching)
export { AssetManager } from './assets.js';

// ── Types (data interfaces) ──
export type {
  Entity, EntityType, BlendMode,
  Camera, ViewportConfig, WorldRegion,
  ImageAsset, VideoAsset,
  FlatEntity, Frame, RenderPass, TextureUpdate,
} from './types.js';

// ══════════════════════════════════════════════════
// OPTIONAL HELPERS — convenience, dev can replace
// ══════════════════════════════════════════════════

export { Scene } from './scene.js';
export type { SceneSettings } from './types.js';
export { RenderView } from './render-view.js';
export { Timeline } from './timeline.js';
export { KeyframeTrack, AnimationManager, Easing } from './animation.js';
export type { EasingFn, Keyframe } from './animation.js';
