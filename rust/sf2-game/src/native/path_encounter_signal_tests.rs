//! Scene-owned synchronization survives actor and diagnostic boundaries.
use super::super::{Behavior, ObjectKind, PathId, ShapeId};
use super::tests::{setup, world};
use super::*;

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn budget(cursor: PathCursor) -> Result<ProgramExit, ProgramError> {
    Err(ProgramError::BudgetExceeded {
        cursor,
        executed: 1,
    })
}

#[test]
fn all_signal_words_use_any_bit_tests_without_consuming_ifnot_or_actor_state() {
    for mask in [0, 1, 0x0100, 0x8000, 0x0181, 0xFFFF] {
        for clear in [false, true] {
            let catalog = PathCatalog::new(vec![vec![Statement::EncounterSignalBranch {
                condition: if clear {
                    EncounterSignalCondition::AllClear(mask)
                } else {
                    EncounterSignalCondition::AnyRaised(mask)
                },
                taken: at(1),
                next: at(2),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.wait_timer = 193;
            actor.base.hit_points = 29;
            actor.base.attack_power = 247;
            actor.extension.path_state.motion_phase = 0xA17D;
            let initial = actor.clone();
            let original_random = random;
            let mut signals = EncounterSignals::default();
            let mut inputs = world(&mut random);
            inputs.encounter_signals = Some(&mut signals);
            for value in 0..=u16::MAX {
                for invert in [false, true] {
                    *objects.get_mut(owner).unwrap() = initial.clone();
                    runtime.branch.invert_next = invert;
                    inputs.encounter_signals.as_deref_mut().unwrap().raised = value;
                    let matches = (value & mask == 0) == clear;
                    let destination = at(if matches { 1 } else { 2 });
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        budget(destination)
                    );
                    let mut expected = initial.clone();
                    expected.base.path = Some(destination);
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                    assert_eq!(runtime.branch.invert_next, invert);
                    assert_eq!(inputs.encounter_signals.as_deref().unwrap().raised, value);
                }
            }
            assert_eq!(random, original_random);
        }
    }
}

#[test]
fn raise_clear_and_reset_preserve_unselected_bits_and_entire_actor_except_cursor() {
    for mask in [0, 1, 0x0100, 0x8000, 0x0181, 0xFFFF] {
        for command in [
            EncounterSignalCommand::Raise(mask),
            EncounterSignalCommand::Clear(mask),
            EncounterSignalCommand::Reset,
        ] {
            let catalog = PathCatalog::new(vec![vec![Statement::EncounterSignal {
                command,
                next: at(1),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.wait_timer = 101;
            actor.extension.path_state.repeat_counter = 207;
            let initial = actor.clone();
            let original_random = random;
            let mut signals = EncounterSignals::default();
            let mut inputs = world(&mut random);
            inputs.encounter_signals = Some(&mut signals);
            for value in 0..=u16::MAX {
                *objects.get_mut(owner).unwrap() = initial.clone();
                runtime.branch.invert_next = value & 1 != 0;
                inputs.encounter_signals.as_deref_mut().unwrap().raised = value;
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    budget(at(1))
                );
                let expected = match command {
                    EncounterSignalCommand::Raise(mask) => value | mask,
                    EncounterSignalCommand::Clear(mask) => value & !mask,
                    EncounterSignalCommand::Reset => 0,
                };
                assert_eq!(
                    inputs.encounter_signals.as_deref().unwrap().raised,
                    expected
                );
                let mut expected_actor = initial.clone();
                expected_actor.base.path = Some(at(1));
                assert_eq!(objects.get(owner).unwrap(), &expected_actor);
                assert_eq!(runtime.branch.invert_next, value & 1 != 0);
            }
            assert_eq!(random, original_random);
        }
    }
}

#[test]
fn missing_signals_fault_atomically_and_live_input_is_sampled_again_on_resume() {
    let cases = [
        Statement::EncounterSignal {
            command: EncounterSignalCommand::Raise(0x8001),
            next: at(0),
        },
        Statement::EncounterSignal {
            command: EncounterSignalCommand::Clear(0x0081),
            next: at(0),
        },
        Statement::EncounterSignal {
            command: EncounterSignalCommand::Reset,
            next: at(0),
        },
        Statement::EncounterSignalBranch {
            condition: EncounterSignalCondition::AnyRaised(0x8001),
            taken: at(1),
            next: at(0),
        },
        Statement::EncounterSignalBranch {
            condition: EncounterSignalCondition::AllClear(0x8001),
            taken: at(1),
            next: at(0),
        },
    ];
    for statement in cases {
        let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let initial = objects.clone();
        runtime.branch.invert_next = true;
        for _ in 0..2 {
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingEncounterSignals)
            );
            assert_eq!(objects, initial);
            assert!(runtime.branch.invert_next);
        }
        for value in [0, 0x8000, 0x0040, 0xFFFF] {
            objects.get_mut(owner).unwrap().base.path = Some(at(0));
            let mut signals = EncounterSignals { raised: value };
            let mut inputs = world(&mut random);
            inputs.encounter_signals = Some(&mut signals);
            let destination = match statement {
                Statement::EncounterSignalBranch { condition, .. } => {
                    let matches = match condition {
                        EncounterSignalCondition::AnyRaised(mask) => value & mask != 0,
                        EncounterSignalCondition::AllClear(mask) => value & mask == 0,
                    };
                    at(u16::from(matches))
                }
                _ => at(0),
            };
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                budget(destination)
            );
            assert!(runtime.branch.invert_next);
        }
    }
}

#[test]
fn producer_and_observer_share_scene_signals_across_independent_runtime_entries() {
    let catalog = PathCatalog::new(vec![vec![
        Statement::EncounterSignal {
            command: EncounterSignalCommand::Raise(0x8001),
            next: at(1),
        },
        Statement::Control(ControlCommand::WaitOne { next: at(2) }),
        Statement::EncounterSignal {
            command: EncounterSignalCommand::Clear(0x8000),
            next: at(3),
        },
        Statement::Control(ControlCommand::Hold),
        Statement::EncounterSignalBranch {
            condition: EncounterSignalCondition::AnyRaised(0x8000),
            taken: at(5),
            next: at(6),
        },
        Statement::Control(ControlCommand::WaitOne { next: at(4) }),
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    let (mut producer_runtime, mut objects, producer, mut random) = setup();
    let mut observer = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
    observer.base.path = Some(at(4));
    let observer = objects.allocate(observer).unwrap();
    let mut observer_runtime = PathRuntime::default();
    observer_runtime.branch.invert_next = true;
    let mut signals = EncounterSignals { raised: 0x0040 };
    let mut inputs = world(&mut random);
    inputs.encounter_signals = Some(&mut signals);
    for (runtime, actor, expected) in [
        (&mut producer_runtime, producer, ControlStep::Movement),
        (&mut observer_runtime, observer, ControlStep::Movement),
    ] {
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, actor, &mut inputs, 2),
            Ok(ProgramExit {
                actor,
                step: expected
            })
        );
    }
    assert_eq!(inputs.encounter_signals.as_deref().unwrap().raised, 0x8041);
    assert_eq!(
        producer_runtime.enter_program(&catalog, &mut objects, producer, &mut inputs, 2),
        Ok(ProgramExit {
            actor: producer,
            step: ControlStep::Movement
        })
    );
    assert_eq!(
        observer_runtime.enter_program(&catalog, &mut objects, observer, &mut inputs, 2),
        Ok(ProgramExit {
            actor: observer,
            step: ControlStep::Ended
        })
    );
    assert_eq!(signals.raised, 0x0041);
    assert!(observer_runtime.branch.invert_next);
}

#[test]
fn authored_relative_lift_completes_all_loops_waits_for_both_signals_and_restarts() {
    use super::super::{authored_paths as paths, Angle};
    let catalog = paths::catalog();
    for initial_y in [i16::MIN, -31, i16::MAX] {
        for elapsed in [0u8, 49, 50, 51, 255] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(paths::SIGNAL_GATED_RELATIVE_LIFT);
            actor.base.wait_timer = elapsed;
            actor.base.roll = Angle::from_units(90);
            actor.extension.relative_position.y = initial_y;
            actor.base.hit_points = 10;
            actor.base.attack_power = 10;
            let mut signals = EncounterSignals { raised: 0xFFFF };
            let mut inputs = world(&mut random);
            inputs.encounter_signals = Some(&mut signals);
            let base = initial_y.wrapping_sub(10);
            let mut step =
                |expected_y: i16, inputs: &mut PathWorld<'_>, objects: &mut ObjectStore| {
                    assert_eq!(
                        runtime.enter_program(&catalog, objects, owner, inputs, 12),
                        Ok(ProgramExit {
                            actor: owner,
                            step: ControlStep::Movement
                        })
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.extension.relative_position.y, expected_y);
                    assert!(actor.base.flags.collision_disabled);
                    assert!(actor.base.flags.far_sort_bias);
                    assert!(!actor.base.flags.remove_after_tick);
                    assert_eq!((actor.base.hit_points, actor.base.attack_power), (10, 10));
                };
            for _ in 0..50u8.wrapping_sub(elapsed) {
                step(base, &mut inputs, &mut objects);
            }
            for count in 1..=25 {
                step(base.wrapping_sub(count * 10), &mut inputs, &mut objects);
            }
            for _ in 0..3 {
                step(base.wrapping_sub(250), &mut inputs, &mut objects);
            }
            inputs.encounter_signals.as_deref_mut().unwrap().raised &= !0x0100;
            for count in 1..=25 {
                step(
                    base.wrapping_sub(250).wrapping_add(count * 10),
                    &mut inputs,
                    &mut objects,
                );
            }
            for _ in 0..3 {
                step(base, &mut inputs, &mut objects);
            }
            objects.get_mut(owner).unwrap().base.roll = Angle::ZERO;
            for count in 1..=20 {
                step(base.wrapping_add(count * 50), &mut inputs, &mut objects);
            }
            for _ in 0..3 {
                step(base.wrapping_add(1000), &mut inputs, &mut objects);
            }
            inputs.encounter_signals.as_deref_mut().unwrap().raised &= !0x0080;
            for count in 1..=20 {
                step(
                    base.wrapping_add(1000).wrapping_sub(count * 50),
                    &mut inputs,
                    &mut objects,
                );
            }
            assert_eq!(objects.get(owner).unwrap().base.wait_timer, 0);
            step(base, &mut inputs, &mut objects); // restart at WAIT, not the one-time initial offset
            assert_eq!(objects.get(owner).unwrap().base.wait_timer, 1);
            assert_eq!(signals.raised, 0xFE7F);
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}

#[test]
fn authored_signal_mesh_stops_at_frame_twelve_or_loops_sixteen_frames_with_timed_sound() {
    use super::super::{authored_paths as paths, AudioState};
    use super::effect_tests::{audio, callbacks, cues};
    let catalog = paths::catalog();
    for initial_invert in [false, true] {
        for start_raised in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(paths::SIGNAL_GATED_LOOPING_MESH);
            actor.base.wait_timer = 83;
            actor.base.hit_points = 10;
            actor.base.attack_power = 10;
            actor.extension.path_state.motion_phase = 0xFFA5;
            let initial_random = random;
            let mut signals = EncounterSignals::default();
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.encounter_signals = Some(&mut signals);
            inputs.audio = Some(audio(&mut events));
            runtime.branch.invert_next = initial_invert;
            let mut expected_frame = 0u8;
            for visit in 0..192 {
                let raised = (visit / 24 % 2 == 0) == start_raised;
                let mask = if raised { 0xFFFF } else { 0xFFDF };
                inputs.encounter_signals.as_deref_mut().unwrap().raised = mask;
                inputs.animation_clock = visit as u8 ^ 255;
                let inverted = initial_invert && visit == 0;
                if !raised && !((expected_frame == 12) ^ inverted) {
                    expected_frame = expected_frame.wrapping_add(1) & 15;
                }
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16),
                    Ok(ProgramExit {
                        actor: owner,
                        step: ControlStep::Movement
                    })
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(
                    actor.extension.path_state.animation.shape.fixed_frame(),
                    Some(expected_frame)
                );
                assert_eq!(actor.extension.animation_frame, expected_frame);
                assert_eq!(
                    actor.extension.path_state.motion_phase,
                    if raised { 0x01A5 } else { 0x00A5 }
                );
                assert_eq!(actor.base.wait_timer, 83);
                assert!(actor.base.flags.collision_disabled);
                assert!(actor.base.flags.casts_shadow);
                assert!(actor.base.contacts.suppress_contacts_next_epoch);
                assert!(!actor.base.flags.remove_after_tick);
                // The raised route's nonzero-byte test also bypasses IFNOT.
                assert_eq!(runtime.branch.invert_next, raised && inverted);
                assert!(cues(&mut inputs).is_empty());
                assert_eq!(
                    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                    1
                );
                if raised {
                    expected_frame = expected_frame.wrapping_add(1) & 15;
                }
                assert_eq!(
                    objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .animation
                        .shape
                        .fixed_frame(),
                    Some(expected_frame)
                );
                assert_eq!(
                    cues(&mut inputs),
                    if expected_frame == 6 {
                        vec![130]
                    } else {
                        vec![]
                    }
                );
                assert_eq!(inputs.encounter_signals.as_deref().unwrap().raised, mask);
            }
            assert_eq!(random, initial_random);
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}
