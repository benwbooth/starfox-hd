//! Authored action-stage gate and allocation-group field contracts.
use super::super::{Behavior, ObjectKind, ObjectSpawnDefaults, PathId, ShapeId};
use super::tests::{setup, world};
use super::*;

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

#[test]
fn literal_gate_comparisons_cover_every_byte_pair_and_leave_ifnot_and_wait_intact() {
    for expected in 0..=u8::MAX {
        for equal in [false, true] {
            let condition = if equal {
                ActionGateCondition::Equal(expected)
            } else {
                ActionGateCondition::NotEqual(expected)
            };
            let catalog = PathCatalog::new(vec![vec![Statement::ActionGateBranch {
                condition,
                taken: at(1),
                next: at(2),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.wait_timer = 137;
            actor.base.attack_power = expected ^ 255;
            let original = actor.clone();
            let initial_random = random;
            let mut gate = ActionGate::default();
            let mut inputs = world(&mut random);
            inputs.action_gate = Some(&mut gate);
            for actual in 0..=u8::MAX {
                for inverted in [false, true] {
                    *objects.get_mut(owner).unwrap() = original.clone();
                    runtime.branch.invert_next = inverted;
                    inputs.action_gate.as_deref_mut().unwrap().code = actual;
                    let destination = at(if (actual == expected) == equal { 1 } else { 2 });
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: destination,
                            executed: 1
                        })
                    );
                    let mut expected_actor = original.clone();
                    expected_actor.base.path = Some(destination);
                    assert_eq!(objects.get(owner).unwrap(), &expected_actor);
                    assert_eq!(inputs.action_gate.as_deref().unwrap().code, actual);
                    assert_eq!(runtime.branch.invert_next, inverted);
                }
            }
            assert_eq!(random, initial_random);
        }
    }
}

#[test]
fn gate_publication_and_import_share_one_live_byte_across_actors() {
    use super::super::path_fields::ByteField;
    for value in 0..=u8::MAX {
        let catalog = PathCatalog::new(vec![vec![
            Statement::SetActionGate { value, next: at(1) },
            Statement::Control(ControlCommand::WaitOne { next: at(2) }),
            Statement::SetActionGate {
                value: 0,
                next: at(3),
            },
            Statement::Control(ControlCommand::Hold),
            Statement::ImportActionGate {
                destination: ByteField::AttackPower,
                next: at(5),
            },
            Statement::Control(ControlCommand::WaitOne { next: at(4) }),
        ]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let mut reader = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath);
        reader.base.path = Some(at(4));
        reader.base.wait_timer = 207;
        let reader = objects.allocate(reader).unwrap();
        let mut gate = ActionGate { code: !value };
        let mut inputs = world(&mut random);
        inputs.action_gate = Some(&mut gate);
        runtime.branch.invert_next = true;
        for (actor, expected_value) in [(owner, value), (reader, value), (owner, 0), (reader, 0)] {
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, actor, &mut inputs, 2),
                Ok(ProgramExit {
                    actor,
                    step: ControlStep::Movement
                })
            );
            assert_eq!(inputs.action_gate.as_deref().unwrap().code, expected_value);
            if actor == reader {
                assert_eq!(
                    objects.get(reader).unwrap().base.attack_power,
                    expected_value
                );
            }
            assert_eq!(objects.get(reader).unwrap().base.wait_timer, 207);
            assert!(runtime.branch.invert_next);
        }
    }
}

#[test]
fn missing_gate_faults_before_writes_and_resumes_against_changed_scene_value() {
    use super::super::path_fields::ByteField;
    for statement in [
        Statement::SetActionGate {
            value: 0,
            next: at(1),
        },
        Statement::SetActionGate {
            value: 255,
            next: at(1),
        },
        Statement::ImportActionGate {
            destination: ByteField::SpawnGroup,
            next: at(1),
        },
        Statement::ActionGateBranch {
            condition: ActionGateCondition::Equal(129),
            taken: at(1),
            next: at(0),
        },
        Statement::ActionGateBranch {
            condition: ActionGateCondition::NotEqual(129),
            taken: at(1),
            next: at(0),
        },
    ] {
        let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        runtime.branch.invert_next = true;
        let original = objects.clone();
        for _ in 0..2 {
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingActionGate)
            );
            assert_eq!(objects, original);
            assert!(runtime.branch.invert_next);
        }
        for value in [129, 0, 255, 129] {
            objects.get_mut(owner).unwrap().base.path = Some(at(0));
            let mut gate = ActionGate { code: value };
            let mut inputs = world(&mut random);
            inputs.action_gate = Some(&mut gate);
            let destination = match statement {
                Statement::ActionGateBranch { condition, .. } => at(u16::from(match condition {
                    ActionGateCondition::Equal(expected) => expected == value,
                    ActionGateCondition::NotEqual(expected) => expected != value,
                })),
                _ => at(1),
            };
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: destination,
                    executed: 1
                })
            );
            assert!(runtime.branch.invert_next);
        }
    }
}

#[test]
fn path_written_group_is_the_same_byte_inherited_by_new_actors() {
    use super::super::path_fields::{ByteField, ByteOperation};
    use super::super::path_spawn::IndependentSpawn;
    for value in 0..=u8::MAX {
        let catalog = PathCatalog::new(vec![vec![
            Statement::Mutate {
                mutation: Mutation::Byte {
                    field: ByteField::SpawnGroup,
                    operation: ByteOperation::Assign(ByteOperand::Literal(value)),
                },
                next: at(1),
            },
            Statement::SpawnIndependent {
                kind: ObjectKind::Effect,
                parameters: IndependentSpawn {
                    shape: ShapeId::EMPTY,
                    path: None,
                    hit_points: 10,
                    attack_power: 0,
                },
                next: at(2),
            },
            Statement::Control(ControlCommand::WaitOne { next: at(3) }),
        ]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let initial = objects.get(owner).unwrap().clone();
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults {
            run_when_paused: false,
            group: !value,
        });
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 3),
            Ok(ProgramExit {
                actor: owner,
                step: ControlStep::Movement
            })
        );
        let child = runtime.spawns.last_spawn.unwrap();
        assert_eq!(objects.get(child).unwrap().extension.spawn_group, value);
        let mut expected = initial;
        expected.extension.spawn_group = value;
        expected.base.next = Some(child);
        expected.base.path = Some(at(3));
        assert_eq!(objects.get(owner).unwrap(), &expected);
    }
}

#[test]
fn authored_sound_hold_samples_gate_once_and_retains_both_scheduled_callbacks() {
    use super::super::{authored_paths as paths, AudioState};
    use super::effect_tests::{audio, callbacks, cues};
    let catalog = paths::catalog();
    for value in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(paths::ACTION_GATED_SOUND_HOLD);
        actor.base.wait_timer = 231;
        let mut gate = ActionGate { code: value };
        let mut events = AudioState::default();
        let mut inputs = world(&mut random);
        inputs.action_gate = Some(&mut gate);
        inputs.audio = Some(audio(&mut events));
        runtime.branch.invert_next = true;
        for visit in 0..4 {
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8),
                Ok(ProgramExit {
                    actor: owner,
                    step: ControlStep::Movement
                })
            );
            let actor = objects.get(owner).unwrap();
            assert!(actor.extension.path_state.hold_latched);
            assert_eq!(actor.extension.depth_offset, 3);
            assert!(actor.base.flags.far_sort_bias);
            assert!(actor.base.flags.collision_disabled);
            assert_eq!(actor.base.wait_timer, 231);
            assert_eq!(
                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                if value == 0 { 2 } else { 1 }
            );
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .animation
                    .shape
                    .fixed_frame(),
                Some(0)
            );
            assert_eq!(
                cues(&mut inputs),
                if value == 0 { vec![107] } else { vec![] }
            );
            assert!(runtime.branch.invert_next);
            inputs.action_gate.as_deref_mut().unwrap().code =
                if visit & 1 == 0 { !value } else { value };
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }
}

#[test]
fn authored_animated_retirement_preserves_both_waits_and_resamples_final_action_gate() {
    use super::super::{authored_paths as paths, AudioState};
    use super::effect_tests::{audio, cues};
    let catalog = paths::catalog();
    for initial_wait in [0u8, 39, 40, 41, 255] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(paths::ACTION_GATED_ANIMATED_RETIREMENT);
        actor.base.wait_timer = initial_wait;
        actor.base.hit_points = 1;
        actor.base.attack_power = 1;
        actor.extension.spawn_group = 17;
        let mut gate = ActionGate { code: 129 };
        let mut events = AudioState::default();
        let mut inputs = world(&mut random);
        inputs.action_gate = Some(&mut gate);
        inputs.audio = Some(audio(&mut events));
        runtime.branch.invert_next = true;
        let frames: Vec<_> = std::iter::repeat_n(0u8, usize::from(40u8.wrapping_sub(initial_wait)))
            .chain(1..=9)
            .chain(std::iter::repeat_n(10, 13 + 3))
            .collect();
        for (visit, frame) in frames.into_iter().enumerate() {
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 12),
                Ok(ProgramExit {
                    actor: owner,
                    step: ControlStep::Movement
                })
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.extension.spawn_group, 255);
            assert_eq!(
                actor.extension.path_state.animation.shape.fixed_frame(),
                Some(frame),
                "visit={visit}"
            );
            assert_eq!((actor.base.hit_points, actor.base.attack_power), (1, 1));
            assert!(actor.base.flags.far_sort_bias);
            assert!(actor.base.flags.collision_disabled);
            assert!(!actor.base.flags.remove_after_tick);
            assert!(runtime.branch.invert_next);
            assert_eq!(
                cues(&mut inputs),
                if frame == 1 { vec![146] } else { vec![] }
            );
        }
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 0);
        inputs.action_gate.as_deref_mut().unwrap().code = 128;
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 2),
            Ok(ProgramExit {
                actor: owner,
                step: ControlStep::Ended
            })
        );
        assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
        assert!(runtime.branch.invert_next);
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }
}

#[test]
fn authored_spin_rise_runs_all_motion_waits_and_expires_random_audio_after_120_callbacks() {
    use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
    use super::super::{
        authored_paths as paths, path_motion::PlayerDisplacement, Angle, AudioState,
    };
    use super::effect_tests::{audio, cues};
    let catalog = paths::catalog();
    for elapsed in [0u8, 39, 40, 41, 255] {
        for seed in [0u8, 127, 255] {
            let (mut runtime, mut objects, owner, _) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(paths::TIMED_SPIN_RISE_EFFECT);
            actor.base.position.y = 32760;
            actor.base.wait_timer = elapsed;
            actor.base.hit_points = 1;
            actor.base.attack_power = 1;
            actor.base.yaw = Angle::from_units(247);
            actor.extension.relative_rotation.yaw = Angle::from_units(250);
            actor.extension.relative_rotation.pitch = Angle::from_units(251);
            actor.extension.path_state.motion_phase = 0xABC7;
            let mut random = RandomState::new([1, 17, 83, seed]);
            let mut expected_random = random;
            // Relative yaw, relative pitch, vertical velocity, WAIT, main cue.
            let mut states = Vec::new();
            for n in 1..16 {
                states.push((250u8.wrapping_add(n), 251, 0i16, elapsed, false));
            }
            for n in 1..=40u8.wrapping_sub(elapsed) {
                states.push((10, 251, 0, elapsed.wrapping_add(n), false));
            }
            for n in 1..16u8 {
                states.push((10, 251u8.wrapping_add(n), i16::from(n) * 7, 0, n == 1));
            }
            for n in 1..=14 {
                states.push((10, 11, 112, n, false));
            }
            states.push((10, 11, 112, 0, false));
            let mut expected_y = 32460i16;
            let mut expected_yaw = 247u8;
            let total = states.len();
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.audio = Some(audio(&mut events));
            for (visit, (relative_yaw, relative_pitch, velocity, wait, main_cue)) in
                states.into_iter().enumerate()
            {
                let ending = visit + 1 == total;
                for marker in &mut inputs
                    .audio
                    .as_mut()
                    .unwrap()
                    .markers
                    .as_mut()
                    .unwrap()
                    .markers
                {
                    marker.position = objects.get(owner).unwrap().base.position;
                }
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 24),
                    Ok(ProgramExit {
                        actor: owner,
                        step: if ending {
                            ControlStep::Ended
                        } else {
                            ControlStep::Movement
                        }
                    })
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.position.y, expected_y, "visit={visit}");
                assert_eq!(actor.extension.spawn_group, 255);
                assert_eq!(actor.extension.relative_rotation.yaw.units(), relative_yaw);
                assert_eq!(
                    actor.extension.relative_rotation.pitch.units(),
                    relative_pitch
                );
                assert_eq!(actor.base.velocity.y, velocity);
                assert_eq!(actor.base.wait_timer, wait);
                assert_eq!(actor.extension.path_state.motion_phase, 0xAB00);
                assert_eq!(cues(&mut inputs), if main_cue { vec![178] } else { vec![] });
                if ending {
                    break;
                }
                assert!(runtime
                    .begin_movement(&mut objects, owner, PlayerDisplacement::default())
                    .unwrap());
                expected_y = expected_y.wrapping_add(velocity);
                let eligible = visit < 120 && visit % 2 == 0;
                let expected_sound = if eligible && expected_random.next_byte() >= 127 {
                    Some(if expected_random.next_byte() < 127 {
                        139
                    } else {
                        112
                    })
                } else {
                    None
                };
                let mut count = 0;
                let mut complete = false;
                for _ in 0..4 {
                    match runtime
                        .step_callbacks(
                            &mut objects,
                            owner,
                            TriggerWorldInputs {
                                strategy_tick: visit as u8,
                                ..Default::default()
                            },
                        )
                        .unwrap()
                    {
                        CallbackStep::Run(_) => {
                            count += 1;
                            assert_eq!(
                                runtime.resume_program(
                                    &catalog,
                                    &mut objects,
                                    owner,
                                    &mut inputs,
                                    8
                                ),
                                Ok(ProgramExit {
                                    actor: owner,
                                    step: ControlStep::ResumeCallbacks
                                })
                            );
                        }
                        CallbackStep::Skipped | CallbackStep::Expired => {}
                        CallbackStep::Complete => {
                            complete = true;
                            break;
                        }
                    }
                }
                assert!(complete);
                // Source removal reduces the active pass budget as well as
                // the subsequent advance: expiring the first of two entries
                // skips the surviving yaw callback for this one pass only.
                let yaw_callback = visit != 120;
                assert_eq!(
                    count,
                    usize::from(yaw_callback) + usize::from(eligible),
                    "elapsed={elapsed} seed={seed} visit={visit}"
                );
                if yaw_callback {
                    expected_yaw = expected_yaw.wrapping_add(relative_yaw);
                }
                runtime
                    .finish_movement(&mut objects, &mut [None; 2])
                    .unwrap();
                assert_eq!(objects.get(owner).unwrap().base.yaw.units(), expected_yaw);
                assert_eq!(objects.get(owner).unwrap().base.position.y, expected_y);
                assert_eq!(
                    cues(&mut inputs),
                    expected_sound.into_iter().collect::<Vec<_>>()
                );
                assert_eq!(*inputs.random, expected_random);
            }
            assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}
