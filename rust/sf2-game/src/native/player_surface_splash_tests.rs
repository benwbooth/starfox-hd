use super::*;
use crate::path_appearance::AnimationControl;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategySchedule};
use crate::{Angle, ObjectSpawnDefaults, RandomState};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("splash escaped native dispatch")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected splash death override")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map continuation")
    }
}

fn fixture() -> (ObjectStore, ScenePathWorld, ObjectId, SplashInputs) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let origin = objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.hit_points = 30;
    actor.base.position = Vector3 {
        x: 500,
        y: -100,
        z: 300,
    };
    actor.base.velocity = Vector3 {
        x: 27,
        y: -99,
        z: -18,
    };
    actor.base.pitch = Angle::from_units(71);
    let actor = objects.get_mut(origin).unwrap();
    actor.base.position = Vector3 {
        x: i16::MAX,
        y: -110,
        z: i16::MIN,
    };
    actor.base.velocity = Vector3 {
        x: -500,
        y: 200,
        z: 500,
    };
    let mut world = ScenePathWorld::new(RandomState::new([43, 91, 121, 202]));
    world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 42,
        run_when_paused: false,
    });
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                surface: Some(crate::player_surface::PlayerSurface {
                    plane_height: 19,
                    material: 4,
                }),
                ..Default::default()
            },
        )
        .unwrap();
    (
        objects,
        world,
        owner,
        SplashInputs {
            origin,
            lateral: 5,
            forward: -7,
            frame: 0,
            size: 7,
        },
    )
}

#[test]
fn splash_placement_uses_distinct_origin_then_first_visit_uses_live_attachment_motion_only() {
    for kind in [SurfaceSplash::Short, SurfaceSplash::Long] {
        let (mut objects, mut world, owner, input) = fixture();
        let expected_number = world.random.next_byte();
        world.random = RandomState::new([43, 91, 121, 202]);
        let child = spawn(&mut objects, &mut world, owner, kind, input).unwrap();
        let actor = objects.get(child).unwrap();
        let initial = actor.base.position;
        assert_eq!(actor.base.attachment, Some(owner));
        assert_eq!(actor.extension.parent, Some(child));
        assert_eq!(actor.base.effect_origin.unwrap().slot(), input.origin);
        assert_eq!(actor.base.child_number, expected_number);
        assert_eq!(
            actor.extension.texture_scroll_x,
            if kind == SurfaceSplash::Short { 0 } else { 7 }
        );
        assert_eq!(actor.base.pitch.units(), 71);
        assert!(actor.base.flags.general_search_eligible);
        assert!(actor.base.flags.remove_with_parent);
        assert_eq!(actor.base.velocity, Vector3::default());
        let parent = objects.get_mut(owner).unwrap();
        parent.base.velocity = Vector3 {
            x: 29,
            y: 2000,
            z: -13,
        };
        step(&mut objects, child).unwrap();
        let moved = Vector3 {
            x: initial.x.wrapping_add(29),
            y: initial.y,
            z: initial.z.wrapping_sub(13),
        };
        assert_eq!(objects.get(child).unwrap().base.position, moved);
        objects.get_mut(owner).unwrap().base.velocity.x = 800;
        step(&mut objects, child).unwrap();
        assert_eq!(objects.get(child).unwrap().base.position, moved);
    }
}

#[test]
fn short_clears_long_inherits_frame_and_size_and_neither_caps_child_count() {
    let (mut objects, mut world, owner, mut input) = fixture();
    input.frame = 239;
    input.size = 193;
    for index in 0..9 {
        let kind = if index % 2 == 0 {
            SurfaceSplash::Short
        } else {
            SurfaceSplash::Long
        };
        let child = spawn(&mut objects, &mut world, owner, kind, input).unwrap();
        let actor = objects.get(child).unwrap();
        assert_eq!(
            actor.extension.path_state.animation.shape.packed(),
            if kind == SurfaceSplash::Short {
                128
            } else {
                239
            }
        );
        assert_eq!(
            actor.extension.texture_scroll_x,
            if kind == SurfaceSplash::Short { 0 } else { 193 }
        );
    }
    assert_eq!(objects.len(), 11);
}

#[test]
fn full_pool_precedes_random_draw_and_missing_surface_faults_after_installation() {
    let (mut objects, mut world, owner, input) = fixture();
    world.player_mut(&objects, owner).unwrap().surface = None;
    assert_eq!(
        spawn(&mut objects, &mut world, owner, SurfaceSplash::Long, input),
        Err(SplashError::MissingSurface(owner))
    );
    assert_eq!(objects.len(), 3);
    while objects.len() < OBJECT_CAPACITY {
        objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
    }
    world.spawn_defaults = None;
    let random = world.random;
    let before = objects.clone();
    assert_eq!(
        spawn(&mut objects, &mut world, owner, SurfaceSplash::Short, input),
        Err(SplashError::ObjectPoolExhausted)
    );
    assert_eq!(world.random, random);
    assert_eq!(objects, before);
}

#[test]
fn all_packed_animation_controls_and_live_limits_keep_manual_terminal_frame() {
    let (mut objects, mut world, owner, input) = fixture();
    let child = spawn(&mut objects, &mut world, owner, SurfaceSplash::Long, input).unwrap();
    for frames in 0..=u8::MAX {
        for packed in 0..=u8::MAX {
            let actor = objects.get_mut(child).unwrap();
            actor.base.behavior = Behavior::SurfaceSplash(SplashPhase::Animate);
            actor.base.flags.remove_after_tick = false;
            actor.extension.path_state.motion_phase = u16::from(frames) << 8 | 0x5A;
            actor.extension.path_state.animation.shape = AnimationControl::from_packed(packed);
            step(&mut objects, child).unwrap();
            let actor = objects.get(child).unwrap();
            let mut next = packed.wrapping_add(1);
            if next < 128 {
                next = next.wrapping_add(frames);
            }
            next &= 127;
            assert_eq!(actor.base.flags.remove_after_tick, next >= frames);
            assert_eq!(
                actor.extension.path_state.animation.shape.packed(),
                if next >= frames {
                    frames.wrapping_sub(1) | 128
                } else {
                    next | 128
                }
            );
            assert_eq!(actor.extension.path_state.motion_phase & 255, 0x5A);
        }
    }
}

#[test]
fn real_scheduler_runs_both_splashes_during_pause_and_retires_with_attachment_cleanup() {
    for kind in [SurfaceSplash::Short, SurfaceSplash::Long] {
        let (mut objects, mut world, owner, input) = fixture();
        let mut execution = SceneExecution::default();
        execution.controls.paused = true;
        let catalog = crate::authored_paths::catalog();
        let mut callbacks = Callbacks;
        let mut schedule = StrategySchedule::default();
        let mut host = SceneActors {
            objects: &mut objects,
            world: &mut world,
            execution: &mut execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 16,
        };
        let child = host
            .spawn_player_surface_splash(owner, kind, input)
            .unwrap();
        for visit in 1..=kind.frames() {
            host.execution.positional.begin_epoch();
            host.begin_strategy_epoch(&mut schedule).unwrap();
            schedule.run_overlapping(&mut host, || false).unwrap();
            schedule.run_remainder(&mut host).unwrap();
            assert_eq!(
                host.objects
                    .get(child)
                    .unwrap()
                    .base
                    .flags
                    .remove_after_tick,
                visit == kind.frames()
            );
            host.clean_epoch().unwrap();
            assert_eq!(host.objects.get(child).is_some(), visit < kind.frames());
        }
        assert_eq!(host.objects.get(owner).unwrap().base.attachment_next, None);
        assert_eq!(host.objects.len(), 2);
    }
}

#[test]
fn scene_wrapper_latches_partial_installation_and_never_allocates_again_on_retry() {
    let (mut objects, mut world, owner, input) = fixture();
    world.player_mut(&objects, owner).unwrap().surface = None;
    let mut execution = SceneExecution::default();
    let catalog = crate::authored_paths::catalog();
    let mut callbacks = Callbacks;
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 16,
    };
    assert_eq!(
        host.spawn_player_surface_splash(owner, SurfaceSplash::Long, input),
        Err(SceneError::PlayerSurfaceSplash(
            SplashError::MissingSurface(owner)
        ))
    );
    assert!(host.execution.is_faulted());
    let before = host.objects.clone();
    assert_eq!(
        host.spawn_player_surface_splash(owner, SurfaceSplash::Long, input),
        Err(SceneError::Faulted)
    );
    assert_eq!(*host.objects, before);
}
