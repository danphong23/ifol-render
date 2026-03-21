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
 * @param time - Current playback time in seconds (for video frame timestamps)
 * @returns Frame ready for Core WASM render_frame()
 */
export function flatten(
  entities: Entity[],
  region: WorldRegion,
  renderW: number,
  renderH: number,
  time: number = 0,
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
    // Anchor point (default: center of entity)
    const ax = e.anchorX ?? 0.5;
    const ay = e.anchorY ?? 0.5;

    // Entity anchor point in world units
    let eax = e.x + e.width * ax;
    let eay = e.y + e.height * ay;

    // Apply inverse camera rotation around camera center
    if (camRot !== 0) {
      const dx = eax - camCX;
      const dy = eay - camCY;
      eax = camCX + dx * cosR - dy * sinR;
      eay = camCY + dx * sinR + dy * cosR;
    }

    // Project anchor point to pixel space
    const anchorPx = (eax - region.left) * scale + offsetX;
    const anchorPy = (eay - region.top) * scale + offsetY;

    // Width and height in pixels
    const pw = e.width * scale;
    const ph = e.height * scale;

    // When anchor != center, pre-compute offset so Core (which always
    // rotates around entity center) produces the correct result.
    // Core receives (x, y) as top-left, rotates around (x + w/2, y + h/2).
    // We need the entity center to end up at the right place after rotation.
    const entityRot = e.rotation - camRot;

    // Offset from anchor to center in entity-local space
    const offX = (0.5 - ax) * pw;
    const offY = (0.5 - ay) * ph;

    // Rotate this offset by entity rotation to get world-space offset
    let centerPx: number, centerPy: number;
    if (Math.abs(entityRot) < 1e-6 || (Math.abs(ax - 0.5) < 1e-6 && Math.abs(ay - 0.5) < 1e-6)) {
      // No rotation or center anchor — simple offset
      centerPx = anchorPx + offX;
      centerPy = anchorPy + offY;
    } else {
      // Rotate offset from anchor to center
      const c = Math.cos(entityRot);
      const s = Math.sin(entityRot);
      centerPx = anchorPx + offX * c - offY * s;
      centerPy = anchorPy + offX * s + offY * c;
    }

    // Core expects top-left (x, y) — derive from center
    const px = centerPx - pw / 2;
    const py = centerPy - ph / 2;

    const flat: FlatEntity = {
      id: idx++,
      x: px,
      y: py,
      width: pw,
      height: ph,
      rotation: entityRot,
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
    // key = cache key (used for GPU texture lookup)
    // path = actual filesystem path (used by CLI export to load from disk)
    // On web, AssetManager pre-caches using key. On CLI, path points to actual file.
    if (e.source && e.type === 'image') {
      const filePath = e.sourcePath ?? e.source;
      flat.textures = [e.source];
      texUpdates.push({ LoadImage: { key: e.source, path: filePath } });
    } else if (e.source && e.type === 'video') {
      const filePath = e.sourcePath ?? e.source;
      flat.textures = [e.source];
      const localTime = Math.max(0, time - (e.startTime ?? 0));
      texUpdates.push({ DecodeVideoFrame: { key: e.source, path: filePath, timestamp_secs: localTime } });
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
