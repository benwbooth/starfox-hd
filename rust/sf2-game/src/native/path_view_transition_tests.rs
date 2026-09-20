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
