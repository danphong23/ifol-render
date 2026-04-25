use crate::ecs::World;
use crate::ecs::components::FitMode;
use crate::ecs::components::draw::{DrawCall, DrawKind, TextureRequest};

/// Generates DrawCalls from pure data components and resolved state.
/// This acts as the bridge between standard ECS data and the render system.
pub fn source_system(world: &mut World) {
    let storages = &world.storages;
    // Split borrows: we mutate entities but only read assets
    let assets = &world.assets;

    for entity in &mut world.entities {
        // Always reset draw payload
        entity.draw.draw_calls.clear();
        entity.draw.texture_requests.clear();

        if !entity.resolved.visible {
            continue;
        }

        // Skip cameras, they only define viewports
        if storages
            .get_component::<crate::ecs::components::CameraComponent>(&entity.id)
            .is_some()
        {
            continue;
        }

        let r = &entity.resolved;
        let mut base_call = DrawCall {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
            rotation: r.rotation,
            anchor_x: r.anchor_x,
            anchor_y: r.anchor_y,
            opacity: r.opacity,
            blend_mode: "normal".into(), // Will map later if needed
            align_x: storages
                .get_component::<crate::ecs::components::Rect>(&entity.id)
                .as_ref()
                .map(|rc| rc.align_x)
                .unwrap_or(0.5),
            align_y: storages
                .get_component::<crate::ecs::components::Rect>(&entity.id)
                .as_ref()
                .map(|rc| rc.align_y)
                .unwrap_or(0.5),
            color: r.color,
            fit_mode: r.fit_mode,
            intrinsic_width: r.intrinsic_width,
            intrinsic_height: r.intrinsic_height,
            ..Default::default()
        };

        // Helper to extract URL from assets bypassing world.resolve_asset_url to split borrows
        let resolve_url = |asset_id: &String| -> String {
            assets
                .get(asset_id)
                .map(|a| match a {
                    crate::scene::AssetDef::Video { url } => url.clone(),
                    crate::scene::AssetDef::Image { url } => url.clone(),
                    crate::scene::AssetDef::Font { url } => url.clone(),
                    crate::scene::AssetDef::Audio { url } => url.clone(),
                    crate::scene::AssetDef::Shader { url } => url.clone(),
                })
                .unwrap_or_else(|| asset_id.clone())
        };

        // Determine content type based on source components
        if let Some(shape) =
            storages.get_component::<crate::ecs::components::ShapeSource>(&entity.id)
        {
            base_call.kind = match shape.kind {
                crate::ecs::components::shape::ShapeKind::Rectangle => DrawKind::SolidRect,
                crate::ecs::components::shape::ShapeKind::Ellipse => DrawKind::SolidEllipse,
            };

            let shape_type = match shape.kind {
                crate::ecs::components::shape::ShapeKind::Rectangle => 0.0, // WGSL sdf_rect
                crate::ecs::components::shape::ShapeKind::Ellipse => 3.0,   // WGSL sdf_ellipse
            };
            let border_width = if shape.stroke_color.is_some() {
                shape.stroke_width
            } else {
                0.0
            };

            // Layout: [shape_type (0=rect, 3=ellipse), param1 (corner_radius), param2 (border_width), pad]
            base_call.params = vec![shape_type, 0.0, border_width, 0.0];

            entity.draw.draw_calls.push(base_call);
        } else if let Some(video) =
            storages.get_component::<crate::ecs::components::VideoSource>(&entity.id)
        {
            base_call.kind = DrawKind::Texture;
            let url = resolve_url(&video.asset_id);
            base_call.texture_key = Some(url.clone());

            entity
                .draw
                .texture_requests
                .push(TextureRequest::DecodeVideoFrame {
                    key: url.clone(),
                    asset_url: url,
                    timestamp_secs: r.playback_time,
                });
            apply_fit_mode(&mut base_call);
            entity.draw.draw_calls.push(base_call);
        } else if let Some(image) =
            storages.get_component::<crate::ecs::components::ImageSource>(&entity.id)
        {
            base_call.kind = DrawKind::Texture;
            let url = resolve_url(&image.asset_id);
            entity
                .draw
                .texture_requests
                .push(TextureRequest::LoadImage {
                    key: url.clone(),
                    asset_url: url.clone(),
                });
            base_call.texture_key = Some(url.clone());
            apply_fit_mode(&mut base_call);
            entity.draw.draw_calls.push(base_call);
        } else if let Some(_color) =
            storages.get_component::<crate::ecs::components::ColorSource>(&entity.id)
        {
            base_call.kind = DrawKind::SolidRect;
            entity.draw.draw_calls.push(base_call);
        } else if let Some(text) =
            storages.get_component::<crate::ecs::components::TextSource>(&entity.id)
        {
            base_call.kind = DrawKind::Text;

            // Map TextSource explicit array color
            base_call.color = text.color;

            let mut real_font_size = text.font_size;
            if text.continuous_rasterization {
                let max_scale = r.scale_x.abs().max(r.scale_y.abs()).max(0.001);
                real_font_size = text.font_size * max_scale;
            }

            // Note: Since text content might be extremely long, we should hash it in the future.
            // For now, let's use a simple key based on entity ID so it re-renders if it changes.
            // If text dynamically updates mid-flight, entity.id string alone won't invalidate the cache.
            // We'll append the content snippet, size, AND real_font_size to force cache eviction if it animatingly changes.
            // Use a content hash to ensure cache invalidation when text changes,
            // even if the length stays the same (e.g. "Hello" → "World").
            let content_hash = {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                text.content.hash(&mut hasher);
                hasher.finish()
            };
            let cache_key = format!(
                "text_{}_{}_{:.2}_{:x}",
                entity.id,
                text.font_size,
                real_font_size,
                content_hash
            );

            base_call.texture_key = Some(cache_key.clone());

            // Map the font URL using resolve_url if it's an asset, otherwise use raw string
            let font_key = resolve_url(&text.font);
            let font_opt = if font_key.is_empty() {
                None
            } else {
                entity.draw.texture_requests.push(TextureRequest::LoadFont {
                    key: font_key.clone(),
                    asset_url: font_key.clone(),
                });
                Some(font_key)
            };

            entity
                .draw
                .texture_requests
                .push(TextureRequest::RasterizeText {
                    key: cache_key,
                    content: text.content.clone(),
                    font_size: real_font_size,
                    color: base_call.color,
                    font_key: font_opt,
                    max_width: None, // Will be filled later if needed
                    line_height: None,
                    alignment: 0, // Left
                });

            entity.draw.draw_calls.push(base_call);
        }
    }
}

/// Apply FitMode adjustments to a texture DrawCall.
///
/// - **Stretch** (default): No change — texture fills DrawCall w/h exactly.
/// - **Contain**: Shrink DrawCall w/h to fit image proportionally within the
///   original rect. Surplus area is transparent (no black fill).
/// - **Cover**: Keep DrawCall at rect size. Image scaled up to cover entirely,
///   cropped edges (UV crop handled by shader).
fn apply_fit_mode(_call: &mut DrawCall) {
    // Fit properties (offset & scale) are now fully calculated via 
    // `FitMode::calculate_uv` right before rendering.
    // Altering the vertex geometry here causes Editor Gizmos and Hit Tests
    // (which rely on the mathematical rect) to permanently desync from visuals.
}
