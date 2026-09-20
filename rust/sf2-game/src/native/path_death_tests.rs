//! Source death marking is distinct from retirement and ordinary movement.
use super::super::path_death::{DeathError, FriendHealth, FRIEND_HEALTH_SLOTS};
use super::super::path_fields::{Axis, ByteField, ByteOperation};
use super::super::path_relationships::RelationshipError;
use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
use super::super::path_triggers::{Trigger, TriggerKind};
use super::super::{Angle, Behavior, ObjectKind, PathId, ShapeId, Vector3};
use super::tests::{setup, world};
use super::*;

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

fn actor() -> Object {
    let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
    actor.base.path = Some(at(0));
    actor.base.hit_points = 99;
    actor.base.wait_timer = 71;
    actor.extension.path_state.repeat_counter = 173;
    actor
}

#[test]
fn death_preserves_loop_counter_and_effect_policy_while_disabling_collision() {
    for counter in 0..=u8::MAX {
        for flags in 0..4 {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.hit_points = 91;
            actor.extension.path_state.repeat_counter = counter;
            actor.extension.path_state.friend_health_slot = 0;
            actor.base.flags.collision_disabled = flags & 1 != 0;
            actor.base.flags.suppress_death_effects = flags & 2 != 0;
            let mut expected = actor.clone();
            expected.base.hit_points = 0;
            expected.base.flags.collision_disabled = true;
            let catalog = PathCatalog::new(vec![vec![Statement::MarkForDeath]]).unwrap();
            assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1).unwrap().step,
                ControlStep::MovementTail);
            assert_eq!(objects.get(owner), Some(&expected));
        }
    }
}

#[test]
fn every_valid_friend_selector_changes_only_its_health_record_and_death_fields() {
    let catalog = PathCatalog::new(vec![vec![Statement::MarkForDeath]]).unwrap();
    assert_eq!(FriendHealth::default().remaining, [40; FRIEND_HEALTH_SLOTS]);
    for selector in 0..=FRIEND_HEALTH_SLOTS as u8 {
        for health in 0..=u8::MAX {
            for with_input in [false, true] {
                if selector != 0 && !with_input {
                    continue;
                }
                let (mut runtime, mut objects, owner, mut random) = setup();
                *objects.get_mut(owner).unwrap() = actor();
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .friend_health_slot = selector;
                runtime.branch.invert_next = true;
                let mut expected = objects.clone();
                expected.get_mut(owner).unwrap().base.hit_points = 0;
                expected
                    .get_mut(owner)
                    .unwrap()
                    .base
                    .flags
                    .collision_disabled = true;
                let before_runtime = runtime.clone();
                let before_random = random;
                let mut friends = FriendHealth {
                    remaining: [health; FRIEND_HEALTH_SLOTS],
                };
                let mut inputs = world(&mut random);
                inputs.friend_health = if with_input { Some(&mut friends) } else { None };
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Ok(ProgramExit {
                        actor: owner,
                        step: ControlStep::MovementTail
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(runtime.resources, before_runtime.resources);
                assert!(runtime.branch.invert_next);
                let mut expected_health = [health; FRIEND_HEALTH_SLOTS];
                if selector != 0 {
                    expected_health[usize::from(selector - 1)] = 0;
                }
                assert_eq!(friends.remaining, expected_health);
                assert_eq!(random, before_random);
            }
        }
    }
}

#[test]
fn death_marks_only_the_gated_direct_sibling_chain_without_unlinking_or_retiring() {
    for enabled in [false, true] {
        for suppress in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            *objects.get_mut(owner).unwrap() = actor();
            let first = objects.allocate(actor()).unwrap();
            let second = objects.allocate(actor()).unwrap();
            let grandchild = objects.allocate(actor()).unwrap();
            let unrelated = objects.allocate(actor()).unwrap();
            let parent = objects.get_mut(owner).unwrap();
            parent.base.first_child = Some(first);
            parent.extension.path_state.motion.refresh_child_chain = enabled;
            parent.extension.path_state.motion.suppress_child_refresh = suppress;
            objects.get_mut(first).unwrap().base.next_sibling = Some(second);
            objects.get_mut(first).unwrap().base.first_child = Some(grandchild);
            objects.get_mut(first).unwrap().base.attachment = Some(owner);
            objects.get_mut(second).unwrap().base.attachment = Some(owner);
            runtime
                .add_trigger(
                    &mut objects,
                    first,
                    Trigger {
                        path: at(0),
                        kind: TriggerKind::Always,
                        timer: 0,
                    },
                )
                .unwrap();
            let resources = runtime.resources.clone();
            let mut expected = objects.clone();
            for id in [owner, first, second] {
                if id == owner || enabled {
                    expected.get_mut(id).unwrap().base.hit_points = 0;
                    expected
                        .get_mut(id)
                        .unwrap()
                        .base
                        .flags
                        .collision_disabled = true;
                }
            }
            let catalog = PathCatalog::new(vec![vec![Statement::MarkForDeath]]).unwrap();
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1)
                    .unwrap()
                    .step,
                ControlStep::MovementTail
            );
            assert_eq!(objects, expected);
            assert_eq!(runtime.resources, resources);
            assert_eq!(objects.get(grandchild).unwrap().base.hit_points, 99);
            assert_eq!(objects.get(unrelated).unwrap().base.hit_points, 99);
        }
    }
}

#[test]
fn missing_health_and_out_of_domain_selectors_fault_atomically_then_can_resume() {
    let catalog = PathCatalog::new(vec![vec![Statement::MarkForDeath]]).unwrap();
    for selector in 1..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .friend_health_slot = selector;
        let before = objects.clone();
        let expected = if usize::from(selector) <= FRIEND_HEALTH_SLOTS {
            DeathError::MissingFriendHealth
        } else {
            DeathError::InvalidFriendSelector(selector)
        };
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::Death(expected))
        );
        assert_eq!(objects, before);
        let mut friends = FriendHealth::default();
        let mut inputs = world(&mut random);
        inputs.friend_health = Some(&mut friends);
        if usize::from(selector) > FRIEND_HEALTH_SLOTS {
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::Death(expected))
            );
            assert_eq!(objects, before);
            assert_eq!(
                inputs.friend_health.as_deref().unwrap(),
                &FriendHealth::default()
            );
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .friend_health_slot = 0;
        }
        assert_eq!(
            runtime
                .resume_program(&catalog, &mut objects, owner, &mut inputs, 1)
                .unwrap()
                .step,
            ControlStep::MovementTail
        );
    }
}

#[test]
fn malformed_child_chains_do_not_partially_mark_actors_or_clear_health() {
    for cycle in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let child = objects.allocate(actor()).unwrap();
        let invalid = if cycle {
            owner
        } else {
            let id = objects.allocate(actor()).unwrap();
            objects.remove(id);
            id
        };
        let actor = objects.get_mut(owner).unwrap();
        actor.extension.path_state.friend_health_slot = 1;
        actor.extension.path_state.motion.refresh_child_chain = true;
        actor.base.first_child = Some(child);
        objects.get_mut(child).unwrap().base.next_sibling = Some(invalid);
        let before = objects.clone();
        let mut friends = FriendHealth::default();
        let mut inputs = world(&mut random);
        inputs.friend_health = Some(&mut friends);
        let catalog = PathCatalog::new(vec![vec![Statement::MarkForDeath]]).unwrap();
        let error = if cycle {
            RelationshipError::ChildCycle(invalid)
        } else {
            RelationshipError::MissingActor(invalid)
        };
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::Death(DeathError::Relationships(error)))
        );
        assert_eq!(objects, before);
        assert_eq!(friends, FriendHealth::default());
    }
}

#[test]
fn callback_tail_observes_zero_health_and_retains_only_post_callback_motion() {
    let catalog = PathCatalog::new(vec![vec![
        Statement::MarkForDeath,
        Statement::Mutate {
            mutation: Mutation::Byte {
                field: ByteField::Rotation(Axis::X),
                operation: ByteOperation::Add(ByteOperand::Literal(7)),
            },
            next: at(2),
        },
        Statement::Control(ControlCommand::Return),
    ]])
    .unwrap();
    for relative in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.velocity = Vector3 {
            x: 20,
            y: -30,
            z: 40,
        };
        actor.base.position = Vector3 {
            x: 300,
            y: -400,
            z: 500,
        };
        actor.extension.relative_position = Vector3 {
            x: 32760,
            y: -32760,
            z: -10,
        };
        actor.base.speed = 42;
        actor.base.target_speed = 55;
        actor.base.acceleration = 3;
        actor.base.roll = Angle::from_units(100);
        actor.base.yaw = Angle::from_units(31);
        actor.base.contacts.new_contact_latched = true;
        actor.base.contacts.hit_by_primary = true;
        actor.base.contacts.hit_by_secondary = true;
        actor.extension.path_state.clear_on_path_exit_latch = true;
        let motion = &mut actor.extension.path_state.motion;
        motion.relative_coordinates = relative;
        motion.generate_velocity_each_step = true;
        motion.bank_turn = true;
        motion.follow_player_displacement = true;
        runtime
            .add_trigger(
                &mut objects,
                owner,
                Trigger {
                    path: at(1),
                    kind: TriggerKind::ZeroHealth,
                    timer: 0,
                },
            )
            .unwrap();
        let mut expected = objects.get(owner).unwrap().clone();
        let mut inputs = world(&mut random);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 1)
                .unwrap()
                .step,
            ControlStep::MovementTail
        );
        expected.base.hit_points = 0;
        expected.base.flags.collision_disabled = true;
        assert_eq!(objects.get(owner).unwrap(), &expected);
        assert!(runtime.begin_movement_tail(&objects, owner).unwrap());
        assert_eq!(objects.get(owner).unwrap(), &expected);
        assert_eq!(
            runtime.begin_movement_tail(&objects, owner),
            Err(PathRuntimeError::MovementAlreadyActive)
        );
        assert_eq!(
            runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
            Ok(CallbackStep::Run(at(1)))
        );
        assert_eq!(
            runtime
                .resume_program(&catalog, &mut objects, owner, &mut inputs, 2)
                .unwrap()
                .step,
            ControlStep::ResumeCallbacks
        );
        assert_eq!(
            runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
            Ok(CallbackStep::Complete)
        );
        runtime
            .finish_movement(&mut objects, &mut [None, None])
            .unwrap();
        expected.base.pitch = Angle::from_units(7);
        if relative {
            expected.extension.relative_position = Vector3 {
                x: -32756,
                y: 32746,
                z: 30,
            };
        }
        expected.base.contacts.new_contact_latched = false;
        expected.base.contacts.hit_by_primary = false;
        expected.base.contacts.hit_by_secondary = false;
        expected.extension.path_state.clear_on_path_exit_latch = false;
        assert_eq!(objects.get(owner).unwrap(), &expected);
        assert_eq!(
            runtime.finish_movement(&mut objects, &mut [None, None]),
            Err(PathRuntimeError::NoMovementActive)
        );
    }
}

#[test]
fn death_is_not_a_valid_callback_root_exit() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    runtime
        .add_trigger(
            &mut objects,
            owner,
            Trigger {
                path: at(0),
                kind: TriggerKind::Always,
                timer: 0,
            },
        )
        .unwrap();
    assert!(runtime.begin_callbacks(&objects, owner).unwrap());
    assert_eq!(
        runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
        Ok(CallbackStep::Run(at(0)))
    );
    let before = objects.clone();
    let before_runtime = runtime.clone();
    let catalog = PathCatalog::new(vec![vec![Statement::MarkForDeath]]).unwrap();
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::Runtime(
            PathRuntimeError::InvalidTerminalCallback
        ))
    );
    assert_eq!(objects, before);
    assert_eq!(runtime, before_runtime);
}

#[test]
fn tail_refreshes_marked_children_and_carries_the_selected_player_without_world_integration() {
    use super::super::path_control::PlayerTarget;
    use super::super::platform_carry::CarriedPlayer;
    let catalog = PathCatalog::new(vec![vec![Statement::MarkForDeath]]).unwrap();
    for suppress in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let child = objects.allocate(actor()).unwrap();
        super::super::path_relationships::attach_fresh_child(&mut objects, owner, child, 1)
            .unwrap();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.position = Vector3 {
            x: 100,
            y: 200,
            z: 300,
        };
        actor.base.velocity = Vector3 {
            x: 10,
            y: 20,
            z: 30,
        };
        actor.extension.path_state.motion.suppress_child_refresh = suppress;
        actor.extension.path_state.motion.carry_selected_player = true;
        actor.extension.path_state.motion.relative_coordinates = true;
        actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
        actor.extension.path_state.motion_delta.x = 1;
        actor.extension.path_state.platform_carry.saved_position = Vector3 {
            x: 90,
            y: 180,
            z: 270,
        };
        let world_position = actor.base.position;
        let player = CarriedPlayer {
            enabled: true,
            carrier: Some(owner),
            origin: Vector3 {
                x: 90,
                y: 180,
                z: 270,
            },
            fine_yaw: 19,
        };
        let mut players = [Some(player); 2];
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1)
                .unwrap()
                .step,
            ControlStep::MovementTail
        );
        assert!(!runtime.begin_movement_tail(&objects, owner).unwrap());
        assert_eq!(
            objects.get(child).unwrap().base.position,
            Vector3::default()
        );
        runtime.finish_movement(&mut objects, &mut players).unwrap();
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.position, world_position);
        assert_eq!(actor.extension.relative_position, actor.base.velocity);
        assert_eq!(
            objects.get(child).unwrap().base.position,
            if suppress {
                Vector3::default()
            } else {
                world_position
            }
        );
        assert_eq!(objects.get(child).unwrap().base.hit_points, 0);
        assert_eq!(players[0], Some(player));
        assert_eq!(
            players[1],
            Some(CarriedPlayer {
                origin: world_position,
                ..player
            })
        );
        assert_eq!(
            actor.extension.path_state.platform_carry.saved_position,
            world_position
        );
    }
}

#[test]
fn authored_hit_detached_part_spins_selects_speed_bounces_clears_signal_then_dies() {
    use super::super::{authored_paths, path_motion, AudioState};
    use super::effect_tests::{audio, cues};
    let catalog = authored_paths::catalog();
    for (distance, speed) in [(0, 236), (500, 221), (1000, 206)] {
        for initial_y in [-1000i16, -30, 32760] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let parent = objects.allocate(actor()).unwrap();
            let selected = objects.allocate(actor()).unwrap();
            objects.get_mut(selected).unwrap().base.position.x = distance;
            super::super::path_relationships::attach_fresh_child(&mut objects, parent, owner, 9)
                .unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::HIT_DETACHED_BOUNCING_PART);
            actor.base.position.y = initial_y;
            actor.base.pitch = Angle::from_units(77);
            actor.extension.relative_rotation.pitch = Angle::from_units(250);
            actor.extension.path_state.motion_phase = 0xAB13;
            actor.base.hit_points = 37;
            let mut signals = EncounterSignals { raised: 0xA500 };
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.encounter_signals = Some(&mut signals);
            inputs.audio = Some(audio(&mut events));
            inputs.selected = Some(selected);
            for spin in 1..=4 {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 8)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(
                    actor.extension.relative_rotation.pitch.units(),
                    250u8.wrapping_add(spin * 8)
                );
                assert_eq!(actor.base.attachment, Some(parent));
                assert_eq!(actor.extension.spawn_group, 255);
                assert!(actor.base.contacts.suppress_contacts_next_epoch);
                assert_eq!(inputs.encounter_signals.as_deref().unwrap().raised, 0xA500);
                assert!(cues(&mut inputs).is_empty());
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
            }
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .conditions
                .hit_event_pending = true;
            let mut expected_position = objects.get(owner).unwrap().base.position;
            let mut expected_y_velocity = 0i16;
            let mut expected_depth_velocity = None;
            let mut death_cursor = None;
            for visit in 0..60u8 {
                let death = visit == 59;
                let mut expected_cues = Vec::new();
                if visit == 0 {
                    expected_cues.push(133);
                }
                if !death {
                    expected_y_velocity = expected_y_velocity.wrapping_add(4);
                    if expected_position.y.wrapping_add(30) >= 0 {
                        expected_cues.push(118);
                        expected_y_velocity = expected_y_velocity.wrapping_neg() / 2;
                    }
                }
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 32)
                        .unwrap()
                        .step,
                    if death {
                        ControlStep::MovementTail
                    } else {
                        ControlStep::Movement
                    }
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.position, expected_position);
                assert_eq!(actor.base.speed, speed);
                assert_eq!(actor.base.velocity.x, 0);
                assert_eq!(actor.base.velocity.y, expected_y_velocity);
                assert_eq!(
                    *expected_depth_velocity.get_or_insert(actor.base.velocity.z),
                    actor.base.velocity.z
                );
                assert_eq!(
                    actor.base.pitch.units(),
                    77u8.wrapping_add(240u8.wrapping_mul((visit + 1).min(59)))
                );
                assert_eq!(
                    actor.extension.path_state.motion_phase,
                    0xAB00 | u16::from(59 - visit)
                );
                assert_eq!(actor.base.attachment, None);
                assert_eq!(actor.base.child_number, 0);
                assert!(!actor.extension.path_state.motion.attached_coordinates);
                assert!(!actor.base.flags.remove_with_parent);
                assert!(!actor.extension.path_state.conditions.hit_event_pending);
                assert!(!actor.base.flags.remove_after_tick);
                assert_eq!(actor.base.hit_points, if death { 0 } else { 37 });
                assert_eq!(actor.base.flags.collision_disabled, death);
                assert!(!actor.base.flags.suppress_death_effects);
                assert!(objects.get(parent).unwrap().base.first_child.is_none());
                assert_eq!(cues(&mut inputs), expected_cues);
                let has_callbacks = if death {
                    death_cursor = objects.get(owner).unwrap().base.path;
                    runtime.begin_movement_tail(&objects, owner).unwrap()
                } else {
                    expected_position.y = expected_position.y.wrapping_add(expected_y_velocity);
                    expected_position.z = expected_position
                        .z
                        .wrapping_add(expected_depth_velocity.unwrap());
                    runtime
                        .begin_movement(
                            &mut objects,
                            owner,
                            path_motion::PlayerDisplacement::default(),
                        )
                        .unwrap()
                };
                let mut ran = 0;
                if has_callbacks {
                    loop {
                        match runtime
                            .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                            .unwrap()
                        {
                            CallbackStep::Run(_) => {
                                ran += 1;
                                assert_eq!(
                                    runtime
                                        .resume_program(
                                            &catalog,
                                            &mut objects,
                                            owner,
                                            &mut inputs,
                                            2
                                        )
                                        .unwrap()
                                        .step,
                                    ControlStep::ResumeCallbacks
                                );
                            }
                            CallbackStep::Skipped | CallbackStep::Expired => {}
                            CallbackStep::Complete => break,
                        }
                    }
                }
                assert_eq!(ran, usize::from(visit == 29));
                assert_eq!(
                    inputs.encounter_signals.as_deref().unwrap().raised,
                    if visit < 29 { 0xA501 } else { 0xA500 }
                );
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
                assert_eq!(objects.get(owner).unwrap().base.position, expected_position);
                assert!(cues(&mut inputs).is_empty());
            }
            assert_eq!(objects.get(owner).unwrap().base.path, death_cursor);
            runtime.release_actor_programs(&mut objects, owner).unwrap();
            assert!(objects.remove(owner).is_some());
        }
    }
}

#[test]
fn rotating_controller_spawn_gate_covers_every_part_number_and_pending_inversion() {
    use super::super::{authored_paths, ObjectSpawnDefaults};
    let catalog = authored_paths::catalog();
    for number in 0..=u8::MAX {
        for inverted in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let mother = objects.allocate(actor()).unwrap();
            super::super::path_relationships::attach_fresh_child(
                &mut objects,
                mother,
                owner,
                number,
            )
            .unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::HIT_DRIVEN_ROTATING_PART_CONTROLLER);
            actor.extension.path_state.needs_path_initialization = true;
            runtime.branch.invert_next = inverted;
            let mut inputs = world(&mut random);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults {
                run_when_paused: true,
                group: 43,
            });
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 6)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert!(!runtime.branch.invert_next);
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.extension.spawn_group, 255);
            assert!(actor.base.flags.collision_disabled);
            assert_eq!(actor.base.child_number, number);
            assert_eq!(actor.base.attachment, Some(mother));
            let should_spawn = (number != 3) != inverted;
            assert_eq!(objects.len(), if should_spawn { 3 } else { 2 });
            if should_spawn {
                let id = runtime.spawns.last_spawn.unwrap();
                let child = objects.get(id).unwrap();
                assert_eq!(child.base.kind, ObjectKind::Enemy);
                assert_eq!(child.base.shape, ShapeId::from_catalog_index(323));
                assert_eq!(
                    child.base.path,
                    Some(authored_paths::HIT_DETACHED_BOUNCING_PART)
                );
                assert_eq!(
                    (
                        child.base.hit_points,
                        child.base.attack_power,
                        child.base.child_number
                    ),
                    (10, 4, 1)
                );
                assert_eq!(child.base.attachment, Some(mother));
                assert_eq!(child.extension.parent, Some(owner));
                assert!(child.extension.path_state.needs_path_initialization);
                assert_eq!(
                    child.extension.relative_position,
                    Vector3 {
                        x: 208,
                        y: 140,
                        z: -140
                    }
                );
                assert_eq!(child.base.position, Vector3::default());
                assert_eq!(child.extension.spawn_group, 255);
                assert!(child.base.contacts.run_when_paused);
                assert_eq!(objects.get(owner).unwrap().base.next_sibling, Some(id));
            } else {
                assert!(runtime.spawns.last_spawn.is_none());
            }
            let before = objects.clone();
            for _ in 0..3 {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 2)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                assert_eq!(objects, before);
            }
        }
    }
}

#[test]
fn rotating_controller_signals_after_twenty_five_increments_then_chases_and_repeats() {
    use super::super::{authored_paths, path_motion, AudioState, ObjectSpawnDefaults};
    use super::effect_tests::{audio, cues};
    let catalog = authored_paths::catalog();
    for initial_pitch in [0u8, 28, 156, 250] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let mother = objects.allocate(actor()).unwrap();
        super::super::path_relationships::attach_fresh_child(&mut objects, mother, owner, 4)
            .unwrap();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::HIT_DRIVEN_ROTATING_PART_CONTROLLER);
        actor.extension.relative_rotation.pitch = Angle::from_units(initial_pitch);
        let mut events = AudioState::default();
        let mut signals = EncounterSignals::default();
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.audio = Some(audio(&mut events));
        inputs.encounter_signals = Some(&mut signals);
        inputs.selected = Some(mother);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 6)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let mut child = runtime.spawns.last_spawn.unwrap();
        for cycle in 0..2 {
            let start_pitch = objects
                .get(owner)
                .unwrap()
                .extension
                .relative_rotation
                .pitch
                .units();
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .conditions
                .hit_event_pending = true;
            for increment in 1..=25u8 {
                let mut expected_pitch = start_pitch.wrapping_add(increment * 4);
                if increment == 25 && expected_pitch != 0 {
                    let delta = 0u8.wrapping_sub(expected_pitch) as i8 as i16;
                    let step = if delta < 0 {
                        delta.min(-8) / 8
                    } else {
                        delta.max(8) / 8
                    };
                    expected_pitch = expected_pitch.wrapping_add(step as u8);
                }
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 10)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                assert_eq!(
                    objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .relative_rotation
                        .pitch
                        .units(),
                    expected_pitch
                );
                assert_eq!(
                    objects
                        .get(child)
                        .unwrap()
                        .extension
                        .path_state
                        .conditions
                        .hit_event_pending,
                    increment == 25
                );
                assert_eq!(objects.get(child).unwrap().base.attachment, Some(mother));
                assert!(
                    !objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .conditions
                        .hit_event_pending
                );
                assert!(!runtime
                    .begin_movement(
                        &mut objects,
                        owner,
                        path_motion::PlayerDisplacement::default()
                    )
                    .unwrap());
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
            }
            // Independently scheduled child consumes the event, detaches,
            // completes its real graph and reaches death while the controller
            // retains its own cursor/stack and waits for its next visit.
            let saved_controller = objects.get(owner).unwrap().base.path;
            for visit in 0..60 {
                let exit = runtime
                    .enter_program(&catalog, &mut objects, child, &mut inputs, 32)
                    .unwrap();
                assert_eq!(exit.actor, child);
                assert_eq!(
                    exit.step,
                    if visit == 59 {
                        ControlStep::MovementTail
                    } else {
                        ControlStep::Movement
                    }
                );
                let active = if visit == 59 {
                    runtime.begin_movement_tail(&objects, child).unwrap()
                } else {
                    runtime
                        .begin_movement(
                            &mut objects,
                            child,
                            path_motion::PlayerDisplacement::default(),
                        )
                        .unwrap()
                };
                if active {
                    loop {
                        match runtime
                            .step_callbacks(&mut objects, child, TriggerWorldInputs::default())
                            .unwrap()
                        {
                            CallbackStep::Run(_) => assert_eq!(
                                runtime
                                    .resume_program(&catalog, &mut objects, child, &mut inputs, 2)
                                    .unwrap()
                                    .step,
                                ControlStep::ResumeCallbacks
                            ),
                            CallbackStep::Skipped | CallbackStep::Expired => {}
                            CallbackStep::Complete => break,
                        }
                    }
                }
                runtime
                    .finish_movement(&mut objects, &mut [None, None])
                    .unwrap();
                cues(&mut inputs);
                assert_eq!(objects.get(owner).unwrap().base.path, saved_controller);
            }
            assert!(objects.get(child).unwrap().base.attachment.is_none());
            assert_eq!(objects.get(child).unwrap().base.hit_points, 0);
            // A wrapped zero on the 25th increment skips the chase yield and
            // immediately spawns the next child BEFORE the old one detaches.
            let immediate = start_pitch.wrapping_add(100) == 0;
            assert_eq!(runtime.spawns.last_spawn != Some(child), immediate);
            if !immediate {
                for visit in 0..64 {
                    let current = objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .relative_rotation
                        .pitch
                        .units();
                    let delta = 0u8.wrapping_sub(current) as i8 as i16;
                    let expected = current.wrapping_add(if delta == 0 {
                        0
                    } else if delta < 0 {
                        (delta.min(-8) / 8) as u8
                    } else {
                        (delta.max(8) / 8) as u8
                    });
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 8)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                    assert_eq!(
                        objects
                            .get(owner)
                            .unwrap()
                            .extension
                            .relative_rotation
                            .pitch
                            .units(),
                        expected
                    );
                    if current == 0 {
                        assert_ne!(runtime.spawns.last_spawn, Some(child));
                        break;
                    }
                    assert_eq!(runtime.spawns.last_spawn, Some(child));
                    assert!(visit < 63, "chase failed to converge");
                }
            }
            let next_child = runtime.spawns.last_spawn.unwrap();
            assert_ne!(next_child, child);
            assert_eq!(
                objects.get(next_child).unwrap().base.attachment,
                Some(mother)
            );
            assert_eq!(
                objects.get(next_child).unwrap().extension.parent,
                Some(owner)
            );
            assert!(
                !objects
                    .get(next_child)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .hit_event_pending
            );
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .relative_rotation
                    .pitch
                    .units(),
                0
            );
            assert_eq!(objects.len(), 4 + cycle);
            runtime.release_actor_programs(&mut objects, child).unwrap();
            child = next_child;
        }
    }
}
