use crate::ecs::World;
use crate::time::TimeState;

/// Rect system (V4) — resolves display size, intrinsic dimensions, fit mode, and aspect ratios.
///
/// **Size resolution order:**
///   1. `resolved.width/height` (set by animation_sys) → if > 0
///   2. Source intrinsic (video/image intrinsic_width/height) → auto-size
///   3. Camera legacy width/height → viewport size
///   4. Default 200×200
///
/// Final resolved size = base_size × scale_x/y.
///
/// Must run AFTER animation_sys.
pub fn rect_system(world: &mut World, _time: &TimeState) {
    let storages = &world.storages;
    for entity in &mut world.entities {
        if !entity.resolved.visible { continue; }

        // === Resolve intrinsic dimensions from source ===
        entity.resolved.intrinsic_width = 0.0;
        entity.resolved.intrinsic_height = 0.0;
        if let Some(vs) = storages.get_component::<crate::ecs::components::VideoSource>(&entity.id) {
            entity.resolved.intrinsic_width = vs.intrinsic_width;
            entity.resolved.intrinsic_height = vs.intrinsic_height;
        } else if let Some(img) = storages.get_component::<crate::ecs::components::ImageSource>(&entity.id) {
            entity.resolved.intrinsic_width = img.intrinsic_width;
            entity.resolved.intrinsic_height = img.intrinsic_height;
        }

        // === Resolve base size ===
        let mut base_w = entity.resolved.width;
        let mut base_h = entity.resolved.height;

        if base_w <= 0.0 || base_h <= 0.0 {
            if entity.resolved.intrinsic_width > 0.0 && entity.resolved.intrinsic_height > 0.0 {
                base_w = entity.resolved.intrinsic_width;
                base_h = entity.resolved.intrinsic_height;
            } else if let Some(cam) = storages.get_component::<crate::ecs::components::CameraComponent>(&entity.id) {
                base_w = cam.resolution_width as f32;
                base_h = cam.resolution_height as f32;
            } else {
                base_w = 200.0;
                base_h = 200.0;
            }
        }

        // === Apply scale ===
        entity.resolved.width = base_w * entity.resolved.scale_x;
        entity.resolved.height = base_h * entity.resolved.scale_y;

        // === Aspect ratios ===
        if entity.resolved.intrinsic_height > 0.0 {
            entity.resolved.aspect_ratio = entity.resolved.intrinsic_width / entity.resolved.intrinsic_height;
        } else {
            entity.resolved.aspect_ratio = 1.0;
        }

        if entity.resolved.height > 0.0 {
            entity.resolved.display_aspect = entity.resolved.width / entity.resolved.height;
        } else {
            entity.resolved.display_aspect = 1.0;
        }
    }
}
