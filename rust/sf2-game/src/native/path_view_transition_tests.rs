use super::super::actor_auxiliary::{AuxiliaryError, AuxiliaryRecord};
use super::super::collision_pass::ExclusionGroups;
use super::super::path_control::PlayerTarget;
use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
use super::super::program_resources::{AllocationFailure, PROGRAM_CAPACITY};
use super::super::program_state::ProgramData;
use super::super::view_transition::{ViewBaseSnapshot, ViewSaveError, ViewTransitionMode};
use super::super::{
    AudioState, Behavior, ObjectKind, ObjectSpawnDefaults, PathId, ShapeId, SoundEvent, Vector3,
};
use super::tests::{setup, world};
use super::*;

#[test]
fn selected_stored_pose_is_live_auxiliary_data_not_actor_or_published_motion() {
    for alias in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let selected = if alias { owner } else { actor(&mut objects) };
        let primary = actor(&mut objects);
        objects.get_mut(selected).unwrap().base.position = Vector3 { x: 1, y: 2, z: 3 };
        let mut auxiliary = SelectedAuxiliaryState {
            mode: 137,
            action_flags: 79,
            stored_world_position: Vector3 {
                x: -32768,
                y: -123,
                z: 32767,
            },
        };
        let catalog = PathCatalog::new(vec![vec![
            Statement::CopySelectedStoredPosition { next: at(1) },
            Statement::CopySelectedStoredPosition { next: at(2) },
        ]])
        .unwrap();
        let mut expected = objects.clone();
        let expected_actor = expected.get_mut(owner).unwrap();
        expected_actor.base.position = auxiliary.stored_world_position;
        expected_actor.base.path = Some(at(1));
        let random_before = random;
        runtime.branch.invert_next = true;
        let resources_before = runtime.resources.clone();
        let auxiliary_before = auxiliary;
        let mut inputs = world(&mut random);
        inputs.primary_player = Some(primary);
        inputs.selected = Some(selected);
        inputs.selected_auxiliary = Some(&mut auxiliary);
        inputs.published_motion = Some(super::super::path_motion::PublishedPlayerMotion::capture(
            objects.get(primary).unwrap(),
            Vector3::default(),
            0,
        ));
        expect_advance(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            at(1),
            1,
        );
        assert_eq!(objects, expected);
        assert_eq!(
            *inputs.selected_auxiliary.as_deref().unwrap(),
            auxiliary_before
        );
        let newer_position = Vector3 {
            x: 234,
            y: 32767,
            z: -32768,
        };
        inputs
            .selected_auxiliary
            .as_deref_mut()
            .unwrap()
            .stored_world_position = newer_position;
        expect_advance(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            at(2),
            1,
        );
        expected.get_mut(owner).unwrap().base.position = newer_position;
        expected.get_mut(owner).unwrap().base.path = Some(at(2));
        assert_eq!(objects, expected);
        assert!(runtime.branch.invert_next);
        assert_eq!(runtime.resources, resources_before);
        assert_eq!(random, random_before);
    }
}

#[test]
fn selected_stored_pose_requires_auxiliary_even_when_selected_actor_is_present() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    let before = objects.clone();
    let catalog = PathCatalog::new(vec![vec![Statement::CopySelectedStoredPosition {
        next: at(1),
    }]])
    .unwrap();
    let mut inputs = world(&mut random);
    inputs.selected = Some(owner);
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::MissingSelectedAuxiliary)
    );
    assert_eq!(objects, before);
}

#[test]
fn fixed_view_motion_preserves_aliases_targets_identity_and_double_depth_chase() {
    use super::super::Angle;
    // Independent scalar model: wrapping subtraction, signed truncation, min step.
    let chase = |current: u16, target: u16| {
        let difference = i32::from(target.wrapping_sub(current) as i16);
        let step = difference.signum() * (difference.abs() / 8).max(1);
        current.wrapping_add(step as u16)
    };
    for snap in [false, true] {
        for same_actor in [false, true] {
            for seed in [0, 1, 7, 8, 9, 127, 128, 255, 256, 32767, 32768, 65535u16] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let view = if same_actor {
                    owner
                } else {
                    actor(&mut objects)
                };
                let other = actor(&mut objects);
                let value = objects.get_mut(owner).unwrap();
                value.base.position = Vector3 {
                    x: -32768,
                    y: 32767,
                    z: 1000,
                };
                value.base.pitch = Angle::from_units(seed as u8);
                value.base.yaw = Angle::from_units((seed as u8).wrapping_add(91));
                value.base.roll = Angle::from_units((seed as u8).wrapping_add(203));
                value.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
                let value = objects.get_mut(view).unwrap();
                if !same_actor {
                    value.base.position = Vector3 {
                        x: 32767,
                        y: -32768,
                        z: -1000,
                    };
                    value.base.pitch = Angle::from_units(17);
                    value.base.yaw = Angle::from_units(33);
                    value.base.roll = Angle::from_units(129);
                }
                value.base.child_number = (seed >> 8) as u8;
                value.extension.path_state.repeat_counter = seed as u8;
                value.base.wait_timer = (seed as u8).wrapping_add(7);
                value.base.view_rear_distance = seed as i16;
                value.base.first_child = Some(other);
                value.base.next_sibling = Some(owner);
                let source = objects.get(owner).unwrap().clone();
                let before = objects.get(view).unwrap().clone();
                let mut expected = objects.clone();
                let target = expected.get_mut(view).unwrap();
                let approach = |a: u16, b: u16| if snap { b } else { chase(a, b) };
                target.base.position.x =
                    approach(before.base.position.x as u16, source.base.position.x as u16) as i16;
                target.base.position.y =
                    approach(before.base.position.y as u16, source.base.position.y as u16) as i16;
                target.base.position.z = approach(
                    approach(before.base.position.z as u16, source.base.position.z as u16),
                    source.base.position.z as u16,
                ) as i16;
                target.base.view_rear_distance = approach(seed, 0) as i16;
                let angles = [source.base.pitch, source.base.yaw, source.base.roll]
                    .map(|angle| (u16::from(angle.units()) * 256).wrapping_neg());
                let pitch = approach(
                    u16::from(before.base.pitch.units())
                        + u16::from(before.base.child_number) * 256,
                    angles[0],
                );
                let yaw = approach(
                    u16::from(before.base.yaw.units())
                        + u16::from(before.extension.path_state.repeat_counter) * 256,
                    angles[1],
                );
                let roll = approach(
                    u16::from(before.base.roll.units()) + u16::from(before.base.wait_timer) * 256,
                    angles[2],
                );
                target.base.pitch = Angle::from_units(pitch as u8);
                target.base.child_number = (pitch >> 8) as u8;
                target.base.yaw = Angle::from_units(yaw as u8);
                target.extension.path_state.repeat_counter = (yaw >> 8) as u8;
                target.base.roll = Angle::from_units(roll as u8);
                target.base.wait_timer = (roll >> 8) as u8;
                expected.get_mut(owner).unwrap().base.path = Some(at(1));
                let catalog =
                    PathCatalog::new(vec![vec![Statement::MoveFixedView { snap, next: at(1) }]])
                        .unwrap();
                let random_before = random;
                runtime.branch.invert_next = true;
                let resources_before = runtime.resources.clone();
                let mut inputs = world(&mut random);
                inputs.fixed_players = [Some(view), Some(other)];
                inputs.primary_player = Some(other);
                inputs.selected = Some(other);
                expect_advance(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    at(1),
                    1,
                );
                assert_eq!(
                    objects, expected,
                    "snap={snap}, alias={same_actor}, seed={seed}"
                );
                assert!(runtime.branch.invert_next);
                assert_eq!(runtime.resources, resources_before);
                assert_eq!(random, random_before);
            }
        }
    }
}

#[test]
fn missing_fixed_view_does_not_partially_move_owner_or_advance_path() {
    for snap in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let before = objects.clone();
        let catalog =
            PathCatalog::new(vec![vec![Statement::MoveFixedView { snap, next: at(1) }]]).unwrap();
        let mut inputs = world(&mut random);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::MissingFixedView)
        );
        assert_eq!(objects, before);
    }
}

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

fn audio(events: &mut AudioState) -> PathAudio<'_> {
    PathAudio {
        events,
        listeners: [CueListener::Other; 2],
        markers: None,
    }
}

fn actor(objects: &mut ObjectStore) -> ObjectId {
    objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap()
}

fn expect_advance(result: Result<ProgramExit, ProgramError>, cursor: PathCursor, executed: usize) {
    assert_eq!(
        result,
        Err(ProgramError::BudgetExceeded { cursor, executed })
    );
}

#[test]
fn paired_transition_saves_fixed_view_after_cleanup_and_restores_only_base_with_primary_cues() {
    let catalog = PathCatalog::new(vec![vec![
        Statement::ViewTransition {
            enabled: true,
            next: at(1),
        },
        Statement::ViewTransition {
            enabled: false,
            next: at(2),
        },
    ]])
    .unwrap();
    for invert in [false, true] {
        for selected in [PlayerTarget::Primary, PlayerTarget::Secondary] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let view = actor(&mut objects);
            let primary = actor(&mut objects);
            let secondary = actor(&mut objects);
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .conditions
                .selected_player = selected;
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .auxiliary
                .set(
                    &mut runtime.resources,
                    owner,
                    AuxiliaryRecord::SceneContinuation(at(79)),
                )
                .unwrap();
            let value = objects.get_mut(view).unwrap();
            value.base.position = Vector3 {
                x: -32768,
                y: 123,
                z: 32767,
            };
            value.base.hit_points = 157;
            value.base.contacts.exclusion_groups = ExclusionGroups::HIT_SIDE_CLASS;
            value.extension.path_state.script_parameter = 137;
            value.extension.path_state.motion_delta = Vector3 { x: 1, y: 2, z: 3 };
            value
                .extension
                .auxiliary
                .set(
                    &mut runtime.resources,
                    view,
                    AuxiliaryRecord::OrdinaryImpactMaterial(27),
                )
                .unwrap();
            let primary_before = objects.get(primary).unwrap().clone();
            let secondary_before = objects.get(secondary).unwrap().clone();
            runtime.branch.invert_next = invert;
            let mut mode = ViewTransitionMode { flags: 0xA5A5 };
            let mut events = AudioState::default();
            let random_before = random;
            let mut inputs = world(&mut random);
            inputs.view_transition_mode = Some(&mut mode);
            inputs.audio = Some(audio(&mut events));
            inputs.fixed_players = [Some(view), Some(secondary)];
            inputs.primary_player = Some(primary);
            inputs.selected = Some(secondary);
            expect_advance(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                at(1),
                1,
            );
            assert_eq!(inputs.view_transition_mode.as_ref().unwrap().flags, 0xA5A7);
            let saved = objects.get(view).unwrap().clone();
            assert_eq!(saved.base.hit_points, 0);
            assert!(saved.base.flags.collision_disabled);
            assert!(saved.base.contacts.run_when_paused);
            assert!(objects.get(owner).unwrap().base.contacts.run_when_paused);
            assert_eq!(runtime.resources.owner_count(owner), 2);
            assert_eq!(runtime.resources.owner_count(view), 1);
            let value = objects.get_mut(view).unwrap();
            value.base.position = Vector3 {
                x: 91,
                y: 92,
                z: 93,
            };
            value.base.hit_points = 89;
            value.extension.path_state.script_parameter = 231;
            value.extension.path_state.motion_delta.x = 1729;
            value
                .extension
                .auxiliary
                .set(
                    &mut runtime.resources,
                    view,
                    AuxiliaryRecord::OrdinaryImpactMaterial(77),
                )
                .unwrap();
            expect_advance(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                at(2),
                1,
            );
            let mut expected = saved;
            expected.extension.path_state.motion_delta.x = 1729;
            assert_eq!(objects.get(view).unwrap(), &expected);
            assert_eq!(
                objects
                    .get(view)
                    .unwrap()
                    .extension
                    .auxiliary
                    .impact_materials(&runtime.resources, view)
                    .unwrap()
                    .ordinary,
                Some(77)
            );
            assert_eq!(inputs.view_transition_mode.as_ref().unwrap().flags, 0xA5A5);
            assert!(!objects.get(owner).unwrap().base.contacts.run_when_paused);
            assert_eq!(runtime.resources.owner_count(owner), 1);
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .auxiliary
                    .scene_continuation(&runtime.resources, owner),
                Ok(Some(at(79)))
            );
            assert_eq!(objects.get(primary).unwrap(), &primary_before);
            assert_eq!(objects.get(secondary).unwrap(), &secondary_before);
            assert_eq!(runtime.branch.invert_next, invert);
            assert_eq!(*inputs.random, random_before);
            assert_eq!(
                inputs
                    .audio
                    .as_mut()
                    .unwrap()
                    .events
                    .take_events()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>(),
                vec![
                    SoundEvent::Authored(AuthoredCue::new(248, 0, PlayerTarget::Primary)),
                    SoundEvent::Authored(AuthoredCue::new(247, 0, PlayerTarget::Primary)),
                ]
            );
        }
    }
}

#[test]
fn absent_save_needs_no_fixed_view_and_disable_does_not_cleanup_projectiles() {
    let catalog = PathCatalog::new(vec![vec![Statement::ViewTransition {
        enabled: false,
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let peer = actor(&mut objects);
    objects
        .get_mut(peer)
        .unwrap()
        .base
        .contacts
        .exclusion_groups = ExclusionGroups::from_authored_class(0x50);
    objects.get_mut(peer).unwrap().base.hit_points = 137;
    let before_peer = objects.get(peer).unwrap().clone();
    objects
        .get_mut(owner)
        .unwrap()
        .base
        .contacts
        .run_when_paused = true;
    let resources = runtime.resources.clone();
    let mut mode = ViewTransitionMode { flags: u16::MAX };
    let mut events = AudioState::default();
    let mut inputs = world(&mut random);
    inputs.view_transition_mode = Some(&mut mode);
    inputs.audio = Some(audio(&mut events));
    expect_advance(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        at(1),
        1,
    );
    assert_eq!(inputs.view_transition_mode.as_ref().unwrap().flags, 0xFFFD);
    assert_eq!(objects.get(peer).unwrap(), &before_peer);
    assert!(!objects.get(owner).unwrap().base.contacts.run_when_paused);
    assert_eq!(runtime.resources, resources);
    assert_eq!(
        inputs
            .audio
            .as_mut()
            .unwrap()
            .events
            .take_events()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
        vec![SoundEvent::Authored(AuthoredCue::new(
            247,
            0,
            PlayerTarget::Primary
        ))]
    );
}

#[test]
fn missing_mode_audio_or_fixed_view_faults_before_mutation() {
    let catalog = PathCatalog::new(vec![vec![Statement::ViewTransition {
        enabled: true,
        next: at(1),
    }]])
    .unwrap();
    for stage in 0..3 {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let before = objects.clone();
        let resources = runtime.resources.clone();
        let mut mode = ViewTransitionMode { flags: 0xA5A5 };
        let mut events = AudioState::default();
        let mut inputs = world(&mut random);
        if stage >= 1 {
            inputs.view_transition_mode = Some(&mut mode);
        }
        if stage >= 2 {
            inputs.audio = Some(audio(&mut events));
        }
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(match stage {
                0 => ProgramError::MissingViewTransitionMode,
                1 => ProgramError::MissingAudio,
                _ => ProgramError::MissingFixedView,
            })
        );
        assert_eq!(objects, before);
        assert_eq!(runtime.resources, resources);
        assert_eq!(mode.flags, 0xA5A5);
        assert!(events
            .take_events()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .is_empty());
    }
}

#[test]
fn owner_view_alias_restores_pause_flag_and_advances_saved_command_not_current_restore() {
    let catalog = PathCatalog::new(vec![vec![
        Statement::ViewTransition {
            enabled: true,
            next: at(2),
        },
        Statement::ViewTransition {
            enabled: false,
            next: at(3),
        },
    ]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.hit_points = 157;
    let mut mode = ViewTransitionMode::default();
    let mut events = AudioState::default();
    let mut inputs = world(&mut random);
    inputs.view_transition_mode = Some(&mut mode);
    inputs.audio = Some(audio(&mut events));
    inputs.fixed_players[0] = Some(owner);
    expect_advance(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        at(2),
        1,
    );
    objects.get_mut(owner).unwrap().base.path = Some(at(1));
    objects.get_mut(owner).unwrap().base.hit_points = 3;
    expect_advance(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        at(2),
        1,
    );
    assert_eq!(objects.get(owner).unwrap().base.hit_points, 157);
    assert!(objects.get(owner).unwrap().base.contacts.run_when_paused);
    assert!(!inputs.view_transition_mode.as_ref().unwrap().active());
    assert_eq!(
        runtime.resources.available_capacity(),
        PROGRAM_CAPACITY - 10
    );
}

#[test]
fn allocation_fault_keeps_mode_cleanup_and_any_unpublished_owned_snapshot() {
    let catalog = PathCatalog::new(vec![vec![Statement::ViewTransition {
        enabled: true,
        next: at(1),
    }]])
    .unwrap();
    for remaining in [8, 76] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let view = actor(&mut objects);
        objects.get_mut(view).unwrap().base.hit_points = 127;
        objects
            .get_mut(view)
            .unwrap()
            .base
            .contacts
            .exclusion_groups = ExclusionGroups::from_authored_class(0x50);
        runtime
            .resources
            .allocate_shared(
                PROGRAM_CAPACITY - remaining - 2,
                ProgramData::PathStack(Default::default()),
            )
            .unwrap();
        let mut mode = ViewTransitionMode::default();
        let mut events = AudioState::default();
        let mut inputs = world(&mut random);
        inputs.view_transition_mode = Some(&mut mode);
        inputs.audio = Some(audio(&mut events));
        inputs.fixed_players[0] = Some(view);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::ViewSave(if remaining == 8 {
                ViewSaveError::Allocation(AllocationFailure::NoContiguousFit)
            } else {
                ViewSaveError::Auxiliary(AuxiliaryError::Allocation(
                    AllocationFailure::NoContiguousFit,
                ))
            }))
        );
        assert!(inputs.view_transition_mode.as_ref().unwrap().active());
        assert!(objects.get(owner).unwrap().base.contacts.run_when_paused);
        assert_eq!(objects.get(owner).unwrap().base.path, Some(at(0)));
        let expected_snapshot = ViewBaseSnapshot::capture(objects.get(view).unwrap());
        assert_eq!(objects.get(view).unwrap().base.hit_points, 0);
        assert!(objects.get(view).unwrap().base.flags.collision_disabled);
        assert!(inputs
            .audio
            .as_mut()
            .unwrap()
            .events
            .take_events()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .is_empty());
        let owned = runtime.resources.release_owner(owner);
        assert_eq!(
            owned,
            if remaining == 8 {
                vec![]
            } else {
                vec![ProgramData::SavedView(Box::new(expected_snapshot))]
            }
        );
    }
}

#[test]
fn spawns_in_one_invocation_observe_each_live_mode_change_not_entry_defaults() {
    let spawn = |next| Statement::SpawnIndependent {
        kind: ObjectKind::Effect,
        parameters: super::super::path_spawn::IndependentSpawn {
            shape: ShapeId::EMPTY,
            path: None,
            hit_points: 1,
            attack_power: 2,
        },
        next,
    };
    let catalog = PathCatalog::new(vec![vec![
        Statement::ViewTransition {
            enabled: true,
            next: at(1),
        },
        spawn(at(2)),
        Statement::ViewTransition {
            enabled: false,
            next: at(3),
        },
        spawn(at(4)),
    ]])
    .unwrap();
    for initial in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let view = actor(&mut objects);
        let mut mode = ViewTransitionMode::default();
        let mut events = AudioState::default();
        let mut inputs = world(&mut random);
        inputs.view_transition_mode = Some(&mut mode);
        inputs.audio = Some(audio(&mut events));
        inputs.fixed_players[0] = Some(view);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults {
            run_when_paused: initial,
            group: 179,
        });
        expect_advance(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2),
            at(2),
            2,
        );
        let first = runtime.spawns.last_spawn.unwrap();
        expect_advance(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2),
            at(4),
            2,
        );
        let second = runtime.spawns.last_spawn.unwrap();
        assert_ne!(first, second);
        assert!(objects.get(first).unwrap().base.contacts.run_when_paused);
        assert!(!objects.get(second).unwrap().base.contacts.run_when_paused);
        assert_eq!(inputs.spawn_defaults.unwrap().run_when_paused, initial);
    }
}
