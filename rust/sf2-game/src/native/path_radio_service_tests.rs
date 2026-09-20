use super::super::path_fields::WordField;
use super::super::path_radio::{
    DeferredMessage, DeferredMessageCommand, PathRadio, RadioLayout, RadioRequest,
};
use super::super::path_scene_state::EncounterCoordination;
use super::super::{authored_paths, FlightControlStyle, PathId};
use super::tests::{setup, world};
use super::*;

#[test]
fn radio_event_byte_transfers_preserve_the_word_high_byte_and_every_other_actor_field() {
    use super::super::path_fields::ByteField;
    use super::super::path_radio::{RadioEvent, RadioEventCommand};
    for importing in [false, true] {
        let command = if importing {
            RadioEventCommand::CopyTo(ByteField::Part)
        } else {
            RadioEventCommand::Assign(ByteOperand::Actor(ByteField::Part))
        };
        let catalog = PathCatalog::new(vec![vec![Statement::RadioEvent {
            command,
            next: at(1),
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let before = objects.get(owner).unwrap().clone();
        let before_random = random;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingRadioEvent)
        );
        assert_eq!(objects.get(owner), Some(&before));
        for number in 0..=u16::MAX {
            *objects.get_mut(owner).unwrap() = before.clone();
            let source = (number as u8) ^ 0xFF;
            objects.get_mut(owner).unwrap().extension.path_state.part = source;
            let mut expected = objects.get(owner).unwrap().clone();
            expected.base.path = Some(at(1));
            if importing {
                expected.extension.path_state.part = number as u8;
            }
            let mut event = RadioEvent { number };
            let mut inputs = world(&mut random);
            inputs.radio_event = Some(&mut event);
            runtime.branch.invert_next = true;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
            assert_eq!(objects.get(owner), Some(&expected));
            assert_eq!(
                event.number,
                if importing {
                    number
                } else {
                    number / 256 * 256 + u16::from(source)
                }
            );
            assert!(runtime.branch.invert_next);
        }
        assert_eq!(random, before_random);
    }
}

#[test]
fn spawn_parameter_is_a_shared_mailbox_not_a_child_or_allocation_field() {
    use super::super::path_fields::ByteField;
    use super::super::path_spawn::{SpawnArgument, SpawnParameterCommand};
    for argument in [SpawnArgument::Primary, SpawnArgument::Companion] {
        for importing in [false, true] {
            let command = if importing {
                SpawnParameterCommand::CopyTo(ByteField::Part)
            } else {
                SpawnParameterCommand::Assign(ByteOperand::Actor(ByteField::Part))
            };
            let catalog = PathCatalog::new(vec![vec![Statement::SpawnParameter {
                argument,
                command,
                next: at(1),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.spawns.last_spawn = Some(owner);
            let before = objects.clone();
            let before_random = random;
            if importing {
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::MissingSpawnParameter(argument))
                );
                assert_eq!(objects, before);
            }
            for value in 0..=u8::MAX {
                objects = before.clone();
                objects.get_mut(owner).unwrap().extension.path_state.part = value ^ 0xFF;
                let mut expected = objects.clone();
                let actor = expected.get_mut(owner).unwrap();
                actor.base.path = Some(at(1));
                if importing {
                    actor.extension.path_state.part = value;
                }
                runtime.spawns.parameter = Some(value.wrapping_add(7));
                runtime.spawns.companion_parameter = Some(value.wrapping_add(11));
                *runtime.spawns.argument_mut(argument) = if importing { Some(value) } else { None };
                let mut expected_spawns = runtime.spawns;
                *expected_spawns.argument_mut(argument) =
                    Some(if importing { value } else { value ^ 0xFF });
                runtime.branch.invert_next = true;
                assert_eq!(
                    runtime.resume_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        1
                    ),
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(1),
                        executed: 1
                    })
                );
                assert_eq!(runtime.spawns, expected_spawns);
                assert_eq!(runtime.spawns.last_spawn, Some(owner));
                assert_eq!(objects, expected);
                assert!(runtime.branch.invert_next);
            }
            assert_eq!(random, before_random);
        }
    }
}

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

#[test]
fn deferred_message_transfers_every_word_without_creating_a_presentation_request() {
    for importing in [false, true] {
        let command = if importing {
            DeferredMessageCommand::CopyTo(WordField::ScriptValue)
        } else {
            DeferredMessageCommand::Assign(WordOperand::Actor(WordField::ScriptValue))
        };
        let catalog = PathCatalog::new(vec![vec![Statement::DeferredMessage {
            command,
            next: at(1),
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let initial = objects.get(owner).unwrap().clone();
        let before_random = random;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingDeferredMessage)
        );
        assert_eq!(objects.get(owner), Some(&initial));
        for value in 0..=u16::MAX {
            *objects.get_mut(owner).unwrap() = initial.clone();
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .script_value = value;
            let mut expected = objects.get(owner).unwrap().clone();
            expected.base.path = Some(at(1));
            let mut deferred = DeferredMessage {
                number: value ^ 0xFFFF,
            };
            if importing {
                expected.extension.path_state.script_value = value ^ 0xFFFF;
            }
            let mut request = RadioRequest::default();
            let mut inputs = world(&mut random);
            inputs.deferred_message = Some(&mut deferred);
            inputs.radio = Some(PathRadio {
                request: &mut request,
                layout: RadioLayout {
                    compact_panel: false,
                    tracked_screen_y: 100,
                },
            });
            runtime.branch.invert_next = true;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
            assert_eq!(
                deferred.number,
                if importing { value ^ 0xFFFF } else { value }
            );
            assert_eq!(objects.get(owner), Some(&expected));
            assert_eq!(request, RadioRequest::default());
            assert!(runtime.branch.invert_next);
        }
        assert_eq!(random, before_random);
    }
}

#[test]
fn wingmate_warning_uses_published_pilot_control_style_and_three_fresh_mask_visits() {
    let catalog = authored_paths::catalog();
    for pilot in 0..=u8::MAX {
        for style in [FlightControlStyle::TypeA, FlightControlStyle::TypeB] {
            for initial_mask in [0, 0xA2, 0xFF] {
                for invert in [false, true] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let before_random = random;
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(authored_paths::WINGMATE_PROXIMITY_WARNING);
                    actor.base.wait_timer = 73;
                    actor.extension.path_state.script_value = 0xABCD;
                    actor.extension.texture_scroll_x = 17;
                    let mut shared = EncounterCoordination {
                        active_messages: initial_mask,
                        handshake: 153,
                        ..Default::default()
                    };
                    let expected_skip = initial_mask & 4 != 0 || ((pilot == 255) != invert);
                    let mut request = RadioRequest::default();
                    runtime.branch.invert_next = invert;
                    let mut inputs = world(&mut random);
                    inputs.coordination = Some(&mut shared);
                    // Gate exits must not require later observations.
                    if initial_mask & 4 == 0 {
                        inputs.scene.wingmate_pilot = Some(pilot);
                    }
                    if !expected_skip {
                        inputs.control_style = Some(style);
                        inputs.radio = Some(PathRadio {
                            request: &mut request,
                            layout: RadioLayout {
                                compact_panel: false,
                                tracked_screen_y: 100,
                            },
                        });
                    }
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 32)
                            .map(|exit| exit.step),
                        Ok(if expected_skip {
                            ControlStep::Ended
                        } else {
                            ControlStep::Movement
                        })
                    );
                    // The direct bit branch preserves IFNOT; the pilot-byte
                    // comparison, if reached, consumes it.
                    assert_eq!(runtime.branch.invert_next, initial_mask & 4 != 0 && invert);
                    assert!(!objects.get(owner).unwrap().base.flags.visible);
                    assert!(objects.get(owner).unwrap().base.flags.collision_disabled);
                    assert_eq!(shared.handshake, 153);
                    if expected_skip {
                        assert_eq!(shared.active_messages, initial_mask);
                        assert_eq!(request, RadioRequest::default());
                        assert_eq!(
                            objects
                                .get(owner)
                                .unwrap()
                                .extension
                                .path_state
                                .script_value,
                            0xABCD
                        );
                        assert_eq!(random, before_random);
                        continue;
                    }
                    let message_word = (i16::from(pilot as i8)
                        + 32
                        + if style == FlightControlStyle::TypeA {
                            0
                        } else {
                            96
                        }) as u16;
                    assert_eq!(
                        objects
                            .get(owner)
                            .unwrap()
                            .extension
                            .path_state
                            .script_value,
                        message_word
                    );
                    assert_eq!(
                        request.message.index(),
                        (usize::from(message_word as u8) + 255) % 256
                    );
                    assert!(request.pending);
                    assert_eq!(request.panel_y, 151);
                    assert_eq!(shared.active_messages, initial_mask | 4);
                    // Both later iterations reimport live shared flags. Supply
                    // no radio, pilot, or controls: they are not read again.
                    let published = request;
                    for (visit, incoming) in [(2, 0x42), (3, 0x91)] {
                        shared.active_messages = incoming;
                        let mut inputs = world(&mut random);
                        inputs.coordination = Some(&mut shared);
                        assert_eq!(
                            runtime
                                .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                                .map(|exit| exit.step),
                            Ok(if visit == 3 {
                                ControlStep::Ended
                            } else {
                                ControlStep::Movement
                            })
                        );
                        assert_eq!(
                            shared.active_messages,
                            if visit == 3 {
                                incoming & !4
                            } else {
                                incoming | 4
                            }
                        );
                        assert_eq!(request, published);
                        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 73);
                    }
                    assert_eq!(random, before_random);
                }
            }
        }
    }
}

#[test]
fn cooldown_holds_bit_for_sixty_nine_yields_resamples_shared_flags_then_retires() {
    let catalog = authored_paths::catalog();
    for initial in 0..=u8::MAX {
        for invert in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let before_random = random;
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::PROXIMITY_WARNING_COOLDOWN);
            let mut shared = EncounterCoordination {
                active_messages: initial,
                completed_parts: 211,
                ..Default::default()
            };
            runtime.branch.invert_next = invert;
            let skip = initial & 4 != 0;
            for visit in 1..=if skip { 1 } else { 70 } {
                if visit > 1 {
                    shared.active_messages = initial.wrapping_add(visit);
                }
                let before = shared.active_messages;
                let mut inputs = world(&mut random);
                inputs.coordination = Some(&mut shared);
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                        .map(|exit| exit.step),
                    Ok(if skip || visit == 70 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    })
                );
                assert_eq!(
                    shared.active_messages,
                    if skip {
                        before
                    } else if visit == 70 {
                        before & !4
                    } else {
                        before | 4
                    }
                );
                assert_eq!(shared.completed_parts, 211);
                assert_eq!(runtime.branch.invert_next, invert);
            }
            assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
            assert_eq!(random, before_random);
        }
    }
}
