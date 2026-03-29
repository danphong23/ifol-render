//! Comprehensive tests for the Dynamic Sparse Set ECS Architecture.
//!
//! Covers:
//! - TC-ECS-01: TypeMap insert/get/get_mut/get_component primitives
//! - TC-ECS-02: ComponentRegistry zero-touch registration & loading
//! - TC-ECS-03: World::load_scene round-trip from JSON
//! - TC-ECS-04: System pipeline integration (time + animation + hierarchy)
//! - TC-ECS-05: NewType wrappers (ParentId, Layer, Materials)
//! - TC-ECS-06: Edge cases (missing components, unknown keys, overwrite)

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::ecs::typemap::TypeMap;
    use crate::ecs::registry::ComponentRegistry;
    use crate::ecs::{Entity, World};
    use crate::ecs::components::*;
    use crate::ecs::components::meta::*;
    use crate::scene::Lifespan;

    // ══════════════════════════════════════════
    // TC-ECS-01: TypeMap Primitives
    // ══════════════════════════════════════════

    #[test]
    fn tc_ecs_01a_typemap_insert_and_get() {
        let mut tm = TypeMap::new();
        tm.insert(42_i32);
        tm.insert("hello".to_string());

        assert_eq!(tm.get::<i32>(), Some(&42));
        assert_eq!(tm.get::<String>(), Some(&"hello".to_string()));
        assert_eq!(tm.get::<f64>(), None); // not inserted
    }

    #[test]
    fn tc_ecs_01b_typemap_get_mut() {
        let mut tm = TypeMap::new();
        tm.insert(10_i32);

        if let Some(val) = tm.get_mut::<i32>() {
            *val = 20;
        }
        assert_eq!(tm.get::<i32>(), Some(&20));
    }

    #[test]
    fn tc_ecs_01c_typemap_get_component_entity_lookup() {
        let mut tm = TypeMap::new();
        let mut col: HashMap<String, f32> = HashMap::new();
        col.insert("ent_a".to_string(), 99.5);
        col.insert("ent_b".to_string(), 42.0);
        tm.insert(col);

        assert_eq!(tm.get_component::<f32>("ent_a"), Some(&99.5));
        assert_eq!(tm.get_component::<f32>("ent_b"), Some(&42.0));
        assert_eq!(tm.get_component::<f32>("ent_c"), None); // nonexistent entity
    }

    #[test]
    fn tc_ecs_01d_typemap_get_component_mut() {
        let mut tm = TypeMap::new();
        let mut col: HashMap<String, f32> = HashMap::new();
        col.insert("hero".to_string(), 100.0);
        tm.insert(col);

        if let Some(hp) = tm.get_component_mut::<f32>("hero") {
            *hp -= 25.0;
        }
        assert_eq!(tm.get_component::<f32>("hero"), Some(&75.0));
    }

    #[test]
    fn tc_ecs_01e_typemap_overwrite_same_type() {
        let mut tm = TypeMap::new();
        tm.insert(1_i32);
        assert_eq!(tm.get::<i32>(), Some(&1));

        tm.insert(999_i32); // overwrite
        assert_eq!(tm.get::<i32>(), Some(&999));
    }

    // ══════════════════════════════════════════
    // TC-ECS-02: ComponentRegistry
    // ══════════════════════════════════════════

    #[test]
    fn tc_ecs_02a_registry_has_all_core_components() {
        let reg = ComponentRegistry::default();
        let expected_keys = vec![
            "shapeSource", "videoSource", "imageSource", "textSource",
            "colorSource", "audioSource", "camera", "transform", "rect",
            "visual", "animation", "composition", "lifespan",
            "parentId", "maskId", "layer", "materials",
            "floatUniforms", "stringUniforms",
        ];
        for key in &expected_keys {
            assert!(
                reg.loaders.contains_key(*key),
                "Registry missing loader for '{}'", key
            );
        }
    }

    #[test]
    fn tc_ecs_02b_registry_loads_transform_from_json() {
        let mut world = World::new();
        world.add_entity(Entity {
            id: "test_ent".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });

        let json_val: serde_json::Value = serde_json::json!({
            "x": 100.0, "y": 200.0, "rotation": 45.0,
            "anchorX": 0.5, "anchorY": 0.5,
            "scaleX": 2.0, "scaleY": 3.0
        });
        
        let loader = world.registry.loaders.get("transform").copied().unwrap();
        loader(&mut world, "test_ent", &json_val).unwrap();

        let t = world.get_component::<Transform>("test_ent").unwrap();
        assert_eq!(t.x, 100.0);
        assert_eq!(t.y, 200.0);
        assert_eq!(t.scale_x, 2.0);
    }

    #[test]
    fn tc_ecs_02c_registry_loads_newtype_parentid() {
        let mut world = World::new();
        world.add_entity(Entity {
            id: "child".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });

        let json_val = serde_json::json!("parent_comp_1");
        let loader = world.registry.loaders.get("parentId").copied().unwrap();
        loader(&mut world, "child", &json_val).unwrap();

        let pid = world.get_component::<ParentId>("child").unwrap();
        assert_eq!(pid.0, "parent_comp_1");
    }

    #[test]
    fn tc_ecs_02d_registry_loads_newtype_layer() {
        let mut world = World::new();
        world.add_entity(Entity {
            id: "layered".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });

        let json_val = serde_json::json!(5);
        let loader = world.registry.loaders.get("layer").copied().unwrap();
        loader(&mut world, "layered", &json_val).unwrap();

        let layer = world.get_component::<Layer>("layered").unwrap();
        assert_eq!(layer.0, 5);
    }

    // ══════════════════════════════════════════
    // TC-ECS-03: World::load_scene Round-Trip
    // ══════════════════════════════════════════

    #[test]
    fn tc_ecs_03a_load_scene_full_entity() {
        let scene_json = serde_json::json!({
            "entities": [
                {
                    "id": "cam1",
                    "camera": { "resolutionHeight": 720 },
                    "transform": { "x": 0.0, "y": 0.0, "rotation": 0.0, "anchorX": 0.0, "anchorY": 0.0, "scaleX": 1.0, "scaleY": 1.0 },
                    "lifespan": { "start": 0.0, "end": 10.0 },
                    "layer": 0
                },
                {
                    "id": "img1",
                    "imageSource": { "assetId": "bg_image" },
                    "transform": { "x": 100.0, "y": 50.0, "rotation": 0.0, "anchorX": 0.5, "anchorY": 0.5, "scaleX": 1.0, "scaleY": 1.0 },
                    "rect": { "width": 300.0, "height": 200.0, "fitMode": "contain", "alignX": 0.5, "alignY": 0.5 },
                    "lifespan": { "start": 0.0, "end": 5.0 },
                    "parentId": "cam1",
                    "layer": 1
                }
            ],
            "assets": {
                "bg_image": { "type": "image", "url": "https://example.com/bg.png" }
            }
        });

        let scene: crate::scene::SceneV2 = serde_json::from_value(scene_json).unwrap();
        let mut world = World::new();
        world.load_scene(&scene);

        // Verify entities exist
        assert_eq!(world.entities.len(), 2);
        assert_eq!(world.entities[0].id, "cam1");
        assert_eq!(world.entities[1].id, "img1");

        // Verify components loaded into storages
        let cam = world.get_component::<CameraComponent>("cam1").unwrap();
        assert_eq!(cam.resolution_height, 720);

        let t = world.get_component::<Transform>("img1").unwrap();
        assert_eq!(t.x, 100.0);
        assert_eq!(t.y, 50.0);

        let r = world.get_component::<Rect>("img1").unwrap();
        assert_eq!(r.width, 300.0);
        assert_eq!(r.height, 200.0);

        let img = world.get_component::<ImageSource>("img1").unwrap();
        assert_eq!(img.asset_id, "bg_image");

        let pid = world.get_component::<ParentId>("img1").unwrap();
        assert_eq!(pid.0, "cam1");

        let layer = world.get_component::<Layer>("img1").unwrap();
        assert_eq!(layer.0, 1);

        let ls = world.get_component::<Lifespan>("img1").unwrap();
        assert_eq!(ls.start, 0.0);
        assert_eq!(ls.end, 5.0);
    }

    #[test]
    fn tc_ecs_03b_load_scene_asset_resolution() {
        let scene_json = serde_json::json!({
            "entities": [],
            "assets": {
                "my_vid": { "type": "video", "url": "https://cdn.example.com/video.mp4" },
                "my_img": { "type": "image", "url": "./local/photo.png" }
            }
        });

        let scene: crate::scene::SceneV2 = serde_json::from_value(scene_json).unwrap();
        let mut world = World::new();
        world.load_scene(&scene);

        assert_eq!(world.resolve_asset_url("my_vid"), Some("https://cdn.example.com/video.mp4"));
        assert_eq!(world.resolve_asset_url("my_img"), Some("./local/photo.png"));
        assert_eq!(world.resolve_asset_url("nonexistent"), None);
    }

    #[test]
    fn tc_ecs_03c_load_scene_ignores_unknown_components() {
        let scene_json = serde_json::json!({
            "entities": [
                {
                    "id": "ent1",
                    "transform": { "x": 1.0, "y": 2.0, "rotation": 0.0, "anchorX": 0.0, "anchorY": 0.0, "scaleX": 1.0, "scaleY": 1.0 },
                    "futureBlurEffect": { "radius": 10.0 }
                }
            ],
            "assets": {}
        });

        let scene: crate::scene::SceneV2 = serde_json::from_value(scene_json).unwrap();
        let mut world = World::new();
        world.load_scene(&scene);

        // Known component loaded
        assert!(world.get_component::<Transform>("ent1").is_some());
        // Unknown component silently ignored (no crash)
        assert_eq!(world.entities.len(), 1);
    }

    // ══════════════════════════════════════════
    // TC-ECS-04: System Pipeline Integration
    // ══════════════════════════════════════════

    #[test]
    fn tc_ecs_04a_full_pipeline_run() {
        let scene_json = serde_json::json!({
            "entities": [
                {
                    "id": "cam",
                    "camera": { "resolutionHeight": 1080 },
                    "transform": { "x": 0.0, "y": 0.0, "rotation": 0.0, "anchorX": 0.0, "anchorY": 0.0, "scaleX": 1.0, "scaleY": 1.0 },
                    "rect": { "width": 1920.0, "height": 1080.0, "fitMode": "stretch", "alignX": 0.5, "alignY": 0.5 },
                    "lifespan": { "start": 0.0, "end": 10.0 }
                },
                {
                    "id": "box1",
                    "shapeSource": { "kind": "rectangle", "cornerRadius": 0.0, "fillColor": { "r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0 } },
                    "transform": { "x": 100.0, "y": 200.0, "rotation": 0.0, "anchorX": 0.5, "anchorY": 0.5, "scaleX": 1.0, "scaleY": 1.0 },
                    "rect": { "width": 300.0, "height": 300.0, "fitMode": "stretch", "alignX": 0.5, "alignY": 0.5 },
                    "visual": { "opacity": 0.8, "blendMode": "normal" },
                    "lifespan": { "start": 0.0, "end": 5.0 }
                }
            ],
            "assets": {}
        });

        let scene: crate::scene::SceneV2 = serde_json::from_value(scene_json).unwrap();
        let mut world = World::new();
        world.load_scene(&scene);

        // Run the full system pipeline at t=1.0
        let time = crate::time::TimeState {
            global_time: 1.0,
            delta_time: 1.0 / 60.0,
            frame_index: 60,
            fps: 60.0,
        };
        crate::ecs::pipeline::run(&mut world, &time, None, None);

        // Camera should be visible
        assert!(world.entities[0].resolved.visible);
        // Box should be visible
        assert!(world.entities[1].resolved.visible);
        // Box resolved position should match transform
        assert_eq!(world.entities[1].resolved.x, 100.0);
        assert_eq!(world.entities[1].resolved.y, 200.0);
        // Box opacity from visual
        assert!((world.entities[1].resolved.opacity - 0.8).abs() < 0.01);
    }

    // ══════════════════════════════════════════
    // TC-ECS-05: NewType Wrapper Uniqueness
    // ══════════════════════════════════════════

    #[test]
    fn tc_ecs_05a_newtype_wrappers_dont_collide() {
        let mut world = World::new();
        world.add_entity(Entity {
            id: "e1".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });

        // ParentId and MaskId are both String wrappers but must occupy different columns
        world.add_component("e1", ParentId("parent_id_value".to_string()));
        world.add_component("e1", MaskId("mask_id_value".to_string()));
        world.add_component("e1", Layer(42));

        let pid = world.get_component::<ParentId>("e1").unwrap();
        let mid = world.get_component::<MaskId>("e1").unwrap();
        let layer = world.get_component::<Layer>("e1").unwrap();

        assert_eq!(pid.0, "parent_id_value");
        assert_eq!(mid.0, "mask_id_value");
        assert_eq!(layer.0, 42);
    }

    // ══════════════════════════════════════════
    // TC-ECS-06: Edge Cases
    // ══════════════════════════════════════════

    #[test]
    fn tc_ecs_06a_get_component_from_empty_world() {
        let world = World::new();
        assert!(world.get_component::<Transform>("nonexistent").is_none());
        assert!(world.get_component::<ParentId>("nonexistent").is_none());
    }

    #[test]
    fn tc_ecs_06b_add_component_creates_column_lazily() {
        let mut world = World::new();
        world.add_entity(Entity {
            id: "e1".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });

        // Column doesn't exist yet
        assert!(world.get_component::<Layer>("e1").is_none());

        // Adding creates the column
        world.add_component("e1", Layer(7));
        assert_eq!(world.get_component::<Layer>("e1").unwrap().0, 7);
    }

    #[test]
    fn tc_ecs_06c_overwrite_component_for_same_entity() {
        let mut world = World::new();
        world.add_entity(Entity {
            id: "e1".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });

        world.add_component("e1", Layer(1));
        assert_eq!(world.get_component::<Layer>("e1").unwrap().0, 1);

        // Overwrite with new value
        world.add_component("e1", Layer(99));
        assert_eq!(world.get_component::<Layer>("e1").unwrap().0, 99);
    }

    #[test]
    fn tc_ecs_06d_multiple_entities_same_component_type() {
        let mut world = World::new();
        for i in 0..5 {
            let id = format!("ent_{}", i);
            world.add_entity(Entity {
                id: id.clone(),
                resolved: Default::default(),
                draw: Default::default(),
            });
            world.add_component(&id, Layer(i as i32 * 10));
        }

        // Each entity has its own value
        for i in 0..5 {
            let id = format!("ent_{}", i);
            assert_eq!(world.get_component::<Layer>(&id).unwrap().0, i as i32 * 10);
        }
    }

    #[test]
    fn tc_ecs_06e_find_camera_uses_storages() {
        let mut world = World::new();

        // Non-camera entity
        world.add_entity(Entity {
            id: "box".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });

        // Camera entity
        world.add_entity(Entity {
            id: "cam1".to_string(),
            resolved: Default::default(),
            draw: Default::default(),
        });
        world.add_component("cam1", CameraComponent::default());
        world.entities[1].resolved.visible = true;

        // find_camera by id
        let found = world.find_camera("cam1");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "cam1");

        // find_camera fallback (empty string = first visible camera)
        let fallback = world.find_camera("");
        assert!(fallback.is_some());
        assert_eq!(fallback.unwrap().id, "cam1");

        // find_camera with non-camera entity should return None
        let not_cam = world.find_camera("box");
        assert!(not_cam.is_none());
    }

    #[test]
    fn tc_ecs_06f_load_scene_clears_previous_state() {
        let scene1 = serde_json::json!({
            "entities": [{ "id": "a", "layer": 1 }],
            "assets": {}
        });
        let scene2 = serde_json::json!({
            "entities": [{ "id": "b", "layer": 2 }, { "id": "c", "layer": 3 }],
            "assets": {}
        });

        let mut world = World::new();
        world.load_scene(&serde_json::from_value(scene1).unwrap());
        assert_eq!(world.entities.len(), 1);
        assert_eq!(world.get_component::<Layer>("a").unwrap().0, 1);

        // Loading a new scene should completely replace the old one
        world.load_scene(&serde_json::from_value(scene2).unwrap());
        assert_eq!(world.entities.len(), 2);
        assert!(world.get_component::<Layer>("a").is_none()); // old entity gone
        assert_eq!(world.get_component::<Layer>("b").unwrap().0, 2);
        assert_eq!(world.get_component::<Layer>("c").unwrap().0, 3);
    }
}
