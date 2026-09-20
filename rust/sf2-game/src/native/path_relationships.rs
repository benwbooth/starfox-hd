//! Authored child attachment, lookup and detachment (`$7F:2A3D..2A8F`,
//! `$7F:9435..94D8`).
//! Detachment changes the sibling chain, not the active object list, world
//! pose, extension parent, or resources owned by the detached actor.

use super::{ObjectId, ObjectStore, OBJECT_CAPACITY};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipError {
    MissingSelected,
    MissingActor(ObjectId),
    MissingParent(ObjectId),
    ChildCycle(ObjectId),
    ChildNotFresh(ObjectId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipCommand {
    UnlinkSelf,
    UnlinkChild { number: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedTransformCommand {
    WorldPosition,
    WorldRotation,
}

/// Source $7F:B8CD and $7F:BF4E/$7F:2BE2 copy only the selected world
/// transform channel. Relative transforms and velocity remain untouched.
pub fn copy_selected_transform(
    objects: &mut ObjectStore,
    owner: ObjectId,
    selected: Option<ObjectId>,
    command: SelectedTransformCommand,
) -> Result<(), RelationshipError> {
    objects
        .get(owner)
        .ok_or(RelationshipError::MissingActor(owner))?;
    let selected = selected.ok_or(RelationshipError::MissingSelected)?;
    let target = objects
        .get(selected)
        .ok_or(RelationshipError::MissingActor(selected))?;
    let position = target.base.position;
    let rotation = (target.base.pitch, target.base.yaw, target.base.roll);
    let actor = objects
        .get_mut(owner)
        .expect("validated transform-copy owner");
    match command {
        SelectedTransformCommand::WorldPosition => actor.base.position = position,
        SelectedTransformCommand::WorldRotation => {
            (actor.base.pitch, actor.base.yaw, actor.base.roll) = rotation;
        }
    }
    Ok(())
}

/// Child-producing paths use the caller's mother when attached, otherwise
/// the caller itself (`$7F:90BB..90C8`). This is a single hop, not a root
/// search, and differs from the owner flag used by numbered-child lookup.
pub fn spawn_parent(
    objects: &ObjectStore,
    caller: ObjectId,
) -> Result<ObjectId, RelationshipError> {
    let actor = objects
        .get(caller)
        .ok_or(RelationshipError::MissingActor(caller))?;
    let parent = if actor.extension.path_state.motion.attached_coordinates {
        actor
            .base
            .attachment
            .ok_or(RelationshipError::MissingParent(caller))?
    } else {
        caller
    };
    objects
        .get(parent)
        .ok_or(RelationshipError::MissingActor(parent))?;
    Ok(parent)
}

/// Link a freshly allocated child at the END of the authored sibling chain
/// (`$7F:2A3D`). Active-list insertion is a separate allocation operation.
/// The extension parent is also independent: a child-producing path later
/// sets that field to its caller, which need not be this attachment parent.
///
/// Only fresh children are accepted. Invalid native links are diagnosed
/// before mutation; the source has no recovery from malformed chains.
pub fn attach_fresh_child(
    objects: &mut ObjectStore,
    parent: ObjectId,
    child: ObjectId,
    number: u8,
) -> Result<(), RelationshipError> {
    let actor = objects
        .get(child)
        .ok_or(RelationshipError::MissingActor(child))?;
    if child == parent
        || actor.base.attachment.is_some()
        || actor.base.next_sibling.is_some()
        || actor.base.first_child.is_some()
    {
        return Err(RelationshipError::ChildNotFresh(child));
    }
    let mut current = objects
        .get(parent)
        .ok_or(RelationshipError::MissingActor(parent))?
        .base
        .first_child;
    let mut tail = None;
    let mut visited = [false; OBJECT_CAPACITY];
    visited[parent.index()] = true;
    while let Some(id) = current {
        if id == child {
            return Err(RelationshipError::ChildNotFresh(child));
        }
        if visited[id.index()] {
            return Err(RelationshipError::ChildCycle(id));
        }
        visited[id.index()] = true;
        current = objects
            .get(id)
            .ok_or(RelationshipError::MissingActor(id))?
            .base
            .next_sibling;
        tail = Some(id);
    }

    let actor = objects.get_mut(child).expect("validated fresh child");
    actor.base.child_number = number;
    actor.base.attachment = Some(parent);
    actor.base.next_sibling = None;
    actor.extension.path_state.motion.attached_coordinates = true;
    actor.base.flags.remove_with_parent = true;
    if let Some(tail) = tail {
        objects
            .get_mut(tail)
            .expect("validated tail")
            .base
            .next_sibling = Some(child);
    } else {
        objects
            .get_mut(parent)
            .expect("validated parent")
            .base
            .first_child = Some(child);
    }
    objects
        .get_mut(parent)
        .expect("validated parent")
        .extension
        .path_state
        .motion
        .refresh_child_chain = true;
    Ok(())
}

/// The source owner flag chooses the caller itself. Otherwise search its
/// mother, even when the caller happens to retain a first-child pointer.
pub fn find_child(
    objects: &ObjectStore,
    owner: ObjectId,
    number: u8,
) -> Result<Option<ObjectId>, RelationshipError> {
    let actor = objects
        .get(owner)
        .ok_or(RelationshipError::MissingActor(owner))?;
    let parent = if actor.extension.path_state.motion.refresh_child_chain {
        Some(owner)
    } else {
        actor.base.attachment
    };
    let Some(parent) = parent else {
        return Ok(None);
    };
    let mut child = objects
        .get(parent)
        .ok_or(RelationshipError::MissingActor(parent))?
        .base
        .first_child;
    let mut visited = [false; OBJECT_CAPACITY];
    while let Some(id) = child {
        if visited[id.index()] {
            return Err(RelationshipError::ChildCycle(id));
        }
        visited[id.index()] = true;
        let actor = objects.get(id).ok_or(RelationshipError::MissingActor(id))?;
        if actor.base.child_number == number {
            return Ok(Some(id));
        }
        child = actor.base.next_sibling;
    }
    Ok(None)
}

fn detach(objects: &mut ObjectStore, child: ObjectId) -> Result<(), RelationshipError> {
    let actor = objects
        .get_mut(child)
        .ok_or(RelationshipError::MissingActor(child))?;
    // These flags are cleared BEFORE searching the parent's chain. If the
    // child is absent from that chain its links and identifier remain intact.
    actor.extension.path_state.motion.attached_coordinates = false;
    actor.base.flags.remove_with_parent = false;
    let parent = actor
        .base
        .attachment
        .ok_or(RelationshipError::MissingParent(child))?;
    let successor = actor.base.next_sibling;
    let mut current = objects
        .get(parent)
        .ok_or(RelationshipError::MissingActor(parent))?
        .base
        .first_child;
    let mut previous = None;
    let mut visited = [false; OBJECT_CAPACITY];
    while let Some(id) = current {
        if visited[id.index()] {
            return Err(RelationshipError::ChildCycle(id));
        }
        visited[id.index()] = true;
        if id == child {
            if let Some(previous) = previous {
                objects
                    .get_mut(previous)
                    .expect("validated sibling")
                    .base
                    .next_sibling = successor;
            } else {
                objects
                    .get_mut(parent)
                    .expect("validated parent")
                    .base
                    .first_child = successor;
            }
            let actor = objects.get_mut(child).expect("validated child");
            actor.base.attachment = None;
            actor.base.next_sibling = None;
            actor.base.child_number = 0;
            return Ok(());
        }
        previous = Some(id);
        current = objects
            .get(id)
            .ok_or(RelationshipError::MissingActor(id))?
            .base
            .next_sibling;
    }
    Ok(())
}

pub fn apply(
    objects: &mut ObjectStore,
    owner: ObjectId,
    command: RelationshipCommand,
) -> Result<(), RelationshipError> {
    let child = match command {
        RelationshipCommand::UnlinkSelf => {
            let actor = objects
                .get(owner)
                .ok_or(RelationshipError::MissingActor(owner))?;
            if !actor.extension.path_state.motion.attached_coordinates {
                return Ok(());
            }
            Some(owner)
        }
        RelationshipCommand::UnlinkChild { number } => find_child(objects, owner, number)?,
    };
    if let Some(child) = child {
        detach(objects, child)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ShapeId};

    fn actor() -> Object {
        Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::FollowPath)
    }

    fn family() -> (ObjectStore, ObjectId, [ObjectId; 3]) {
        let mut objects = ObjectStore::new();
        let parent = objects.allocate(actor()).unwrap();
        let children = std::array::from_fn(|_| objects.allocate(actor()).unwrap());
        objects.get_mut(parent).unwrap().base.first_child = Some(children[0]);
        objects
            .get_mut(parent)
            .unwrap()
            .extension
            .path_state
            .motion
            .refresh_child_chain = true;
        for (index, child) in children.iter().copied().enumerate() {
            let actor = objects.get_mut(child).unwrap();
            actor.base.child_number = index as u8 + 1;
            actor.base.attachment = Some(parent);
            actor.base.next_sibling = children.get(index + 1).copied();
            actor.base.flags.remove_with_parent = true;
            actor.extension.path_state.motion.attached_coordinates = true;
            actor.extension.parent = Some(parent);
            actor.base.position.x = 123;
        }
        (objects, parent, children)
    }

    #[test]
    fn attachment_appends_siblings_without_reordering_active_objects_or_changing_pose() {
        let mut objects = ObjectStore::new();
        let parent = objects.allocate(actor()).unwrap();
        let unrelated = objects.allocate(actor()).unwrap();
        let mut children = Vec::new();
        for number in [0, 255, 255] {
            let mut value = actor();
            value.base.position = super::super::Vector3 {
                x: -123,
                y: 456,
                z: -789,
            };
            value.extension.parent = Some(unrelated);
            value.extension.relative_position.x = 222;
            let child = objects.allocate_after(Some(parent), value).unwrap();
            let before = objects.clone();
            attach_fresh_child(&mut objects, parent, child, number).unwrap();
            assert_eq!(objects.active_ids(), before.active_ids());
            let mut expected = before.get(child).unwrap().clone();
            expected.base.child_number = number;
            expected.base.attachment = Some(parent);
            expected.base.flags.remove_with_parent = true;
            expected.extension.path_state.motion.attached_coordinates = true;
            assert_eq!(objects.get(child), Some(&expected));
            assert_eq!(objects.get(unrelated), before.get(unrelated));
            children.push(child);
            assert_eq!(
                objects.get(parent).unwrap().base.first_child,
                Some(children[0])
            );
            assert!(
                objects
                    .get(parent)
                    .unwrap()
                    .extension
                    .path_state
                    .motion
                    .refresh_child_chain
            );
            for (index, id) in children.iter().copied().enumerate() {
                assert_eq!(
                    objects.get(id).unwrap().base.next_sibling,
                    children.get(index + 1).copied()
                );
            }
        }
        assert_eq!(
            objects.active_ids(),
            &[unrelated, parent, children[2], children[1], children[0]]
        );
        assert_eq!(find_child(&objects, parent, 255), Ok(Some(children[1])));
        apply(
            &mut objects,
            parent,
            RelationshipCommand::UnlinkChild { number: 255 },
        )
        .unwrap();
        assert_eq!(
            objects.get(children[0]).unwrap().base.next_sibling,
            Some(children[2])
        );
    }

    #[test]
    fn spawn_parent_is_one_hop_and_uses_attachment_flag_not_refresh_flag() {
        let (mut objects, parent, children) = family();
        let grandparent = objects.allocate(actor()).unwrap();
        objects.get_mut(parent).unwrap().base.attachment = Some(grandparent);
        objects
            .get_mut(parent)
            .unwrap()
            .extension
            .path_state
            .motion
            .attached_coordinates = true;
        for attached in [false, true] {
            for refresh in [false, true] {
                let actor = objects.get_mut(children[0]).unwrap();
                actor.extension.path_state.motion.attached_coordinates = attached;
                actor.extension.path_state.motion.refresh_child_chain = refresh;
                assert_eq!(
                    spawn_parent(&objects, children[0]),
                    Ok(if attached { parent } else { children[0] })
                );
            }
        }
        objects.get_mut(children[0]).unwrap().base.attachment = None;
        assert_eq!(
            spawn_parent(&objects, children[0]),
            Err(RelationshipError::MissingParent(children[0]))
        );
        objects
            .get_mut(children[0])
            .unwrap()
            .extension
            .path_state
            .motion
            .attached_coordinates = false;
        assert_eq!(spawn_parent(&objects, children[0]), Ok(children[0]));
    }

    #[test]
    fn attachment_rejects_nonfresh_or_malformed_chains_without_partial_mutation() {
        let (mut objects, parent, children) = family();
        let fresh = objects.allocate(actor()).unwrap();
        for child in [parent, children[0], children[2]] {
            let before = objects.clone();
            assert_eq!(
                attach_fresh_child(&mut objects, parent, child, 9),
                Err(RelationshipError::ChildNotFresh(child))
            );
            assert_eq!(objects, before);
        }
        objects.get_mut(children[2]).unwrap().base.next_sibling = Some(children[0]);
        let before = objects.clone();
        assert_eq!(
            attach_fresh_child(&mut objects, parent, fresh, 9),
            Err(RelationshipError::ChildCycle(children[0]))
        );
        assert_eq!(objects, before);
        objects.get_mut(children[2]).unwrap().base.next_sibling = Some(fresh);
        let before = objects.clone();
        assert_eq!(
            attach_fresh_child(&mut objects, parent, fresh, 9),
            Err(RelationshipError::ChildNotFresh(fresh))
        );
        assert_eq!(objects, before);
        objects.get_mut(children[2]).unwrap().base.next_sibling = None;
        objects.remove(children[2]);
        // Ordinary pool removal repairs references; create the malformed
        // reference explicitly after removal for this diagnostic case.
        objects.get_mut(children[1]).unwrap().base.next_sibling = Some(children[2]);
        let before = objects.clone();
        assert_eq!(
            attach_fresh_child(&mut objects, parent, fresh, 9),
            Err(RelationshipError::MissingActor(children[2]))
        );
        assert_eq!(objects, before);
    }

    #[test]
    fn unlink_splices_each_position_and_keeps_object_pose_extension_and_active_order() {
        for index in 0..3 {
            let (mut objects, parent, children) = family();
            let order = objects.active_ids().to_vec();
            apply(
                &mut objects,
                parent,
                RelationshipCommand::UnlinkChild {
                    number: index as u8 + 1,
                },
            )
            .unwrap();
            assert_eq!(objects.active_ids(), order);
            let child = objects.get(children[index]).unwrap();
            assert_eq!(child.base.attachment, None);
            assert_eq!(child.base.next_sibling, None);
            assert_eq!(child.base.child_number, 0);
            assert_eq!(child.base.position.x, 123);
            assert_eq!(child.extension.parent, Some(parent));
            assert!(!child.extension.path_state.motion.attached_coordinates);
            assert!(!child.base.flags.remove_with_parent);
            let remaining: Vec<_> = children
                .into_iter()
                .filter(|child| *child != children[index])
                .collect();
            assert_eq!(
                objects.get(parent).unwrap().base.first_child,
                Some(remaining[0])
            );
            assert_eq!(
                objects.get(remaining[0]).unwrap().base.next_sibling,
                Some(remaining[1])
            );
            assert_eq!(objects.get(remaining[1]).unwrap().base.next_sibling, None);
            assert!(
                objects
                    .get(parent)
                    .unwrap()
                    .extension
                    .path_state
                    .motion
                    .refresh_child_chain
            );
        }
    }

    #[test]
    fn sibling_search_uses_owner_flag_and_first_matching_number_including_zero() {
        let (mut objects, parent, children) = family();
        assert_eq!(find_child(&objects, children[0], 3), Ok(Some(children[2])));
        objects.get_mut(children[1]).unwrap().base.child_number = 0;
        objects.get_mut(children[2]).unwrap().base.child_number = 0;
        assert_eq!(find_child(&objects, parent, 0), Ok(Some(children[1])));
        let before = objects.clone();
        apply(
            &mut objects,
            parent,
            RelationshipCommand::UnlinkChild { number: 99 },
        )
        .unwrap();
        assert_eq!(objects, before);
        objects
            .get_mut(parent)
            .unwrap()
            .extension
            .path_state
            .motion
            .refresh_child_chain = false;
        assert_eq!(find_child(&objects, parent, 0), Ok(None));
    }

    #[test]
    fn self_unlink_requires_attached_flag_and_absent_chain_preserves_links() {
        let (mut objects, parent, children) = family();
        objects
            .get_mut(children[1])
            .unwrap()
            .extension
            .path_state
            .motion
            .attached_coordinates = false;
        let before = objects.clone();
        apply(&mut objects, children[1], RelationshipCommand::UnlinkSelf).unwrap();
        assert_eq!(objects, before);
        objects
            .get_mut(children[1])
            .unwrap()
            .extension
            .path_state
            .motion
            .attached_coordinates = true;
        objects.get_mut(parent).unwrap().base.first_child = None;
        apply(&mut objects, children[1], RelationshipCommand::UnlinkSelf).unwrap();
        let child = objects.get(children[1]).unwrap();
        assert_eq!(child.base.attachment, Some(parent));
        assert_eq!(child.base.next_sibling, Some(children[2]));
        assert_eq!(child.base.child_number, 2);
        assert!(!child.extension.path_state.motion.attached_coordinates);
        assert!(!child.base.flags.remove_with_parent);
    }

    #[test]
    fn self_unlink_and_sibling_initiated_unlink_reach_the_same_parent_chain() {
        let (mut direct, parent, children) = family();
        let mut sibling = direct.clone();
        apply(&mut direct, children[1], RelationshipCommand::UnlinkSelf).unwrap();
        apply(
            &mut sibling,
            children[0],
            RelationshipCommand::UnlinkChild { number: 2 },
        )
        .unwrap();
        assert_eq!(direct, sibling);
        assert_eq!(
            direct.get(parent).unwrap().base.first_child,
            Some(children[0])
        );
        assert_eq!(
            direct.get(children[0]).unwrap().base.next_sibling,
            Some(children[2])
        );
        assert_eq!(direct.get(children[1]).unwrap().base.attachment, None);
    }

    #[test]
    fn malformed_chains_are_errors_not_silent_success_or_infinite_search() {
        let (mut objects, parent, children) = family();
        objects.get_mut(children[2]).unwrap().base.next_sibling = Some(children[0]);
        assert_eq!(
            find_child(&objects, parent, 99),
            Err(RelationshipError::ChildCycle(children[0]))
        );
        objects.get_mut(children[0]).unwrap().base.attachment = None;
        assert_eq!(
            apply(&mut objects, children[0], RelationshipCommand::UnlinkSelf),
            Err(RelationshipError::MissingParent(children[0]))
        );
    }
}
