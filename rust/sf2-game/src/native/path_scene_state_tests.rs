use super::super::path_fields::{ByteField, BytePart, WordField};
use super::super::path_scene_state::*;
use super::super::{authored_paths, PathId};
use super::tests::{setup, world};
use super::*;

const FIELDS: [CoordinationField; 5] = [
    CoordinationField::Progress,
    CoordinationField::SecondaryProgress,
    CoordinationField::CompletedParts,
    CoordinationField::ActiveMessages,
    CoordinationField::Handshake,
];

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn state(value: u8) -> EncounterCoordination {
    EncounterCoordination {
        progress: value,
        secondary_progress: value,
        completed_parts: value,
        active_messages: value,
        handshake: value,
    }
}

fn replace_field(state: &mut EncounterCoordination, field: CoordinationField, value: u8) {
    match field {
        CoordinationField::Progress => state.progress = value,
        CoordinationField::SecondaryProgress => state.secondary_progress = value,
        CoordinationField::CompletedParts => state.completed_parts = value,
        CoordinationField::ActiveMessages => state.active_messages = value,
        CoordinationField::Handshake => state.handshake = value,
    }
}

#[test]
fn coordination_commands_preserve_other_fields_actor_state_ifnot_and_randomness() {
    let field = ByteField::WordPart {
        field: WordField::MotionPhase,
        part: BytePart::Low,
    };
    for selected in FIELDS {
        for command in [
            CoordinationCommand::CopyTo(field),
            CoordinationCommand::Assign(ByteOperand::Actor(field)),
            CoordinationCommand::Assign(ByteOperand::Literal(255)),
            CoordinationCommand::Increment,
            CoordinationCommand::Decrement,
        ] {
            let catalog = PathCatalog::new(vec![vec![Statement::Coordination {
                field: selected,
                command,
                next: at(1),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            let initial = objects.clone();
            let before_random = random;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingCoordination)
            );
            assert_eq!(objects, initial);
            for value in 0..=u8::MAX {
                for invert in [false, true] {
                    objects = initial.clone();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.extension.path_state.motion_phase = 0xA500 | u16::from(value ^ 0xFF);
                    actor.base.wait_timer = 73;
                    let mut expected_objects = objects.clone();
                    let expected_actor = expected_objects.get_mut(owner).unwrap();
                    expected_actor.base.path = Some(at(1));
                    let mut shared = state(value);
                    let mut expected_shared = shared;
                    let updated = match command {
                        CoordinationCommand::CopyTo(_) => {
                            expected_actor.extension.path_state.motion_phase =
                                0xA500 | u16::from(value);
                            value
                        }
                        CoordinationCommand::Assign(ByteOperand::Actor(_)) => value ^ 0xFF,
                        CoordinationCommand::Assign(_) => 255,
                        CoordinationCommand::Increment => ((u16::from(value) + 1) % 256) as u8,
                        CoordinationCommand::Decrement => ((u16::from(value) + 255) % 256) as u8,
                    };
                    replace_field(&mut expected_shared, selected, updated);
                    runtime.branch.invert_next = invert;
                    let mut inputs = world(&mut random);
                    inputs.coordination = Some(&mut shared);
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: at(1),
                            executed: 1
                        })
                    );
                    assert_eq!(objects, expected_objects);
                    assert_eq!(shared, expected_shared);
                    assert_eq!(runtime.branch.invert_next, invert);
                }
            }
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn sound_bank_request_is_a_retained_byte_not_an_immediate_audio_cue() {
    let catalog = PathCatalog::new(vec![vec![Statement::RequestSoundBank {
        selection: ByteOperand::Actor(ByteField::Part),
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let initial = objects.clone();
    let before_random = random;
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::MissingSoundBankRequest)
    );
    assert_eq!(objects, initial);
    for value in 0..=u8::MAX {
        objects = initial.clone();
        objects.get_mut(owner).unwrap().extension.path_state.part = value;
        let mut expected = objects.clone();
        expected.get_mut(owner).unwrap().base.path = Some(at(1));
        let mut request = SoundBankRequest {
            selection: value ^ 0xFF,
        };
        runtime.branch.invert_next = true;
        let mut inputs = world(&mut random);
        inputs.sound_bank_request = Some(&mut request);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::BudgetExceeded {
                cursor: at(1),
                executed: 1
            })
        );
        assert_eq!(request.selection, value);
        assert_eq!(objects, expected);
        assert!(runtime.branch.invert_next);
    }
    assert_eq!(random, before_random);
}

#[test]
fn clear_global_path_latches_uses_all_selector_bytes_without_touching_encounter_signals() {
    // Read the decoded source mask table from the complete reset path; no
    // processor program or source address resolution participates in this test.
    let authored = authored_paths::catalog();
    let mut cursor = authored_paths::SCENE_COORDINATION_RESET;
    let masks = loop {
        match authored.statement(cursor).unwrap() {
            Statement::ClearPathLatches {
                mask: WordOperand::IndexedBitMask { masks, .. },
                ..
            } => break masks,
            _ => cursor.command_index += 1,
        }
    };
    let catalog = PathCatalog::new(vec![vec![Statement::ClearPathLatches {
        mask: WordOperand::IndexedBitMask {
            selector: ByteOperand::Actor(ByteField::Part),
            masks,
        },
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let initial = objects.clone();
    let before_random = random;
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::MissingPathLatches)
    );
    assert_eq!(objects, initial);
    for selector in 0..=u8::MAX {
        for value in [0, 0xFFFF, 0xA55A, 1, 0x8000] {
            objects = initial.clone();
            objects.get_mut(owner).unwrap().extension.path_state.part = selector;
            let mut expected = objects.clone();
            expected.get_mut(owner).unwrap().base.path = Some(at(1));
            let mut latches = PathLatches { raised: value };
            let mut signals = EncounterSignals { raised: 0xA5C3 };
            runtime.branch.invert_next = true;
            let mut inputs = world(&mut random);
            inputs.path_latches = Some(&mut latches);
            inputs.encounter_signals = Some(&mut signals);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
            let index = (usize::from(selector) + 127) % 128;
            assert_eq!(latches.raised, value & !masks[index]);
            assert_eq!(signals.raised, 0xA5C3);
            assert_eq!(objects, expected);
            assert!(runtime.branch.invert_next);
        }
    }
    assert_eq!(random, before_random);
}

#[test]
fn complete_scene_reset_copies_initial_values_then_clears_sixteen_masks_without_yielding() {
    use super::super::path_countdown::PathCountdown;
    let catalog = authored_paths::catalog();
    let entry = authored_paths::SCENE_COORDINATION_RESET;
    let end = PathCursor {
        command_index: entry.command_index + 14,
        ..entry
    };
    assert!(matches!(
        catalog.statement(end),
        Ok(Statement::Control(ControlCommand::End))
    ));
    let masks = match catalog
        .statement(PathCursor {
            command_index: entry.command_index + 12,
            ..entry
        })
        .unwrap()
    {
        Statement::ClearPathLatches {
            mask: WordOperand::IndexedBitMask { masks, .. },
            ..
        } => masks,
        _ => panic!("complete reset contains its decoded mask lookup"),
    };
    for value in 0..=u8::MAX {
        for invert in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let before_random = random;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(entry);
            actor.extension.path_state.motion_phase = 0xA500 | u16::from(value);
            actor.extension.path_state.script_value = 0xB319;
            actor.base.wait_timer = 79;
            actor.extension.path_state.repeat_counter = 163;
            let mut expected = objects.clone();
            let actor = expected.get_mut(owner).unwrap();
            actor.base.path = Some(end);
            actor.base.flags.visible = false;
            actor.base.flags.collision_disabled = true;
            actor.base.flags.remove_after_tick = true;
            actor.extension.path_state.motion_phase = 0xA500 | u16::from(value.wrapping_add(16));
            // DO allocates stack storage; completing the loop empties but
            // retains that resource until actor cleanup.
            let mut expected_resources = runtime.resources.clone();
            actor
                .extension
                .path_state
                .stack
                .begin(&mut expected_resources, owner, end, 1)
                .unwrap();
            actor
                .extension
                .path_state
                .stack
                .next(&mut expected_resources)
                .unwrap();
            let mut coordination = state(value ^ 0xFF);
            let mut sound = SoundBankRequest { selection: 63 };
            let mut latches = PathLatches { raised: 0xFFFF };
            let mut expected_latches = 0xFFFF;
            for iteration in 0..16 {
                expected_latches &= !masks[(usize::from(value) + iteration) % 128];
            }
            let mut countdown = PathCountdown { remaining: 59 };
            let mut scenery = SceneryDistanceState { near_mask: 253 };
            let mut pickups = PickupHistory {
                collected_mask: 0xA002,
            };
            let mut signals = EncounterSignals { raised: 0xFFFF };
            runtime.branch.invert_next = invert;
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut coordination);
            inputs.sound_bank_request = Some(&mut sound);
            inputs.path_latches = Some(&mut latches);
            inputs.countdown = Some(&mut countdown);
            inputs.scenery_distance = Some(&mut scenery);
            inputs.pickup_history = Some(&mut pickups);
            inputs.encounter_signals = Some(&mut signals);
            assert_eq!(
                runtime
                    .resume_program(&catalog, &mut objects, owner, &mut inputs, 64)
                    .map(|exit| exit.step),
                Ok(ControlStep::Ended)
            );
            assert_eq!(coordination, state(value));
            assert_eq!(sound.selection, value);
            assert_eq!(countdown.remaining, value);
            assert_eq!(scenery.near_mask, value);
            assert_eq!(pickups.collected_mask, 0xB319);
            assert_eq!(latches.raised, expected_latches);
            assert_eq!(signals.raised, 0xFFFF);
            assert_eq!(objects, expected);
            assert_eq!(runtime.resources, expected_resources);
            assert_eq!(runtime.branch.invert_next, invert);
            assert_eq!(random, before_random);
        }
    }
}
