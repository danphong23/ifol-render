use crate::ecs::World;
use crate::time::TimeState;

/// Transform system — resolves spatial properties ONLY.
///
/// Single responsibility: x, y, rotation, anchor, scale.
/// Does NOT resolve: width/height (rect_sys), opacity (visual_sys).
///
/// Must run BEFORE rect_sys (provides scale_x/y for size calculation).
pub fn transform_system(world: &mut World, _time: &TimeState) {
    for entity in &mut world.entities {
        if !entity.resolved.visible { continue; }

        if let Some(track) = &entity.components.transform {
            let t = entity.resolved.time.local_time;

            entity.resolved.x = track.x.evaluate(t, 0.0) as f32;
            entity.resolved.y = track.y.evaluate(t, 0.0) as f32;
            entity.resolved.rotation = track.rotation.evaluate(t, 0.0) as f32;
            entity.resolved.anchor_x = track.anchor_x.evaluate(t, 0.0) as f32;
            entity.resolved.anchor_y = track.anchor_y.evaluate(t, 0.0) as f32;
            entity.resolved.scale_x = track.scale_x.evaluate(t, 1.0) as f32;
            entity.resolved.scale_y = track.scale_y.evaluate(t, 1.0) as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{FloatTrack, Keyframe, Interpolation, TransformTrack, Lifespan};
    use crate::ecs::{Entity, Components, ResolvedState};

    // ── Helpers ──
    fn ft(v: f32) -> FloatTrack {
        FloatTrack { keyframes: vec![Keyframe { time: 0.0, value: v, interpolation: Interpolation::Linear }] }
    }

    fn ft_anim(kfs: &[(f64, f32)]) -> FloatTrack {
        FloatTrack {
            keyframes: kfs.iter().map(|&(t, v)| Keyframe {
                time: t, value: v, interpolation: Interpolation::Linear,
            }).collect(),
        }
    }

    fn make_entity(id: &str, transform: Option<TransformTrack>) -> Entity {
        let mut comps = Components::default();
        comps.transform = transform;
        Entity {
            id: id.to_string(),
            components: comps,
            resolved: ResolvedState { visible: true, ..Default::default() },
        }
    }

    fn time_state(t: f64) -> TimeState {
        let mut ts = TimeState::new(30.0);
        ts.seek(t);
        ts
    }

    // ══════════════════════════════════════
    // Isolated transform_system tests
    // ══════════════════════════════════════

    #[test]
    fn transform_resolves_static_values() {
        let tf = TransformTrack {
            x: ft(100.0), y: ft(50.0), rotation: ft(45.0),
            anchor_x: ft(0.5), anchor_y: ft(0.5),
            scale_x: ft(2.0), scale_y: ft(0.5),
            ..Default::default()
        };
        let mut world = World::new();
        world.add_entity(make_entity("e1", Some(tf)));
        world.entities[0].resolved.visible = true;
        world.entities[0].resolved.time.local_time = 0.0;

        transform_system(&mut world, &time_state(0.0));

        let r = &world.entities[0].resolved;
        assert!((r.x - 100.0).abs() < 0.01);
        assert!((r.y - 50.0).abs() < 0.01);
        assert!((r.rotation - 45.0).abs() < 0.01);
        assert!((r.anchor_x - 0.5).abs() < 0.01);
        assert!((r.anchor_y - 0.5).abs() < 0.01);
        assert!((r.scale_x - 2.0).abs() < 0.01);
        assert!((r.scale_y - 0.5).abs() < 0.01);
    }

    #[test]
    fn transform_skips_invisible_entity() {
        let tf = TransformTrack {
            x: ft(999.0), y: ft(999.0), ..Default::default()
        };
        let mut world = World::new();
        let mut ent = make_entity("hidden", Some(tf));
        ent.resolved.visible = false; // invisible
        world.add_entity(ent);

        transform_system(&mut world, &time_state(0.0));

        // Should NOT have been resolved — x/y should remain default (0.0)
        assert!((world.entities[0].resolved.x - 0.0).abs() < 0.01);
        assert!((world.entities[0].resolved.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn transform_no_track_keeps_defaults() {
        let mut world = World::new();
        let mut ent = make_entity("no_transform", None);
        ent.resolved.visible = true;
        world.add_entity(ent);

        transform_system(&mut world, &time_state(0.0));

        let r = &world.entities[0].resolved;
        // Default ResolvedState has scale 1.0 and everything else 0.0
        assert!((r.x - 0.0).abs() < 0.01);
        assert!((r.y - 0.0).abs() < 0.01);
        assert!((r.scale_x - 1.0).abs() < 0.01);
        assert!((r.scale_y - 1.0).abs() < 0.01);
    }

    #[test]
    fn transform_animated_keyframes() {
        let tf = TransformTrack {
            x: ft_anim(&[(0.0, 0.0), (4.0, 400.0)]),
            y: ft_anim(&[(0.0, 100.0), (2.0, 300.0)]),
            rotation: ft_anim(&[(0.0, 0.0), (4.0, 360.0)]),
            ..Default::default()
        };
        let mut world = World::new();
        world.add_entity(make_entity("animated", Some(tf)));

        // Test at t=0
        world.entities[0].resolved.visible = true;
        world.entities[0].resolved.time.local_time = 0.0;
        transform_system(&mut world, &time_state(0.0));
        assert!((world.entities[0].resolved.x - 0.0).abs() < 0.01);
        assert!((world.entities[0].resolved.y - 100.0).abs() < 0.01);

        // Test at t=2 (midpoint of x, end of y)
        world.entities[0].resolved.time.local_time = 2.0;
        transform_system(&mut world, &time_state(2.0));
        assert!((world.entities[0].resolved.x - 200.0).abs() < 0.01);
        assert!((world.entities[0].resolved.y - 300.0).abs() < 0.01);
        assert!((world.entities[0].resolved.rotation - 180.0).abs() < 0.01);

        // Test at t=4 (end)
        world.entities[0].resolved.time.local_time = 4.0;
        transform_system(&mut world, &time_state(4.0));
        assert!((world.entities[0].resolved.x - 400.0).abs() < 0.01);
        assert!((world.entities[0].resolved.rotation - 360.0).abs() < 0.01);
    }

    #[test]
    fn transform_default_scale_is_one() {
        // TransformTrack with no scale keyframes → scale defaults to 1.0
        let tf = TransformTrack {
            x: ft(50.0), y: ft(50.0),
            ..Default::default()
        };
        let mut world = World::new();
        world.add_entity(make_entity("e1", Some(tf)));
        world.entities[0].resolved.visible = true;
        world.entities[0].resolved.time.local_time = 0.0;

        transform_system(&mut world, &time_state(0.0));

        // Default FloatTrack is empty, so evaluate(t, default) returns default
        // scale_x default = 1.0, scale_y default = 1.0
        assert!((world.entities[0].resolved.scale_x - 1.0).abs() < 0.01);
        assert!((world.entities[0].resolved.scale_y - 1.0).abs() < 0.01);
    }

    // ══════════════════════════════════════
    // Pipeline integration: composition → timeline → transform
    // ══════════════════════════════════════

    #[test]
    fn pipeline_single_entity_no_lifespan() {
        // Entity without lifespan = always visible, local_time = global_time
        let tf = TransformTrack {
            x: ft_anim(&[(0.0, 0.0), (10.0, 500.0)]),
            y: ft(100.0),
            ..Default::default()
        };
        let mut world = World::new();
        world.add_entity(make_entity("e1", Some(tf)));
        world.rebuild_index();

        let ts = time_state(5.0);
        // Run full pipeline
        crate::ecs::pipeline::run(&mut world, &ts);

        let r = &world.entities[0].resolved;
        assert!(r.visible);
        assert!((r.x - 250.0).abs() < 0.01, "x was {} (expected 250)", r.x);
        assert!((r.y - 100.0).abs() < 0.01);
    }

    #[test]
    fn pipeline_single_entity_with_lifespan_visible() {
        let tf = TransformTrack { x: ft(100.0), y: ft(50.0), ..Default::default() };
        let mut comps = Components::default();
        comps.transform = Some(tf);
        comps.lifespan = Some(Lifespan { start: 2.0, end: 8.0 });
        let mut ent = Entity { id: "e1".into(), components: comps, resolved: ResolvedState::default() };
        ent.resolved.visible = false;

        let mut world = World::new();
        world.add_entity(ent);
        world.rebuild_index();

        // At t=5.0 → inside lifespan [2,8), local_time = 5 - 2 = 3
        let ts = time_state(5.0);
        crate::ecs::pipeline::run(&mut world, &ts);

        let r = &world.entities[0].resolved;
        assert!(r.visible, "should be visible at t=5 in lifespan [2,8)");
        assert!((r.time.local_time - 3.0).abs() < 0.001, "local_time was {}", r.time.local_time);
        assert!((r.x - 100.0).abs() < 0.01);
    }

    #[test]
    fn pipeline_single_entity_with_lifespan_hidden() {
        let tf = TransformTrack { x: ft(100.0), y: ft(50.0), ..Default::default() };
        let mut comps = Components::default();
        comps.transform = Some(tf);
        comps.lifespan = Some(Lifespan { start: 2.0, end: 8.0 });
        let ent = Entity { id: "e1".into(), components: comps, resolved: ResolvedState::default() };

        let mut world = World::new();
        world.add_entity(ent);
        world.rebuild_index();

        // At t=1.0 → before lifespan [2,8)
        let ts = time_state(1.0);
        crate::ecs::pipeline::run(&mut world, &ts);
        assert!(!world.entities[0].resolved.visible, "should be hidden at t=1 before lifespan [2,8)");

        // At t=8.0 → at end (exclusive)
        let ts2 = time_state(8.0);
        crate::ecs::pipeline::run(&mut world, &ts2);
        assert!(!world.entities[0].resolved.visible, "should be hidden at t=8 (end is exclusive)");
    }

    #[test]
    fn pipeline_animated_transform_with_lifespan() {
        // Entity with lifespan [1, 5) and x: 0→400 over [0, 4] (relative to local_time)
        let tf = TransformTrack {
            x: ft_anim(&[(0.0, 0.0), (4.0, 400.0)]),
            ..Default::default()
        };
        let mut comps = Components::default();
        comps.transform = Some(tf);
        comps.lifespan = Some(Lifespan { start: 1.0, end: 5.0 });
        let ent = Entity { id: "e1".into(), components: comps, resolved: ResolvedState::default() };

        let mut world = World::new();
        world.add_entity(ent);
        world.rebuild_index();

        // At global t=3.0, local_time = 3 - 1 = 2.0
        // x at local_time=2.0: 0 + (400-0) * 2/4 = 200
        let ts = time_state(3.0);
        crate::ecs::pipeline::run(&mut world, &ts);

        let r = &world.entities[0].resolved;
        assert!(r.visible);
        assert!((r.time.local_time - 2.0).abs() < 0.001);
        assert!((r.x - 200.0).abs() < 0.01, "x was {} (expected 200)", r.x);
    }
}

