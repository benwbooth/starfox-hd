use super::super::path_fields::WordField;
use super::super::path_scene_state::{EncounterObjectiveCounts, ObjectiveCompletion};
use super::super::{authored_paths, PathId};
use super::tests::{setup, world};
use super::*;

fn caller(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(1),
        command_index: index,
    }
}

fn called_catalog(target: PathCursor) -> PathCatalog {
    let mut catalog = authored_paths::catalog();
    catalog.paths.push(vec![
        Statement::Control(ControlCommand::Call {
            target,
            next: caller(1),
        }),
        Statement::Control(ControlCommand::WaitOne { next: caller(0) }),
    ]);
    catalog
}

// Static extraction tests independently bind these already-extracted mask
// values to the source table. Out-of-range selectors deliberately read its
// adjacent data, not a guessed shift or modulo-sixteen bit index.
fn query_masks(catalog: &PathCatalog) -> &'static [u16; 128] {
    let mut cursor = authored_paths::QUERY_OBJECTIVE_COMPLETION;
    for _ in 0..2 {
        cursor = match catalog.statement(cursor).unwrap() {
            Statement::ImportObjectiveCompletion { next, .. } | Statement::Mutate { next, .. } => {
                next
            }
            other => panic!("unexpected query prefix: {other:?}"),
        };
    }
    match catalog.statement(cursor).unwrap() {
        Statement::Compare {
            condition: ActorCondition::AnyWordBitsSet(_, WordOperand::IndexedBitMask { masks, .. }),
            ..
        } => masks,
        other => panic!("unexpected objective mask predicate: {other:?}"),
    }
}

#[test]
fn completion_word_transfer_preserves_all_bits_other_state_and_pending_ifnot() {
    for original in 0..=u16::MAX {
        for export in [false, true] {
            let statement = if export {
                Statement::ExportObjectiveCompletion {
                    source: WordOperand::Actor(WordField::ScriptValue),
                    next: caller(1),
                }
            } else {
                Statement::ImportObjectiveCompletion {
                    destination: WordField::ScriptValue,
                    next: caller(1),
                }
            };
            let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            runtime.enter(&objects, owner).unwrap();
            runtime.branch.invert_next = original & 1 != 0;
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .script_value = !original;
            let before = objects.clone();
            let before_runtime = runtime.clone();
            let before_random = random;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingObjectiveCompletion)
            );
            assert_eq!(objects, before);
            assert_eq!(runtime, before_runtime);
            let mut completion = ObjectiveCompletion { bits: original };
            let mut inputs = world(&mut random);
            inputs.objective_completion = Some(&mut completion);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                Err(ProgramError::BudgetExceeded {
                    cursor: PathCursor {
                        path: PathId::from_catalog_index(0),
                        command_index: 0
                    },
                    executed: 0
                })
            );
            assert_eq!(objects, before);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: caller(1),
                    executed: 1
                })
            );
            let mut expected = before;
            expected.get_mut(owner).unwrap().base.path = Some(caller(1));
            if !export {
                expected
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .script_value = original;
            }
            assert_eq!(objects, expected);
            assert_eq!(completion.bits, if export { !original } else { original });
            assert_eq!(runtime, before_runtime);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn authored_query_increments_parameter_wraps_selector_and_returns_zero_or_one_hundred() {
    let catalog = called_catalog(authored_paths::QUERY_OBJECTIVE_COMPLETION);
    let masks = query_masks(&catalog);
    for parameter in 0..=u8::MAX {
        let selector = parameter.wrapping_add(1);
        let mask = masks[usize::from(parameter & 127)];
        for flags in [0, u16::MAX, mask, !mask, 0x00FF, 0xFF00] {
            for invert in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                // Establish the retained empty call storage before snapshotting.
                let _ = runtime
                    .execute_control(
                        &mut objects,
                        owner,
                        ControlCommand::Call {
                            target: caller(0),
                            next: caller(0),
                        },
                    )
                    .unwrap();
                let _ = runtime
                    .execute_control(&mut objects, owner, ControlCommand::Return)
                    .unwrap();
                runtime.branch.invert_next = invert;
                let actor = objects.get_mut(owner).unwrap();
                actor.extension.path_state.script_parameter = parameter;
                actor.extension.path_state.script_value = 0xA55A;
                actor.extension.path_state.motion_phase = 0xABCD;
                let mut expected = objects.clone();
                let expected_actor = expected.get_mut(owner).unwrap();
                expected_actor.extension.path_state.script_parameter = selector;
                expected_actor.extension.path_state.script_value =
                    if flags & mask != 0 { 100 } else { 0 };
                let mut completion = ObjectiveCompletion { bits: flags };
                let mut inputs = world(&mut random);
                inputs.objective_completion = Some(&mut completion);
                assert_eq!(
                    runtime
                        .resume_program(&catalog, &mut objects, owner, &mut inputs, 16)
                        .unwrap(),
                    ProgramExit {
                        actor: owner,
                        step: ControlStep::Movement
                    }
                );
                assert_eq!(
                    objects, expected,
                    "parameter {parameter}, flags {flags}, IFNOT {invert}"
                );
                assert_eq!(completion.bits, flags);
                // Variable-bit branches bypass and preserve IFNOT.
                assert_eq!(runtime.branch.invert_next, invert);
            }
        }
    }
}

#[test]
fn authored_record_restores_scratch_and_decrements_the_selected_packed_count() {
    let catalog = called_catalog(authored_paths::RECORD_OBJECTIVE_COMPLETION);
    let masks = query_masks(&catalog);
    for parameter in 0..=u8::MAX {
        let mask = masks[usize::from(parameter.wrapping_sub(1) & 127)];
        for count in 0..=u8::MAX {
            for invert in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let _ = runtime
                    .execute_control(
                        &mut objects,
                        owner,
                        ControlCommand::Call {
                            target: caller(0),
                            next: caller(0),
                        },
                    )
                    .unwrap();
                let _ = runtime
                    .execute_control(&mut objects, owner, ControlCommand::Return)
                    .unwrap();
                runtime.branch.invert_next = invert;
                let actor = objects.get_mut(owner).unwrap();
                actor.extension.path_state.script_parameter = parameter;
                actor.extension.path_state.script_value = 0xA500 | u16::from(count);
                actor.extension.path_state.motion_phase = 0x5A00 | u16::from(count ^ 0xFF);
                let expected = objects.clone();
                let mut completion = ObjectiveCompletion {
                    bits: u16::from(count) * 257,
                };
                let expected_bits = completion.bits | mask;
                let mut counts = EncounterObjectiveCounts {
                    remaining_word: 0xABCD,
                    node_record: count,
                };
                let mut inputs = world(&mut random);
                inputs.objective_completion = Some(&mut completion);
                inputs.objective_counts = Some(&mut counts);
                assert_eq!(
                    runtime
                        .resume_program(&catalog, &mut objects, owner, &mut inputs, 32)
                        .unwrap(),
                    ProgramExit {
                        actor: owner,
                        step: ControlStep::Movement
                    }
                );
                assert_eq!(objects, expected);
                // Source lower edge excluded and upper edge included. The
                // sign-wrapped comparisons also reject all other selectors.
                let lower_nibble = ((1..=8).contains(&parameter)) ^ invert;
                let decrement = if lower_nibble { 1 } else { 16 };
                assert_eq!(
                    counts,
                    EncounterObjectiveCounts {
                        remaining_word: 0xABCD,
                        node_record: count.wrapping_sub(decrement)
                    }
                );
                assert_eq!(completion.bits, expected_bits);
                assert!(!runtime.branch.invert_next);
            }
        }
    }
}

#[test]
fn record_resumes_after_missing_count_without_losing_saved_scratch() {
    let catalog = called_catalog(authored_paths::RECORD_OBJECTIVE_COMPLETION);
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(caller(0));
    objects
        .get_mut(owner)
        .unwrap()
        .extension
        .path_state
        .script_parameter = 9;
    objects
        .get_mut(owner)
        .unwrap()
        .extension
        .path_state
        .script_value = 0x1234;
    objects
        .get_mut(owner)
        .unwrap()
        .extension
        .path_state
        .motion_phase = 0xABCD;
    let mut completion = ObjectiveCompletion { bits: 0x8000 };
    let mut inputs = world(&mut random);
    inputs.objective_completion = Some(&mut completion);
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 32),
        Err(ProgramError::MissingObjectiveCounts)
    );
    assert_eq!(inputs.objective_completion.as_ref().unwrap().bits, 0x8000);
    assert_eq!(
        objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .script_value,
        0x8000
    );
    let paused = objects.clone();
    let paused_runtime = runtime.clone();
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 32),
        Err(ProgramError::MissingObjectiveCounts)
    );
    assert_eq!(objects, paused);
    assert_eq!(runtime, paused_runtime);
    let mut counts = EncounterObjectiveCounts {
        remaining_word: 0xABCD,
        node_record: 0,
    };
    inputs.objective_counts = Some(&mut counts);
    assert_eq!(
        runtime
            .resume_program(&catalog, &mut objects, owner, &mut inputs, 32)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    assert_eq!(completion.bits, 0x8100);
    assert_eq!(counts.node_record, 240);
    assert_eq!(
        objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .script_value,
        0x1234
    );
    assert_eq!(
        objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase,
        0xABCD
    );
}

#[test]
fn another_actor_observes_published_completion_before_the_separate_count_write() {
    let mut catalog = called_catalog(authored_paths::RECORD_OBJECTIVE_COMPLETION);
    let query_start = PathCursor {
        path: PathId::from_catalog_index(2),
        command_index: 0,
    };
    let query_return = PathCursor {
        command_index: 1,
        ..query_start
    };
    catalog.paths.push(vec![
        Statement::Control(ControlCommand::Call {
            target: authored_paths::QUERY_OBJECTIVE_COMPLETION,
            next: query_return,
        }),
        Statement::Control(ControlCommand::WaitOne { next: query_start }),
    ]);
    let (mut writer, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(caller(0));
    objects
        .get_mut(owner)
        .unwrap()
        .extension
        .path_state
        .script_parameter = 9;
    let mut observer = objects.get(owner).unwrap().clone();
    observer.base.path = Some(query_start);
    observer.extension.path_state.script_parameter = 8;
    let other = objects.allocate(observer).unwrap();
    let mut reader = PathRuntime::default();
    let mut completion = ObjectiveCompletion { bits: 0 };
    let mut counts = EncounterObjectiveCounts {
        remaining_word: 0xABCD,
        node_record: 0x21,
    };
    let mut inputs = world(&mut random);
    inputs.objective_completion = Some(&mut completion);
    inputs.objective_counts = Some(&mut counts);
    let mut published = false;
    for _ in 0..16 {
        let cursor = objects.get(owner).unwrap().base.path.unwrap();
        let export = matches!(
            catalog.statement(cursor).unwrap(),
            Statement::ExportObjectiveCompletion { .. }
        );
        assert!(matches!(
            writer.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::BudgetExceeded { executed: 1, .. })
        ));
        if export {
            published = true;
            break;
        }
    }
    assert!(published);
    assert_eq!(inputs.objective_counts.as_ref().unwrap().node_record, 0x21);
    assert_eq!(
        reader
            .resume_program(&catalog, &mut objects, other, &mut inputs, 16)
            .unwrap(),
        ProgramExit {
            actor: other,
            step: ControlStep::Movement
        }
    );
    assert_eq!(
        objects
            .get(other)
            .unwrap()
            .extension
            .path_state
            .script_value,
        100
    );
    assert_eq!(
        writer
            .resume_program(&catalog, &mut objects, owner, &mut inputs, 32)
            .unwrap(),
        ProgramExit {
            actor: owner,
            step: ControlStep::Movement
        }
    );
    assert_eq!(completion.bits, 256);
    assert_eq!(
        counts,
        EncounterObjectiveCounts {
            remaining_word: 0xABCD,
            node_record: 0x11
        }
    );
}
