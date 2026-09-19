//! Attached-object publication (`$7F:2229`, `$7F:2319`).
//! Local coordinates remain retained; refreshing a pose does not integrate them.

use super::{ObjectId, ObjectStore, Rotation, Vector3, OBJECT_CAPACITY};
use sf_core::snes_trig::{cos_q15, gsu_fmult_q15 as multiply_q15, matrix_rotate_q15, sin_q15};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentError {
    MissingActor(ObjectId),
    MissingParent(ObjectId),
    ChildCycle(ObjectId),
}

/// `$03:8C27` / geometry `$01:92C6`: attachment composition differs from
/// view rotation. Finish the matrix before transforming; successive point
/// rotations would truncate differently.
pub fn attachment_matrix(rotation: Rotation) -> [[i16; 3]; 3] {
    let pitch_sine = sin_q15(rotation.pitch.units());
    let pitch_cosine = cos_q15(rotation.pitch.units());
    let yaw_sine = sin_q15(rotation.yaw.units());
    let yaw_cosine = cos_q15(rotation.yaw.units());
    let roll_sine = sin_q15(rotation.roll.units());
    let roll_cosine = cos_q15(rotation.roll.units());
    let cosine_sine = multiply_q15(roll_cosine, yaw_sine);
    let cosine_cosine = multiply_q15(roll_cosine, yaw_cosine);
    let sine_sine = multiply_q15(roll_sine, yaw_sine);
    let sine_cosine = multiply_q15(roll_sine, yaw_cosine);
    [
        [
            cosine_cosine.wrapping_sub(multiply_q15(sine_sine, pitch_sine)),
            multiply_q15(pitch_cosine, roll_sine).wrapping_neg(),
            multiply_q15(sine_cosine, pitch_sine).wrapping_add(cosine_sine),
        ],
        [
            multiply_q15(cosine_sine, pitch_sine).wrapping_add(sine_cosine),
            multiply_q15(pitch_cosine, roll_cosine),
            sine_sine.wrapping_sub(multiply_q15(cosine_cosine, pitch_sine)),
        ],
        [
            multiply_q15(pitch_cosine, yaw_sine).wrapping_neg(),
            pitch_sine,
            multiply_q15(pitch_cosine, yaw_cosine),
        ],
    ]
}

/// Extension parent takes precedence over the base attachment. An explicit
/// extension self-reference is a no-op, not a request to use the fallback.
pub fn refresh_actor(objects: &mut ObjectStore, owner: ObjectId) -> Result<(), AttachmentError> {
    let actor = objects
        .get(owner)
        .ok_or(AttachmentError::MissingActor(owner))?;
    if actor.extension.parent == Some(owner) {
        return Ok(());
    }
    let parent = actor
        .extension
        .parent
        .or(actor.base.attachment)
        .ok_or(AttachmentError::MissingParent(owner))?;
    let offset = actor.extension.relative_position;
    let local_rotation = actor.extension.relative_rotation;
    let base_self_reference = parent == owner;
    let parent = objects
        .get(parent)
        .ok_or(AttachmentError::MissingActor(parent))?;
    let rotation = Rotation {
        pitch: parent.base.pitch,
        yaw: parent.base.yaw,
        roll: parent.base.roll,
    };
    let published_rotation = Rotation {
        pitch: rotation
            .pitch
            .wrapping_add(local_rotation.pitch.units() as i8),
        yaw: rotation.yaw.wrapping_add(local_rotation.yaw.units() as i8),
        roll: rotation
            .roll
            .wrapping_add(local_rotation.roll.units() as i8),
    };
    // Only extension self-reference bypasses the service. A base self-link
    // observes the newly published angles when constructing its matrix.
    let matrix_rotation = if base_self_reference {
        published_rotation
    } else {
        rotation
    };
    let (x, y, z) = matrix_rotate_q15(
        attachment_matrix(matrix_rotation),
        offset.x,
        offset.y,
        offset.z,
    );
    let position = Vector3 {
        x: parent.base.position.x.wrapping_add(x),
        y: parent.base.position.y.wrapping_add(y),
        z: parent.base.position.z.wrapping_add(z),
    };
    let actor = objects.get_mut(owner).expect("validated attachment actor");
    actor.base.position = position;
    actor.base.pitch = published_rotation.pitch;
    actor.base.yaw = published_rotation.yaw;
    actor.base.roll = published_rotation.roll;
    Ok(())
}

/// Walk first-child links, publishing each pose before visiting its child.
/// Sibling links are deliberately not followed by this source service.
pub fn refresh_child_chain(
    objects: &mut ObjectStore,
    owner: ObjectId,
) -> Result<(), AttachmentError> {
    let mut visited = [false; OBJECT_CAPACITY];
    visited[owner.index()] = true;
    let mut current = owner;
    loop {
        let child = objects
            .get(current)
            .ok_or(AttachmentError::MissingActor(current))?
            .base
            .first_child;
        let Some(child) = child else { return Ok(()) };
        if visited[child.index()] {
            return Err(AttachmentError::ChildCycle(child));
        }
        visited[child.index()] = true;
        refresh_actor(objects, child)?;
        current = child;
    }
}

/// Path movement's post-callback gate (`$7F:9E73..9E8B`). This is separate
/// from relative integration, which must still run when refresh is disabled.
pub fn refresh_after_callbacks(
    objects: &mut ObjectStore,
    owner: ObjectId,
) -> Result<(), AttachmentError> {
    let motion = objects
        .get(owner)
        .ok_or(AttachmentError::MissingActor(owner))?
        .extension
        .path_state
        .motion;
    if motion.refresh_child_chain && !motion.suppress_child_refresh {
        refresh_child_chain(objects, owner)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Angle, Behavior, Object, ObjectKind, ShapeId};

    fn actor() -> Object {
        Object::new(
            ObjectKind::Effect,
            ShapeId::from_catalog_index(0),
            Behavior::Effect,
        )
    }

    #[test]
    fn extension_parent_precedes_fallback_and_self_reference_is_noop() {
        let mut objects = ObjectStore::new();
        let fallback = objects.allocate(actor()).unwrap();
        let parent = objects.allocate(actor()).unwrap();
        objects.get_mut(parent).unwrap().base.position.x = 200;
        let mut child = actor();
        child.base.attachment = Some(fallback);
        child.extension.parent = Some(parent);
        child.extension.relative_position.x = 100;
        let child = objects.allocate(child).unwrap();
        refresh_actor(&mut objects, child).unwrap();
        assert_eq!(objects.get(child).unwrap().base.position.x, 299);
        objects.get_mut(child).unwrap().extension.parent = Some(child);
        let before = objects.get(child).unwrap().clone();
        refresh_actor(&mut objects, child).unwrap();
        assert_eq!(objects.get(child).unwrap(), &before);
    }

    #[test]
    fn fallback_publishes_wrapped_pose_without_advancing_relative_state() {
        let mut objects = ObjectStore::new();
        let mut parent = actor();
        parent.base.position.x = i16::MAX;
        parent.base.roll = Angle::from_units(255);
        let parent = objects.allocate(parent).unwrap();
        let mut child = actor();
        child.base.attachment = Some(parent);
        child.extension.relative_position = Vector3 {
            x: 100,
            y: -200,
            z: 300,
        };
        child.extension.relative_rotation.roll = Angle::from_units(2);
        let relative = child.extension.clone();
        let child = objects.allocate(child).unwrap();
        refresh_actor(&mut objects, child).unwrap();
        let child = objects.get(child).unwrap();
        assert!(child.base.position.x < 0);
        assert_eq!(child.base.roll.units(), 1);
        assert_eq!(child.extension, relative);
    }

    #[test]
    fn base_self_reference_uses_newly_published_angles_in_transform() {
        let mut objects = ObjectStore::new();
        let owner = objects.allocate(actor()).unwrap();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.attachment = Some(owner);
        actor.extension.relative_position.x = 100;
        actor.extension.relative_rotation.yaw = Angle::from_units(64);
        refresh_actor(&mut objects, owner).unwrap();
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.yaw.units(), 64);
        assert_eq!(actor.base.position, Vector3 { x: 0, y: 0, z: 99 });
    }

    #[test]
    fn chain_updates_use_fresh_parent_pose_and_ignore_siblings() {
        let mut objects = ObjectStore::new();
        let parent = objects.allocate(actor()).unwrap();
        let child = objects.allocate(actor()).unwrap();
        let grandchild = objects.allocate(actor()).unwrap();
        let sibling = objects.allocate(actor()).unwrap();
        objects.get_mut(parent).unwrap().base.position.x = 400;
        objects.get_mut(parent).unwrap().base.first_child = Some(child);
        objects.get_mut(child).unwrap().base.attachment = Some(parent);
        objects.get_mut(child).unwrap().base.first_child = Some(grandchild);
        objects.get_mut(child).unwrap().base.next_sibling = Some(sibling);
        objects.get_mut(grandchild).unwrap().base.attachment = Some(child);
        refresh_child_chain(&mut objects, parent).unwrap();
        assert_eq!(objects.get(grandchild).unwrap().base.position.x, 400);
        assert_eq!(objects.get(sibling).unwrap().base.position.x, 0);
        objects.get_mut(grandchild).unwrap().base.first_child = Some(parent);
        assert_eq!(
            refresh_child_chain(&mut objects, parent),
            Err(AttachmentError::ChildCycle(parent))
        );
    }

    #[test]
    fn gates_are_independent_of_parent_links_and_missing_parent_is_explicit() {
        let mut objects = ObjectStore::new();
        let parent = objects.allocate(actor()).unwrap();
        let child = objects.allocate(actor()).unwrap();
        objects.get_mut(parent).unwrap().base.first_child = Some(child);
        refresh_after_callbacks(&mut objects, parent).unwrap();
        objects
            .get_mut(parent)
            .unwrap()
            .extension
            .path_state
            .motion
            .refresh_child_chain = true;
        assert_eq!(
            refresh_after_callbacks(&mut objects, parent),
            Err(AttachmentError::MissingParent(child))
        );
        objects
            .get_mut(parent)
            .unwrap()
            .extension
            .path_state
            .motion
            .suppress_child_refresh = true;
        refresh_after_callbacks(&mut objects, parent).unwrap();
    }
}
