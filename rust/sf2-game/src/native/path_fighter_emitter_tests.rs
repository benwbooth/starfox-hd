//! Source-lowered fighter controllers and their independently scheduled actors.
use super::super::path_scene_state::EncounterCoordination;
use super::super::{
    authored_paths, Angle, Behavior, ObjectKind, ObjectSpawnDefaults, PathId, ShapeId, Vector3,
};
use super::paired_patrol_tests::callbacks;
use super::tests::{setup, world};
use super::*;

const ROOTS: [PathCursor; 2] = [
    authored_paths::DISTANCE_GATED_FIGHTER_EMITTER,
    authored_paths::FIGHTER_EMITTER,
];

fn cursor(path: u16, command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(path),
        command_index,
    }
}

fn anchor(objects: &mut ObjectStore) -> ObjectId {
    let mut actor = Object::new(
        ObjectKind::Enemy,
        ShapeId::from_catalog_index(390),
        Behavior::FollowPath,
    );
    actor.base.hit_points = 100;
    objects.allocate(actor).unwrap()
}

#[test]
fn waiting_for_anchor_has_no_selection_allocation_or_random_dependency() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for parameter in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let original_random = random;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(root);
            actor.extension.path_state.script_parameter = parameter;
            let mut shared = EncounterCoordination {
                phase: 255,
                ..Default::default()
            };
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            for _ in 0..5 {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                assert_eq!(objects.len(), 1);
                assert_eq!(inputs.random, &original_random);
                assert_eq!(inputs.coordination.as_deref().unwrap().phase, 0);
                assert_eq!(
                    objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .script_parameter,
                    parameter.wrapping_add(u8::from(root == ROOTS[0]))
                );
            }
        }
    }
}

#[test]
fn spawn_gate_uses_full_wrapping_signed_comparison_and_distance_variant() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for count in 0..=u8::MAX {
            for distance in [0, 9999, 10000, 10001] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                objects.get_mut(owner).unwrap().base.path = Some(root);
                anchor(&mut objects);
                let mut player =
                    Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                player.base.position.z = distance;
                let player = objects.allocate(player).unwrap();
                let mut shared = EncounterCoordination {
                    handshake: count,
                    ..Default::default()
                };
                let mut inputs = world(&mut random);
                inputs.coordination = Some(&mut shared);
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                inputs.selected = Some(player);
                for _ in 0..2 {
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                }
                // The branch tests the sign of (limit - count), so equality
                // still spawns; values past the signed wrap also spawn.
                let room = 4_u8.wrapping_sub(count) & 128 == 0;
                assert_eq!(objects.len(), if room { 4 } else { 3 }, "count={count}");
                assert_eq!(
                    inputs.coordination.as_deref().unwrap().handshake,
                    count.wrapping_add(u8::from(room))
                );
                if room {
                    let fighter = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                    let rolling = root == ROOTS[0] && distance >= 10000;
                    assert_eq!(
                        fighter.base.shape,
                        ShapeId::from_catalog_index(if rolling { 137 } else { 172 })
                    );
                    assert_eq!(fighter.base.hit_points, if rolling { 100 } else { 1 });
                    assert_eq!(fighter.base.attack_power, 4);
                    assert_eq!(fighter.base.attachment, None);
                }
            }
        }
    }
}

#[test]
fn linked_position_callback_restores_borrowed_path_and_retains_last_publication_when_link_is_lost()
{
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(ROOTS[1]);
    let linked = anchor(&mut objects);
    let position = Vector3 {
        x: i16::MIN,
        y: i16::MAX,
        z: -1234,
    };
    let linked_path = Some(authored_paths::FADE_SPRITE);
    objects.get_mut(linked).unwrap().base.path = linked_path;
    let player = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap();
    let mut shared = EncounterCoordination {
        handshake: 5,
        ..Default::default()
    };
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut shared);
    inputs.selected = Some(player);
    for _ in 0..2 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
    }
    assert_eq!(objects.get(owner).unwrap().base.attachment, Some(linked));
    objects.get_mut(linked).unwrap().base.position = position;
    let mut borrowed = objects.get(linked).unwrap().clone();
    let path = objects.get(owner).unwrap().base.path;
    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
    assert_eq!(objects.get(owner).unwrap().base.position, position);
    assert_eq!(objects.get(owner).unwrap().base.path, path);
    // GOSUB executes on the borrowed actor, allocating its own empty stack
    // resource. Unbecome restores only its path, not its resource identity.
    borrowed.extension.path_state.stack = objects
        .get(linked)
        .unwrap()
        .extension
        .path_state
        .stack
        .clone();
    assert_eq!(objects.get(linked).unwrap(), &borrowed);
    assert_eq!(runtime.captured_world_position, Some(position));
    objects.get_mut(owner).unwrap().base.attachment = None;
    objects.get_mut(owner).unwrap().base.position = Vector3::default();
    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 2);
    assert_eq!(objects.get(owner).unwrap().base.position, position);
    assert_eq!(objects.get(linked).unwrap(), &borrowed);
}

fn spawned_fighter(
    rolling: bool,
    seed: u8,
) -> (PathRuntime, ObjectStore, ObjectId, ObjectId, RandomState) {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, _) = setup();
    let mut random = RandomState::new([seed, 31, 73, 171]);
    objects.get_mut(owner).unwrap().base.path = Some(if rolling { ROOTS[0] } else { ROOTS[1] });
    anchor(&mut objects);
    let mut player = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
    player.base.position.z = 11000;
    let player = objects.allocate(player).unwrap();
    let mut shared = EncounterCoordination::default();
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut shared);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    inputs.selected = Some(player);
    for _ in 0..2 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
    }
    assert_eq!(inputs.coordination.as_deref().unwrap().handshake, 1);
    let fighter = runtime.spawns.last_spawn.unwrap();
    (runtime, objects, fighter, player, random)
}

#[test]
fn every_phase_byte_selects_distinct_immediate_or_thirty_tick_abort_without_decrementing_count() {
    let catalog = authored_paths::catalog();
    for rolling in [false, true] {
        for phase in 0..=u8::MAX {
            let (mut runtime, mut objects, fighter, player, mut random) =
                spawned_fighter(rolling, phase);
            let mut shared = EncounterCoordination {
                handshake: 1,
                phase,
                ..Default::default()
            };
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.selected = Some(player);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, fighter, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(objects.get(fighter).unwrap().base.hit_points, 100);
            assert_eq!(
                objects.get(fighter).unwrap().base.speed,
                if rolling { 50 } else { 60 }
            );
            let random_after_init = *inputs.random;
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                fighter,
                &mut inputs,
                1,
            );
            let visits = if !rolling && phase != 0 { 31 } else { 1 };
            for visit in 1..=visits {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, fighter, &mut inputs, 100)
                        .unwrap()
                        .step,
                    if phase != 0 && visit == visits {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                assert_eq!(
                    objects.get(fighter).unwrap().base.flags.remove_after_tick,
                    phase != 0 && visit == visits
                );
            }
            let actor = objects.get(fighter).unwrap();
            assert_eq!(actor.base.flags.collision_disabled, !rolling && phase != 0);
            assert_eq!(actor.base.hit_points, 100);
            assert_eq!(inputs.random, &random_after_init);
            assert_eq!(inputs.coordination.as_deref().unwrap().handshake, 1);
            assert_eq!(objects.len(), 4);
        }
    }
}

#[test]
fn rolling_fighter_consumes_one_draw_rolls_each_callback_and_decrements_only_after_full_wait() {
    let catalog = authored_paths::catalog();
    for seed in 0..=u8::MAX {
        let (mut runtime, mut objects, fighter, player, mut random) = spawned_fighter(true, seed);
        let mut expected_random = random;
        let roll_step = (expected_random.next_byte() & 15).wrapping_add(248);
        objects.get_mut(fighter).unwrap().base.roll = Angle::from_units(233);
        let mut shared = EncounterCoordination {
            handshake: seed,
            ..Default::default()
        };
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        inputs.selected = Some(player);
        for visit in 1..=101 {
            if visit > 1 {
                callbacks(
                    &mut runtime,
                    &catalog,
                    &mut objects,
                    fighter,
                    &mut inputs,
                    visit as u8,
                );
            }
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, fighter, &mut inputs, 100)
                    .unwrap()
                    .step,
                if visit == 101 {
                    ControlStep::MovementTail
                } else {
                    ControlStep::Movement
                }
            );
            let actor = objects.get(fighter).unwrap();
            assert_eq!(
                actor.base.roll.units(),
                233_u8.wrapping_add(roll_step.wrapping_mul((visit - 1) as u8))
            );
            assert_eq!(actor.base.hit_points, if visit == 101 { 0 } else { 100 });
            assert_eq!(actor.base.flags.collision_disabled, visit == 101);
            assert!(!actor.base.flags.remove_after_tick);
            assert_eq!(
                inputs.coordination.as_deref().unwrap().handshake,
                seed.wrapping_sub(u8::from(visit == 101))
            );
            assert_eq!(inputs.random, &expected_random);
        }
        assert_eq!(objects.len(), 4);
    }
}

#[test]
fn guided_fighter_new_contact_awards_saturating_score_and_emits_four_fades_before_death() {
    use super::super::path_score::PlayerScore;
    let catalog = authored_paths::catalog();
    for count in [0, 1, 255] {
        for points in [0, 65485, 65486, 65535] {
            let (mut runtime, mut objects, fighter, player, mut random) =
                spawned_fighter(false, count);
            let mut shared = EncounterCoordination {
                handshake: count,
                ..Default::default()
            };
            let mut score = PlayerScore::from_parts(points, 171);
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.selected = Some(player);
            inputs.selected_score = Some(&mut score);
            let mut objective_counts = super::super::path_scene_state::EncounterObjectiveCounts { remaining_word: 0, ..Default::default() };
            inputs.objective_counts = Some(&mut objective_counts);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, fighter, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            objects
                .get_mut(fighter)
                .unwrap()
                .base
                .contacts
                .new_contact_latched = true;
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                fighter,
                &mut inputs,
                1,
            );
            let mut previous_random = *inputs.random;
            previous_random.next_byte(); // 8C87 chooses whether to attempt a drop, even with zero objectives.
            for visit in 1..=4 {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, fighter, &mut inputs, 100)
                        .unwrap()
                        .step,
                    if visit == 4 {
                        ControlStep::MovementTail
                    } else {
                        ControlStep::Movement
                    }
                );
                assert_eq!(
                    inputs.coordination.as_deref().unwrap().handshake,
                    count.wrapping_sub(1)
                );
                assert_eq!(
                    inputs.selected_score.as_deref().unwrap().points(),
                    171 * 65536 + u32::from(points.saturating_add(50))
                );
                assert_eq!(objects.len(), 4 + visit);
                let fade = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                assert_eq!(
                    fade.base.path,
                    Some(authored_paths::RANDOM_SIZE_MOTION_FADE_SPRITE)
                );
                assert_eq!(fade.base.hit_points, 100);
                assert_eq!(fade.base.attack_power, 50);
                assert!(fade.base.contacts.run_when_paused);
                assert_eq!(fade.extension.path_state.part, 6);
                assert_eq!(inputs.random, &previous_random);
            }
            assert_eq!(objects.get(fighter).unwrap().base.hit_points, 0);
            assert!(!objects.get(fighter).unwrap().base.flags.remove_after_tick);
        }
    }
}

#[test]
fn guided_fighter_chases_pitch_then_fires_on_its_randomized_thirty_step_cadence() {
    use super::super::weapon_dispatch::WeaponState;
    use super::super::AudioState;
    use super::projectile_tests::audio;
    let catalog = authored_paths::catalog();
    for seed in [0, 1, 31, 64, 127, 128, 255] {
        for mode in [0, 16, 31, 32, 255] {
            let (mut runtime, mut objects, fighter, player, mut random) =
                spawned_fighter(false, seed);
            // The plane predicate uses SELECTED-player orientation. Keep
            // the fighter on its nonnegative side through the firing phase.
            objects.get_mut(player).unwrap().base.yaw = Angle::from_units(128);
            let mut expected_random = random;
            expected_random.next_byte(); // initial yaw, later replaced by facing
            let mut pitch = (expected_random.next_byte() & 63).wrapping_sub(32);
            let initial_pitch = pitch;
            let mut chase_visits = 0;
            while pitch != 0 {
                let delta = 0_u8.wrapping_sub(pitch) as i8;
                let step = if delta < 0 {
                    i16::from(delta).min(-8)
                } else {
                    i16::from(delta).max(8)
                } / 8;
                pitch = pitch.wrapping_add(step as u8);
                chase_visits += 1;
            }
            let first_attack = 46 + chase_visits;
            let timer_start = expected_random.next_byte() & 15;
            let first_shot = first_attack + usize::from(29 - timer_start);
            let mut shared = EncounterCoordination {
                handshake: 1,
                ..Default::default()
            };
            let mut weapons = WeaponState::default();
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.selected = Some(player);
            inputs.primary_player = Some(player);
            inputs.primary_motion = Some(PrimaryMotionInput {
                auxiliary_mode: mode,
                displacement: Vector3::default(),
            });
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.weapons = Some(&mut weapons);
            inputs.audio = Some(audio(&mut events));
            for visit in 1..first_attack + 64 {
                if visit > 1 {
                    callbacks(
                        &mut runtime,
                        &catalog,
                        &mut objects,
                        fighter,
                        &mut inputs,
                        visit as u8,
                    );
                }
                // Keep the selected actor exactly ahead after the authored X
                // callback. Ordinary movement integration is outside this test.
                let position = objects.get(fighter).unwrap().base.position;
                objects.get_mut(player).unwrap().base.position = Vector3 {
                    z: position.z + 1000,
                    ..position
                };
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, fighter, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                let expected_shots =
                    usize::from(visit >= first_shot) + usize::from(visit >= first_shot + 30);
                assert_eq!(objects.len(), 4 + expected_shots, "seed={seed}, mode={mode}, visit={visit}, first={first_attack}, start={timer_start}, actor={:?}, invert={}", objects.get(fighter).unwrap().extension.relative_rotation, runtime.branch.invert_next);
                if visit <= 45 {
                    assert_eq!(
                        objects.get(fighter).unwrap().base.pitch.units(),
                        initial_pitch
                    );
                }
                if visit == first_shot || visit == first_shot + 30 {
                    let shot = objects
                        .active_ids()
                        .iter()
                        .filter_map(|id| objects.get(*id))
                        .filter(|actor| {
                            actor.base.path == Some(authored_paths::DIFFICULTY_HOMING_PROJECTILE)
                        })
                        .last()
                        .unwrap();
                    assert_eq!(shot.base.speed, if mode & 0xF0 == 0x10 { 40 } else { 70 });
                    assert_eq!(shot.base.pitch, objects.get(fighter).unwrap().base.pitch);
                }
            }
            assert_eq!(
                objects
                    .get(fighter)
                    .unwrap()
                    .extension
                    .relative_rotation
                    .roll
                    .units(),
                64
            );
            assert_eq!(inputs.coordination.as_deref().unwrap().handshake, 1);
        }
    }
}

#[test]
fn emitter_waits_twenty_five_plus_two_visits_per_attack_parameter_between_spawns() {
    let catalog = authored_paths::catalog();
    for delay in [1, 2, 4, 127, 255] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(ROOTS[1]);
        objects.get_mut(owner).unwrap().base.attack_power = delay;
        anchor(&mut objects);
        let mut shared = EncounterCoordination::default();
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        let second_spawn = 27 + 2 * usize::from(delay);
        for visit in 1..=second_spawn {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let expected = u8::from(visit >= 2) + u8::from(visit == second_spawn);
            assert_eq!(
                inputs.coordination.as_deref().unwrap().handshake,
                expected,
                "delay={delay}, visit={visit}"
            );
            assert_eq!(objects.len(), 2 + usize::from(expected));
        }
    }
}

#[test]
fn full_pool_still_increments_counter_without_mutating_last_spawn_or_other_actors() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for previous in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.path = Some(root);
            let linked = anchor(&mut objects);
            let selected = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            while objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::FollowPath,
                ))
                .is_some()
            {}
            let bystanders: Vec<_> = objects
                .active_ids()
                .iter()
                .copied()
                .filter(|id| *id != owner)
                .map(|id| (id, objects.get(id).unwrap().clone()))
                .collect();
            runtime.spawns.last_spawn = previous.then_some(linked);
            let last_spawn = runtime.spawns.last_spawn;
            let mut shared = EncounterCoordination::default();
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.selected = Some(selected);
            for _ in 0..2 {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
            }
            assert_eq!(inputs.coordination.as_deref().unwrap().handshake, 1);
            assert_eq!(runtime.spawns.last_spawn, last_spawn);
            for (id, before) in bystanders {
                assert_eq!(objects.get(id).unwrap(), &before);
            }
        }
    }
}

#[test]
fn complete_position_transfers_preserve_other_fields_and_missing_input_faults_before_mutation() {
    let catalog = PathCatalog::new(vec![vec![
        Statement::CaptureWorldPosition { next: cursor(0, 2) },
        Statement::RestoreWorldPosition { next: cursor(0, 2) },
        Statement::Control(ControlCommand::Goto {
            target: cursor(0, 0),
        }),
    ]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
    let saved = objects.get(owner).unwrap().clone();
    let mut inputs = world(&mut random);
    assert_eq!(
        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100),
        Err(ProgramError::MissingCapturedWorldPosition)
    );
    assert_eq!(objects.get(owner).unwrap(), &saved);
    for value in [i16::MIN, -1, 0, 1, i16::MAX] {
        let position = Vector3 {
            x: value,
            y: value.wrapping_add(1),
            z: value.wrapping_sub(1),
        };
        objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 0));
        objects.get_mut(owner).unwrap().base.position = position;
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(runtime.captured_world_position, Some(position));
        objects.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
        objects.get_mut(owner).unwrap().base.position = Vector3::default();
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects.get(owner).unwrap().base.position, position);
    }
}
