//! Nearest-shape source contracts, without running original machine code.
use super::*;
use crate::{Behavior, Object, ObjectKind, ShapeId, Vector3};

fn actor(shape: ShapeId, x: i16, z: i16) -> Object {
    let mut actor = Object::new(ObjectKind::Enemy, shape, Behavior::FollowPath);
    actor.base.position = Vector3 { x, y: 0, z };
    actor
}

fn search(objects: &mut ObjectStore, owner: ObjectId, shape: Option<ShapeId>) {
    apply(objects, owner, RelationshipCommand::FindNearest { shape }).unwrap();
}

#[test]
fn nearest_shape_filters_differ_and_only_the_attachment_is_replaced() {
    let mut objects = ObjectStore::new();
    let shape = ShapeId::from_catalog_index(19);
    let old = objects.allocate(actor(shape, 500, 0)).unwrap();
    let owner = objects.allocate(actor(shape, 0, 0)).unwrap();
    let general = objects.allocate(actor(ShapeId::EMPTY, 10, 0)).unwrap();
    let specific = objects.allocate(actor(shape, 20, 0)).unwrap();
    objects
        .get_mut(general)
        .unwrap()
        .base
        .flags
        .general_search_eligible = true;
    let candidate = objects.get_mut(specific).unwrap();
    candidate.base.flags.general_search_eligible = false;
    candidate.base.flags.collision_disabled = true;
    candidate.base.flags.remove_after_tick = true;
    candidate.base.hit_points = 0;
    candidate.base.position.y = i16::MIN;
    let candidate_before = candidate.clone();
    let owner_actor = objects.get_mut(owner).unwrap();
    owner_actor.base.attachment = Some(old);
    owner_actor.base.first_child = Some(old);
    owner_actor.base.next_sibling = Some(general);
    owner_actor.extension.parent = Some(old);
    owner_actor.extension.relative_position = Vector3 {
        x: 77,
        y: -88,
        z: 99,
    };
    owner_actor.extension.path_state.motion.attached_coordinates = true;
    owner_actor.extension.path_state.motion.relative_coordinates = true;
    let mut expected = owner_actor.clone();
    let order = objects.active_ids().to_vec();
    for (filter, found) in [
        (None, Some(general)),
        (Some(shape), Some(specific)),
        (Some(ShapeId::EMPTY), Some(general)),
        (Some(ShapeId::from_catalog_index(20)), None),
    ] {
        search(&mut objects, owner, filter);
        expected.base.attachment = found;
        assert_eq!(objects.get(owner), Some(&expected));
        assert_eq!(objects.get(specific), Some(&candidate_before));
        assert_eq!(objects.active_ids(), order);
    }
}

#[test]
fn nearest_shape_ties_follow_active_order_not_slot_order_and_skip_self() {
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor(ShapeId::EMPTY, 0, 0)).unwrap();
    let first_allocated = objects.allocate(actor(ShapeId::EMPTY, -100, 0)).unwrap();
    let first_in_list = objects.allocate(actor(ShapeId::EMPTY, 100, 0)).unwrap();
    assert_eq!(
        objects.active_ids(),
        &[first_in_list, first_allocated, owner]
    );
    search(&mut objects, owner, Some(ShapeId::EMPTY));
    assert_eq!(
        objects.get(owner).unwrap().base.attachment,
        Some(first_in_list)
    );
    let closer = objects
        .allocate_scoped_after(owner, actor(ShapeId::EMPTY, 2, 0))
        .unwrap();
    search(&mut objects, owner, Some(ShapeId::EMPTY));
    assert_eq!(objects.get(owner).unwrap().base.attachment, Some(closer));
    objects.remove(first_in_list).unwrap();
    objects.remove(first_allocated).unwrap();
    objects.remove(closer).unwrap();
    search(&mut objects, owner, Some(ShapeId::EMPTY));
    assert_eq!(objects.get(owner).unwrap().base.attachment, None);
}

#[test]
fn nearest_shape_signed_range_edges_wrapping_and_ignored_height() {
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor(ShapeId::EMPTY, 0, 0)).unwrap();
    let candidate = objects.allocate(actor(ShapeId::EMPTY, 0, 0)).unwrap();
    // Exact outputs of the statically pinned length arithmetic. Negative
    // wrapped lengths are excluded, even when geometrically nearby in Y.
    for (dx, dz, distance) in [
        (0i16, 0i16, 0i16),
        (2, 0, 1),
        (3, 0, 1),
        (12442, 0, 6998),
        (12444, 0, 6999),
        (12446, 0, 7000),
        (32767, 0, -6146),
        (i16::MIN, 0, 4096),
        (i16::MIN, i16::MIN, -6144),
        (20000, 20000, -5826),
    ] {
        assert_eq!(sf_core::aim_angle::sf2_xz_angle_distance(dx, dz), distance);
        for origin in [0i16, i16::MIN, i16::MAX] {
            objects.get_mut(owner).unwrap().base.position = Vector3 {
                x: origin,
                y: i16::MAX,
                z: origin,
            };
            objects.get_mut(candidate).unwrap().base.position = Vector3 {
                x: origin.wrapping_sub(dx),
                y: i16::MIN,
                z: origin.wrapping_sub(dz),
            };
            search(&mut objects, owner, Some(ShapeId::EMPTY));
            assert_eq!(
                objects.get(owner).unwrap().base.attachment,
                (distance >= 0 && distance < 7000).then_some(candidate)
            );
        }
    }
}

#[test]
fn nearest_shape_full_pool_every_owner_and_atomic_missing_owner() {
    let mut objects = ObjectStore::new();
    for _ in 0..OBJECT_CAPACITY {
        objects.allocate(actor(ShapeId::EMPTY, 0, 0)).unwrap();
    }
    let order = objects.active_ids().to_vec();
    for &owner in &order {
        search(&mut objects, owner, Some(ShapeId::EMPTY));
        assert_eq!(
            objects.get(owner).unwrap().base.attachment,
            order.iter().copied().find(|id| *id != owner)
        );
    }
    let removed = order[0];
    objects.remove(removed).unwrap();
    let before: Vec<_> = objects
        .active_objects()
        .map(|(id, value)| (id, value.clone()))
        .collect();
    assert_eq!(
        apply(
            &mut objects,
            removed,
            RelationshipCommand::FindNearest { shape: None }
        ),
        Err(RelationshipError::MissingActor(removed))
    );
    assert_eq!(
        objects
            .active_objects()
            .map(|(id, value)| (id, value.clone()))
            .collect::<Vec<_>>(),
        before
    );
}
