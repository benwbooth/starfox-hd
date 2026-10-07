use super::*;
use crate::path_runtime::PathRuntime;
use crate::path_scene_state::EncounterCoordination;
use crate::player_palette::PlayerPaletteControl;
use crate::player_storage::PlayerStorageInputs;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId, Vector3};

fn fixture() -> (ObjectStore, ScenePathWorld, PathRuntime, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let mut world = ScenePathWorld::new(RandomState::default());
    let mut runtime = PathRuntime::default();
    player_storage::replace(
        &mut objects,
        &mut world,
        &mut runtime,
        owner,
        PlayerStorageInputs {
            pilot_code: 2,
            reserve_shield: 40,
            score: Default::default(),
        },
    )
    .unwrap();
    world.scene.player_configuration = Some(0);
    world.contacts_enabled = Some(false);
    world.primary_player = Some(owner);
    (objects, world, runtime, owner)
}

#[test]
fn post_motion_cooldown_and_contact_edges_cover_all_bytes_without_retriggering() {
    let (mut objects, mut world, runtime, owner) = fixture();
    for cooldown in 0..=u8::MAX {
        for flags in 0..=u8::MAX {
            let r = world.player_mut(&objects, owner).unwrap();
            r.contact.as_mut().unwrap().hit.deflection_sound_cooldown = cooldown;
            r.motion.as_mut().unwrap().contact_flags = flags;
            r.charge.as_mut().unwrap().speed_impulse = -31;
            r.charge.as_mut().unwrap().speed_impulse_ticks = 197;
            prepare(&mut objects, &mut world, &runtime.resources, owner).unwrap();
            let r = world.player(&objects, owner).unwrap();
            assert_eq!(
                r.contact.unwrap().hit.deflection_sound_cooldown,
                if cooldown == 0 { 0 } else { (cooldown - 1) & 7 }
            );
            assert_eq!(
                r.motion.unwrap().contact_flags,
                if flags & 16 == 0 {
                    flags & !24
                } else {
                    (flags & !16) | 8
                }
            );
            let fresh = flags & 24 == 16;
            assert_eq!(
                r.charge.unwrap().speed_impulse,
                if fresh { 70 } else { -31 }
            );
            assert_eq!(r.charge.unwrap().speed_impulse_ticks, 197);
            assert_eq!(
                world.audio.take_events().into_iter().flatten().count(),
                usize::from(fresh)
            );
        }
    }
}

#[test]
fn post_motion_numbered_child_uses_fine_heading_including_pending_removal() {
    let (mut objects, mut world, mut runtime, owner) = fixture();
    let position = Vector3 {
        x: i16::MIN,
        y: 213,
        z: i16::MAX,
    };
    objects.get_mut(owner).unwrap().base.position = position;
    objects.get_mut(owner).unwrap().base.yaw = Angle::from_units(17);
    player_storage::get_mut(&objects, &mut runtime.resources, owner)
        .unwrap()
        .fine_yaw = 0xCA37;
    let first = objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let second = objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    path_relationships::attach_fresh_child(&mut objects, owner, first, 22).unwrap();
    path_relationships::attach_fresh_child(&mut objects, owner, second, 22).unwrap();
    let selected = path_relationships::find_direct_child(&objects, owner, 22)
        .unwrap()
        .unwrap();
    let untouched = if selected == first { second } else { first };
    objects
        .get_mut(selected)
        .unwrap()
        .base
        .flags
        .remove_after_tick = true;
    objects.get_mut(selected).unwrap().base.pitch = Angle::from_units(73);
    objects.get_mut(selected).unwrap().base.roll = Angle::from_units(47);
    let retained = objects.get(untouched).unwrap().clone();
    prepare(&mut objects, &mut world, &runtime.resources, owner).unwrap();
    let selected = objects.get(selected).unwrap();
    assert_eq!(selected.base.position, position);
    assert_eq!(selected.base.pitch, Angle::ZERO);
    assert_eq!(selected.base.roll, Angle::ZERO);
    assert_eq!(selected.base.yaw, Angle::from_units(202));
    assert!(selected.base.flags.remove_after_tick);
    assert_eq!(objects.get(untouched), Some(&retained));
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .motion
            .unwrap()
            .previous_position,
        position
    );
}

#[test]
fn post_motion_progress_request_has_location_specific_sentinel_and_a_sticky_latch() {
    let (mut objects, mut world, runtime, owner) = fixture();
    for configuration in [0, 8, 9, 255] {
        for location in [0, 11, 12, u16::MAX] {
            for progress in 0..=u8::MAX {
                for flags in [0, 0x20, 0x40, 0x60, 0x80, 0xA0, 0xC0, 0xE0] {
                    world.scene.player_configuration = Some(configuration);
                    world.scene.encounter_location = Some(location);
                    world.coordination = Some(EncounterCoordination {
                        progress,
                        ..Default::default()
                    });
                    world.player_mut(&objects, owner).unwrap().palette_effects =
                        Some(PlayerPaletteControl::from_control(flags));
                    prepare(&mut objects, &mut world, &runtime.resources, owner).unwrap();
                    let request =
                        configuration == 9 && progress == if location == 11 { 255 } else { 254 };
                    assert_eq!(
                        world
                            .player(&objects, owner)
                            .unwrap()
                            .palette_effects
                            .unwrap()
                            .bits(),
                        if request && flags & 128 == 0 {
                            flags | 0xE0
                        } else {
                            flags
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn post_motion_music_is_gated_and_latched_per_player_without_touching_other_flags() {
    let (mut objects, mut world, runtime, owner) = fixture();
    for flags in 0..=u8::MAX {
        for gates in 0..8 {
            world.contacts_enabled = Some(gates & 1 != 0);
            world.interception_active = Some(gates & 2 != 0);
            world.interception_music_ready = Some(gates & 4 != 0);
            world
                .player_mut(&objects, owner)
                .unwrap()
                .mission
                .as_mut()
                .unwrap()
                .flags = flags;
            world
                .audio
                .request_music_control(MusicControlRequest::EncounterExit);
            prepare(&mut objects, &mut world, &runtime.resources, owner).unwrap();
            assert_eq!(
                world
                    .player(&objects, owner)
                    .unwrap()
                    .mission
                    .unwrap()
                    .flags,
                if gates == 7 { flags | 0x20 } else { flags }
            );
            let expected = if gates == 7 && flags & 0x20 == 0 {
                MusicControlRequest::InterceptionReady
            } else {
                MusicControlRequest::EncounterExit
            };
            assert_eq!(world.audio.pending_music_control(), Some(expected));
            world
                .audio
                .request_music_control(MusicControlRequest::EncounterExit);
            prepare(&mut objects, &mut world, &runtime.resources, owner).unwrap();
            assert_eq!(
                world.audio.pending_music_control(),
                Some(MusicControlRequest::EncounterExit)
            );
        }
    }
}

#[test]
fn post_motion_walker_skips_feedback_and_flight_preserves_existing_recoil_and_impulse_age() {
    let (mut objects, mut world, runtime, owner) = fixture();
    for mode in 0..=u8::MAX {
        for flags in [0, 0x20, 255] {
            for recoil in [i16::MIN, -1, 0, 1, i16::MAX] {
                let r = world.player_mut(&objects, owner).unwrap();
                r.auxiliary.as_mut().unwrap().mode = mode;
                r.auxiliary.as_mut().unwrap().action_flags = flags;
                r.motion.as_mut().unwrap().contact_flags = 0xD7;
                r.contact.as_mut().unwrap().hit.camera_pitch_recoil = recoil;
                r.contact.as_mut().unwrap().hit.feedback_flags = 0x91;
                r.contact.as_mut().unwrap().hit.feedback_duration = 239;
                r.charge.as_mut().unwrap().speed_impulse = -17;
                r.charge.as_mut().unwrap().speed_impulse_ticks = 247;
                let before = *r;
                prepare(&mut objects, &mut world, &runtime.resources, owner).unwrap();
                let r = world.player(&objects, owner).unwrap();
                assert_eq!(r.motion.unwrap().contact_flags, 0xCF);
                if mode & 0xF0 == 0x20 {
                    assert_eq!(r.contact, before.contact);
                    assert_eq!(r.charge, before.charge);
                    assert!(world.audio.take_events().iter().all(Option::is_none));
                } else {
                    assert_eq!(r.contact.unwrap().hit.feedback_duration, 2);
                    assert_eq!(r.contact.unwrap().hit.feedback_flags, 0xF9);
                    assert_eq!(
                        r.contact.unwrap().hit.camera_pitch_recoil,
                        if recoil == 0 { 128 } else { recoil }
                    );
                    assert_eq!(
                        r.charge.unwrap().speed_impulse,
                        if flags & 32 == 0 { 70 } else { 10 }
                    );
                    assert_eq!(r.charge.unwrap().speed_impulse_ticks, 247);
                    let expected = Some(SoundEvent::Authored(AuthoredCue::new(
                        167,
                        0,
                        PlayerTarget::Primary,
                    )));
                    assert_eq!(world.audio.take_events()[0], expected);
                }
            }
        }
    }
}

#[test]
fn post_motion_missing_late_input_retains_prefix_and_scene_fault_prevents_replay() {
    use crate::path_program::PathCatalog;
    use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
    use crate::strategy_schedule::StrategyCompletion;
    struct Callbacks;
    impl SceneCallbacks for Callbacks {
        type Error = ();
        fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
            panic!("not dispatched")
        }
        fn death_override(
            _: &mut SceneActors<'_, Self>,
            _: ObjectId,
        ) -> Result<Option<StrategyCompletion>, ()> {
            panic!("not dispatched")
        }
        fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
            panic!("not dispatched")
        }
    }
    let (mut objects, mut world, runtime, owner) = fixture();
    world.contacts_enabled = Some(true);
    world
        .player_mut(&objects, owner)
        .unwrap()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .deflection_sound_cooldown = 5;
    let position = Vector3 {
        x: 919,
        y: -291,
        z: 331,
    };
    objects.get_mut(owner).unwrap().base.position = position;
    let mut execution = SceneExecution::default();
    execution.paths.runtime = runtime;
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut callbacks = Callbacks;
    let mut scene = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 10,
    };
    assert_eq!(
        scene.advance_player_post_motion(owner, 71),
        Err(SceneError::PlayerPostMotion(
            PostMotionError::MissingInterceptionGate
        ))
    );
    let r = scene.world.player(scene.objects, owner).unwrap();
    assert_eq!(r.contact.unwrap().hit.deflection_sound_cooldown, 4);
    assert_eq!(r.motion.unwrap().previous_position, position);
    scene.world.interception_active = Some(false);
    assert_eq!(
        scene.advance_player_post_motion(owner, 71),
        Err(SceneError::Faulted)
    );
    assert_eq!(
        scene
            .world
            .player(scene.objects, owner)
            .unwrap()
            .contact
            .unwrap()
            .hit
            .deflection_sound_cooldown,
        4
    );
}

#[test]
fn post_motion_authored_readiness_reaches_the_same_music_gate_without_prior_publication() {
    use crate::path_commands::ControlCommand;
    use crate::path_control::PlayerTarget;
    use crate::path_invocation::InvocationWorld;
    use crate::path_program::{PathCatalog, PathWorld, ProgramError, Statement};
    use crate::{PathCursor, PathId};
    let (mut objects, mut world, mut runtime, owner) = fixture();
    world.contacts_enabled = Some(true);
    world.interception_active = Some(true);
    let cursor = |index| PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    };
    let catalog = PathCatalog::new(vec![vec![
        Statement::PublishInterceptionMusicReady {
            ready: true,
            next: cursor(1),
        },
        Statement::Control(ControlCommand::WaitOne { next: cursor(2) }),
    ]])
    .unwrap();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
    runtime.enter(&objects, owner).unwrap();
    let before = objects.clone();
    let before_runtime = runtime.clone();
    let mut random = RandomState::default();
    assert_eq!(
        runtime.resume_program(
            &catalog,
            &mut objects,
            owner,
            &mut PathWorld::unbound(&mut random, 0),
            1
        ),
        Err(ProgramError::MissingInterceptionMusicReadiness)
    );
    assert_eq!(objects, before);
    assert_eq!(runtime, before_runtime);
    assert_eq!(world.interception_music_ready, None);
    let mut inputs = world
        .path_world(&objects, owner, PlayerTarget::Primary)
        .unwrap();
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
        Err(ProgramError::BudgetExceeded {
            cursor: cursor(0),
            executed: 0
        })
    );
    let exit = runtime
        .resume_program(&catalog, &mut objects, owner, &mut inputs, 2)
        .unwrap();
    assert_eq!(exit.actor, owner);
    assert_eq!(exit.step, crate::path_commands::ControlStep::Movement);
    assert_eq!(world.interception_music_ready, Some(true));
    prepare(&mut objects, &mut world, &runtime.resources, owner).unwrap();
    assert_eq!(
        world.audio.pending_music_control(),
        Some(MusicControlRequest::InterceptionReady)
    );
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .mission
            .unwrap()
            .flags,
        0x20
    );
}
