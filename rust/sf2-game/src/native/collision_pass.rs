//! Source collision queue (`$7F:32A1`) and ordered pass (`$7F:402D..494C`).
//! Shape profiles are captured before intervening frame work; actor poses,
//! links, and pair filters remain live. Detection has no response callbacks.

use super::collision_boxes::{hit_by_probe, Collider};
use super::collision_contacts::{self, ContactError, ContactHost, ContactStore, SeparationError};
use super::{Object, ObjectId, ObjectStore, Rotation, ShapeId};
use sf2_data::contact_box_data::ContactBoxGroupId;

/// Shared membership in any of the five authored exclusion groups prevents
/// a pair. These bits are NOT the existing high-level CollisionClass enum.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExclusionGroups(u8);

impl ExclusionGroups {
    /// Exclusion membership assigned by authored path spawners (31 bit 10).
    pub const PATH_SPAWN: Self = Self(0x10);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn from_authored_class(class: u8) -> Self {
        Self(class & 0xF8)
    }

    pub const fn excludes(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

/// Contact-specific actor state. Health, pose, shape, retirement, collision
/// disable, explosion, links, and accumulated hit flags use ordinary fields.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ActorContacts {
    /// Source class bit 04; cleared by the first strategy visit.
    pub first_strategy_visit: bool,
    pub exclusion_groups: ExclusionGroups,
    /// Both actors must opt in before equal shapes may collide (22 bit 20).
    pub allow_same_shape: bool,
    /// Source 20 bit 80, set by detection and consumed by strategy selection.
    pub pending_hit: bool,
    /// Copy of the preceding epoch's pending flag (21 bit 02).
    pub previous_hit: bool,
    /// Source 22 bit 08, copied to skip_contacts during cleanup.
    pub suppress_contacts_next_epoch: bool,
    /// Source 25 bit 10, read by contact response.
    pub skip_contacts: bool,
    /// Suppress the hit marker (24 bit 08), not collision detection.
    pub suppress_hit_marker: bool,
    /// Shared hit marker (20 bit 02), independent of pending_hit.
    pub hit_marked: bool,
    /// Two actors of this class do not damage one another (31 bit 80).
    pub mutually_non_damaging: bool,
    /// Attribute contacts to this actor's player side (31 bit 08).
    pub credits_hit_side: bool,
    /// Outgoing attribution (23 bit 40), not path target selection.
    pub hit_side: super::hit_response::HitSide,
    /// Incoming attribution (26 bits 02 and 04), also used by path triggers.
    pub hit_by_primary: bool,
    pub hit_by_secondary: bool,
    /// Source class 31 bit 01 suppresses outgoing default damage.
    pub suppress_attack_damage: bool,
    /// Source 26 bit 01 enables the new-contact latch (22 bit 02).
    pub latch_new_contact: bool,
    pub new_contact_latched: bool,
    /// Shared assigned-strategy pause exemption (26 bit 08).
    pub run_when_paused: bool,
}

impl ActorContacts {
    fn advance_epoch(&mut self) {
        self.skip_contacts = self.suppress_contacts_next_epoch;
        self.previous_hit = std::mem::take(&mut self.pending_hit);
    }
}

#[derive(Debug, Clone, Copy)]
struct QueueEntry {
    actor: ObjectId,
    half_extents: [u16; 3],
    boxes: Option<ContactBoxGroupId>,
}

impl QueueEntry {
    fn collider(self, actor: &Object) -> Collider {
        Collider {
            position: actor.base.position,
            rotation: Rotation {
                pitch: actor.base.pitch,
                yaw: actor.base.yaw,
                roll: actor.base.roll,
            },
            half_extents: self.half_extents,
            boxes: self.boxes,
            animation_frame: actor.extension.animation_frame,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollisionError {
    MissingActor(ObjectId),
    MissingShape(ShapeId),
    Contacts(ContactError),
}

/// A single-use snapshot: the original queue is exhausted by the pass.
#[derive(Debug, Default)]
pub struct CollisionQueue {
    entries: Vec<QueueEntry>,
}

fn eligible(actor: &Object) -> bool {
    !actor.base.contacts.first_strategy_visit
        && !actor.base.flags.collision_disabled
        && !actor.base.flags.remove_after_tick
        && actor.base.hit_points != 0
        && !actor.base.flags.exploding
}

fn pair_allowed(first_id: ObjectId, first: &Object, second_id: ObjectId, second: &Object) -> bool {
    !first
        .base
        .contacts
        .exclusion_groups
        .excludes(second.base.contacts.exclusion_groups)
        && first.base.linked_object != Some(second_id)
        && second.base.linked_object != Some(first_id)
        && (first.base.shape != second.base.shape
            || (first.base.contacts.allow_same_shape && second.base.contacts.allow_same_shape))
}

impl CollisionQueue {
    pub fn build(objects: &ObjectStore, disabled: bool) -> Result<Self, CollisionError> {
        let mut queue = Self::default();
        if disabled {
            return Ok(queue);
        }
        for &id in objects.active_ids() {
            let actor = objects.get(id).ok_or(CollisionError::MissingActor(id))?;
            if !eligible(actor) {
                continue;
            }
            let profile = Collider::from_shape(
                actor.base.shape,
                actor.base.position,
                Rotation::default(),
                actor.extension.animation_frame,
            )
            .ok_or(CollisionError::MissingShape(actor.base.shape))?;
            queue.entries.push(QueueEntry {
                actor: id,
                half_extents: profile.half_extents,
                boxes: profile.boxes,
            });
        }
        Ok(queue)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Call after epoch cleanup, with the actual shared strategy clock.
    /// Returns detections, NOT unique pairs. A compound probe's entire later
    /// candidate list is visited before advancing to its next authored box.
    pub fn detect(
        self,
        objects: &mut ObjectStore,
        contacts: &mut ContactStore,
        strategy_clock: u8,
    ) -> Result<usize, CollisionError> {
        let mut detections = 0;
        for (index, entry) in self.entries.iter().copied().enumerate() {
            let first = objects
                .get(entry.actor)
                .ok_or(CollisionError::MissingActor(entry.actor))?;
            let collider = entry.collider(first);
            for probe in collider.world_boxes(strategy_clock) {
                for candidate in &self.entries[index + 1..] {
                    let first = objects
                        .get(entry.actor)
                        .ok_or(CollisionError::MissingActor(entry.actor))?;
                    let second = objects
                        .get(candidate.actor)
                        .ok_or(CollisionError::MissingActor(candidate.actor))?;
                    if !pair_allowed(entry.actor, first, candidate.actor, second) {
                        continue;
                    }
                    let Some(flags) =
                        hit_by_probe(probe, candidate.collider(second), strategy_clock)
                    else {
                        continue;
                    };
                    contacts
                        .record_pair(entry.actor, candidate.actor, [Some(probe.hit_flags), flags])
                        .map_err(CollisionError::Contacts)?;
                    let first = objects.get_mut(entry.actor).expect("validated probe actor");
                    first.base.contacts.pending_hit = true;
                    first.base.hit_flags |= probe.hit_flags;
                    let second = objects
                        .get_mut(candidate.actor)
                        .expect("validated candidate actor");
                    second.base.contacts.pending_hit = true;
                    if let Some(flags) = flags {
                        second.base.hit_flags |= flags;
                    }
                    detections += 1;
                }
            }
        }
        Ok(detections)
    }
}

/// The world owns full retirement (attachments, contacts, callbacks, and
/// finally the object slot). Cleanup cannot substitute a bare pool removal.
pub trait CollisionEpochHost: ContactHost {
    fn objects(&self) -> &ObjectStore;
    fn objects_mut(&mut self) -> &mut ObjectStore;
    fn retire_object(&mut self, actor: ObjectId) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpochError<E> {
    MissingActor(ObjectId),
    Separation(SeparationError<E>),
    Host(E),
}

/// Before detection: retire deferred actors, roll the two flag latches, and
/// clean each directional contact list. Retiring actors save their next link
/// BEFORE callbacks; survivors read their next link AFTER contact cleanup.
pub fn clean_epoch<H: CollisionEpochHost>(host: &mut H) -> Result<(), EpochError<H::Error>> {
    let mut cursor = host.objects().active_ids().first().copied();
    while let Some(id) = cursor {
        let actor = host.objects().get(id).ok_or(EpochError::MissingActor(id))?;
        if actor.base.flags.remove_after_tick {
            cursor = actor.base.next;
            host.retire_object(id).map_err(EpochError::Host)?;
        } else {
            host.objects_mut()
                .get_mut(id)
                .ok_or(EpochError::MissingActor(id))?
                .base
                .contacts
                .advance_epoch();
            collision_contacts::clean_owner(host, id).map_err(EpochError::Separation)?;
            cursor = host
                .objects()
                .get(id)
                .ok_or(EpochError::MissingActor(id))?
                .base
                .next;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{Behavior, ObjectKind, OBJECT_CAPACITY};
    use super::*;
    use crate::collision_contacts::{Contact, ContactId};

    fn actor(shape: ShapeId, depth: i16) -> Object {
        let mut actor = Object::new(ObjectKind::Enemy, shape, Behavior::FollowPath);
        actor.base.hit_points = 10;
        actor.base.contacts.first_strategy_visit = false;
        actor.base.position.z = depth;
        actor
    }

    fn pair() -> (ObjectStore, ObjectId, ObjectId) {
        let mut objects = ObjectStore::new();
        let target = objects.allocate(actor(ShapeId::EMPTY, 0)).unwrap();
        let probe = objects.allocate(actor(ShapeId::ENEMY_LASER, 0)).unwrap();
        (objects, probe, target)
    }

    #[test]
    fn queue_filters_every_disqualifier_without_inventing_visibility_or_class_gates() {
        for mask in 0..32 {
            let mut objects = ObjectStore::new();
            let mut object = actor(ShapeId::EMPTY, 0);
            object.base.contacts.first_strategy_visit = mask & 1 != 0;
            object.base.flags.collision_disabled = mask & 2 != 0;
            object.base.flags.remove_after_tick = mask & 4 != 0;
            object.base.hit_points = if mask & 8 != 0 { 0 } else { 1 };
            object.base.flags.exploding = mask & 16 != 0;
            object.base.flags.visible = false;
            objects.allocate(object).unwrap();
            assert_eq!(
                CollisionQueue::build(&objects, false).unwrap().len(),
                usize::from(mask == 0)
            );
            assert!(CollisionQueue::build(&objects, true).unwrap().is_empty());
        }
        let object = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Effect);
        assert!(object.base.contacts.first_strategy_visit);
    }

    #[test]
    fn groups_mask_only_authored_exclusion_bits_and_same_shape_requires_both_opt_ins() {
        for left in 0..=u8::MAX {
            for right in 0..=u8::MAX {
                assert_eq!(
                    ExclusionGroups::from_authored_class(left)
                        .excludes(ExclusionGroups::from_authored_class(right)),
                    left & right & 0xF8 != 0
                );
            }
        }
        let (mut objects, first, second) = pair();
        objects.get_mut(first).unwrap().base.shape = ShapeId::EMPTY;
        for mask in 0..4 {
            objects
                .get_mut(first)
                .unwrap()
                .base
                .contacts
                .allow_same_shape = mask & 1 != 0;
            objects
                .get_mut(second)
                .unwrap()
                .base
                .contacts
                .allow_same_shape = mask & 2 != 0;
            assert_eq!(
                pair_allowed(
                    first,
                    objects.get(first).unwrap(),
                    second,
                    objects.get(second).unwrap()
                ),
                mask == 3
            );
        }
        for link_owner in [first, second] {
            objects.get_mut(link_owner).unwrap().base.linked_object =
                Some(if link_owner == first { second } else { first });
            assert!(!pair_allowed(
                first,
                objects.get(first).unwrap(),
                second,
                objects.get(second).unwrap()
            ));
            objects.get_mut(link_owner).unwrap().base.linked_object = None;
        }
    }

    #[test]
    fn queue_captures_profiles_but_reads_new_poses_shapes_and_filters() {
        let (mut objects, probe, target) = pair();
        let queue = CollisionQueue::build(&objects, false).unwrap();
        // The queued laser's old three-box profile remains after a shape
        // change. Its new origin puts the last center on the ordinary target.
        let first = objects.get_mut(probe).unwrap();
        first.base.shape = ShapeId::FOX_FALCO_FLIGHT_CRAFT;
        first.base.position.z = 160;
        let mut contacts = ContactStore::default();
        assert_eq!(queue.detect(&mut objects, &mut contacts, 0), Ok(1));
        assert_eq!(
            contacts
                .get(contacts.first(probe).unwrap())
                .unwrap()
                .hit_flags,
            7
        );
        assert!(objects.get(target).unwrap().base.contacts.pending_hit);
        let queue = CollisionQueue::build(&objects, false).unwrap();
        objects.get_mut(probe).unwrap().base.linked_object = Some(target);
        assert_eq!(queue.detect(&mut objects, &mut contacts, 0), Ok(0));
    }

    #[test]
    fn all_probe_boxes_visit_later_candidates_before_advancing_to_next_box() {
        let mut objects = ObjectStore::new();
        let early_box_target = objects.allocate(actor(ShapeId::EMPTY, 0)).unwrap();
        let late_box_target = objects.allocate(actor(ShapeId::EMPTY, -160)).unwrap();
        let probe = objects.allocate(actor(ShapeId::ENEMY_LASER, 0)).unwrap();
        let mut contacts = ContactStore::default();
        let queue = CollisionQueue::build(&objects, false).unwrap();
        assert_eq!(queue.detect(&mut objects, &mut contacts, 0), Ok(2));
        let head = contacts.get(contacts.first(probe).unwrap()).unwrap();
        assert_eq!(head.other, early_box_target);
        assert_eq!(
            contacts.get(head.next().unwrap()).unwrap().other,
            late_box_target
        );
    }

    #[test]
    fn each_overlapping_box_refreshes_both_directions_and_preserves_simple_target_flags() {
        let (mut objects, probe, target) = pair();
        let mut queue = CollisionQueue::build(&objects, false).unwrap();
        // A synthetic ordinary profile spanning every authored laser box.
        queue
            .entries
            .iter_mut()
            .find(|entry| entry.actor == target)
            .unwrap()
            .half_extents = [200; 3];
        let mut contacts = ContactStore::default();
        let ids = contacts
            .record_pair(probe, target, [Some(1), Some(0x40)])
            .unwrap();
        objects.get_mut(target).unwrap().base.hit_flags = 0x20;
        assert_eq!(queue.detect(&mut objects, &mut contacts, 0), Ok(3));
        assert_eq!(contacts.get(ids[0]).unwrap().touches, 4);
        assert_eq!(contacts.get(ids[1]).unwrap().touches, 4);
        assert_eq!(contacts.get(ids[0]).unwrap().hit_flags, 7);
        assert_eq!(contacts.get(ids[1]).unwrap().hit_flags, 0x40);
        assert_eq!(objects.get(probe).unwrap().base.hit_flags, 7);
        assert_eq!(objects.get(target).unwrap().base.hit_flags, 0x20);
    }

    #[test]
    fn compound_target_flags_accumulate_once_per_probe_box() {
        let mut objects = ObjectStore::new();
        let target = objects
            .allocate(actor(ShapeId::FOX_FALCO_FLIGHT_CRAFT, 0))
            .unwrap();
        let probe = objects.allocate(actor(ShapeId::EMPTY, 0)).unwrap();
        let mut queue = CollisionQueue::build(&objects, false).unwrap();
        queue.entries[0].half_extents = [40; 3];
        let mut contacts = ContactStore::default();
        assert_eq!(queue.detect(&mut objects, &mut contacts, 0), Ok(1));
        assert_eq!(objects.get(target).unwrap().base.hit_flags, 7);
        assert_eq!(objects.get(probe).unwrap().base.hit_flags, 0);
        assert_eq!(
            contacts
                .get(contacts.first(target).unwrap())
                .unwrap()
                .touches,
            1
        );
    }

    #[test]
    fn missing_catalog_entries_and_removed_queued_actors_are_explicit_errors() {
        let (mut objects, probe, _) = pair();
        let queue = CollisionQueue::build(&objects, false).unwrap();
        objects.remove(probe).unwrap();
        assert_eq!(
            queue.detect(&mut objects, &mut ContactStore::default(), 0),
            Err(CollisionError::MissingActor(probe))
        );
        let invalid = ShapeId::from_catalog_index(u16::MAX);
        objects.allocate(actor(invalid, 0)).unwrap();
        assert_eq!(
            CollisionQueue::build(&objects, false).unwrap_err(),
            CollisionError::MissingShape(invalid)
        );
    }

    #[derive(Default)]
    struct World {
        objects: ObjectStore,
        contacts: ContactStore,
        separated: Vec<ObjectId>,
        insert_after: Option<ObjectId>,
        child: Option<ObjectId>,
    }

    impl ContactHost for World {
        type Error = &'static str;
        fn contacts(&self) -> &ContactStore {
            &self.contacts
        }
        fn contacts_mut(&mut self) -> &mut ContactStore {
            &mut self.contacts
        }
        fn on_separation(&mut self, _: ContactId, contact: Contact) -> Result<(), Self::Error> {
            self.separated.push(contact.owner);
            if let Some(parent) = self.insert_after.take() {
                let mut child = actor(ShapeId::EMPTY, 0);
                child.base.contacts.pending_hit = true;
                self.child = Some(
                    self.objects
                        .allocate_after(Some(parent), child)
                        .ok_or("allocation failed")?,
                );
            }
            Ok(())
        }
    }

    impl CollisionEpochHost for World {
        fn objects(&self) -> &ObjectStore {
            &self.objects
        }
        fn objects_mut(&mut self) -> &mut ObjectStore {
            &mut self.objects
        }
        fn retire_object(&mut self, id: ObjectId) -> Result<(), Self::Error> {
            // This test world has contacts but no attachment/auxiliary owners.
            collision_contacts::retire_owner(self, id).map_err(|_| "separation failed")?;
            self.objects.remove(id).ok_or("missing retired actor")?;
            Ok(())
        }
    }

    #[test]
    fn epoch_rolls_latches_before_detection_without_clearing_accumulated_box_flags() {
        let (objects, probe, target) = pair();
        let mut world = World {
            objects,
            ..World::default()
        };
        {
            let object = world.objects.get_mut(probe).unwrap();
            object.base.contacts.pending_hit = true;
            object.base.contacts.suppress_contacts_next_epoch = true;
            object.base.hit_flags = 0x80;
        }
        let queue = CollisionQueue::build(&world.objects, false).unwrap();
        clean_epoch(&mut world).unwrap();
        let state = world.objects.get(probe).unwrap().base.contacts;
        assert!(state.previous_hit && state.skip_contacts && !state.pending_hit);
        queue
            .detect(&mut world.objects, &mut world.contacts, 0)
            .unwrap();
        assert!(world.objects.get(probe).unwrap().base.contacts.pending_hit);
        assert!(world.objects.get(target).unwrap().base.contacts.pending_hit);
        assert_eq!(world.objects.get(probe).unwrap().base.hit_flags, 0x87);
        clean_epoch(&mut world).unwrap();
        assert!(world.separated.is_empty());
        clean_epoch(&mut world).unwrap();
        assert_eq!(world.separated, [probe, target]);
        assert!(!world.objects.get(probe).unwrap().base.contacts.previous_hit);
    }

    #[test]
    fn survivor_follows_callback_inserted_next_but_retirement_keeps_saved_successor() {
        for retiring in [false, true] {
            let (objects, probe, target) = pair();
            let mut world = World {
                objects,
                ..World::default()
            };
            world
                .contacts
                .record_pair(probe, target, [None; 2])
                .unwrap();
            clean_epoch(&mut world).unwrap();
            world
                .objects
                .get_mut(probe)
                .unwrap()
                .base
                .flags
                .remove_after_tick = retiring;
            world.insert_after = Some(probe);
            clean_epoch(&mut world).unwrap();
            let child = world.objects.get(world.child.unwrap()).unwrap();
            assert_eq!(child.base.contacts.previous_hit, !retiring);
            assert_eq!(child.base.contacts.pending_hit, retiring);
            assert_eq!(world.objects.get(probe).is_none(), retiring);
            assert_eq!(world.separated, [probe, target]);
        }
    }

    #[test]
    fn full_contact_pool_reports_failure_without_setting_unrecorded_actor_hits() {
        let mut objects = ObjectStore::new();
        for _ in 0..OBJECT_CAPACITY {
            objects.allocate(actor(ShapeId::EMPTY, 0)).unwrap();
        }
        let ids = objects.active_ids().to_vec();
        let probe = ids[0];
        let target = ids[2];
        let mut contacts = ContactStore::default();
        for pair in ids.chunks_exact(2) {
            contacts.record_pair(pair[0], pair[1], [None; 2]).unwrap();
        }
        objects.get_mut(probe).unwrap().base.shape = ShapeId::ENEMY_LASER;
        let mut queue = CollisionQueue::build(&objects, false).unwrap();
        queue
            .entries
            .retain(|entry| entry.actor == probe || entry.actor == target);
        assert_eq!(
            queue.detect(&mut objects, &mut contacts, 0),
            Err(CollisionError::Contacts(ContactError::CapacityReached))
        );
        assert!(!objects.get(probe).unwrap().base.contacts.pending_hit);
        assert!(!objects.get(target).unwrap().base.contacts.pending_hit);
        assert_eq!(contacts.len(), collision_contacts::CONTACT_CAPACITY);
    }
}
