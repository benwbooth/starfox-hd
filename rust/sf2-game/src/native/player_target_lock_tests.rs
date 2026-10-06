use super::*;
use crate::path_commands::ControlCommand;
use crate::path_invocation::{InvocationEntry, PathInvocation};
use crate::path_program::{PathCatalog, SelectedAuxiliaryState, Statement};
use crate::path_scene_state::EncounterObjectiveCounts;
use crate::path_target::TargetingUpgradeState;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_transition::ViewTransitionMode;
use crate::{Behavior, Object, ObjectKind, PathCursor, PathId, RandomState, ShapeId};

fn actor(objects: &mut ObjectStore) -> ObjectId {
    objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap()
}

fn setup() -> (ObjectStore, ScenePathWorld, ObjectId, ObjectId, ObjectId) {
    let mut objects = ObjectStore::new();
    let primary = actor(&mut objects);
    let secondary = actor(&mut objects);
    let target = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(primary);
    world.secondary_player = Some(secondary);
    world.player_display_subject = Some(secondary);
    world.targeting_upgrade = Some(TargetingUpgradeState { pilot_flags: 0x80 });
    world.contacts_enabled = Some(false); // stale: the live low byte owns the gate
    world.objective_counts = Some(EncounterObjectiveCounts {
        remaining_word: 0xAB01,
        ..Default::default()
    });
    world.view_transition_mode = Some(ViewTransitionMode::default());
    world.target_reticle = TargetReticle {
        horizontal: Some(128),
        vertical: Some(128),
    };
    world.published_homing_target = Some(PublishedHomingTarget {
        object: Some(secondary),
    });
    let records = PlayerPathRecords {
        target_selection: Some(TargetSelection {
            candidate: Some(target),
            screen: [104, 104],
            display_status: 213,
            control_flags: 0xD7,
            ..Default::default()
        }),
        target_lock: Some(TargetLock {
            previous_candidate: Some(target),
            marker_style: 171,
            ..Default::default()
        }),
        auxiliary: Some(SelectedAuxiliaryState {
            mode: 0,
            action_flags: 1,
            stored_world_position: Default::default(),
            stored_rotation: Default::default(),
        }),
        ..Default::default()
    };
    world.bind_player(&objects, primary, records).unwrap();
    world.bind_player(&objects, secondary, records).unwrap();
    (objects, world, primary, secondary, target)
}

fn events(world: &mut ScenePathWorld) -> Vec<SoundEvent> {
    world.audio.take_events().into_iter().flatten().collect()
}

#[test]
fn every_byte_coordinate_preserves_wrapped_half_open_window() {
    for reticle in 0..=u8::MAX {
        for candidate in 0..=u8::MAX {
            assert_eq!(
                inside_window(reticle, candidate),
                (0..64).any(|offset| candidate.wrapping_sub(8).wrapping_add(offset) == reticle),
                "{reticle} {candidate}"
            );
        }
    }
}

#[test]
fn reticle_tracking_keeps_asymmetric_clamp_and_wrap_boundaries() {
    // At the projected point plus 24 there is no easing: inspect the exact
    // selected goal, including the source's signed-subtraction overflow.
    for (projected, goal) in [
        ([15, 31], [40, 40]),
        ([16, 32], [40, 56]),
        ([17, 33], [41, 57]),
        ([208, 176], [232, 200]),
        ([209, 177], [232, 200]),
        ([-32768, -32768], [232, 200]),
        ([-32752, -32736], [40, 40]),
    ] {
        let mut reticle = TargetReticle {
            horizontal: Some(goal[0]),
            vertical: Some(goal[1]),
        };
        reticle.track_projected(projected).unwrap();
        assert_eq!(reticle.horizontal, Some(goal[0]));
        assert_eq!(reticle.vertical, Some(goal[1]));
    }
}

#[test]
fn reticle_easing_truncates_negative_odd_steps_and_updates_axes_in_order() {
    for (previous, expected) in [
        (127, 128),
        (129, 128),
        (131, 130),
        (125, 126),
        (0, 192),
        (255, 192),
    ] {
        let mut reticle = TargetReticle {
            horizontal: Some(previous),
            vertical: None,
        };
        assert_eq!(
            reticle.track_projected([104, 104]),
            Err(ReticlePositionError::MissingVertical)
        );
        assert_eq!(reticle.horizontal, Some(expected));
        assert_eq!(reticle.vertical, None);
    }
    let mut reticle = TargetReticle {
        horizontal: None,
        vertical: Some(71),
    };
    assert_eq!(
        reticle.track_projected([104, 104]),
        Err(ReticlePositionError::MissingHorizontal)
    );
    assert_eq!(reticle.vertical, Some(71));
}

#[test]
fn acquisition_updates_only_primary_and_repeated_lock_does_not_repeat_sound() {
    let (objects, mut world, primary, secondary, target) = setup();
    let other_before = *world.player(&objects, secondary).unwrap();
    for visit in 0..3 {
        update(&objects, &mut world).unwrap();
        let records = world.player(&objects, primary).unwrap();
        assert_eq!(records.target_selection.unwrap().forced_owner, Some(target));
        assert_eq!(records.target_selection.unwrap().display_status, 213);
        assert_eq!(records.target_selection.unwrap().control_flags, 0xC7);
        assert_eq!(
            records.target_lock.unwrap(),
            TargetLock {
                previous_candidate: Some(target),
                acquisition_clock: 0,
                grace_remaining: 10,
                marker_style: 160,
            }
        );
        assert_eq!(world.published_homing_target.unwrap().object, Some(target));
        assert_eq!(
            events(&mut world),
            if visit == 0 {
                vec![SoundEvent::Authored(AuthoredCue::new(
                    59,
                    0,
                    PlayerTarget::Primary,
                ))]
            } else {
                vec![]
            }
        );
        assert_eq!(*world.player(&objects, secondary).unwrap(), other_before);
    }
}

#[test]
fn nonzero_acquisition_clock_wraps_before_next_visit_acquires() {
    for clock in 1..=u8::MAX {
        let (objects, mut world, primary, _, target) = setup();
        lock_mut(&objects, &mut world, primary)
            .unwrap()
            .acquisition_clock = clock;
        update(&objects, &mut world).unwrap();
        let records = world.player(&objects, primary).unwrap();
        assert_eq!(
            records.target_lock.unwrap().acquisition_clock,
            clock.wrapping_add(1)
        );
        assert_eq!(records.target_selection.unwrap().forced_owner, None);
        assert_eq!(world.published_homing_target.unwrap().object, None);
        assert!(events(&mut world).is_empty());
        if clock == 255 {
            update(&objects, &mut world).unwrap();
            assert_eq!(world.published_homing_target.unwrap().object, Some(target));
            assert_eq!(events(&mut world).len(), 1);
        }
    }
}

#[test]
fn outside_window_publishes_retained_owner_not_fresh_candidate_and_grace_expires_later() {
    for grace in 0..=u8::MAX {
        let (objects, mut world, primary, secondary, target) = setup();
        let records = world.player_mut(&objects, primary).unwrap();
        records.target_selection.as_mut().unwrap().forced_owner = Some(secondary);
        let lock = records.target_lock.as_mut().unwrap();
        lock.grace_remaining = grace;
        lock.acquisition_clock = 193;
        world.target_reticle.horizontal = Some(0);
        world.target_reticle.vertical = None; // not read after horizontal rejection
        update(&objects, &mut world).unwrap();
        let records = world.player(&objects, primary).unwrap();
        let lock = records.target_lock.unwrap();
        if grace == 0 {
            assert_eq!(lock.previous_candidate, Some(target));
            assert_eq!(lock.acquisition_clock, 0);
            assert_eq!(records.target_selection.unwrap().forced_owner, None);
            assert_eq!(world.published_homing_target.unwrap().object, None);
        } else {
            assert_eq!(lock.previous_candidate, Some(secondary));
            assert_eq!(lock.grace_remaining, grace - 1);
            assert_eq!(lock.acquisition_clock, 193);
            assert_eq!(
                world.published_homing_target.unwrap().object,
                Some(secondary)
            );
        }
        assert!(events(&mut world).is_empty());
    }
    let (objects, mut world, primary, _, target) = setup();
    update(&objects, &mut world).unwrap();
    world.target_reticle.horizontal = Some(0);
    for remaining in (0..10).rev() {
        update(&objects, &mut world).unwrap();
        assert_eq!(world.published_homing_target.unwrap().object, Some(target));
        assert_eq!(
            world
                .player(&objects, primary)
                .unwrap()
                .target_lock
                .unwrap()
                .grace_remaining,
            remaining
        );
    }
    update(&objects, &mut world).unwrap();
    assert_eq!(world.published_homing_target.unwrap().object, None);
}

#[test]
fn cancellations_preserve_shared_publication_except_explicit_clear_routes() {
    for gate in 0..8 {
        for published in [false, true] {
            let (objects, mut world, primary, secondary, _) = setup();
            if !published {
                world.published_homing_target = None;
            }
            let prior = world.published_homing_target;
            let records = world.player_mut(&objects, primary).unwrap();
            records.target_selection.as_mut().unwrap().forced_owner = Some(secondary);
            records.target_lock.as_mut().unwrap().acquisition_clock = 71;
            records.target_lock.as_mut().unwrap().grace_remaining = 93;
            match gate {
                0 => world.targeting_upgrade = Some(TargetingUpgradeState { pilot_flags: 0x7F }),
                1 => records.target_selection.as_mut().unwrap().candidate = None,
                2 => world.objective_counts.as_mut().unwrap().remaining_word = 0xFF00,
                3 => world
                    .view_transition_mode
                    .as_mut()
                    .unwrap()
                    .set_active(true),
                4 => records.auxiliary.as_mut().unwrap().action_flags = 0xFE,
                5 => records.target_selection.as_mut().unwrap().control_flags |= 8,
                6 => records.target_lock.as_mut().unwrap().previous_candidate = Some(secondary),
                7 => records.target_selection.as_mut().unwrap().control_flags |= 32,
                _ => unreachable!(),
            }
            world.target_reticle = TargetReticle::default(); // every case skips both axes
            update(&objects, &mut world).unwrap();
            let records = world.player(&objects, primary).unwrap();
            assert_eq!(
                records.target_lock.unwrap(),
                TargetLock {
                    previous_candidate: records.target_selection.unwrap().candidate,
                    marker_style: 171,
                    ..Default::default()
                }
            );
            assert_eq!(records.target_selection.unwrap().forced_owner, None);
            assert_eq!(records.target_selection.unwrap().control_flags & 0x18, 0);
            assert_eq!(
                world.published_homing_target,
                if [0, 5, 7].contains(&gate) {
                    Some(PublishedHomingTarget::default())
                } else {
                    prior
                }
            );
            assert!(events(&mut world).is_empty());
        }
    }
}

#[test]
fn missing_inputs_fail_lazily_and_keep_exact_partial_writes() {
    for missing in 0..9 {
        let (objects, mut world, primary, _, _) = setup();
        let initial = *world.player(&objects, primary).unwrap();
        let publication = world.published_homing_target;
        let error = match missing {
            0 => {
                world.targeting_upgrade = None;
                TargetLockError::MissingUpgrade
            }
            1 => {
                world
                    .player_mut(&objects, primary)
                    .unwrap()
                    .target_selection = None;
                WorldInputError::MissingTargetSelection(primary).into()
            }
            2 => {
                world.objective_counts = None;
                world.contacts_enabled = None;
                WorldInputError::MissingContactEnable.into()
            }
            3 => {
                world.view_transition_mode = None;
                TargetLockError::MissingScriptedViewMode
            }
            4 => {
                world.player_mut(&objects, primary).unwrap().auxiliary = None;
                WorldInputError::MissingAuxiliary(primary).into()
            }
            5 => {
                world.player_mut(&objects, primary).unwrap().target_lock = None;
                TargetLockError::MissingLock(primary)
            }
            6 => {
                world.target_reticle.horizontal = None;
                TargetLockError::MissingHorizontalReticle
            }
            7 => {
                world.target_reticle.vertical = None;
                TargetLockError::MissingVerticalReticle
            }
            8 => {
                world.targeting_upgrade.as_mut().unwrap().pilot_flags = 0;
                world.player_mut(&objects, primary).unwrap().target_lock = None;
                TargetLockError::MissingLock(primary)
            }
            _ => unreachable!(),
        };
        let mut expected = *world.player(&objects, primary).unwrap();
        if [6, 7].contains(&missing) {
            expected.target_lock.as_mut().unwrap().previous_candidate = None;
        }
        assert_eq!(update(&objects, &mut world), Err(error));
        assert_eq!(*world.player(&objects, primary).unwrap(), expected);
        assert_eq!(
            world.published_homing_target,
            if missing >= 6 {
                Some(PublishedHomingTarget::default())
            } else {
                publication
            }
        );
        assert_eq!(
            world.player(&objects, primary).unwrap().target_selection,
            if missing == 1 {
                None
            } else {
                initial.target_selection
            }
        );
        assert!(events(&mut world).is_empty());
    }
}

#[test]
fn retained_object_identity_is_not_liveness_filtered_but_player_binding_is() {
    let (mut objects, mut world, primary, _, target) = setup();
    objects.remove(target).unwrap();
    update(&objects, &mut world).unwrap();
    assert_eq!(world.published_homing_target.unwrap().object, Some(target));
    objects.remove(primary).unwrap();
    let replacement = actor(&mut objects);
    assert_eq!(replacement, primary);
    assert_eq!(
        update(&objects, &mut world),
        Err(WorldInputError::StalePlayerRecord(primary).into())
    );
}

#[test]
fn storage_release_invalidates_lock_and_replacement_clears_it_without_clearing_publication() {
    use crate::path_runtime::PathRuntime;
    use crate::player_storage::{self, PlayerStorageInputs};
    let (mut objects, mut world, primary, _, target) = setup();
    let mut runtime = PathRuntime::default();
    let inputs = PlayerStorageInputs {
        pilot_code: 1,
        reserve_shield: 31,
        score: Default::default(),
    };
    let records = *world.player(&objects, primary).unwrap();
    player_storage::replace(&mut objects, &mut world, &mut runtime, primary, inputs).unwrap();
    world.bind_player(&objects, primary, records).unwrap();
    update(&objects, &mut world).unwrap();
    let lifetime = objects.lifetime_id(primary);
    runtime
        .release_actor_programs(&mut objects, primary)
        .unwrap();
    assert_eq!(objects.lifetime_id(primary), lifetime);
    assert_eq!(
        update(&objects, &mut world),
        Err(WorldInputError::StalePlayerRecord(primary).into())
    );
    assert_eq!(world.published_homing_target.unwrap().object, Some(target));
    player_storage::replace(&mut objects, &mut world, &mut runtime, primary, inputs).unwrap();
    assert_eq!(
        world.player(&objects, primary).unwrap().target_lock,
        Some(TargetLock::default())
    );
    assert_eq!(
        world.player(&objects, primary).unwrap().target_selection,
        Some(TargetSelection::default())
    );
    assert_eq!(world.published_homing_target.unwrap().object, Some(target));
}

#[test]
fn acquisition_uses_shared_sound_ring_even_when_queue_wrap_hides_the_cue() {
    for pending in 0..=32 {
        let (objects, mut world, _, _, target) = setup();
        for _ in 0..pending {
            world.audio.queue(SoundEvent::RapidLaser);
        }
        update(&objects, &mut world).unwrap();
        assert_eq!(world.published_homing_target.unwrap().object, Some(target));
        let events = events(&mut world);
        let exposed = (pending + 1) % crate::SOUND_EVENT_CAPACITY;
        assert_eq!(events.len(), exposed);
        if exposed != 0 {
            assert_eq!(
                events.last(),
                Some(&SoundEvent::Authored(AuthoredCue::new(
                    59,
                    0,
                    PlayerTarget::Primary
                )))
            );
            assert!(events[..exposed - 1]
                .iter()
                .all(|event| *event == SoundEvent::RapidLaser));
        }
    }
}

#[test]
fn projectile_command_copies_the_retained_publication_and_keeps_its_snapshot() {
    let (mut objects, mut world, primary, secondary, target) = setup();
    let projectile = actor(&mut objects);
    world
        .player_mut(&objects, primary)
        .unwrap()
        .target_selection
        .as_mut()
        .unwrap()
        .forced_owner = Some(secondary);
    lock_mut(&objects, &mut world, primary)
        .unwrap()
        .grace_remaining = 1;
    world.target_reticle.horizontal = Some(0);
    update(&objects, &mut world).unwrap();
    let at = |index| PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    };
    let catalog = PathCatalog::new(vec![vec![
        Statement::AttachPublishedHomingTarget { next: at(1) },
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    objects.get_mut(projectile).unwrap().base.path = Some(at(0));
    let mut paths = PathInvocation::default();
    paths.begin(projectile, InvocationEntry::Program).unwrap();
    assert_eq!(
        paths.resume(&catalog, &mut objects, &mut world, 10),
        Ok(projectile)
    );
    assert_eq!(
        objects.get(projectile).unwrap().base.attachment,
        Some(secondary)
    );
    assert_ne!(Some(target), world.published_homing_target.unwrap().object);
    world.targeting_upgrade.as_mut().unwrap().pilot_flags = 0;
    update(&objects, &mut world).unwrap();
    assert_eq!(world.published_homing_target.unwrap().object, None);
    assert_eq!(
        objects.get(projectile).unwrap().base.attachment,
        Some(secondary)
    );
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("display retention must not invoke a strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("display retention must not invoke death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("display retention must not resume a map")
    }
}

#[test]
fn scene_wrapper_latches_partial_error_and_cannot_replay_cancel_after_input_repair() {
    let (mut objects, mut world, primary, _, target) = setup();
    let mut execution = SceneExecution::default();
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut callbacks = Callbacks;
    world.target_reticle.horizontal = None;
    let mut scene = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 10,
    };
    assert_eq!(
        scene.retain_primary_target(),
        Err(SceneError::TargetLock(
            TargetLockError::MissingHorizontalReticle
        ))
    );
    assert_eq!(
        scene
            .world
            .player(scene.objects, primary)
            .unwrap()
            .target_lock
            .unwrap()
            .previous_candidate,
        None
    );
    scene.world.target_reticle.horizontal = Some(128);
    scene.world.published_homing_target = Some(PublishedHomingTarget {
        object: Some(target),
    });
    for _ in 0..3 {
        assert_eq!(scene.retain_primary_target(), Err(SceneError::Faulted));
        assert_eq!(
            scene.world.published_homing_target.unwrap().object,
            Some(target)
        );
        assert_eq!(
            scene
                .world
                .player(scene.objects, primary)
                .unwrap()
                .target_lock
                .unwrap()
                .previous_candidate,
            None
        );
    }
}

#[test]
fn scene_reticle_position_error_latches_before_lock_retention_or_repeated_easing() {
    let (mut objects, mut world, _, _, target) = setup();
    let mut execution = SceneExecution::default();
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut callbacks = Callbacks;
    world.target_reticle = TargetReticle {
        horizontal: Some(0),
        vertical: None,
    };
    let mut scene = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 10,
    };
    assert_eq!(
        scene.track_target_reticle([104, 104]),
        Err(SceneError::ReticlePosition(
            ReticlePositionError::MissingVertical
        ))
    );
    assert_eq!(scene.world.target_reticle.horizontal, Some(192));
    scene.world.target_reticle.vertical = Some(0);
    scene.world.published_homing_target = Some(PublishedHomingTarget {
        object: Some(target),
    });
    assert_eq!(
        scene.track_target_reticle([104, 104]),
        Err(SceneError::Faulted)
    );
    assert_eq!(scene.retain_primary_target(), Err(SceneError::Faulted));
    assert_eq!(scene.world.target_reticle.horizontal, Some(192));
    assert_eq!(
        scene.world.published_homing_target.unwrap().object,
        Some(target)
    );
}

#[test]
fn individual_projection_and_reticle_easing_feed_the_next_retention_visit() {
    use crate::intro_projection::{project_individual_point, ProjectionViewport};
    let (objects, mut world, primary, _, target) = setup();
    world.target_reticle = TargetReticle {
        horizontal: Some(0),
        vertical: Some(0),
    };
    world
        .player_mut(&objects, primary)
        .unwrap()
        .target_selection
        .as_mut()
        .unwrap()
        .screen = [112, 96];
    let point = project_individual_point(
        [0, 0, 1024],
        ProjectionViewport {
            center: [112, 96],
            left: 0,
            right: 224,
            top: 0,
            bottom: 192,
        },
    );
    for (step, coordinates) in [[196, 60], [166, 90], [151, 105]].into_iter().enumerate() {
        world
            .target_reticle
            .track_projected([point.x, point.y])
            .unwrap();
        assert_eq!(
            world.target_reticle,
            TargetReticle {
                horizontal: Some(coordinates[0]),
                vertical: Some(coordinates[1])
            }
        );
        update(&objects, &mut world).unwrap();
        assert_eq!(
            world.published_homing_target.unwrap().object,
            (step != 0).then_some(target)
        );
        assert_eq!(events(&mut world).len(), usize::from(step == 1));
    }
}
