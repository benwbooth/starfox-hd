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
    SignalLinked,
    SignalChild { number: u8 },
    RefreshLinkedRotation,
    ClearRelativeReference,
    UseSelfRelativeFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedTransformCommand {
    WorldPosition,
    WorldRotation,
    RelativeFrame,
}

/// World-copy forms (`$7F:B8CD`, `$7F:BF4E`) change only their named world
/// channel. Relative-frame capture (`$7F:AA5E`) instead retains world pose
/// and velocity while replacing the relative reference and transform.
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
    // $7F:AA5E uses the view-order matrix of NEGATED selected angles,
    // not the attachment matrix, its transpose, or successive point turns.
    let relative = if command == SelectedTransformCommand::RelativeFrame {
        let actor = objects.get(owner).expect("validated frame owner");
        let matrix = sf_core::snes_trig::zxy_matrix_q15(
            rotation.0.units().wrapping_neg(),
            rotation.1.units().wrapping_neg(),
            rotation.2.units().wrapping_neg(),
        );
        let (x, y, z) = sf_core::snes_trig::matrix_rotate_q15(
            matrix,
            actor.base.position.x.wrapping_sub(position.x),
            actor.base.position.y.wrapping_sub(position.y),
            actor.base.position.z.wrapping_sub(position.z),
        );
        let difference = |angle: super::Angle, origin: super::Angle| {
            super::Angle::from_units(angle.units().wrapping_sub(origin.units()))
        };
        Some((
            super::Vector3 { x, y, z },
            super::Rotation {
                pitch: difference(actor.base.pitch, rotation.0),
                yaw: difference(actor.base.yaw, rotation.1),
                roll: difference(actor.base.roll, rotation.2),
            },
        ))
    } else {
        None
    };
    let actor = objects
        .get_mut(owner)
        .expect("validated transform-copy owner");
    match command {
        SelectedTransformCommand::WorldPosition => actor.base.position = position,
        SelectedTransformCommand::WorldRotation => {
            (actor.base.pitch, actor.base.yaw, actor.base.roll) = rotation;
        }
        SelectedTransformCommand::RelativeFrame => {
            let (position, rotation) = relative.expect("captured selected frame");
            actor.extension.parent = Some(selected);
            actor.extension.relative_position = position;
            actor.extension.relative_rotation = rotation;
            actor.extension.path_state.motion.relative_coordinates = true;
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

/// A missing parent is NOT a dead child (`$7F:938B`). Only an available
/// parent whose numbered-child search fails takes the authored branch.
pub fn child_missing(
    objects: &ObjectStore,
    owner: ObjectId,
    number: u8,
) -> Result<bool, RelationshipError> {
    let actor = objects
        .get(owner)
        .ok_or(RelationshipError::MissingActor(owner))?;
    if !actor.extension.path_state.motion.refresh_child_chain && actor.base.attachment.is_none() {
        return Ok(false);
    }
    Ok(find_child(objects, owner, number)?.is_none())
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
        RelationshipCommand::SignalLinked | RelationshipCommand::SignalChild { .. } => {
            let target = if let RelationshipCommand::SignalChild { number } = command {
                find_child(objects, owner, number)?
            } else {
                objects
                    .get(owner)
                    .ok_or(RelationshipError::MissingActor(owner))?
                    .base
                    .attachment
            };
            // Both linked forms ($7F:94DB/94F1) and numbered signaling
            // ($7F:9400) OR the same one-shot event latch. This is neither
            // damage nor retirement, and needs no attached-coordinate gate.
            if let Some(target) = target {
                objects
                    .get_mut(target)
                    .ok_or(RelationshipError::MissingActor(target))?
                    .extension
                    .path_state
                    .conditions
                    .hit_event_pending = true;
            }
            return Ok(());
        }
        RelationshipCommand::ClearRelativeReference | RelationshipCommand::UseSelfRelativeFrame => {
            let actor = objects
                .get_mut(owner)
                .ok_or(RelationshipError::MissingActor(owner))?;
            if command == RelationshipCommand::ClearRelativeReference {
                actor.extension.parent = None;
            } else {
                actor.extension.parent = Some(owner);
                actor.extension.relative_position = super::Vector3::default();
                actor.extension.relative_rotation = super::Rotation::default();
            }
            // Neither command changes relative/attached-coordinate gates,
            // child-list attachment, world pose, or selected-player identity.
            return Ok(());
        }
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
        RelationshipCommand::RefreshLinkedRotation => {
            let actor = objects
                .get(owner)
                .ok_or(RelationshipError::MissingActor(owner))?;
            let Some(link) = actor.base.attachment else {
                return Ok(());
            };
            let linked = objects
                .get(link)
                .ok_or(RelationshipError::MissingActor(link))?;
            // $7F:BACC reads the live link, without checking either relative
            // coordinate mode or the attached-coordinate flag. Read before
            // writing so self-links produce three zero differences.
            let difference = |angle: super::Angle, origin: super::Angle| {
                super::Angle::from_units(angle.units().wrapping_sub(origin.units()))
            };
            let rotation = super::Rotation {
                pitch: difference(actor.base.pitch, linked.base.pitch),
                yaw: difference(actor.base.yaw, linked.base.yaw),
                roll: difference(actor.base.roll, linked.base.roll),
            };
            objects
                .get_mut(owner)
                .expect("validated rotation owner")
                .extension
                .relative_rotation = rotation;
            return Ok(());
        }
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

    #[test]
    fn relative_reference_controls_preserve_independent_relationships_and_motion_gates() {
        use super::super::{Angle, Rotation, Vector3};
        let mut objects = ObjectStore::new();
        let actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        let other = objects.allocate(actor.clone()).unwrap();
        let owner = objects.allocate(actor).unwrap();
        for mode in 0..4 {
            for parent in [None, Some(other), Some(owner)] {
                for command in [
                    RelationshipCommand::ClearRelativeReference,
                    RelationshipCommand::UseSelfRelativeFrame,
                ] {
                    let mut before = objects.get(owner).unwrap().clone();
                    before.base.attachment = Some(other);
                    before.extension.parent = parent;
                    before.extension.path_state.motion.relative_coordinates = mode & 1 != 0;
                    before.extension.path_state.motion.attached_coordinates = mode & 2 != 0;
                    before.extension.relative_position = Vector3 {
                        x: i16::MIN,
                        y: i16::MAX,
                        z: -59,
                    };
                    before.extension.relative_rotation = Rotation {
                        pitch: Angle::from_units(91),
                        yaw: Angle::from_units(227),
                        roll: Angle::from_units(35),
                    };
                    *objects.get_mut(owner).unwrap() = before.clone();
                    let mut expected = before;
                    if command == RelationshipCommand::ClearRelativeReference {
                        expected.extension.parent = None;
                    } else {
                        expected.extension.parent = Some(owner);
                        expected.extension.relative_position = Vector3::default();
                        expected.extension.relative_rotation = Rotation::default();
                    }
                    apply(&mut objects, owner, command).unwrap();
                    assert_eq!(objects.get(owner).unwrap(), &expected);
                }
            }
        }
    }

    #[test]
    fn selected_frame_capture_preserves_source_matrix_order_and_per_product_truncation() {
        use super::super::{Angle, Rotation, Vector3};
        use sf_core::snes_trig::{cos_q15, sin_q15};
        let mut objects = ObjectStore::new();
        let mut actor = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
        actor.base.pitch = Angle::from_units(93);
        actor.base.yaw = Angle::from_units(71);
        actor.base.roll = Angle::from_units(219);
        actor.base.velocity = Vector3 {
            x: 67,
            y: -305,
            z: i16::MAX,
        };
        let owner = objects.allocate(actor.clone()).unwrap();
        let target = objects.allocate(actor.clone()).unwrap();
        let mul = |a: i16, b: i16| ((i64::from(a) * i64::from(b)) >> 15) as i16;
        for bits in 0..=u16::MAX {
            let angles = [
                bits as u8,
                (bits >> 8) as u8,
                (bits as u8) ^ (bits >> 8) as u8,
            ];
            let selected = objects.get_mut(target).unwrap();
            selected.base.pitch = Angle::from_units(angles[0]);
            selected.base.yaw = Angle::from_units(angles[1]);
            selected.base.roll = Angle::from_units(angles[2]);
            selected.base.position = Vector3 {
                x: i16::MAX,
                y: i16::MIN,
                z: -97,
            };
            let selected_before = selected.clone();
            actor.base.position = Vector3 {
                x: bits as i16,
                y: (bits as i16).wrapping_neg(),
                z: (bits as i16).wrapping_add(173),
            };
            actor.base.attachment = Some(target);
            actor.extension.path_state.motion.relative_coordinates = bits & 1 != 0;
            actor.extension.path_state.motion.attached_coordinates = bits & 2 != 0;
            *objects.get_mut(owner).unwrap() = actor.clone();
            let [pitch, yaw, roll] = angles.map(u8::wrapping_neg);
            let (sp, cp, sy, cy, sr, cr) = (
                sin_q15(pitch),
                cos_q15(pitch),
                sin_q15(yaw),
                cos_q15(yaw),
                sin_q15(roll),
                cos_q15(roll),
            );
            // Source geometry 919B -> 9266 emits input-axis rows. Each
            // product truncates separately; sums wrap before the next use.
            let a = mul(cr, sy);
            let b = mul(cr, cy);
            let c = mul(sr, sy);
            let d = mul(sr, cy);
            let coefficients = [
                [
                    mul(c, sp).wrapping_add(b),
                    mul(a, sp).wrapping_sub(d),
                    mul(cp, sy),
                ],
                [mul(cp, sr), mul(cp, cr), sp.wrapping_neg()],
                [
                    mul(d, sp).wrapping_sub(a),
                    mul(b, sp).wrapping_add(c),
                    mul(cp, cy),
                ],
            ];
            let delta = [
                actor.base.position.x.wrapping_sub(i16::MAX),
                actor.base.position.y.wrapping_sub(i16::MIN),
                actor.base.position.z.wrapping_add(97),
            ];
            let output: [i16; 3] = std::array::from_fn(|axis| {
                (0..3)
                    .map(|input| i64::from(mul(delta[input], coefficients[input][axis])))
                    .sum::<i64>() as i16
            });
            let mut expected = actor.clone();
            expected.extension.parent = Some(target);
            expected.extension.relative_position = Vector3 {
                x: output[0],
                y: output[1],
                z: output[2],
            };
            expected.extension.relative_rotation = Rotation {
                pitch: Angle::from_units(93_u8.wrapping_sub(angles[0])),
                yaw: Angle::from_units(71_u8.wrapping_sub(angles[1])),
                roll: Angle::from_units(219_u8.wrapping_sub(angles[2])),
            };
            expected.extension.path_state.motion.relative_coordinates = true;
            copy_selected_transform(
                &mut objects,
                owner,
                Some(target),
                SelectedTransformCommand::RelativeFrame,
            )
            .unwrap();
            assert_eq!(objects.get(owner).unwrap(), &expected);
            assert_eq!(objects.get(target).unwrap(), &selected_before);
        }
    }

    #[test]
    fn linked_rotation_differences_wrap_all_angle_pairs_without_changing_world_pose() {
        use super::super::{Angle, Rotation, Vector3};
        let mut objects = ObjectStore::default();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let linked = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        objects.get_mut(owner).unwrap().base.attachment = Some(linked);
        for angle in 0..=u8::MAX {
            for origin in 0..=u8::MAX {
                let target = objects.get_mut(linked).unwrap();
                target.base.pitch = Angle::from_units(origin);
                target.base.yaw = Angle::from_units(origin.wrapping_add(17));
                target.base.roll = Angle::from_units(origin.wrapping_sub(91));
                let target_before = target.clone();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.pitch = Angle::from_units(angle);
                actor.base.yaw = Angle::from_units(angle.wrapping_sub(73));
                actor.base.roll = Angle::from_units(angle.wrapping_add(99));
                actor.base.child_number = 123;
                actor.base.wait_timer = 57;
                actor.extension.relative_position = Vector3 { x: 1, y: -2, z: 3 };
                actor.extension.path_state.motion.attached_coordinates = angle & 1 != 0;
                actor.extension.path_state.motion.relative_coordinates = origin & 1 != 0;
                let mut expected = actor.clone();
                let delta = (i16::from(angle) - i16::from(origin)) as u8;
                expected.extension.relative_rotation = Rotation {
                    pitch: Angle::from_units(delta),
                    yaw: Angle::from_units(delta.wrapping_sub(90)),
                    roll: Angle::from_units(delta.wrapping_add(190)),
                };
                apply(
                    &mut objects,
                    owner,
                    RelationshipCommand::RefreshLinkedRotation,
                )
                .unwrap();
                assert_eq!(objects.get(owner).unwrap(), &expected);
                assert_eq!(objects.get(linked).unwrap(), &target_before);
            }
        }
        let before = objects.get(owner).unwrap().clone();
        objects.get_mut(owner).unwrap().base.attachment = None;
        let mut expected = before;
        expected.base.attachment = None;
        apply(
            &mut objects,
            owner,
            RelationshipCommand::RefreshLinkedRotation,
        )
        .unwrap();
        assert_eq!(objects.get(owner).unwrap(), &expected);
        objects.get_mut(owner).unwrap().base.attachment = Some(owner);
        expected.base.attachment = Some(owner);
        expected.extension.relative_rotation = Rotation::default();
        apply(
            &mut objects,
            owner,
            RelationshipCommand::RefreshLinkedRotation,
        )
        .unwrap();
        assert_eq!(objects.get(owner).unwrap(), &expected);
    }

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
    fn signals_change_only_the_first_matching_targets_event_latch() {
        let (mut objects, parent, children) = family();
        for number in 0..=u8::MAX {
            // Duplicate identifiers retain first-match semantics, including
            // zero and high-bit identifiers. Relative references are decoys.
            objects.get_mut(children[0]).unwrap().base.child_number = number.wrapping_add(1);
            objects.get_mut(children[1]).unwrap().base.child_number = number;
            objects.get_mut(children[2]).unwrap().base.child_number = number;
            for owner in [parent, children[0]] {
                for attached in [false, true] {
                    objects.get_mut(owner).unwrap().extension.parent = Some(children[2]);
                    objects
                        .get_mut(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .motion
                        .attached_coordinates = attached;
                    for pending in [false, true] {
                        objects
                            .get_mut(children[1])
                            .unwrap()
                            .extension
                            .path_state
                            .conditions
                            .hit_event_pending = pending;
                        let mut expected = objects.clone();
                        expected
                            .get_mut(children[1])
                            .unwrap()
                            .extension
                            .path_state
                            .conditions
                            .hit_event_pending = true;
                        apply(
                            &mut objects,
                            owner,
                            RelationshipCommand::SignalChild { number },
                        )
                        .unwrap();
                        assert_eq!(objects, expected);
                    }
                }
            }
        }
        for link in [None, Some(parent), Some(children[0])] {
            for attached in [false, true] {
                let owner = children[0];
                objects.get_mut(owner).unwrap().base.attachment = link;
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion
                    .attached_coordinates = attached;
                let mut expected = objects.clone();
                if let Some(target) = link {
                    expected
                        .get_mut(target)
                        .unwrap()
                        .extension
                        .path_state
                        .conditions
                        .hit_event_pending = true;
                }
                apply(&mut objects, owner, RelationshipCommand::SignalLinked).unwrap();
                assert_eq!(objects, expected);
            }
        }
    }

    #[test]
    fn child_missing_distinguishes_absent_parent_from_empty_search_and_retains_state() {
        let (mut objects, parent, children) = family();
        for refresh in [false, true] {
            for mother in [None, Some(parent)] {
                let owner = children[0];
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion
                    .refresh_child_chain = refresh;
                objects.get_mut(owner).unwrap().base.attachment = mother;
                objects.get_mut(owner).unwrap().base.first_child = Some(children[2]);
                for number in 0..=u8::MAX {
                    let before = objects.clone();
                    let missing = if refresh {
                        number != 3
                    } else if mother.is_some() {
                        ![1, 2, 3].contains(&number)
                    } else {
                        false
                    };
                    assert_eq!(child_missing(&objects, owner, number), Ok(missing));
                    assert_eq!(objects, before);
                    let mut expected = before;
                    let target = if refresh {
                        (number == 3).then_some(children[2])
                    } else if mother.is_some() {
                        children
                            .iter()
                            .copied()
                            .find(|id| objects.get(*id).unwrap().base.child_number == number)
                    } else {
                        None
                    };
                    if let Some(target) = target {
                        expected
                            .get_mut(target)
                            .unwrap()
                            .extension
                            .path_state
                            .conditions
                            .hit_event_pending = true;
                    }
                    apply(
                        &mut objects,
                        owner,
                        RelationshipCommand::SignalChild { number },
                    )
                    .unwrap();
                    assert_eq!(objects, expected);
                }
            }
        }
        objects.get_mut(parent).unwrap().base.first_child = None;
        assert_eq!(child_missing(&objects, parent, 0), Ok(true));
    }

    #[test]
    fn signaling_and_missing_child_diagnose_broken_links_without_mutating() {
        let (mut objects, parent, children) = family();
        objects.get_mut(children[2]).unwrap().base.next_sibling = Some(children[0]);
        let before = objects.clone();
        assert_eq!(
            child_missing(&objects, parent, 255),
            Err(RelationshipError::ChildCycle(children[0]))
        );
        assert_eq!(
            apply(
                &mut objects,
                parent,
                RelationshipCommand::SignalChild { number: 255 }
            ),
            Err(RelationshipError::ChildCycle(children[0]))
        );
        assert_eq!(objects, before);
        let missing = objects.allocate(actor()).unwrap();
        objects.remove(missing).unwrap();
        objects.get_mut(children[0]).unwrap().base.attachment = Some(missing);
        let before = objects.clone();
        for command in [
            RelationshipCommand::SignalLinked,
            RelationshipCommand::SignalChild { number: 1 },
        ] {
            assert_eq!(
                apply(&mut objects, children[0], command),
                Err(RelationshipError::MissingActor(missing))
            );
            assert_eq!(objects, before);
        }
        assert_eq!(
            child_missing(&objects, children[0], 1),
            Err(RelationshipError::MissingActor(missing))
        );
        assert_eq!(objects, before);
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
