//! Temporary path ownership (`$7F:A86D..A995`).
//!
//! The source retains ONE caller and ONE borrowed path, independently. A
//! second selection overwrites them; restoration neither pops nor clears
//! them. Only path ownership changes: player selection, callbacks, waits,
//! stacks, transforms and attachment relationships stay with their actors.

use super::path_relationships::{find_child, RelationshipError};
use super::{ObjectId, ObjectStore, PathCursor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorSelection {
    LastSpawn,
    Linked,
    LinkedOrBranch {
        missing: PathCursor,
    },
    /// Literal and field-derived forms share this operation after reading
    /// the complete number byte from the original executing actor.
    ChildOrBranch {
        number: u8,
        missing: PathCursor,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorContextError {
    MissingActor(ObjectId),
    MissingPath(ObjectId),
    MissingLinkedActor(ObjectId),
    MissingLastSpawn,
    MissingSavedCaller,
    Relationships(RelationshipError),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActorContextState {
    saved_caller: Option<ObjectId>,
    /// None is a valid saved path: borrowed actors need not run a path of
    /// their own. It is not an indication that selection never happened.
    saved_target_path: Option<PathCursor>,
}

impl ActorContextState {
    /// Return the actor that executes `next`, or the original actor when
    /// taking a missing-target branch. Continuations are decoded catalog
    /// indices, never arithmetic on original command addresses.
    ///
    /// Invalid native links are diagnosed before mutation. The source has
    /// no valid recovery for null direct targets or malformed child chains.
    pub fn select(
        &mut self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        last_spawn: Option<ObjectId>,
        selection: ActorSelection,
        next: PathCursor,
    ) -> Result<ObjectId, ActorContextError> {
        let actor = objects
            .get(owner)
            .ok_or(ActorContextError::MissingActor(owner))?;
        actor
            .base
            .path
            .ok_or(ActorContextError::MissingPath(owner))?;
        let target = match selection {
            ActorSelection::LastSpawn => last_spawn.ok_or(ActorContextError::MissingLastSpawn)?,
            ActorSelection::Linked => actor
                .base
                .attachment
                .ok_or(ActorContextError::MissingLinkedActor(owner))?,
            ActorSelection::LinkedOrBranch { missing } => {
                if let Some(linked) = actor.base.attachment {
                    linked
                } else {
                    // $A894 branches BEFORE saving either context value.
                    objects.get_mut(owner).expect("validated caller").base.path = Some(missing);
                    return Ok(owner);
                }
            }
            ActorSelection::ChildOrBranch { number, missing } => {
                // Unlike the child-missing predicate, these handlers have
                // no null-mother guard before numbered-child traversal.
                if !actor.extension.path_state.motion.refresh_child_chain
                    && actor.base.attachment.is_none()
                {
                    return Err(ActorContextError::MissingLinkedActor(owner));
                }
                let child =
                    find_child(objects, owner, number).map_err(ActorContextError::Relationships)?;
                if let Some(child) = child {
                    child
                } else {
                    // $A8C7 / $A912 save the caller even on lookup failure,
                    // leaving the previous borrowed-path value untouched.
                    self.saved_caller = Some(owner);
                    objects.get_mut(owner).expect("validated caller").base.path = Some(missing);
                    return Ok(owner);
                }
            }
        };
        let saved_path = objects
            .get(target)
            .ok_or(ActorContextError::MissingActor(target))?
            .base
            .path;
        self.saved_caller = Some(owner);
        self.saved_target_path = saved_path;
        objects
            .get_mut(target)
            .expect("validated selected actor")
            .base
            .path = Some(next);
        Ok(target)
    }

    /// Restore the current actor's saved path and put the continuation on
    /// the saved caller. The write order matters when both IDs are equal:
    /// the continuation wins. Saved values remain available for reuse.
    pub fn restore(
        &self,
        objects: &mut ObjectStore,
        owner: ObjectId,
        next: PathCursor,
    ) -> Result<ObjectId, ActorContextError> {
        objects
            .get(owner)
            .ok_or(ActorContextError::MissingActor(owner))?
            .base
            .path
            .ok_or(ActorContextError::MissingPath(owner))?;
        let caller = self
            .saved_caller
            .ok_or(ActorContextError::MissingSavedCaller)?;
        objects
            .get(caller)
            .ok_or(ActorContextError::MissingActor(caller))?;
        objects
            .get_mut(owner)
            .expect("validated borrowed actor")
            .base
            .path = self.saved_target_path;
        objects
            .get_mut(caller)
            .expect("validated saved caller")
            .base
            .path = Some(next);
        Ok(caller)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, PathId, ShapeId};

    fn cursor(index: u16) -> PathCursor {
        PathCursor {
            path: PathId::from_catalog_index(index / 10),
            command_index: index % 10,
        }
    }

    fn actor(index: u16) -> Object {
        let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        actor.base.path = Some(cursor(index));
        actor.base.wait_timer = index as u8;
        actor.base.hit_points = index as u8;
        actor.base.position.x = -(index as i16);
        actor.extension.path_state.repeat_counter = index as u8;
        actor.extension.path_state.script_value = index;
        actor
    }

    #[test]
    fn direct_forms_replace_only_target_path_then_restore_only_paths() {
        for selection in [
            ActorSelection::LastSpawn,
            ActorSelection::Linked,
            ActorSelection::LinkedOrBranch {
                missing: cursor(99),
            },
        ] {
            for target_has_path in [false, true] {
                let mut objects = ObjectStore::new();
                let caller = objects.allocate(actor(11)).unwrap();
                let mut target_actor = actor(22);
                if !target_has_path {
                    target_actor.base.path = None;
                }
                let target = objects.allocate(target_actor).unwrap();
                objects.get_mut(caller).unwrap().base.attachment = Some(target);
                let original = objects.clone();
                let mut context = ActorContextState::default();
                assert_eq!(
                    context.select(&mut objects, caller, Some(target), selection, cursor(33)),
                    Ok(target)
                );
                let mut expected = original.clone();
                expected.get_mut(target).unwrap().base.path = Some(cursor(33));
                assert_eq!(objects, expected);
                assert_eq!(context.saved_caller, Some(caller));
                assert_eq!(
                    context.saved_target_path,
                    original.get(target).unwrap().base.path
                );
                let saved = context;
                assert_eq!(
                    context.restore(&mut objects, target, cursor(44)),
                    Ok(caller)
                );
                let mut expected = original;
                expected.get_mut(caller).unwrap().base.path = Some(cursor(44));
                assert_eq!(objects, expected);
                assert_eq!(context, saved);
            }
        }
    }

    #[test]
    fn numbered_selection_reads_full_byte_and_searches_flag_selected_chain_in_order() {
        for number in u8::MIN..=u8::MAX {
            for owns_chain in [false, true] {
                let mut objects = ObjectStore::new();
                let caller = objects.allocate(actor(11)).unwrap();
                let parent = objects.allocate(actor(22)).unwrap();
                let wrong = objects.allocate(actor(33)).unwrap();
                let first_match = objects.allocate(actor(44)).unwrap();
                let duplicate = objects.allocate(actor(55)).unwrap();
                let actor = objects.get_mut(caller).unwrap();
                actor.base.attachment = Some(parent);
                actor.extension.path_state.motion.refresh_child_chain = owns_chain;
                let root = if owns_chain { caller } else { parent };
                let ignored = if owns_chain { parent } else { caller };
                objects.get_mut(root).unwrap().base.first_child = Some(wrong);
                objects.get_mut(ignored).unwrap().base.first_child = Some(duplicate);
                objects.get_mut(wrong).unwrap().base.child_number = number.wrapping_add(1);
                objects.get_mut(wrong).unwrap().base.next_sibling = Some(first_match);
                objects.get_mut(first_match).unwrap().base.child_number = number;
                objects.get_mut(first_match).unwrap().base.next_sibling = Some(duplicate);
                objects.get_mut(duplicate).unwrap().base.child_number = number;
                let mut expected = objects.clone();
                expected.get_mut(first_match).unwrap().base.path = Some(cursor(66));
                let mut context = ActorContextState::default();
                assert_eq!(
                    context.select(
                        &mut objects,
                        caller,
                        Some(duplicate),
                        ActorSelection::ChildOrBranch {
                            number,
                            missing: cursor(77)
                        },
                        cursor(66)
                    ),
                    Ok(first_match)
                );
                assert_eq!(objects, expected);
                assert_eq!(context.saved_caller, Some(caller));
                assert_eq!(context.saved_target_path, Some(cursor(44)));
            }
        }
    }

    #[test]
    fn missing_mother_retains_both_slots_but_missing_child_replaces_only_caller() {
        for prior_selection in [false, true] {
            let mut objects = ObjectStore::new();
            let previous = objects.allocate(actor(11)).unwrap();
            let borrowed = objects.allocate(actor(22)).unwrap();
            let caller = objects.allocate(actor(33)).unwrap();
            let mut context = ActorContextState::default();
            if prior_selection {
                context
                    .select(
                        &mut objects,
                        previous,
                        Some(borrowed),
                        ActorSelection::LastSpawn,
                        cursor(44),
                    )
                    .unwrap();
            }
            let saved = context;
            assert_eq!(
                context.select(
                    &mut objects,
                    caller,
                    None,
                    ActorSelection::LinkedOrBranch {
                        missing: cursor(55)
                    },
                    cursor(66)
                ),
                Ok(caller)
            );
            assert_eq!(context, saved);
            assert_eq!(objects.get(caller).unwrap().base.path, Some(cursor(55)));
            objects
                .get_mut(caller)
                .unwrap()
                .extension
                .path_state
                .motion
                .refresh_child_chain = true;
            assert_eq!(
                context.select(
                    &mut objects,
                    caller,
                    None,
                    ActorSelection::ChildOrBranch {
                        number: 255,
                        missing: cursor(77)
                    },
                    cursor(88)
                ),
                Ok(caller)
            );
            assert_eq!(context.saved_caller, Some(caller));
            assert_eq!(context.saved_target_path, saved.saved_target_path);
            // Restoration uses these independently saved slots even though
            // the most recent lookup never switched to a borrowed actor.
            let saved = context;
            assert_eq!(
                context.restore(&mut objects, borrowed, cursor(99)),
                Ok(caller)
            );
            assert_eq!(
                objects.get(borrowed).unwrap().base.path,
                saved.saved_target_path
            );
            assert_eq!(objects.get(caller).unwrap().base.path, Some(cursor(99)));
            assert_eq!(context, saved);
        }
    }

    #[test]
    fn nested_selection_overwrites_single_slot_and_repeated_restore_does_not_pop() {
        let mut objects = ObjectStore::new();
        let first = objects.allocate(actor(11)).unwrap();
        let second = objects.allocate(actor(22)).unwrap();
        let third = objects.allocate(actor(33)).unwrap();
        let mut context = ActorContextState::default();
        context
            .select(
                &mut objects,
                first,
                Some(second),
                ActorSelection::LastSpawn,
                cursor(44),
            )
            .unwrap();
        context
            .select(
                &mut objects,
                second,
                Some(third),
                ActorSelection::LastSpawn,
                cursor(55),
            )
            .unwrap();
        let saved = context;
        assert_eq!(context.restore(&mut objects, third, cursor(66)), Ok(second));
        assert_eq!(objects.get(third).unwrap().base.path, Some(cursor(33)));
        assert_eq!(
            context.restore(&mut objects, second, cursor(77)),
            Ok(second)
        );
        assert_eq!(objects.get(first).unwrap().base.path, Some(cursor(11)));
        assert_eq!(objects.get(second).unwrap().base.path, Some(cursor(77)));
        assert_eq!(context, saved);
    }

    #[test]
    fn self_selection_saves_original_path_and_restore_continuation_wins_alias() {
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(actor(11)).unwrap();
        let mut context = ActorContextState::default();
        context
            .select(
                &mut objects,
                owner,
                Some(owner),
                ActorSelection::LastSpawn,
                cursor(22),
            )
            .unwrap();
        assert_eq!(context.saved_target_path, Some(cursor(11)));
        let saved = context;
        assert_eq!(context.restore(&mut objects, owner, cursor(33)), Ok(owner));
        assert_eq!(objects.get(owner).unwrap().base.path, Some(cursor(33)));
        assert_eq!(context, saved);
    }

    #[test]
    fn invalid_direct_targets_and_null_child_parent_fault_without_mutating_objects_or_context() {
        let mut objects = ObjectStore::new();
        let caller = objects.allocate(actor(11)).unwrap();
        let stale = objects.allocate(actor(22)).unwrap();
        objects.remove(stale).unwrap();
        let mut context = ActorContextState::default();
        let cases = [
            (
                None,
                ActorSelection::LastSpawn,
                ActorContextError::MissingLastSpawn,
            ),
            (
                Some(stale),
                ActorSelection::LastSpawn,
                ActorContextError::MissingActor(stale),
            ),
            (
                None,
                ActorSelection::Linked,
                ActorContextError::MissingLinkedActor(caller),
            ),
            (
                None,
                ActorSelection::ChildOrBranch {
                    number: 0,
                    missing: cursor(33),
                },
                ActorContextError::MissingLinkedActor(caller),
            ),
        ];
        let original = objects.clone();
        for (spawn, selection, expected) in cases {
            assert_eq!(
                context.select(&mut objects, caller, spawn, selection, cursor(44)),
                Err(expected)
            );
            assert_eq!(objects, original);
            assert_eq!(context, ActorContextState::default());
        }
        assert_eq!(
            context.restore(&mut objects, caller, cursor(44)),
            Err(ActorContextError::MissingSavedCaller)
        );
        objects.get_mut(caller).unwrap().base.attachment = Some(stale);
        let original = objects.clone();
        assert_eq!(
            context.select(
                &mut objects,
                caller,
                None,
                ActorSelection::LinkedOrBranch {
                    missing: cursor(33)
                },
                cursor(44)
            ),
            Err(ActorContextError::MissingActor(stale))
        );
        assert_eq!(objects, original);
    }

    #[test]
    fn malformed_child_chains_and_missing_restore_caller_are_explicit_errors() {
        let mut objects = ObjectStore::new();
        let caller = objects.allocate(actor(11)).unwrap();
        let child = objects.allocate(actor(22)).unwrap();
        let mut context = ActorContextState::default();
        context
            .select(
                &mut objects,
                caller,
                Some(child),
                ActorSelection::LastSpawn,
                cursor(33),
            )
            .unwrap();
        objects
            .get_mut(caller)
            .unwrap()
            .extension
            .path_state
            .motion
            .refresh_child_chain = true;
        objects.get_mut(caller).unwrap().base.first_child = Some(child);
        objects.get_mut(child).unwrap().base.next_sibling = Some(child);
        let saved = context;
        let original = objects.clone();
        assert_eq!(
            context.select(
                &mut objects,
                caller,
                None,
                ActorSelection::ChildOrBranch {
                    number: 1,
                    missing: cursor(44)
                },
                cursor(55)
            ),
            Err(ActorContextError::Relationships(
                RelationshipError::ChildCycle(child)
            ))
        );
        assert_eq!(objects, original);
        assert_eq!(context, saved);
        objects.get_mut(child).unwrap().base.next_sibling = None;
        objects.get_mut(caller).unwrap().base.first_child = None;
        objects.remove(caller).unwrap();
        let original = objects.clone();
        assert_eq!(
            context.restore(&mut objects, child, cursor(66)),
            Err(ActorContextError::MissingActor(caller))
        );
        assert_eq!(objects, original);
        assert_eq!(context, saved);
    }
}
