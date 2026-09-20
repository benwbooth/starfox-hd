//! Progress-controlled exit gates, using authored data and no gameplay traces.
use super::super::path_scene_state::{CoordinationField, EncounterCoordination};
use super::super::{authored_paths, Angle, ShapeId};
use super::tests::{setup, world};
use super::*;

#[test]
fn exit_gate_preserves_signed_byte_comparison_order_before_the_completion_sentinel() {
    let catalog = authored_paths::catalog();
    for oriented in [false, true] {
        for stage in [0u8, 1, 127, 128, 253, 254, 255] {
            for progress in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(if oriented {
                    authored_paths::ORIENTED_PROGRESS_GATED_EXIT
                } else {
                    authored_paths::PROGRESS_GATED_EXIT
                });
                actor.base.position.y = i16::from_le_bytes([stage.wrapping_add(1), 137]);
                actor.base.hit_points = 193;
                actor.base.yaw = Angle::from_units(71);
                let initial_shape = actor.base.shape;
                let mut shared = EncounterCoordination {
                    progress,
                    boundary_corrections: 171,
                    phase: 117,
                    ..Default::default()
                };
                let before_shared = shared;
                let before_random = random;
                let mut inputs = world(&mut random);
                inputs.coordination = Some(&mut shared);
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 40)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.position.y, 0);
                assert_eq!(actor.base.yaw.units(), if oriented { 193 } else { 71 });
                assert_eq!(actor.extension.path_state.script_parameter, stage);
                assert_eq!((actor.base.hit_points, actor.base.attack_power), (100, 4));
                assert!(actor.base.flags.proximity_warning_source);
                assert!(actor.base.flags.collision_disabled);
                let opens = (stage.wrapping_sub(progress) as i8) < 0;
                let holds = !opens && progress == 254;
                let statement = catalog.statement(actor.base.path.unwrap()).unwrap();
                if opens {
                    assert!(
                        matches!(statement, Statement::Wait { .. }),
                        "stage {stage}, progress {progress}, oriented {oriented}: {statement:?}"
                    );
                    assert_eq!(actor.base.wait_timer, 1);
                } else if holds {
                    assert!(matches!(
                        statement,
                        Statement::Control(ControlCommand::Hold)
                    ));
                    assert_eq!(actor.base.wait_timer, 0);
                } else {
                    assert!(matches!(
                        statement,
                        Statement::Coordination {
                            field: CoordinationField::Progress,
                            ..
                        }
                    ));
                    assert_eq!(actor.base.wait_timer, 0);
                }
                assert_eq!(actor.base.flags.exclude_from_shape_footprint_search, holds);
                assert_eq!(
                    actor.base.shape,
                    if holds {
                        ShapeId::from_catalog_index(435)
                    } else {
                        initial_shape
                    }
                );
                assert_eq!(objects.len(), 1);
                assert_eq!(shared, before_shared);
                assert_eq!(random, before_random);
            }
        }
    }
}

#[test]
fn waiting_exit_reads_live_progress_and_keeps_the_full_fifteen_tick_delay() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(authored_paths::PROGRESS_GATED_EXIT);
    objects.get_mut(owner).unwrap().base.position.y = 5;
    let mut shared = EncounterCoordination::default();
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut shared);
    for progress in [0, 1, 4, 255] {
        inputs.coordination.as_deref_mut().unwrap().progress = progress;
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 40)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 0);
    }
    inputs.coordination.as_deref_mut().unwrap().progress = 5;
    for elapsed in 1..=15 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 40)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, elapsed);
        assert_eq!(objects.len(), 1);
    }
}
