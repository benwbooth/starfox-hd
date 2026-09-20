//! Source-derived gate paths; no recorded state or original-machine execution.
use super::super::path_fields::{Axis, ByteField, BytePart, WordField};
use super::super::path_relationships::find_child;
use super::super::path_scene_state::{EncounterCoordination, EncounterHandoff, HandoffCommand};
use super::super::path_spawn::SpawnParameterCommand;
use super::super::{
    authored_paths, Angle, Behavior, ObjectKind, ObjectSpawnDefaults, PathId, ShapeId, Vector3,
};
use super::tests::{setup, world};
use super::*;

#[test]
fn gate_retains_sampled_mode_and_pauses_firing_countdown_outside_range() {
    use super::super::path_target::{PrimaryTarget, TargetAnchor, TargetSelection};
    use super::paired_patrol_tests::callbacks;
    let catalog = authored_paths::catalog();
    for initial_mode in [0, 255] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::FIVE_PART_ENCOUNTER_GATE);
        let selected = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let mut target = TargetSelection {
            distance: u16::MAX,
            ..Default::default()
        };
        let mut shared = EncounterCoordination {
            phase: 72,
            ..Default::default()
        };
        let mut handoff = EncounterHandoff {
            player_flags: 5,
            heading_word: 0xDB7F,
            ..Default::default()
        };
        let before_handoff = handoff;
        let mut expected_random = random;
        let mut inputs = world(&mut random);
        inputs.scene.entry_heading = Some(99);
        inputs.scene.encounter_node_mode = Some(initial_mode);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.selected = Some(selected);
        inputs.primary_player = Some(selected);
        inputs.primary_target = Some(PrimaryTarget {
            anchor: TargetAnchor {
                position: Vector3 {
                    z: -1000,
                    ..Default::default()
                },
                pitch: 0,
                yaw: 0,
            },
            selection: &mut target,
        });
        inputs.coordination = Some(&mut shared);
        inputs.handoff = Some(&mut handoff);
        let mut countdown = 0;
        let mut shots = 0;
        for visit in 1..=150 {
            let in_range = !(21..=40).contains(&visit);
            objects.get_mut(selected).unwrap().base.position.z =
                if in_range { 10000 } else { 16000 };
            if visit > 1 {
                callbacks(
                    &mut runtime,
                    &catalog,
                    &mut objects,
                    owner,
                    &mut inputs,
                    visit,
                );
                // The entry heading and node mode are sampled once, even
                // though callback target selection keeps observing live state.
                inputs.scene.entry_heading = None;
                inputs.scene.encounter_node_mode = Some(initial_mode ^ 255);
            }
            if in_range && initial_mode == 0 {
                if countdown == 0 {
                    shots += 1;
                    countdown = (expected_random.next_byte() & 31) + 25;
                } else {
                    countdown -= 1;
                }
            }
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 160)
                    .unwrap(),
                ProgramExit {
                    actor: owner,
                    step: ControlStep::Movement
                }
            );
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase,
                u16::from(initial_mode) * 256 + u16::from(countdown),
                "visit {visit}"
            );
            assert_eq!(objects.get(owner).unwrap().base.yaw, Angle::from_units(99));
            assert_eq!(
                objects
                    .active_objects()
                    .filter(|(_, a)| a.base.kind == ObjectKind::Projectile)
                    .count(),
                shots
            );
            assert_eq!(*inputs.random, expected_random);
            assert_eq!(inputs.handoff.as_deref(), Some(&before_handoff));
            assert_eq!(inputs.coordination.as_deref().unwrap().phase, 72);
            assert_eq!(runtime.spawns.parameter, Some(5));
        }
        assert_eq!(
            find_child(&objects, owner, 6).unwrap().is_some(),
            initial_mode != 0
        );
        objects.get_mut(selected).unwrap().base.position.z = 5999;
        callbacks(
            &mut runtime,
            &catalog,
            &mut objects,
            owner,
            &mut inputs,
            151,
        );
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 160)
                .unwrap(),
            ProgramExit {
                actor: owner,
                step: ControlStep::Ended
            }
        );
        assert_eq!(
            inputs.handoff.as_deref(),
            Some(&EncounterHandoff {
                player_flags: 69,
                heading_word: 0xDB63,
                ..Default::default()
            })
        );
        assert_eq!(inputs.coordination.as_deref().unwrap().phase, 73);
        assert_eq!(*inputs.random, expected_random);
    }
}

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

fn handoff_leaf(command: HandoffCommand) -> PathCatalog {
    PathCatalog::new(vec![vec![Statement::EncounterHandoff {
        command,
        next: at(1),
    }]])
    .unwrap()
}

#[test]
fn handoff_requires_publication_storage_before_any_mutation() {
    for command in [
        HandoffCommand::Request,
        HandoffCommand::StoreX(WordOperand::Actor(WordField::Position(Axis::X))),
        HandoffCommand::StoreZ(WordOperand::Actor(WordField::Position(Axis::Z))),
        HandoffCommand::StoreHeading(ByteOperand::Actor(ByteField::Rotation(Axis::Y))),
    ] {
        for invert in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let before = objects.clone();
            let original_random = random;
            runtime.branch.invert_next = invert;
            let catalog = handoff_leaf(command);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(0),
                    executed: 0
                })
            );
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingEncounterHandoff)
            );
            assert_eq!(objects, before);
            assert_eq!(random, original_random);
            assert_eq!(runtime.branch.invert_next, invert);
        }
    }
}

#[test]
fn handoff_retains_other_flags_and_heading_companion_for_all_byte_pairs() {
    for heading in [false, true] {
        let catalog = handoff_leaf(if heading {
            HandoffCommand::StoreHeading(ByteOperand::Actor(ByteField::Rotation(Axis::Y)))
        } else {
            HandoffCommand::Request
        });
        let (mut runtime, mut objects, owner, mut random) = setup();
        let original_random = random;
        for retained in 0..=u8::MAX {
            for value in 0..=u8::MAX {
                objects.get_mut(owner).unwrap().base.path = Some(at(0));
                objects.get_mut(owner).unwrap().base.yaw = Angle::from_units(value);
                let mut expected_objects = objects.clone();
                expected_objects.get_mut(owner).unwrap().base.path = Some(at(1));
                let mut handoff = EncounterHandoff {
                    player_flags: retained,
                    x: -317,
                    z: 79,
                    heading_word: u16::from(retained) * 256 + u16::from(value ^ 255),
                };
                let mut expected = handoff;
                if heading {
                    expected.heading_word = u16::from(retained) * 256 + u16::from(value);
                } else {
                    expected.player_flags = retained | 64;
                }
                let mut inputs = world(&mut random);
                inputs.handoff = Some(&mut handoff);
                runtime.branch.invert_next = value & 1 != 0;
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(1),
                        executed: 1
                    })
                );
                assert_eq!(handoff, expected);
                assert_eq!(objects, expected_objects);
                assert_eq!(runtime.branch.invert_next, value & 1 != 0);
            }
        }
        assert_eq!(random, original_random);
    }
}

#[test]
fn handoff_copies_each_complete_signed_coordinate_without_publishing_height() {
    for axis in [Axis::X, Axis::Z] {
        let operand = WordOperand::Actor(WordField::Position(axis));
        let catalog = handoff_leaf(if axis == Axis::X {
            HandoffCommand::StoreX(operand)
        } else {
            HandoffCommand::StoreZ(operand)
        });
        let (mut runtime, mut objects, owner, mut random) = setup();
        let original_random = random;
        for bits in 0..=u16::MAX {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(at(0));
            actor.base.position = Vector3 {
                x: bits as i16,
                y: -999,
                z: (bits ^ 65535) as i16,
            };
            let mut expected_objects = objects.clone();
            expected_objects.get_mut(owner).unwrap().base.path = Some(at(1));
            let mut handoff = EncounterHandoff {
                player_flags: 183,
                x: 719,
                z: -37,
                heading_word: 0xD543,
            };
            let mut expected = handoff;
            if axis == Axis::X {
                expected.x = bits as i16;
            } else {
                expected.z = (bits ^ 65535) as i16;
            }
            let mut inputs = world(&mut random);
            inputs.handoff = Some(&mut handoff);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
            assert_eq!(handoff, expected);
            assert_eq!(objects, expected_objects);
        }
        assert_eq!(random, original_random);
    }
}

#[test]
fn spawn_parameter_increment_requires_input_wraps_and_preserves_last_spawn() {
    let catalog = PathCatalog::new(vec![vec![Statement::SpawnParameter {
        command: SpawnParameterCommand::Increment,
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    runtime.spawns.last_spawn = Some(owner);
    let before = objects.clone();
    let original_random = random;
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::MissingSpawnParameter)
    );
    assert_eq!(objects, before);
    for value in 0..=u8::MAX {
        objects.get_mut(owner).unwrap().base.path = Some(at(0));
        let mut expected = objects.clone();
        expected.get_mut(owner).unwrap().base.path = Some(at(1));
        runtime.spawns.parameter = Some(value);
        runtime.branch.invert_next = true;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::BudgetExceeded {
                cursor: at(1),
                executed: 1
            })
        );
        assert_eq!(runtime.spawns.parameter, Some(value.wrapping_add(1)));
        assert_eq!(runtime.spawns.last_spawn, Some(owner));
        assert_eq!(objects, expected);
        assert!(runtime.branch.invert_next);
    }
    assert_eq!(random, original_random);
}

#[test]
fn five_part_shape_selection_is_bounded_and_does_not_reset_other_actor_state() {
    let source = authored_paths::catalog();
    let shapes = (0..authored_paths::LOWERED_COMMAND_COUNT as u16)
        .find_map(|i| match source.statement(at(i)).unwrap() {
            Statement::SelectShape { shapes, .. } if shapes.len() == 5 => Some(shapes),
            _ => None,
        })
        .unwrap();
    let selector = ByteField::WordPart {
        field: WordField::MotionPhase,
        part: BytePart::Low,
    };
    let catalog = PathCatalog::new(vec![vec![Statement::SelectShape {
        selector,
        shapes,
        next: at(1),
    }]])
    .unwrap();
    for value in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xA500 | u16::from(value);
        let mut expected = objects.clone();
        let original_random = random;
        runtime.branch.invert_next = true;
        let result =
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1);
        if value < 5 {
            expected.get_mut(owner).unwrap().base.shape =
                ShapeId::from_catalog_index([390, 384, 387, 393, 123][usize::from(value)]);
            expected.get_mut(owner).unwrap().base.path = Some(at(1));
            assert_eq!(
                result,
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
        } else {
            assert_eq!(
                result,
                Err(ProgramError::ShapeSelectionOutOfBounds {
                    index: value,
                    count: 5
                })
            );
        }
        assert_eq!(objects, expected);
        assert_eq!(random, original_random);
        assert!(runtime.branch.invert_next);
    }
}

#[test]
fn gate_builds_all_five_parts_in_one_visit_and_nonzero_mode_adds_attachments() {
    let catalog = authored_paths::catalog();
    for mode in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::FIVE_PART_ENCOUNTER_GATE);
        actor.base.position = Vector3 {
            x: -301,
            y: 991,
            z: 501,
        };
        actor.extension.path_state.motion_phase = 0xAA07;
        let origin = actor.base.position;
        let selected = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        objects.get_mut(selected).unwrap().base.position = Vector3 {
            z: origin.z + 16000,
            ..origin
        };
        let mut selected_before = objects.get(selected).unwrap().clone();
        let original_random = random;
        let mut inputs = world(&mut random);
        inputs.scene.entry_heading = Some(137);
        inputs.scene.encounter_node_mode = Some(mode);
        inputs.selected = Some(selected);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        let exit = runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 150)
            .unwrap();
        assert_eq!(
            exit,
            ProgramExit {
                actor: owner,
                step: ControlStep::Movement
            }
        );
        assert_eq!(objects.len(), if mode == 0 { 7 } else { 9 });
        assert_eq!(runtime.spawns.parameter, Some(5));
        for (index, (shape, z)) in [
            (390, -800),
            (384, 3840),
            (387, 1056),
            (393, -2880),
            (123, -1280),
        ]
        .into_iter()
        .enumerate()
        {
            let part = find_child(&objects, owner, index as u8 + 1)
                .unwrap()
                .unwrap();
            let actor = objects.get(part).unwrap();
            assert_eq!(actor.base.shape, ShapeId::from_catalog_index(shape));
            assert_eq!(
                actor.extension.relative_position,
                Vector3 {
                    x: 0,
                    y: if index == 4 { -1760 } else { 0 },
                    z
                }
            );
            assert_eq!(
                actor.extension.path_state.motion_phase as u8,
                index as u8 + 1
            );
            assert_eq!(actor.base.path, Some(authored_paths::BIASED_DEPTH_HOLD));
            assert_eq!((actor.base.hit_points, actor.base.attack_power), (1, 1));
            // Child publication belongs to the common movement boundary,
            // not the path spawn or borrowed-actor field writes.
            assert_eq!(actor.base.position, Vector3::default());
        }
        for (number, shape, position) in [
            (
                6,
                65,
                Vector3 {
                    x: 0,
                    y: -480,
                    z: 3840,
                },
            ),
            (
                7,
                18,
                Vector3 {
                    x: 0,
                    y: -960,
                    z: 5440,
                },
            ),
        ] {
            let child = find_child(&objects, owner, number).unwrap();
            assert_eq!(child.is_some(), mode != 0);
            if let Some(child) = child {
                let actor = objects.get(child).unwrap();
                assert_eq!(actor.base.shape, ShapeId::from_catalog_index(shape));
                assert_eq!(actor.extension.relative_position, position);
                assert_eq!(actor.base.attack_power, if number == 6 { 2 } else { 1 });
            }
        }
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.shape, ShapeId::EMPTY);
        assert_eq!((actor.base.hit_points, actor.base.attack_power), (100, 4));
        assert_eq!(actor.base.yaw, Angle::from_units(137));
        assert_eq!(
            actor.extension.path_state.motion_phase,
            u16::from(mode) * 256 + 7
        );
        selected_before.base.previous = objects.get(selected).unwrap().base.previous;
        assert_eq!(objects.get(selected), Some(&selected_before));
        assert_eq!(random, original_random);
    }
}

#[test]
fn gate_firing_and_handoff_obey_both_strict_distance_boundaries() {
    let catalog = authored_paths::catalog();
    for mode in [0, 1, 255] {
        for distance in [0, 5999, 6000, 6001, 14999, 15000, 15001] {
            for phase in [0, 1, 255] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::FIVE_PART_ENCOUNTER_GATE);
                actor.base.position = Vector3 {
                    x: -30,
                    y: -317,
                    z: 17,
                };
                actor.extension.path_state.motion_phase = phase;
                let position = actor.base.position;
                let selected = objects
                    .allocate(Object::new(
                        ObjectKind::Player,
                        ShapeId::EMPTY,
                        Behavior::PlayerFlight,
                    ))
                    .unwrap();
                objects.get_mut(selected).unwrap().base.position = Vector3 {
                    z: 17 + distance,
                    ..position
                };
                let mut shared = EncounterCoordination {
                    phase: 255,
                    ..Default::default()
                };
                let mut handoff = EncounterHandoff {
                    player_flags: 0x95,
                    x: 731,
                    z: -431,
                    heading_word: 0xB257,
                };
                let before_handoff = handoff;
                let mut expected_random = random;
                let firing = (6000..15000).contains(&distance) && mode == 0 && phase == 0;
                let expected_phase = if firing {
                    u16::from((expected_random.next_byte() & 31) + 25)
                } else if (6000..15000).contains(&distance) && mode == 0 {
                    phase - 1
                } else {
                    phase
                };
                let mut inputs = world(&mut random);
                inputs.scene.entry_heading = Some(177);
                inputs.scene.encounter_node_mode = Some(mode);
                inputs.selected = Some(selected);
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                inputs.coordination = Some(&mut shared);
                inputs.handoff = Some(&mut handoff);
                let exit = runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 160)
                    .unwrap();
                assert_eq!(
                    exit.step,
                    if distance < 6000 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                assert_eq!(exit.actor, owner);
                assert_eq!(shared.phase, if distance < 6000 { 0 } else { 255 });
                assert_eq!(
                    handoff,
                    if distance < 6000 {
                        EncounterHandoff {
                            player_flags: 0xD5,
                            x: position.x,
                            z: position.z,
                            heading_word: 0xB2B1,
                        }
                    } else {
                        before_handoff
                    }
                );
                let shots: Vec<_> = objects
                    .active_objects()
                    .filter(|(_, actor)| actor.base.kind == ObjectKind::Projectile)
                    .collect();
                assert_eq!(shots.len(), usize::from(firing));
                if firing {
                    let shot = shots[0].1;
                    assert_eq!(
                        shot.base.path,
                        Some(authored_paths::DISTANCE_AIMED_PROJECTILE)
                    );
                    assert_eq!(shot.base.shape, ShapeId::from_catalog_index(396));
                    assert_eq!(shot.base.position, position);
                    assert_eq!((shot.base.hit_points, shot.base.attack_power), (100, 8));
                }
                assert_eq!(
                    objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .motion_phase,
                    u16::from(mode) * 256 + expected_phase
                );
                assert_eq!(random, expected_random);
            }
        }
    }
}
