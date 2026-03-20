// ══════════════════════════════════════════════════
// ifol-render-sdk — Flatten (pure function)
//
// Converts unit-space entities → pixel-space Frame
// for a given WorldRegion and render target size.
// No side effects, no state.
// ══════════════════════════════════════════════════

import type {
  Entity, WorldRegion, FlatEntity, Frame,
  RenderPass, TextureUpdate, BlendMode,
} from './types.js';

/**
 * Flatten visible entities into a pixel-space Frame.
 *
 * @param entities - Entities to render (already filtered by time + excludes)
 * @param region - World region to render (from Camera.getRegion)
 * @param renderW - Render target width in pixels
 * @param renderH - Render target height in pixels
 * @returns Frame ready for Core WASM render_frame()
 */
export function flatten(
  entities: Entity[],
  region: WorldRegion,
  renderW: number,
  renderH: number,
): Frame {
  // Uniform scale: fit region into render target (no distortion)
  const scaleX = renderW / region.width;
  const scaleY = renderH / region.height;
  const scale = Math.min(scaleX, scaleY);

  // Center offset for letterbox (when aspect ratios differ)
  const offsetX = (renderW - region.width * scale) / 2;
  const offsetY = (renderH - region.height * scale) / 2;

  // Camera rotation: rotate entities around camera center (inverse)
  const camRot = region.rotation || 0;
  const cosR = Math.cos(-camRot);
  const sinR = Math.sin(-camRot);
  // Camera center in world units
  const camCX = region.left + region.width / 2;
  const camCY = region.top + region.height / 2;

  const flatEntities: FlatEntity[] = [];
  const texUpdates: TextureUpdate[] = [];

  // Sort by layer for correct draw order
  const sorted = [...entities].sort((a, b) => a.layer - b.layer);

  let idx = 0;
  for (const e of sorted) {
    // Entity center in world units
    let ecx = e.x + e.width / 2;
    let ecy = e.y + e.height / 2;

    // Apply inverse camera rotation around camera center
    if (camRot !== 0) {
      const dx = ecx - camCX;
      const dy = ecy - camCY;
      ecx = camCX + dx * cosR - dy * sinR;
      ecy = camCY + dx * sinR + dy * cosR;
    }

    // Project rotated center to pixel space
    const px = (ecx - e.width / 2 - region.left) * scale + offsetX;
    const py = (ecy - e.height / 2 - region.top) * scale + offsetY;

    const flat: FlatEntity = {
      id: idx++,
      x: px,
      y: py,
      width: e.width * scale,
      height: e.height * scale,
      rotation: e.rotation - camRot, // combine entity + inverse camera rotation
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
      // Both image and video use LoadImage (video frames are cached as images)
      texUpdates.push({ LoadImage: { key: e.source, path: e.source } });
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

function blendModeToInt(mode: BlendMode): number {
  switch (mode) {
    case 'normal': return 0;
    case 'multiply': return 1;
    case 'screen': return 2;
    case 'overlay': return 3;
    default: return 0;
  }
}
