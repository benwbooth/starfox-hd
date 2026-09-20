use super::super::collision_contacts::ContactStore;
use super::super::path_contact::ContactCommand;
use super::super::path_fields::ByteField;
use super::super::path_impact::ImpactState;
use super::super::{Behavior, ObjectKind, PathId, ShapeId};
use super::tests::{setup, world};
use super::*;

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

#[test]
fn pair_suppression_import_preserves_material_and_branch_latch() {
    let catalog = PathCatalog::new(vec![vec![Statement::ImportPairSuppression {
        destination: ByteField::Part,
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let before = objects.clone();
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::MissingImpactState)
    );
    assert_eq!(objects, before);
    let random_before = random;
    for suppressed in [false, true] {
        for material in 0..=u8::MAX {
            for invert in [false, true] {
                objects = before.clone();
                let mut state = ImpactState {
                    material,
                    pair_suppressed: suppressed,
                };
                let state_before = state;
                let mut expected = objects.clone();
                ByteField::Part.write(expected.get_mut(owner).unwrap(), u8::from(suppressed));
                expected.get_mut(owner).unwrap().base.path = Some(at(1));
                let mut inputs = world(&mut random);
                inputs.impact = Some(&mut state);
                runtime.branch.invert_next = invert;
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(1),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(state, state_before);
                assert_eq!(runtime.branch.invert_next, invert);
            }
        }
    }
    assert_eq!(random, random_before);
}

#[test]
fn linked_shot_statements_update_attached_player_without_using_selection_or_ifnot() {
    use super::super::path_shots::{ActiveShots, LinkedShotCount, ShotCountCommand};
    let (mut runtime, mut objects, owner, mut random) = setup();
    let player = objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::FollowPath,
        ))
        .unwrap();
    objects.get_mut(owner).unwrap().base.attachment = Some(player);
    let before = objects.clone();
    let random_before = random;
    for command in [ShotCountCommand::Increment, ShotCountCommand::Decrement] {
        let catalog = PathCatalog::new(vec![vec![Statement::LinkedShotCount {
            command,
            next: at(1),
        }]])
        .unwrap();
        objects = before.clone();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingLinkedShotCount)
        );
        assert_eq!(objects, before);
        for count in 0..=u8::MAX {
            for invert in [false, true] {
                objects = before.clone();
                let mut state = ActiveShots::from_count(count);
                let mut expected = objects.clone();
                expected.get_mut(owner).unwrap().base.path = Some(at(1));
                let mut inputs = world(&mut random);
                inputs.selected = Some(owner);
                inputs.primary_player = Some(owner);
                inputs.linked_shot_count = Some(LinkedShotCount {
                    owner: player,
                    state: &mut state,
                });
                runtime.branch.invert_next = invert;
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(1),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(runtime.branch.invert_next, invert);
                assert_eq!(
                    state.count(),
                    match command {
                        ShotCountCommand::Increment => count.wrapping_add(1),
                        ShotCountCommand::Decrement => count.saturating_sub(1),
                    }
                );
            }
        }
    }
    assert_eq!(random, random_before);
}

#[test]
fn published_homing_target_copies_absence_and_retained_identity_without_fresh_selection() {
    use super::super::path_target::PublishedHomingTarget;
    let catalog = PathCatalog::new(vec![vec![Statement::AttachPublishedHomingTarget {
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let retired = objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::FollowPath,
        ))
        .unwrap();
    objects.remove(retired).unwrap();
    let before = objects.clone();
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::MissingPublishedHomingTarget)
    );
    assert_eq!(objects, before);
    for target in [None, Some(owner), Some(retired)] {
        for invert in [false, true] {
            objects = before.clone();
            objects.get_mut(owner).unwrap().base.attachment = Some(owner);
            let mut expected = objects.clone();
            expected.get_mut(owner).unwrap().base.attachment = target;
            expected.get_mut(owner).unwrap().base.path = Some(at(1));
            let mut inputs = world(&mut random);
            inputs.published_homing_target = Some(PublishedHomingTarget { object: target });
            inputs.selected = Some(owner);
            runtime.branch.invert_next = invert;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
            assert_eq!(objects, expected);
            assert_eq!(runtime.branch.invert_next, invert);
        }
    }
}

#[test]
fn impact_handoff_runs_movement_and_registered_callbacks_with_stopped_or_forced_continuation() {
    use super::super::path_effect::ImpactBurstPhase;
    use super::super::path_fields::ByteOperation;
    use super::super::path_motion::PlayerDisplacement;
    use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
    use super::super::path_triggers::{Trigger, TriggerKind};
    use super::super::Vector3;
    for force in [false, true] {
        let callback = if force {
            Statement::Control(ControlCommand::ForceAfterCallbacks {
                target: at(3),
                next: at(2),
            })
        } else {
            Statement::Mutate {
                mutation: Mutation::Byte {
                    field: ByteField::Part,
                    operation: ByteOperation::Add(ByteOperand::Literal(1)),
                },
                next: at(2),
            }
        };
        let catalog = PathCatalog::new(vec![vec![
            Statement::InstallImpactBurst,
            callback,
            Statement::Control(ControlCommand::Return),
            Statement::Control(ControlCommand::Hold),
        ]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.velocity = Vector3 { x: 3, y: -4, z: 5 };
        actor.extension.render_parameter = 222;
        actor.extension.path_state.part = 255;
        actor.extension.path_state.clear_on_path_exit_latch = true;
        actor.base.wait_timer = 93;
        actor.extension.path_state.repeat_counter = 81;
        runtime
            .add_trigger(
                &mut objects,
                owner,
                Trigger {
                    path: at(1),
                    kind: TriggerKind::Always,
                    timer: 0,
                },
            )
            .unwrap();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .stack
            .save_word(&mut runtime.resources, owner, 0xA55A)
            .unwrap();
        let before_resources = runtime.resources.clone();
        let before_random = random;
        let mut expected = objects.clone();
        let actor = expected.get_mut(owner).unwrap();
        actor.base.path = None;
        actor.base.behavior = Behavior::ImpactBurst(ImpactBurstPhase::Initialize);
        actor.extension.render_parameter = 0;
        assert_eq!(
            runtime
                .resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects, expected);
        assert!(runtime
            .begin_movement(&mut objects, owner, PlayerDisplacement::default())
            .unwrap());
        assert_eq!(
            runtime
                .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                .unwrap(),
            CallbackStep::Run(at(1))
        );
        assert_eq!(
            runtime
                .resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 4)
                .unwrap()
                .step,
            ControlStep::ResumeCallbacks
        );
        assert_eq!(
            runtime
                .step_callbacks(&mut objects, owner, TriggerWorldInputs::default())
                .unwrap(),
            CallbackStep::Complete
        );
        runtime
            .finish_movement(&mut objects, &mut [None; 2])
            .unwrap();
        let actor = expected.get_mut(owner).unwrap();
        actor.base.position = Vector3 { x: 3, y: -4, z: 5 };
        actor.extension.path_state.clear_on_path_exit_latch = false;
        if force {
            actor.base.path = Some(at(3));
            actor.base.behavior = Behavior::FollowPath;
            actor.base.wait_timer = 0;
            actor.extension.path_state.repeat_counter = 0;
        } else {
            actor.extension.path_state.part = 0;
        }
        assert_eq!(objects, expected);
        assert_eq!(runtime.resources, before_resources);
        assert_eq!(random, before_random);
    }
}

#[test]
fn material_commands_replace_only_their_optional_record_and_import_preserves_other_state() {
    for importing in [false, true] {
        for suppressed in [false, true] {
            for value in 0..=u8::MAX {
                let statement = if importing {
                    Statement::ImportImpactMaterial {
                        destination: ByteField::Part,
                        next: at(1),
                    }
                } else {
                    Statement::Contact {
                        command: if suppressed {
                            ContactCommand::SuppressedImpactMaterial(value)
                        } else {
                            ContactCommand::OrdinaryImpactMaterial(value)
                        },
                        next: at(1),
                    }
                };
                let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
                let (mut runtime, mut objects, owner, mut random) = setup();
                runtime.branch.invert_next = true;
                let actor = objects.get_mut(owner).unwrap();
                actor.extension.impact_materials.ordinary = Some(!value);
                actor.extension.impact_materials.suppressed = Some(!value);
                actor.base.wait_timer = 79;
                actor.extension.path_state.part = !value;
                let mut expected = objects.clone();
                let actor = expected.get_mut(owner).unwrap();
                actor.base.path = Some(at(1));
                if importing {
                    actor.extension.path_state.part = value;
                } else if suppressed {
                    actor.extension.impact_materials.suppressed = Some(value);
                } else {
                    actor.extension.impact_materials.ordinary = Some(value);
                }
                let mut impact = ImpactState {
                    material: value,
                    pair_suppressed: true,
                };
                let before_random = random;
                let mut inputs = world(&mut random);
                inputs.impact = Some(&mut impact);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(1),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(
                    impact,
                    ImpactState {
                        material: value,
                        pair_suppressed: true
                    }
                );
                assert!(runtime.branch.invert_next);
                assert_eq!(random, before_random);
            }
        }
    }
}

#[test]
fn impact_dispatch_selects_each_edge_immediately_without_consuming_ifnot_or_randomness() {
    let catalog = PathCatalog::new(vec![vec![Statement::ImpactBranch {
        first: at(1),
        second: at(2),
        third: at(3),
        next: at(4),
    }]])
    .unwrap();
    for expected_edge in 1..=4 {
        for inverted in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let peer = objects
                .allocate(Object::new(
                    ObjectKind::Enemy,
                    ShapeId::EMPTY,
                    Behavior::FollowPath,
                ))
                .unwrap();
            let target = objects.get_mut(peer).unwrap();
            target.base.flags.exclude_from_shape_footprint_search = true;
            target.base.hit_points = if expected_edge == 1 { 0 } else { 255 };
            target.base.contacts.suppress_contacts_next_epoch = expected_edge == 3;
            target.extension.impact_materials.ordinary = Some(42);
            target.extension.impact_materials.suppressed = Some(51);
            objects.get_mut(owner).unwrap().base.contacts.pending_hit = expected_edge != 4;
            let mut contacts = ContactStore::default();
            contacts.record_pair(owner, peer, [None; 2]).unwrap();
            let mut expected = objects.clone();
            expected.get_mut(owner).unwrap().base.path = Some(at(expected_edge));
            let mut impact = ImpactState {
                material: 231,
                pair_suppressed: true,
            };
            let before_random = random;
            let mut inputs = world(&mut random);
            inputs.impact = Some(&mut impact);
            inputs.contacts = Some(&contacts);
            inputs.surface_mode = Some(Default::default());
            runtime.branch.invert_next = inverted;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(expected_edge),
                    executed: 1
                })
            );
            assert_eq!(objects, expected);
            assert_eq!(
                impact.material,
                match expected_edge {
                    2 => 42,
                    3 => 51,
                    _ => 0,
                }
            );
            assert_eq!(impact.pair_suppressed, expected_edge >= 3);
            assert_eq!(runtime.branch.invert_next, inverted);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn impact_operations_require_explicit_shared_state_before_mutation() {
    for statement in [
        Statement::ImpactBranch {
            first: at(1),
            second: at(2),
            third: at(3),
            next: at(4),
        },
        Statement::ImportImpactMaterial {
            destination: ByteField::Part,
            next: at(1),
        },
    ] {
        let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let before = objects.clone();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingImpactState)
        );
        assert_eq!(objects, before);
    }
}
