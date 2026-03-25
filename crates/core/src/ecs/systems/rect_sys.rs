use crate::ecs::World;
use crate::time::TimeState;

/// Rect system — resolves display size, intrinsic dimensions, fit mode, and aspect ratios.
///
/// **Size resolution order:**
///   1. `Rect.width/height` (entity-level display rect) → if set and > 0
///   2. Source intrinsic (video/image intrinsic_width/height) → auto-size
///   3. Camera legacy width/height from transform → viewport size
///   4. Default 200×200
///
/// Final resolved size = base_size × scale_x/y (from transform_sys).
///
/// Must run AFTER transform_sys (needs scale_x/y).
pub fn rect_system(world: &mut World, _time: &TimeState) {
    for entity in &mut world.entities {
        if !entity.resolved.visible { continue; }

        let t = entity.resolved.time.local_time;

        // === Resolve intrinsic dimensions from source ===
        entity.resolved.intrinsic_width = 0.0;
        entity.resolved.intrinsic_height = 0.0;
        if let Some(vs) = &entity.components.video_source {
            entity.resolved.intrinsic_width = vs.intrinsic_width;
            entity.resolved.intrinsic_height = vs.intrinsic_height;
        } else if let Some(img) = &entity.components.image_source {
            entity.resolved.intrinsic_width = img.intrinsic_width;
            entity.resolved.intrinsic_height = img.intrinsic_height;
        }

        // === Resolve base size ===
        let (base_w, base_h) = if let Some(rect) = &entity.components.rect {
            let rw = rect.width.evaluate(t, 0.0) as f32;
            let rh = rect.height.evaluate(t, 0.0) as f32;
            if rw > 0.0 && rh > 0.0 {
                (rw, rh)
            } else {
                intrinsic_or_default(entity, t)
            }
        } else {
            intrinsic_or_default(entity, t)
        };

        // === Apply scale ===
        entity.resolved.width = base_w * entity.resolved.scale_x;
        entity.resolved.height = base_h * entity.resolved.scale_y;

        // === Fit mode: prefer rect, fallback to legacy ===
        if let Some(rect) = &entity.components.rect {
            entity.resolved.fit_mode = rect.fit_mode;
        } else if let Some(track) = &entity.components.fit_mode {
            let val = track.evaluate(t, "stretch");
            entity.resolved.fit_mode = crate::ecs::components::FitMode::from_str(val);
        }

        // === Aspect ratios ===
        if entity.resolved.intrinsic_height > 0.0 {
            entity.resolved.aspect_ratio = entity.resolved.intrinsic_width / entity.resolved.intrinsic_height;
        } else {
            entity.resolved.aspect_ratio = 1.0;
        }
        if entity.resolved.height > 0.0 {
            entity.resolved.display_aspect = entity.resolved.width / entity.resolved.height;
        }
    }
}

/// Resolve intrinsic size from source or default.
fn intrinsic_or_default(entity: &crate::ecs::Entity, t: f64) -> (f32, f32) {
    if let Some(vs) = &entity.components.video_source {
        (vs.intrinsic_width.max(1.0), vs.intrinsic_height.max(1.0))
    } else if let Some(img) = &entity.components.image_source {
        (img.intrinsic_width.max(1.0), img.intrinsic_height.max(1.0))
    } else if entity.components.camera.is_some() {
        if let Some(track) = &entity.components.transform {
            (track.width.evaluate(t, 1280.0) as f32, track.height.evaluate(t, 720.0) as f32)
        } else {
            (1280.0, 720.0)
        }
    } else {
        (200.0, 200.0)
    }
}
