use super::super::path_scene_state::EncounterCoordination;
use super::super::{authored_paths, PathId};
use super::tests::{setup, world};
use super::*;

fn caller(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(1),
        command_index: index,
    }
}

fn catalog() -> PathCatalog {
    let mut catalog = authored_paths::catalog();
    catalog.paths.push(vec![
        Statement::Control(ControlCommand::Call {
            target: authored_paths::WAIT_FOR_TRANSITION_READY,
            next: caller(1),
        }),
        Statement::Control(ControlCommand::WaitOne { next: caller(0) }),
    ]);
    catalog
}

#[test]
fn transition_wait_samples_the_whole_byte_preserves_ifnot_and_restores_scratch() {
    let catalog = catalog();
    for ready in 0..=u8::MAX {
        for phase in [0, 0xABCD, u16::MAX] {
            for invert in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                objects.get_mut(owner).unwrap().base.path = Some(caller(0));
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase = phase;
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .script_value = 0x5AA5;
                objects.get_mut(owner).unwrap().base.wait_timer = 137;
                runtime.branch.invert_next = invert;
                let mut shared = EncounterCoordination {
                    transition_ready: ready,
                    ..EncounterCoordination::default()
                };
                let before_shared = shared;
                let before_random = random;
                let mut inputs = world(&mut random);
                inputs.coordination = Some(&mut shared);
                assert_eq!(
                    runtime
                        .resume_program(&catalog, &mut objects, owner, &mut inputs, 16)
                        .unwrap(),
                    ProgramExit {
                        actor: owner,
                        step: ControlStep::Movement
                    }
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(
                    actor.base.path,
                    Some(if ready != 0 {
                        caller(0)
                    } else {
                        authored_paths::WAIT_FOR_TRANSITION_READY
                    })
                );
                assert_eq!(actor.extension.path_state.motion_phase, phase);
                assert_eq!(actor.extension.path_state.script_value, 0x5AA5);
                assert_eq!(actor.base.wait_timer, 137);
                assert_eq!(shared, before_shared);
                assert_eq!(runtime.branch.invert_next, invert);
                assert_eq!(random, before_random);
            }
        }
    }
}

#[test]
fn transition_wait_rereads_publication_and_returns_to_the_original_caller() {
    let catalog = catalog();
    for ready in 1..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        // Retain empty stack storage before the expected-state snapshot.
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
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0x5AA5;
        let expected = objects.clone();
        let mut shared = EncounterCoordination::default();
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        for _ in 0..4 {
            assert_eq!(
                runtime
                    .resume_program(&catalog, &mut objects, owner, &mut inputs, 16)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(
                objects.get(owner).unwrap().base.path,
                Some(authored_paths::WAIT_FOR_TRANSITION_READY)
            );
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase,
                0x5AA5
            );
        }
        inputs.coordination.as_deref_mut().unwrap().transition_ready = ready;
        assert_eq!(
            runtime
                .resume_program(&catalog, &mut objects, owner, &mut inputs, 16)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects, expected);
        // Clearing publication after return must gate the next fresh call.
        inputs.coordination.as_deref_mut().unwrap().transition_ready = 0;
        assert_eq!(
            runtime
                .resume_program(&catalog, &mut objects, owner, &mut inputs, 16)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(
            objects.get(owner).unwrap().base.path,
            Some(authored_paths::WAIT_FOR_TRANSITION_READY)
        );
    }
}

#[test]
fn missing_transition_input_retains_one_saved_byte_until_successful_resume() {
    let catalog = catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(caller(0));
    objects
        .get_mut(owner)
        .unwrap()
        .extension
        .path_state
        .motion_phase = 0xABCD;
    let mut inputs = world(&mut random);
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 16),
        Err(ProgramError::MissingCoordination)
    );
    let before = objects.clone();
    let before_runtime = runtime.clone();
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 16),
        Err(ProgramError::MissingCoordination)
    );
    assert_eq!(objects, before);
    assert_eq!(runtime, before_runtime);
    let mut shared = EncounterCoordination {
        transition_ready: 255,
        ..EncounterCoordination::default()
    };
    inputs.coordination = Some(&mut shared);
    assert_eq!(
        runtime
            .resume_program(&catalog, &mut objects, owner, &mut inputs, 16)
            .unwrap()
            .step,
        ControlStep::Movement
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
    assert_eq!(objects.get(owner).unwrap().base.path, Some(caller(0)));
}
