//! Context-aware native dispatch, including explicit scheduler boundaries.
use super::super::path_control::PlayerTarget;
use super::super::path_fields::{ByteField, ByteOperation};
use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
use super::super::path_triggers::{Trigger, TriggerKind};
use super::super::{Behavior, ObjectKind, PathId, ShapeId};
use super::tests::{setup, world};
use super::*;

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn actor(objects: &mut ObjectStore, path: Option<PathCursor>) -> ObjectId {
    let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Effect);
    actor.base.path = path;
    actor.base.wait_timer = 137;
    actor.base.hit_points = 41;
    actor.extension.path_state.needs_path_initialization = true;
    objects.allocate(actor).unwrap()
}

fn assign(field: ByteField, value: ByteOperand, next: u16) -> Statement {
    Statement::Mutate {
        mutation: Mutation::Byte {
            field,
            operation: ByteOperation::Assign(value),
        },
        next: at(next),
    }
}

#[test]
fn nearest_shape_service_changes_only_nearest_weapon_selection_then_restores_and_ends() {
    use super::super::authored_paths;
    let catalog = authored_paths::catalog();
    let Statement::Relationship { next, .. } = catalog.statement(authored_paths::NEAREST_SHAPE_WEAPON_DISABLE_SERVICE).unwrap() else { panic!("search entry") };
    let Statement::SelectActor { selection: ActorSelection::LinkedOrBranch { missing: terminal }, .. } = catalog.statement(next).unwrap() else { panic!("search branch") };
    assert_eq!(catalog.statement(terminal).unwrap(), Statement::Control(ControlCommand::End));
    for present in [false, true] {
        for inverted in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let shape = ShapeId::from_catalog_index(3);
            let far = actor(&mut objects, Some(at(99)));
            let near = actor(&mut objects, Some(at(77)));
            for (id, distance) in [(far, 500), (near, 100)] {
                let candidate = objects.get_mut(id).unwrap();
                candidate.base.shape = if present { shape } else { ShapeId::EMPTY };
                candidate.base.position.x = distance;
                candidate.base.flags.general_search_eligible = false;
                candidate.extension.path_state.weapon_selection = 7;
                candidate.extension.path_state.conditions.selected_player = PlayerTarget::Primary;
            }
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::NEAREST_SHAPE_WEAPON_DISABLE_SERVICE);
            actor.base.shape = shape;
            actor.base.attachment = Some(far);
            actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
            actor.extension.path_state.weapon_selection = 3;
            let mut expected_owner = actor.clone();
            let mut expected_near = objects.get(near).unwrap().clone();
            let expected_far = objects.get(far).unwrap().clone();
            runtime.branch.invert_next = inverted;
            let original_random = random;
            assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 5),
                Ok(ProgramExit { actor: owner, step: ControlStep::Ended }));
            expected_owner.base.path = Some(terminal);
            expected_owner.base.flags.remove_after_tick = true;
            expected_owner.base.attachment = present.then_some(near);
            if present {
                expected_near.extension.path_state.weapon_selection = 255;
            }
            assert_eq!(objects.get(owner), Some(&expected_owner));
            assert_eq!(objects.get(near), Some(&expected_near));
            assert_eq!(objects.get(far), Some(&expected_far));
            assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
            assert_eq!(runtime.branch.invert_next, inverted);
            assert_eq!(runtime.program_actor(), Some(owner));
            assert_eq!(random, original_random);
        }
    }
}

#[test]
fn switched_yield_reports_borrowed_actor_without_refreshing_selection_or_initializing_it() {
    for original_path in [None, Some(at(99))] {
        for selection in [
            ActorSelection::LastSpawn,
            ActorSelection::Linked,
            ActorSelection::LinkedOrBranch { missing: at(99) },
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let borrowed = actor(&mut objects, original_path);
            runtime.spawns.last_spawn = Some(borrowed);
            runtime.branch.invert_next = true;
            objects.get_mut(owner).unwrap().base.attachment = Some(borrowed);
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .conditions
                .selected_player = PlayerTarget::Secondary;
            let owner_before = objects.get(owner).unwrap().clone();
            let catalog = PathCatalog::new(vec![vec![
                Statement::SelectActor {
                    selection,
                    next: at(1),
                },
                assign(
                    ByteField::AttackPower,
                    ByteOperand::Actor(ByteField::Health),
                    2,
                ),
                Statement::Control(ControlCommand::WaitOne { next: at(3) }),
                Statement::RestoreActor { next: at(4) },
                assign(ByteField::AttackPower, ByteOperand::Literal(71), 5),
                Statement::Control(ControlCommand::End),
            ]])
            .unwrap();
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 3),
                Ok(ProgramExit {
                    actor: borrowed,
                    step: ControlStep::Movement
                })
            );
            assert_eq!(objects.get(owner).unwrap(), &owner_before);
            let target = objects.get(borrowed).unwrap();
            assert_eq!(target.base.path, Some(at(3)));
            assert_eq!(target.base.attack_power, 41);
            assert_eq!(target.base.wait_timer, 137);
            assert!(target.extension.path_state.needs_path_initialization);
            assert_eq!(target.base.behavior, Behavior::Effect);
            assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
            assert_eq!(runtime.program_actor(), Some(borrowed));
            assert!(runtime.branch.invert_next);
            let context = runtime.actor_context;
            assert_eq!(
                runtime.resume_program(
                    &catalog,
                    &mut objects,
                    borrowed,
                    &mut world(&mut random),
                    3
                ),
                Ok(ProgramExit {
                    actor: owner,
                    step: ControlStep::Ended
                })
            );
            assert_eq!(objects.get(borrowed).unwrap().base.path, original_path);
            assert_eq!(objects.get(borrowed).unwrap().base.attack_power, 41);
            assert_eq!(objects.get(owner).unwrap().base.attack_power, 71);
            assert_eq!(runtime.actor_context, context);
            assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
            assert!(runtime.branch.invert_next);
        }
    }
}

#[test]
fn budget_errors_retain_current_actor_and_continuation_without_inventing_a_tick() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    let borrowed = actor(&mut objects, Some(at(99)));
    runtime.spawns.last_spawn = Some(borrowed);
    let catalog = PathCatalog::new(vec![vec![
        Statement::SelectActor {
            selection: ActorSelection::LastSpawn,
            next: at(1),
        },
        assign(ByteField::Health, ByteOperand::Literal(251), 2),
        Statement::RestoreActor { next: at(3) },
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    assert_eq!(
        runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::BudgetExceeded {
            cursor: at(1),
            executed: 1
        })
    );
    assert_eq!(runtime.program_actor(), Some(borrowed));
    assert_eq!(objects.get(borrowed).unwrap().base.hit_points, 41);
    assert_eq!(objects.get(owner).unwrap().base.path, Some(at(0)));
    let next_actor = runtime.program_actor().unwrap();
    assert_eq!(
        runtime.resume_program(
            &catalog,
            &mut objects,
            next_actor,
            &mut world(&mut random),
            1
        ),
        Err(ProgramError::BudgetExceeded {
            cursor: at(2),
            executed: 1
        })
    );
    assert_eq!(runtime.program_actor(), Some(borrowed));
    assert_eq!(objects.get(borrowed).unwrap().base.hit_points, 251);
    assert_eq!(objects.get(borrowed).unwrap().base.wait_timer, 137);
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, borrowed, &mut world(&mut random), 1),
        Err(ProgramError::BudgetExceeded {
            cursor: at(3),
            executed: 1
        })
    );
    assert_eq!(runtime.program_actor(), Some(owner));
    assert_eq!(objects.get(borrowed).unwrap().base.path, Some(at(99)));
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Ok(ProgramExit {
            actor: owner,
            step: ControlStep::Ended
        })
    );
}

#[test]
fn movement_service_uses_the_returned_actor_without_moving_the_original_caller() {
    use super::super::path_motion::PlayerDisplacement;
    use super::super::Vector3;
    let (mut runtime, mut objects, owner, mut random) = setup();
    let borrowed = actor(&mut objects, Some(at(99)));
    objects.get_mut(owner).unwrap().base.velocity = Vector3 {
        x: 91,
        y: 92,
        z: 93,
    };
    objects.get_mut(borrowed).unwrap().base.velocity = Vector3 { x: 5, y: -7, z: 9 };
    let caller = objects.get(owner).unwrap().clone();
    runtime.spawns.last_spawn = Some(borrowed);
    let catalog = PathCatalog::new(vec![vec![
        Statement::SelectActor {
            selection: ActorSelection::LastSpawn,
            next: at(1),
        },
        Statement::Control(ControlCommand::WaitOne { next: at(2) }),
        Statement::RestoreActor { next: at(3) },
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    let exit = runtime
        .enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 2)
        .unwrap();
    assert_eq!(
        exit,
        ProgramExit {
            actor: borrowed,
            step: ControlStep::Movement
        }
    );
    assert_eq!(
        runtime.begin_movement(
            &mut objects,
            exit.actor,
            PlayerDisplacement {
                world_delta: Vector3::default(),
                suppress_horizontal: false,
            }
        ),
        Ok(false)
    );
    runtime
        .finish_movement(&mut objects, &mut [None, None])
        .unwrap();
    assert_eq!(objects.get(owner).unwrap(), &caller);
    assert_eq!(
        objects.get(borrowed).unwrap().base.position,
        Vector3 { x: 5, y: -7, z: 9 }
    );
    assert_eq!(
        runtime.resume_program(
            &catalog,
            &mut objects,
            exit.actor,
            &mut world(&mut random),
            2
        ),
        Ok(ProgramExit {
            actor: owner,
            step: ControlStep::Ended
        })
    );
    assert_eq!(objects.get(borrowed).unwrap().base.path, Some(at(99)));
}

#[test]
fn child_operand_is_read_before_switch_and_missing_branches_preserve_ifnot_and_wait() {
    for number in u8::MIN..=u8::MAX {
        for from_field in [false, true] {
            for present in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let child = actor(&mut objects, None);
                objects.get_mut(child).unwrap().base.child_number = number;
                let parent = objects.get_mut(owner).unwrap();
                parent.extension.path_state.motion.refresh_child_chain = true;
                parent.base.first_child = present.then_some(child);
                parent.base.hit_points = number;
                parent.base.wait_timer = 231;
                runtime.branch.invert_next = true;
                let catalog = PathCatalog::new(vec![vec![
                    Statement::SelectChild {
                        number: if from_field {
                            ByteOperand::Actor(ByteField::Health)
                        } else {
                            ByteOperand::Literal(number)
                        },
                        missing: at(4),
                        next: at(1),
                    },
                    assign(ByteField::AttackPower, ByteOperand::Literal(93), 2),
                    Statement::RestoreActor { next: at(3) },
                    Statement::Control(ControlCommand::WaitOne { next: at(5) }),
                    Statement::Control(ControlCommand::WaitOne { next: at(6) }),
                ]])
                .unwrap();
                assert_eq!(
                    runtime.enter_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        4
                    ),
                    Ok(ProgramExit {
                        actor: owner,
                        step: ControlStep::Movement
                    })
                );
                assert_eq!(
                    objects.get(owner).unwrap().base.path,
                    Some(at(if present { 5 } else { 6 }))
                );
                assert_eq!(objects.get(owner).unwrap().base.wait_timer, 231);
                assert_eq!(
                    objects.get(child).unwrap().base.attack_power,
                    if present { 93 } else { 0 }
                );
                assert_eq!(objects.get(child).unwrap().base.path, None);
                assert!(runtime.branch.invert_next);
            }
        }
    }
}

#[test]
fn callback_switch_keeps_calls_and_mutations_on_borrowed_actor_then_restores_batch_owner() {
    for restore_before_return in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let borrowed = actor(&mut objects, Some(at(99)));
        let unrelated = actor(&mut objects, Some(at(98)));
        runtime.spawns.last_spawn = Some(borrowed);
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .selected_player = PlayerTarget::Secondary;
        let catalog = PathCatalog::new(vec![vec![
            Statement::SelectActor {
                selection: ActorSelection::LastSpawn,
                next: at(1),
            },
            Statement::Control(ControlCommand::Call {
                target: at(5),
                next: at(2),
            }),
            if restore_before_return {
                Statement::RestoreActor { next: at(3) }
            } else {
                Statement::Control(ControlCommand::Return)
            },
            Statement::Control(ControlCommand::Return),
            Statement::Control(ControlCommand::End),
            assign(
                ByteField::AttackPower,
                ByteOperand::Actor(ByteField::Health),
                6,
            ),
            Statement::Control(ControlCommand::Return),
        ]])
        .unwrap();
        runtime
            .add_trigger(
                &mut objects,
                owner,
                Trigger {
                    path: at(0),
                    kind: TriggerKind::Always,
                    timer: 0,
                },
            )
            .unwrap();
        objects.get_mut(owner).unwrap().base.path = Some(at(88));
        assert_eq!(runtime.begin_callbacks(&objects, owner), Ok(true));
        assert_eq!(
            runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
            Ok(CallbackStep::Run(at(0)))
        );
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
            Err(ProgramError::BudgetExceeded {
                cursor: at(5),
                executed: 2
            })
        );
        assert_eq!(runtime.program_actor(), Some(borrowed));
        assert_eq!(runtime.resources.owner_count(borrowed), 1);
        assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
        let before = objects.clone();
        assert_eq!(
            runtime.resume_program(
                &catalog,
                &mut objects,
                unrelated,
                &mut world(&mut random),
                1
            ),
            Err(ProgramError::Runtime(PathRuntimeError::WrongCallbackOwner))
        );
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::Runtime(PathRuntimeError::WrongCallbackOwner))
        );
        assert_eq!(objects, before);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, borrowed, &mut world(&mut random), 4),
            Ok(ProgramExit {
                actor: owner,
                step: ControlStep::ResumeCallbacks
            })
        );
        assert_eq!(runtime.program_actor(), Some(owner));
        assert_eq!(objects.get(borrowed).unwrap().base.attack_power, 41);
        assert_eq!(objects.get(owner).unwrap().base.attack_power, 0);
        assert_eq!(
            objects.get(borrowed).unwrap().base.path,
            Some(at(if restore_before_return { 99 } else { 2 }))
        );
        assert_eq!(runtime.selected_player(), PlayerTarget::Secondary);
        let context = runtime.actor_context;
        assert_eq!(
            runtime.step_callbacks(&mut objects, owner, TriggerWorldInputs::default()),
            Ok(CallbackStep::Complete)
        );
        assert_eq!(objects.get(owner).unwrap().base.path, Some(at(88)));
        assert_eq!(runtime.actor_context, context);
    }
}

#[test]
fn fresh_common_entry_replaces_current_actor_but_keeps_saved_context_pair() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    let borrowed = actor(&mut objects, Some(at(99)));
    let other = actor(&mut objects, Some(at(2)));
    objects
        .get_mut(other)
        .unwrap()
        .extension
        .path_state
        .needs_path_initialization = false;
    runtime.spawns.last_spawn = Some(borrowed);
    let catalog = PathCatalog::new(vec![vec![
        Statement::SelectActor {
            selection: ActorSelection::LastSpawn,
            next: at(1),
        },
        Statement::Control(ControlCommand::Hold),
        Statement::RestoreActor { next: at(3) },
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    assert_eq!(
        runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 2),
        Ok(ProgramExit {
            actor: borrowed,
            step: ControlStep::Movement
        })
    );
    let context = runtime.actor_context;
    // Source UNBECOME restores its current actor, not a separately retained
    // borrowed identity. A subsequent entry can therefore use the old pair.
    assert_eq!(
        runtime.enter_program(&catalog, &mut objects, other, &mut world(&mut random), 2),
        Ok(ProgramExit {
            actor: owner,
            step: ControlStep::Ended
        })
    );
    assert_eq!(objects.get(other).unwrap().base.path, Some(at(99)));
    assert_eq!(objects.get(borrowed).unwrap().base.path, Some(at(1)));
    assert_eq!(runtime.actor_context, context);
}
