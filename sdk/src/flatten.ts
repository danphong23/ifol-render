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

  const flatEntities: FlatEntity[] = [];
  const texUpdates: TextureUpdate[] = [];

  // Sort by layer for correct draw order
  const sorted = [...entities].sort((a, b) => a.layer - b.layer);

  let idx = 0;
  for (const e of sorted) {
    // Transform: unit → pixel
    // If camera has rotation, we'd apply inverse rotation here.
    // For now, rotation=0 regions use direct projection.
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

function blendModeToInt(mode: BlendMode): number {
  switch (mode) {
    case 'normal': return 0;
    case 'multiply': return 1;
    case 'screen': return 2;
    case 'overlay': return 3;
    default: return 0;
  }
}
