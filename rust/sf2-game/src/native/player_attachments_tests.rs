use super::*;
use crate::path_program::SelectedAuxiliaryState;
use crate::scene_path_world::PlayerPathRecords;
use crate::{Angle, Behavior, Object, ObjectKind, RandomState, ShapeId, Vector3};

fn fixture() -> (ObjectStore, ScenePathWorld, ObjectId, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let child = objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    crate::path_relationships::attach_fresh_child(&mut objects, owner, child, 18).unwrap();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.position = Vector3 {
        x: 17,
        y: 91,
        z: 203,
    };
    actor.base.yaw = Angle::from_units(254);
    let mut world = ScenePathWorld::new(RandomState::default());
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                contact: Some(Default::default()),
                auxiliary: Some(SelectedAuxiliaryState {
                    mode: 0x2F,
                    action_flags: 0,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                }),
                camera_ground: Some(crate::player_camera_ground::PlayerCameraGround {
                    carried_target_height: -1000,
                    ..Default::default()
                }),
                motion: Some(crate::player_motion::PlayerMotion {
                    walker_attachment_yaw: 5,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .unwrap();
    (objects, world, owner, child)
}

#[test]
fn walker_publishes_temporary_pose_to_children_and_restores_only_owner_height_and_yaw() {
    let (mut objects, world, owner, child) = fixture();
    let before = objects.get(owner).unwrap().clone();
    let record = *world.player(&objects, owner).unwrap();
    publish(&mut objects, &world, owner).unwrap();
    assert_eq!(objects.get(owner).unwrap(), &before);
    assert_eq!(*world.player(&objects, owner).unwrap(), record);
    let child = objects.get(child).unwrap();
    assert_eq!(
        child.base.position,
        Vector3 {
            x: 17,
            y: -1000,
            z: 203
        }
    );
    assert_eq!(child.base.yaw.units(), 3);
}

#[test]
fn ignoring_contacts_and_nonwalker_skip_unobserved_records_but_still_refresh() {
    for ignored in [true, false] {
        let (mut objects, mut world, owner, child) = fixture();
        let record = world.player_mut(&objects, owner).unwrap();
        record.contact.as_mut().unwrap().ignores_contacts = ignored;
        record.camera_ground = None;
        record.motion = None;
        if ignored {
            record.auxiliary = None;
        } else {
            record.auxiliary.as_mut().unwrap().mode = 0xFF;
        }
        publish(&mut objects, &world, owner).unwrap();
        assert_eq!(
            objects.get(child).unwrap().base.position,
            objects.get(owner).unwrap().base.position
        );
        assert_eq!(objects.get(child).unwrap().base.yaw.units(), 254);
    }
}

#[test]
fn missing_dependencies_keep_the_exact_completed_temporary_pose_prefix() {
    for stage in 0..4 {
        let (mut objects, mut world, owner, _) = fixture();
        let record = world.player_mut(&objects, owner).unwrap();
        let error = match stage {
            0 => {
                record.contact = None;
                PlayerAttachmentError::World(WorldInputError::MissingPlayerContact(owner))
            }
            1 => {
                record.auxiliary = None;
                PlayerAttachmentError::World(WorldInputError::MissingAuxiliary(owner))
            }
            2 => {
                record.camera_ground = None;
                PlayerAttachmentError::MissingGround(owner)
            }
            _ => {
                record.motion = None;
                PlayerAttachmentError::MissingMotion(owner)
            }
        };
        assert_eq!(publish(&mut objects, &world, owner), Err(error));
        assert_eq!(
            objects.get(owner).unwrap().base.position.y,
            if stage == 3 { -1000 } else { 91 }
        );
        assert_eq!(objects.get(owner).unwrap().base.yaw.units(), 254);
    }
}

#[test]
fn invalid_child_keeps_walker_override_and_earlier_child_publication() {
    let (mut objects, world, owner, first) = fixture();
    let second = objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    objects.get_mut(first).unwrap().base.next_sibling = Some(second);
    assert_eq!(
        publish(&mut objects, &world, owner),
        Err(PlayerAttachmentError::Attachment(
            AttachmentError::MissingParent(second)
        ))
    );
    assert_eq!(objects.get(first).unwrap().base.position.y, -1000);
    assert_eq!(objects.get(owner).unwrap().base.position.y, -1000);
    assert_eq!(objects.get(owner).unwrap().base.yaw.units(), 3);
}
