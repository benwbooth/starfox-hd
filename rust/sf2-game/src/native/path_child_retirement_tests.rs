//! Numbered-child retirement is a deferred flag, not unlinking or death.
use super::super::path_relationships::{RelationshipCommand, RelationshipError};
use super::super::path_triggers::{Trigger, TriggerKind};
use super::super::{Behavior, ObjectKind, PathId, ShapeId};
use super::tests::{setup, world};
use super::*;

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

fn actor(number: u8) -> Object {
    let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
    actor.base.child_number = number;
    actor.base.hit_points = 129;
    actor.base.wait_timer = 78;
    actor.base.path = Some(at(0));
    actor
}

fn catalog(number: u8) -> PathCatalog {
    PathCatalog::new(vec![vec![Statement::Relationship {
        command: RelationshipCommand::RetireChild { number },
        next: at(1),
    }]])
    .unwrap()
}

#[test]
fn all_number_bytes_mark_only_first_match_on_flag_selected_chain_and_preserve_resources() {
    for number in 0..=u8::MAX {
        for owns_chain in [false, true] {
            for inverted in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let mother = objects.allocate(actor(number.wrapping_add(1))).unwrap();
                let first = objects.allocate(actor(number)).unwrap();
                let duplicate = objects.allocate(actor(number)).unwrap();
                let ignored = objects.allocate(actor(number)).unwrap();
                let parent = if owns_chain { owner } else { mother };
                let actor = objects.get_mut(owner).unwrap();
                actor.base.attachment = Some(mother);
                actor.extension.path_state.motion.refresh_child_chain = owns_chain;
                actor.extension.path_state.motion.attached_coordinates = !owns_chain;
                actor.extension.path_state.motion.suppress_child_refresh = true;
                actor.base.wait_timer = 227;
                objects
                    .get_mut(if owns_chain { mother } else { owner })
                    .unwrap()
                    .base
                    .first_child = Some(ignored);
                objects.get_mut(parent).unwrap().base.first_child = Some(first);
                objects.get_mut(first).unwrap().base.next_sibling = Some(duplicate);
                for child in [first, duplicate] {
                    objects.get_mut(child).unwrap().base.attachment = Some(parent);
                    runtime
                        .add_trigger(
                            &mut objects,
                            child,
                            Trigger {
                                path: at(0),
                                kind: TriggerKind::Always,
                                timer: 0,
                            },
                        )
                        .unwrap();
                }
                runtime.branch.invert_next = inverted;
                let resources = runtime.resources.clone();
                let random_before = random;
                let mut expected = objects.clone();
                expected
                    .get_mut(first)
                    .unwrap()
                    .base
                    .flags
                    .remove_after_tick = true;
                expected.get_mut(owner).unwrap().base.path = Some(at(1));
                assert_eq!(
                    runtime.resume_program(
                        &catalog(number),
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
                assert_eq!(objects, expected);
                assert_eq!(runtime.resources, resources);
                assert_eq!(runtime.branch.invert_next, inverted);
                assert_eq!(random, random_before);
                // A second immediate mark still finds the same linked child,
                // even though its deferred-retirement flag is already set.
                objects.get_mut(owner).unwrap().base.path = Some(at(0));
                assert_eq!(
                    runtime.resume_program(
                        &catalog(number),
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
                assert_eq!(objects, expected);
            }
        }
    }
}

#[test]
fn absent_and_malformed_targets_fault_without_mutation_or_cursor_advance() {
    for case in 0..5 {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let mother = objects.allocate(actor(0)).unwrap();
        let child = objects.allocate(actor(1)).unwrap();
        let missing = objects.allocate(actor(0)).unwrap();
        objects.remove(missing).unwrap();
        let expected = match case {
            0 => RelationshipError::MissingParent(owner),
            1 => {
                objects.get_mut(owner).unwrap().base.attachment = Some(missing);
                RelationshipError::MissingActor(missing)
            }
            2 => {
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion
                    .refresh_child_chain = true;
                RelationshipError::MissingChild { owner, number: 255 }
            }
            _ => {
                objects.get_mut(owner).unwrap().base.attachment = Some(mother);
                objects.get_mut(mother).unwrap().base.first_child = Some(child);
                objects.get_mut(child).unwrap().base.next_sibling =
                    Some(if case == 3 { missing } else { child });
                if case == 3 {
                    RelationshipError::MissingActor(missing)
                } else {
                    RelationshipError::ChildCycle(child)
                }
            }
        };
        let before = objects.clone();
        let resources = runtime.resources.clone();
        assert_eq!(
            runtime.resume_program(
                &catalog(255),
                &mut objects,
                owner,
                &mut world(&mut random),
                1
            ),
            Err(ProgramError::Relationship(expected))
        );
        assert_eq!(objects, before);
        assert_eq!(runtime.resources, resources);
    }
}
