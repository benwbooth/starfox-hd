//! Complete authored effect graphs that temporarily borrow other actors.
use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
use super::super::{
    authored_paths as paths, path_motion::PublishedPlayerMotion, AudioState, Behavior, ObjectKind,
    ObjectSpawnDefaults, ShapeId, Vector3,
};
use super::effect_tests::{audio, callbacks, cues};
use super::tests::{setup, world};
use super::*;

fn run(
    runtime: &mut PathRuntime,
    catalog: &PathCatalog,
    objects: &mut ObjectStore,
    owner: ObjectId,
    inputs: &mut PathWorld<'_>,
) -> ControlStep {
    let exit = runtime
        .enter_program(catalog, objects, owner, inputs, 300_000)
        .unwrap();
    assert_eq!(exit.actor, owner);
    exit.step
}

fn finish_fade(
    runtime: &mut PathRuntime,
    catalog: &PathCatalog,
    objects: &mut ObjectStore,
    child: ObjectId,
    inputs: &mut PathWorld<'_>,
) {
    assert!(
        objects
            .get(child)
            .unwrap()
            .extension
            .path_state
            .needs_path_initialization
    );
    runtime.initialize_path_strategy(objects, child).unwrap();
    let mut auxiliary = SelectedAuxiliaryState {
        mode: 0x20,
        action_flags: 0,
    };
    // This mode suppresses the motion callback, but not randomization or audio.
    for visit in 0..8 {
        let mut child_world = world(inputs.random);
        child_world.selected_auxiliary = Some(&mut auxiliary);
        let mut events = AudioState::default();
        child_world.audio = Some(audio(&mut events));
        assert_eq!(
            run(runtime, catalog, objects, child, &mut child_world),
            if visit == 7 {
                ControlStep::Ended
            } else {
                ControlStep::Movement
            }
        );
        assert_eq!(
            objects
                .get(child)
                .unwrap()
                .extension
                .path_state
                .animation
                .color
                .fixed_frame(),
            Some(visit)
        );
    }
    runtime.release_actor_programs(objects, child).unwrap();
    objects.remove(child).unwrap();
}

#[test]
fn four_pulse_emitter_restores_caller_each_visit_and_dies_after_final_spawn() {
    let catalog = paths::catalog();
    for part in [0, 1, 249, 255] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(paths::FOUR_PULSE_DEATH_EMITTER);
        actor.base.position = Vector3 { x: 32760, y: -1234, z: -32760 };
        actor.base.velocity = Vector3 { x: 17, y: -21, z: 29 };
        actor.base.hit_points = 93;
        actor.extension.path_state.part = part;
        let mut expected_position = actor.base.position;
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        for visit in 0..4 {
            let death = visit == 3;
            assert_eq!(run(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                if death { ControlStep::MovementTail } else { ControlStep::Movement });
            let child = runtime.spawns.last_spawn.unwrap();
            let spawned = objects.get(child).unwrap();
            assert_eq!(spawned.base.path, Some(paths::RANDOM_SIZE_MOTION_FADE_SPRITE));
            assert_eq!(spawned.base.shape.catalog_index(), 14);
            assert_eq!(spawned.base.position, expected_position);
            assert_eq!((spawned.base.hit_points, spawned.base.attack_power), (100, 50));
            assert_eq!(spawned.extension.path_state.part, 6);
            assert!(spawned.base.contacts.run_when_paused);
            assert_eq!(spawned.base.attachment, None);
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.position, expected_position);
            assert_eq!(actor.extension.path_state.part, part);
            assert_eq!(actor.base.hit_points, if death { 0 } else { 93 });
            assert_eq!(actor.base.flags.collision_disabled, death);
            assert!(!actor.base.flags.suppress_death_effects);
            assert!(!actor.base.flags.remove_after_tick);
            finish_fade(&mut runtime, &catalog, &mut objects, child, &mut inputs);
            assert_eq!(objects.len(), 1);
            let pending = if death { runtime.begin_movement_tail(&objects, owner).unwrap() }
                else { runtime.begin_movement(&mut objects, owner, super::super::path_motion::PlayerDisplacement::default()).unwrap() };
            assert!(!pending);
            runtime.finish_movement(&mut objects, &mut [None, None]).unwrap();
            if !death {
                expected_position.x = expected_position.x.wrapping_add(17);
                expected_position.y = expected_position.y.wrapping_sub(21);
                expected_position.z = expected_position.z.wrapping_add(29);
            }
            assert_eq!(objects.get(owner).unwrap().base.position, expected_position);
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }
}

#[test]
fn periodic_emitters_keep_caller_context_and_run_spawned_fades_through_retirement() {
    let catalog = paths::catalog();
    for timed in [false, true] {
        for initial_wait in [0u8, 33, 34, 35, 255] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(if timed {
                paths::TIMED_GROUND_JITTER_EMITTER
            } else {
                paths::PERIODIC_PART_MOTION_EMITTER
            });
            actor.base.position = Vector3 {
                x: 32760,
                y: -1234,
                z: -32760,
            };
            actor.base.hit_points = if timed { 10 } else { 100 };
            actor.base.attack_power = if timed { 10 } else { 0 };
            actor.base.wait_timer = initial_wait;
            actor.extension.path_state.part = 231;
            let original = actor.clone();
            let lifetime = if timed {
                usize::from(34u8.wrapping_sub(initial_wait))
            } else {
                256
            };
            let mut inputs = world(&mut random);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults {
                run_when_paused: false,
                group: 23,
            });
            for visit in 0..=lifetime {
                let end = timed && visit == lifetime;
                assert_eq!(
                    run(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                    if end {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                let source = objects.get(owner).unwrap();
                assert!(!source.base.flags.visible);
                assert_eq!(source.base.position, original.base.position);
                assert_eq!(source.extension.path_state.part, 231);
                if end {
                    break;
                }
                let mut saved = source.clone();
                assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                let trigger = TriggerWorldInputs {
                    strategy_tick: visit as u8,
                    ..Default::default()
                };
                if visit % 4 == 0 {
                    assert!(matches!(
                        runtime.step_callbacks(&mut objects, owner, trigger),
                        Ok(CallbackStep::Run(_))
                    ));
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 12),
                        Ok(ProgramExit {
                            actor: owner,
                            step: ControlStep::ResumeCallbacks
                        })
                    );
                    assert_eq!(
                        runtime.step_callbacks(&mut objects, owner, trigger),
                        Ok(CallbackStep::Complete)
                    );
                    let child = runtime.spawns.last_spawn.unwrap();
                    saved.base.next = Some(child); // allocator links the independent actor after its caller
                    let spawned = objects.get(child).unwrap();
                    assert_eq!(
                        spawned.base.path,
                        Some(if timed {
                            paths::PART_JITTER_FADE_SPRITE
                        } else {
                            paths::RANDOM_SIZE_MOTION_FADE_SPRITE
                        })
                    );
                    assert_eq!(
                        spawned.base.shape,
                        ShapeId::from_catalog_index(if timed { 8 } else { 13 })
                    );
                    assert_eq!(
                        spawned.base.position,
                        Vector3 {
                            y: if timed { 0 } else { -1234 },
                            ..original.base.position
                        }
                    );
                    assert_eq!(
                        (spawned.base.hit_points, spawned.base.attack_power),
                        (100, if timed { 0 } else { 100 })
                    );
                    assert_eq!(spawned.extension.path_state.part, if timed { 0 } else { 6 });
                    assert_eq!(spawned.extension.texture_scroll_x, 0);
                    assert!(spawned.base.contacts.run_when_paused);
                    assert_eq!(objects.get(owner).unwrap(), &saved);
                    finish_fade(&mut runtime, &catalog, &mut objects, child, &mut inputs);
                    assert_eq!(objects.len(), 1);
                } else {
                    assert_eq!(
                        runtime.step_callbacks(&mut objects, owner, trigger),
                        Ok(CallbackStep::Skipped)
                    );
                    assert_eq!(
                        runtime.step_callbacks(&mut objects, owner, trigger),
                        Ok(CallbackStep::Complete)
                    );
                }
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}

#[test]
fn pulse_pair_entries_finish_both_actors_and_position_chase_is_immediate() {
    let catalog = paths::catalog();
    for root in [
        paths::LOW_HEALTH_PULSE_PAIR,
        paths::PULSE_PAIR,
        paths::PLAYER_POSITION_PULSE_PAIR,
    ] {
        for health in [0u8, 1, 2, 127, 255] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let position = Vector3 {
                x: 32760,
                y: -32760,
                z: 1,
            };
            let target = Vector3 {
                x: -32760,
                y: 32760,
                z: -99,
            };
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(root);
            actor.base.position = position;
            actor.base.hit_points = health;
            actor.base.attack_power = 0;
            actor.base.wait_timer = 201;
            let mut expected = position;
            if root == paths::PLAYER_POSITION_PULSE_PAIR {
                for _ in 0..if health == 0 {
                    65_536
                } else {
                    usize::from(health)
                } {
                    for (value, target) in [
                        (&mut expected.x, target.x),
                        (&mut expected.y, target.y),
                        (&mut expected.z, target.z),
                    ] {
                        let delta = i32::from(target.wrapping_sub(*value));
                        let step = if delta == 0 {
                            0
                        } else {
                            delta.signum() * (delta.abs() / 8).max(1)
                        };
                        *value = value.wrapping_add(step as i16);
                    }
                }
            }
            let mut inputs = world(&mut random);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults {
                run_when_paused: false,
                group: 3,
            });
            inputs.published_motion = Some(PublishedPlayerMotion {
                position: target,
                delta: Vector3::default(),
            });
            let colors: Vec<_> = [1, 2, 3, 3, 2, 1, 0]
                .repeat(3)
                .into_iter()
                .chain(1..=7)
                .collect();
            let mut child = None;
            for (visit, &color) in colors.iter().enumerate() {
                let end = visit == 27;
                assert_eq!(
                    run(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                    if end {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.position, expected);
                assert_eq!(actor.base.wait_timer, 201);
                assert_eq!(
                    actor.extension.path_state.animation.color.fixed_frame(),
                    Some(color)
                );
                assert_eq!(
                    actor.base.contacts.run_when_paused,
                    root == paths::PLAYER_POSITION_PULSE_PAIR
                );
                if visit == 0 {
                    child = runtime.spawns.last_spawn;
                    let spawned = objects.get(child.unwrap()).unwrap();
                    assert_eq!(
                        spawned.base.path,
                        Some(paths::PHASE_INCREMENTED_MOTION_FADE_SPRITE)
                    );
                    assert_eq!(spawned.base.position, expected);
                    assert_eq!(
                        (spawned.base.hit_points, spawned.base.attack_power),
                        (
                            if root == paths::LOW_HEALTH_PULSE_PAIR {
                                1
                            } else {
                                100
                            },
                            10
                        )
                    );
                    assert!(spawned.base.contacts.run_when_paused);
                    assert_eq!(spawned.extension.texture_scroll_x, 0);
                }
                if !end {
                    assert_eq!(
                        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                        1
                    );
                    expected.y = expected.y.wrapping_sub(20);
                }
                assert_eq!(objects.len(), 2);
            }
            finish_fade(
                &mut runtime,
                &catalog,
                &mut objects,
                child.unwrap(),
                &mut inputs,
            );
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}

#[test]
fn linked_reveal_changes_only_mother_appearance_after_all_seventeen_yields() {
    let catalog = paths::catalog();
    for mother_path in [None, Some(paths::TEN_TICK_EFFECT)] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let mut mother = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Effect);
        mother.base.path = mother_path;
        mother.base.hit_points = 171;
        mother.base.wait_timer = 39;
        mother.base.position = Vector3 {
            x: -17,
            y: 37,
            z: 71,
        };
        mother.extension.clipping_plane =
            super::super::render::ClippingPlaneSelection::from_selector_byte(4);
        let mother_id = objects.allocate(mother.clone()).unwrap();
        mother = objects.get(mother_id).unwrap().clone();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(paths::LINKED_SHAPE_REVEAL_ATTACHMENT);
        actor.base.attachment = Some(mother_id);
        actor.base.wait_timer = 173;
        let mut events = AudioState::default();
        let mut inputs = world(&mut random);
        inputs.audio = Some(audio(&mut events));
        for visit in 1..=20 {
            assert_eq!(
                run(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                ControlStep::Movement
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                actor.extension.relative_position.z,
                if visit <= 16 { 400 - visit * 19 } else { 86 }
            );
            assert_eq!(actor.extension.relative_rotation.pitch.units(), 200);
            assert_eq!(actor.base.wait_timer, 173);
            assert_eq!(actor.extension.path_state.hold_latched, visit >= 18);
            assert!(actor.base.flags.collision_disabled);
            assert!(actor.base.flags.far_sort_bias);
            assert_eq!(
                cues(&mut inputs),
                if visit == 1 { vec![155] } else { vec![] }
            );
            if visit == 18 {
                mother.base.shape = ShapeId::from_catalog_index(316);
                mother.extension.clipping_plane =
                    super::super::render::ClippingPlaneSelection::from_selector_byte(0);
            }
            assert_eq!(objects.get(mother_id).unwrap(), &mother);
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }
}

#[test]
fn repeating_emitter_preserves_wait_wrap_and_goto_yield_between_complete_pulses() {
    let catalog = paths::catalog();
    for initial_wait in [0u8, 14, 15, 16, 255] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(paths::REPEATING_PULSE_EMITTER);
        actor.base.position = Vector3 {
            x: 137,
            y: -91,
            z: 311,
        };
        actor.base.hit_points = 10;
        actor.base.attack_power = 10;
        actor.base.wait_timer = initial_wait;
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults {
            run_when_paused: false,
            group: 0,
        });
        for cycle in 0..3 {
            let initial = if cycle == 0 { initial_wait } else { 0 };
            let waits = usize::from(15u8.wrapping_sub(initial));
            let mut child = None;
            for visit in 0..=waits {
                assert_eq!(
                    run(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                    ControlStep::Movement
                );
                if visit == 0 {
                    child = runtime.spawns.last_spawn;
                }
                assert_eq!(runtime.spawns.last_spawn, child);
                assert_eq!(objects.len(), 2);
                let actor = objects.get(owner).unwrap();
                assert!(!actor.base.flags.visible);
                assert!(actor.base.contacts.run_when_paused);
                assert_eq!(
                    actor.base.wait_timer,
                    if visit == waits {
                        0
                    } else {
                        initial.wrapping_add(visit as u8).wrapping_add(1)
                    }
                );
                assert!(!actor.base.flags.remove_after_tick);
            }
            // The final parent visit only executes the yielding GOTO. Its
            // next spawn is deferred to the next invocation, including WAIT=15.
            let child = child.unwrap();
            let spawned = objects.get(child).unwrap();
            assert_eq!(spawned.base.path, Some(paths::DRIFTING_PULSE_SPRITE));
            assert_eq!(
                spawned.base.position,
                Vector3 {
                    x: 137,
                    y: -91,
                    z: 311
                }
            );
            assert_eq!(
                (spawned.base.hit_points, spawned.base.attack_power),
                (100, 0)
            );
            assert!(spawned.base.contacts.run_when_paused);
            assert!(spawned.extension.path_state.needs_path_initialization);
            for visit in 0..28 {
                assert_eq!(
                    run(&mut runtime, &catalog, &mut objects, child, &mut inputs),
                    if visit == 27 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                assert_eq!(
                    objects.get(child).unwrap().base.position,
                    Vector3 {
                        x: 137,
                        y: -91 - 20 * visit,
                        z: 311
                    }
                );
                if visit != 27 {
                    assert_eq!(
                        callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs),
                        1
                    );
                }
            }
            runtime.release_actor_programs(&mut objects, child).unwrap();
            objects.remove(child).unwrap();
        }
    }
}

#[test]
fn exhausted_emitter_spawns_borrow_retained_last_actor_or_report_missing_selection() {
    use super::super::OBJECT_CAPACITY;
    let catalog = paths::catalog();
    for timed in [false, true] {
        for prior in [None, Some(None), Some(Some(paths::TEN_TICK_EFFECT))] {
            for part in [0u8, 249, 250, 255] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                objects.get_mut(owner).unwrap().base.path = Some(if timed {
                    paths::TIMED_GROUND_JITTER_EMITTER
                } else {
                    paths::PERIODIC_PART_MOTION_EMITTER
                });
                let mut previous = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Effect);
                previous.base.path = prior.flatten();
                previous.base.position = Vector3 {
                    x: 51,
                    y: -791,
                    z: 83,
                };
                previous.base.hit_points = 37;
                previous.base.wait_timer = 103;
                previous.extension.path_state.part = part;
                previous.extension.path_state.needs_path_initialization = true;
                let previous = objects.allocate(previous).unwrap();
                for _ in objects.len()..OBJECT_CAPACITY {
                    objects
                        .allocate(Object::new(
                            ObjectKind::Enemy,
                            ShapeId::EMPTY,
                            Behavior::Effect,
                        ))
                        .unwrap();
                }
                runtime.spawns.last_spawn = prior.map(|_| previous);
                let mut expected = objects.get(previous).unwrap().clone();
                let before_random = random;
                let mut inputs = world(&mut random);
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                assert_eq!(
                    run(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                    ControlStep::Movement
                );
                assert!(runtime.begin_callbacks(&objects, owner).unwrap());
                assert!(matches!(
                    runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                    Ok(CallbackStep::Run(_))
                ));
                let result = runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 12);
                if prior.is_none() {
                    assert_eq!(
                        result,
                        Err(ProgramError::ActorContext(
                            ActorContextError::MissingLastSpawn
                        ))
                    );
                    assert_eq!(runtime.program_actor(), Some(owner));
                } else {
                    assert_eq!(
                        result,
                        Ok(ProgramExit {
                            actor: owner,
                            step: ControlStep::ResumeCallbacks
                        })
                    );
                    assert_eq!(
                        runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
                        Ok(CallbackStep::Complete)
                    );
                    expected.base.contacts.run_when_paused = true;
                    if timed {
                        expected.base.position.y = 0;
                    } else {
                        expected.extension.path_state.part = part.wrapping_add(6);
                    }
                }
                assert_eq!(objects.get(previous).unwrap(), &expected);
                assert_eq!(runtime.spawns.last_spawn, prior.map(|_| previous));
                assert_eq!(objects.len(), OBJECT_CAPACITY);
                assert_eq!(random, before_random);
            }
        }
    }
}
