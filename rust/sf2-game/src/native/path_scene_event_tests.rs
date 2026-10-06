use super::super::path_fields::WordField;
use super::super::path_radio::{MessageIndex, PathRadio, RadioLayout, RadioRequest};
use super::super::path_scene_state::{
    EncounterCoordination, ObjectiveCompletion, SceneEventCommand, SceneEventFlags,
};
use super::super::{authored_paths, ObjectSpawnDefaults, PathId, ShapeId};
use super::tests::{setup, world};
use super::*;

fn cursor(path: u16, command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(path),
        command_index,
    }
}

fn announcement_catalog() -> PathCatalog {
    let mut catalog = authored_paths::catalog();
    catalog.paths.push(vec![
        Statement::Control(ControlCommand::Call {
            target: authored_paths::ANNOUNCE_WINGMATE_SCENE_EVENT,
            next: cursor(1, 1),
        }),
        Statement::Control(ControlCommand::WaitOne { next: cursor(1, 2) }),
    ]);
    catalog
}

#[test]
fn scene_event_transfer_retains_the_full_word_and_never_changes_other_shared_state() {
    for export in [false, true] {
        let command = if export {
            SceneEventCommand::Assign(WordOperand::Actor(WordField::ScriptValue))
        } else {
            SceneEventCommand::CopyTo(WordField::ScriptValue)
        };
        let catalog = PathCatalog::new(vec![vec![Statement::SceneEvent {
            command,
            next: cursor(0, 1),
        }]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        runtime.enter(&objects, owner).unwrap();
        runtime.branch.invert_next = true;
        let before = objects.clone();
        let before_runtime = runtime.clone();
        let before_random = random;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingSceneEvents)
        );
        assert_eq!(objects, before);
        assert_eq!(runtime, before_runtime);
        for bits in 0..=u16::MAX {
            objects = before.clone();
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .script_value = !bits;
            let mut expected = objects.clone();
            expected.get_mut(owner).unwrap().base.path = Some(cursor(0, 1));
            if !export {
                expected
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .script_value = bits;
            }
            let mut events = SceneEventFlags { bits };
            let mut objective = ObjectiveCompletion {
                bits: bits ^ 0xA55A,
            };
            let mut inputs = world(&mut random);
            inputs.scene_events = Some(&mut events);
            inputs.objective_completion = Some(&mut objective);
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 0),
                    executed: 0
                })
            );
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(0, 1),
                    executed: 1
                })
            );
            assert_eq!(events.bits, if export { !bits } else { bits });
            assert_eq!(objective.bits, bits ^ 0xA55A);
            assert_eq!(objects, expected);
            assert_eq!(runtime, before_runtime);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn wingmate_announcement_preserves_arguments_and_publishes_before_optional_radio() {
    let catalog = announcement_catalog();
    for pilot in 0..=u8::MAX {
        for bits in [0u16, 1, 0x0200, 0xFDFF, u16::MAX] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(cursor(1, 0));
            actor.extension.path_state.script_value = bits ^ 0x9137;
            actor.extension.path_state.motion_phase = 0xDEAD;
            actor.extension.path_state.weapon_selection = 71;
            let before = actor.clone();
            let before_random = random;
            let mut events = SceneEventFlags { bits };
            let mut request = RadioRequest::default();
            let before_request = request;
            let mut inputs = world(&mut random);
            inputs.scene_events = Some(&mut events);
            inputs.scene.wingmate_pilot = Some(pilot);
            if pilot != u8::MAX {
                inputs.radio = Some(PathRadio {
                    request: &mut request,
                    layout: RadioLayout {
                        compact_panel: false,
                        tracked_screen_y: 80,
                    },
                });
            }
            assert_eq!(
                runtime
                    .resume_program(&catalog, &mut objects, owner, &mut inputs, 64)
                    .unwrap(),
                ProgramExit {
                    actor: owner,
                    step: ControlStep::Movement
                }
            );
            let actor = objects.get(owner).unwrap();
            let mut expected = before;
            expected.base.path = Some(cursor(1, 2));
            expected.extension.path_state.stack = actor.extension.path_state.stack.clone();
            assert_eq!(
                expected
                    .extension
                    .path_state
                    .stack
                    .pop_call(&mut runtime.resources),
                Err(super::super::program_state::PathStackError::MissingLoop)
            );
            assert_eq!(actor, &expected);
            assert_eq!(events.bits, bits | 0x0200);
            if pilot == u8::MAX {
                assert_eq!(request, before_request);
            } else {
                assert_eq!(
                    request.message,
                    MessageIndex::from_authored_number(146u8.wrapping_add(pilot))
                );
                assert!(request.pending);
                assert_eq!(request.panel_y, 151);
            }
            assert_eq!(random, before_random);
            assert_eq!(objects.len(), 1);
        }
    }
}

#[test]
fn wingmate_announcement_missing_pilot_faults_after_event_publication_then_resumes() {
    let catalog = announcement_catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(cursor(1, 0));
    let mut events = SceneEventFlags { bits: 0x8101 };
    let mut inputs = world(&mut random);
    inputs.scene_events = Some(&mut events);
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 64),
        Err(ProgramError::MissingSceneByte(SceneByte::WingmatePilot))
    );
    assert_eq!(inputs.scene_events.as_deref().unwrap().bits, 0x8301);
    // Already-published flags stay live across a failed later input read.
    inputs.scene_events.as_deref_mut().unwrap().bits = 0x4000;
    inputs.scene.wingmate_pilot = Some(u8::MAX);
    assert_eq!(
        runtime
            .resume_program(&catalog, &mut objects, owner, &mut inputs, 64)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    assert_eq!(events.bits, 0x4000);
    assert_eq!(
        objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .script_value,
        0
    );
}

#[test]
fn objective_gated_patrol_skips_completed_slots_or_creates_one_numbered_child_before_waiting() {
    let catalog = authored_paths::catalog();
    for health in 0..16 {
        for complete in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::OBJECTIVE_GATED_PULSE_PATROL);
            actor.base.hit_points = health;
            actor.base.attack_power = 173;
            actor.base.position.y = 32760;
            let before = actor.clone();
            let before_random = random;
            let mut objective = ObjectiveCompletion {
                bits: if complete { 1 << health } else { 0 },
            };
            let mut coordination = EncounterCoordination::default();
            let mut inputs = world(&mut random);
            inputs.objective_completion = Some(&mut objective);
            inputs.coordination = Some(&mut coordination);
            inputs.scene.height_offset = Some(20);
            inputs.scene.entry_heading = Some(37);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            for _ in 0..3 {
                let result = runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 128)
                    .unwrap();
                let actor = objects.get(owner).unwrap();
                if complete {
                    assert_eq!(result.step, ControlStep::Ended);
                    let mut expected = before.clone();
                    expected.base.path = actor.base.path;
                    expected.base.flags.remove_after_tick = true;
                    expected.extension.path_state.script_parameter = health + 1;
                    expected.extension.path_state.script_value = 100;
                    expected.extension.path_state.stack = actor.extension.path_state.stack.clone();
                    assert_eq!(actor, &expected);
                    assert_eq!(objects.len(), 1);
                    break;
                }
                assert_eq!(result.step, ControlStep::Movement);
                assert_eq!((actor.base.hit_points, actor.base.attack_power), (100, 4));
                assert_eq!(actor.base.position.y, 32760i16.wrapping_add(20));
                assert_eq!(actor.base.yaw.units(), 37);
                assert_eq!(actor.base.speed, 48);
                assert_eq!(objects.len(), 2);
                let child = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                assert_eq!(child.base.path, Some(authored_paths::HIT_TOGGLE_SPRITE));
                assert_eq!(child.base.shape, ShapeId::from_catalog_index(19));
                assert_eq!((child.base.hit_points, child.base.attack_power), (100, 70));
                assert_eq!(child.base.attachment, Some(owner));
                assert_eq!(child.base.child_number, 1);
                assert_eq!(child.extension.relative_position.z, -400);
            }
            assert_eq!(random, before_random);
        }
    }
}
