//! Paired collision-contact lifetime, from `$7F:3ED2..4028` and `$7F:4090`.
//!
//! Contacts are directional: one pair consumes two entries from the shared
//! pool. The world owns this store alongside its objects and must retire all
//! contacts before releasing an object slot. Geometry and hit response are
//! separate systems; neither can be replaced by contact bookkeeping.

use super::{ObjectId, OBJECT_CAPACITY};

/// The source initializes sixty entries, allowing thirty simultaneous pairs.
pub const CONTACT_CAPACITY: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContactId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contact {
    pub owner: ObjectId,
    pub other: ObjectId,
    pub reciprocal: ContactId,
    /// Authored collision-box flags for this direction, not object flags.
    pub hit_flags: u8,
    pub new_contact: bool,
    pub refreshed: bool,
    /// Wrapping number of collision detections, not elapsed frames.
    pub touches: u8,
    next: Option<ContactId>,
    previous: Option<ContactId>,
}

impl Contact {
    pub fn next(self) -> Option<ContactId> {
        self.next
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactError {
    SelfContact,
    CapacityReached,
    MissingContact(ContactId),
    ContactsRemainAfterRetirement(ObjectId),
}

#[derive(Debug, Clone)]
pub struct ContactStore {
    entries: [Option<Contact>; CONTACT_CAPACITY],
    heads: [Option<ContactId>; OBJECT_CAPACITY],
    free: Vec<ContactId>,
}

impl Default for ContactStore {
    fn default() -> Self {
        Self {
            entries: [None; CONTACT_CAPACITY],
            heads: [None; OBJECT_CAPACITY],
            free: (0..CONTACT_CAPACITY).rev().map(ContactId).collect(),
        }
    }
}

impl ContactStore {
    pub fn len(&self) -> usize {
        CONTACT_CAPACITY - self.free.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, id: ContactId) -> Option<&Contact> {
        self.entries[id.0].as_ref()
    }

    pub fn first(&self, owner: ObjectId) -> Option<ContactId> {
        self.heads[owner.index()]
    }

    pub fn find(&self, owner: ObjectId, other: ObjectId) -> Option<ContactId> {
        let mut cursor = self.first(owner);
        while let Some(id) = cursor {
            let contact = self.get(id).expect("linked contact must exist");
            if contact.other == other {
                return Some(id);
            }
            cursor = contact.next;
        }
        None
    }

    /// Both directions are refreshed, even when only one has new box flags.
    /// Simple collisions update only the probe's flags; compound collisions
    /// update both. Existing pairs are keyed by actors, never by hit group.
    /// The caller must also set both actors' pending-hit flags and accumulate
    /// the supplied box flags into the respective actor's hit flags.
    pub fn record_pair(
        &mut self,
        owner: ObjectId,
        other: ObjectId,
        hit_flags: [Option<u8>; 2],
    ) -> Result<[ContactId; 2], ContactError> {
        if owner == other {
            return Err(ContactError::SelfContact);
        }
        let pair = if let Some(id) = self.find(owner, other) {
            [id, self.get(id).expect("found contact").reciprocal]
        } else {
            // Valid source lists always allocate and release whole pairs.
            // Exhaustion is a diagnostic, never eviction or a partial pair.
            if self.free.len() < 2 {
                return Err(ContactError::CapacityReached);
            }
            let first = self.free.pop().expect("reserved first contact");
            let second = self.free.pop().expect("reserved second contact");
            self.insert(first, owner, other, second);
            self.insert(second, other, owner, first);
            [first, second]
        };
        for (id, flags) in pair.into_iter().zip(hit_flags) {
            let contact = self.entries[id.0].as_mut().expect("paired contact");
            contact.refreshed = true;
            contact.touches = contact.touches.wrapping_add(1);
            if let Some(flags) = flags {
                contact.hit_flags = flags;
            }
        }
        Ok(pair)
    }

    /// Hit response consumes this before calling the new-contact callback.
    pub fn take_new_contact(&mut self, id: ContactId) -> Result<bool, ContactError> {
        let entry = self.entries[id.0]
            .as_mut()
            .ok_or(ContactError::MissingContact(id))?;
        Ok(std::mem::take(&mut entry.new_contact))
    }

    fn insert(&mut self, id: ContactId, owner: ObjectId, other: ObjectId, reciprocal: ContactId) {
        // New contacts go AFTER the current head, rather than at either end.
        let previous = self.first(owner);
        let next = previous.and_then(|head| self.get(head).expect("contact head").next);
        self.entries[id.0] = Some(Contact {
            owner,
            other,
            reciprocal,
            hit_flags: 0,
            new_contact: true,
            refreshed: false,
            touches: 0,
            next,
            previous,
        });
        if let Some(previous) = previous {
            self.entries[previous.0]
                .as_mut()
                .expect("contact head")
                .next = Some(id);
        } else {
            self.heads[owner.index()] = Some(id);
        }
        if let Some(next) = next {
            self.entries[next.0]
                .as_mut()
                .expect("next contact")
                .previous = Some(id);
        }
    }

    fn release(&mut self, id: ContactId) -> Result<(), ContactError> {
        let entry = self.entries[id.0]
            .take()
            .ok_or(ContactError::MissingContact(id))?;
        if let Some(previous) = entry.previous {
            self.entries[previous.0]
                .as_mut()
                .expect("previous contact")
                .next = entry.next;
        } else {
            self.heads[entry.owner.index()] = entry.next;
        }
        if let Some(next) = entry.next {
            self.entries[next.0]
                .as_mut()
                .expect("next contact")
                .previous = entry.previous;
        }
        self.free.push(id);
        Ok(())
    }
}

/// Required world callbacks; there is deliberately no ignored-callback default.
/// A callback can modify gameplay and other contact pairs. It must not release
/// the pair currently being separated or free either endpoint's object slot.
pub trait ContactHost {
    type Error;
    fn contacts(&self) -> &ContactStore;
    fn contacts_mut(&mut self) -> &mut ContactStore;
    fn on_separation(&mut self, id: ContactId, contact: Contact) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeparationError<E> {
    Contacts(ContactError),
    Host(E),
}

fn contact<H: ContactHost>(host: &H, id: ContactId) -> Result<Contact, SeparationError<H::Error>> {
    host.contacts()
        .get(id)
        .copied()
        .ok_or(SeparationError::Contacts(ContactError::MissingContact(id)))
}

/// Both callbacks run while BOTH entries are still linked. The reciprocal
/// entry is released first, making the initiating entry the next free slot.
pub fn separate_pair<H: ContactHost>(
    host: &mut H,
    id: ContactId,
) -> Result<(), SeparationError<H::Error>> {
    let first = contact(host, id)?;
    host.on_separation(id, first)
        .map_err(SeparationError::Host)?;
    let reciprocal = contact(host, id)?.reciprocal;
    let reverse = contact(host, reciprocal)?;
    host.on_separation(reciprocal, reverse)
        .map_err(SeparationError::Host)?;
    host.contacts_mut()
        .release(reciprocal)
        .map_err(SeparationError::Contacts)?;
    host.contacts_mut()
        .release(id)
        .map_err(SeparationError::Contacts)
}

/// End-of-epoch cleanup visits each owner's directional list independently.
/// Save next BEFORE separation callbacks, matching the source cleanup loop.
pub fn clean_owner<H: ContactHost>(
    host: &mut H,
    owner: ObjectId,
) -> Result<(), SeparationError<H::Error>> {
    let mut cursor = host.contacts().first(owner);
    while let Some(id) = cursor {
        let entry = contact(host, id)?;
        cursor = entry.next;
        if entry.refreshed {
            host.contacts_mut().entries[id.0]
                .as_mut()
                .expect("current contact")
                .refreshed = false;
        } else {
            separate_pair(host, id)?;
        }
    }
    Ok(())
}

/// Call before object-slot retirement, regardless of the refresh flags.
pub fn retire_owner<H: ContactHost>(
    host: &mut H,
    owner: ObjectId,
) -> Result<(), SeparationError<H::Error>> {
    let mut cursor = host.contacts().first(owner);
    while let Some(id) = cursor {
        cursor = contact(host, id)?.next;
        separate_pair(host, id)?;
    }
    if host.contacts().first(owner).is_some() {
        return Err(SeparationError::Contacts(
            ContactError::ContactsRemainAfterRetirement(owner),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, Object, ObjectKind, ObjectStore, ShapeId};

    fn actors(count: usize) -> Vec<ObjectId> {
        let mut objects = ObjectStore::new();
        (0..count)
            .map(|_| {
                objects
                    .allocate(Object::new(
                        ObjectKind::Effect,
                        ShapeId::EMPTY,
                        Behavior::Effect,
                    ))
                    .unwrap()
            })
            .collect()
    }

    #[derive(Default)]
    struct World {
        contacts: ContactStore,
        separated: Vec<(ContactId, Contact, usize)>,
        fail_callback: bool,
    }

    impl ContactHost for World {
        type Error = &'static str;
        fn contacts(&self) -> &ContactStore {
            &self.contacts
        }
        fn contacts_mut(&mut self) -> &mut ContactStore {
            &mut self.contacts
        }
        fn on_separation(&mut self, id: ContactId, entry: Contact) -> Result<(), Self::Error> {
            assert_eq!(self.contacts.find(entry.owner, entry.other), Some(id));
            assert_eq!(
                self.contacts.find(entry.other, entry.owner),
                Some(entry.reciprocal)
            );
            self.separated.push((id, entry, self.contacts.len()));
            if self.fail_callback {
                Err("callback failure")
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn pair_refresh_preserves_new_flag_and_directional_flags_and_wraps_touches() {
        let ids = actors(2);
        let mut contacts = ContactStore::default();
        let pair = contacts
            .record_pair(ids[0], ids[1], [Some(0x80), Some(0x04)])
            .unwrap();
        assert_eq!(contacts.get(pair[0]).unwrap().touches, 1);
        assert!(contacts.take_new_contact(pair[0]).unwrap());
        for _ in 1..256 {
            assert_eq!(
                contacts
                    .record_pair(ids[0], ids[1], [Some(0x02), None])
                    .unwrap(),
                pair
            );
        }
        assert_eq!(contacts.len(), 2);
        assert_eq!(contacts.get(pair[0]).unwrap().touches, 0);
        assert_eq!(contacts.get(pair[0]).unwrap().hit_flags, 0x02);
        assert!(!contacts.get(pair[0]).unwrap().new_contact);
        assert!(contacts.get(pair[1]).unwrap().new_contact);
        assert_eq!(contacts.get(pair[1]).unwrap().hit_flags, 0x04);
        assert_eq!(
            contacts.record_pair(ids[1], ids[0], [None, None]).unwrap(),
            [pair[1], pair[0]]
        );
    }

    #[test]
    fn insertion_keeps_first_head_and_inserts_later_contacts_immediately_after_it() {
        let ids = actors(4);
        let mut world = World::default();
        let first = world
            .contacts
            .record_pair(ids[0], ids[1], [None; 2])
            .unwrap();
        let second = world
            .contacts
            .record_pair(ids[0], ids[2], [None; 2])
            .unwrap();
        let third = world
            .contacts
            .record_pair(ids[0], ids[3], [None; 2])
            .unwrap();
        assert_eq!(world.contacts.first(ids[0]), Some(first[0]));
        assert_eq!(world.contacts.get(first[0]).unwrap().next(), Some(third[0]));
        assert_eq!(
            world.contacts.get(third[0]).unwrap().next(),
            Some(second[0])
        );
        separate_pair(&mut world, third[0]).unwrap();
        assert_eq!(
            world.contacts.get(first[0]).unwrap().next(),
            Some(second[0])
        );
        separate_pair(&mut world, first[0]).unwrap();
        assert_eq!(world.contacts.first(ids[0]), Some(second[0]));
        separate_pair(&mut world, second[0]).unwrap();
        assert!(world.contacts.is_empty());
        assert!(ids.into_iter().all(|id| world.contacts.first(id).is_none()));
    }

    #[test]
    fn both_callbacks_see_linked_pair_and_release_order_controls_reuse() {
        let ids = actors(2);
        let mut world = World::default();
        let pair = world
            .contacts
            .record_pair(ids[0], ids[1], [None; 2])
            .unwrap();
        separate_pair(&mut world, pair[1]).unwrap();
        assert_eq!(
            world
                .separated
                .iter()
                .map(|event| (event.0, event.2))
                .collect::<Vec<_>>(),
            vec![(pair[1], 2), (pair[0], 2)]
        );
        assert_eq!(
            world
                .contacts
                .record_pair(ids[0], ids[1], [None; 2])
                .unwrap(),
            [pair[1], pair[0]]
        );
    }

    #[test]
    fn directional_cleanup_clears_refresh_once_then_separates_on_next_epoch() {
        let ids = actors(2);
        let mut world = World::default();
        let pair = world
            .contacts
            .record_pair(ids[0], ids[1], [None; 2])
            .unwrap();
        clean_owner(&mut world, ids[0]).unwrap();
        assert!(!world.contacts.get(pair[0]).unwrap().refreshed);
        assert!(world.contacts.get(pair[1]).unwrap().refreshed);
        clean_owner(&mut world, ids[1]).unwrap();
        assert!(world.separated.is_empty());
        clean_owner(&mut world, ids[1]).unwrap();
        assert!(world.contacts.is_empty());
        assert_eq!(world.separated[0].0, pair[1]);
        clean_owner(&mut world, ids[0]).unwrap();
        assert_eq!(world.separated.len(), 2);
    }

    #[test]
    fn retirement_separates_refreshed_contacts_in_live_list_order() {
        let ids = actors(4);
        let mut world = World::default();
        let pairs: Vec<_> = ids[1..]
            .iter()
            .map(|&other| {
                world
                    .contacts
                    .record_pair(ids[0], other, [None; 2])
                    .unwrap()
            })
            .collect();
        retire_owner(&mut world, ids[0]).unwrap();
        assert!(world.contacts.is_empty());
        assert_eq!(
            world
                .separated
                .iter()
                .step_by(2)
                .map(|event| event.0)
                .collect::<Vec<_>>(),
            vec![pairs[0][0], pairs[2][0], pairs[1][0]]
        );
    }

    #[test]
    fn full_pool_diagnoses_without_eviction_but_existing_pair_can_refresh() {
        let ids = actors(OBJECT_CAPACITY);
        let mut contacts = ContactStore::default();
        for pair in ids.chunks_exact(2) {
            contacts.record_pair(pair[0], pair[1], [None; 2]).unwrap();
        }
        assert_eq!(contacts.len(), CONTACT_CAPACITY);
        assert_eq!(
            contacts.record_pair(ids[0], ids[2], [None; 2]),
            Err(ContactError::CapacityReached)
        );
        assert!(contacts.record_pair(ids[0], ids[1], [None; 2]).is_ok());
        assert_eq!(
            contacts.record_pair(ids[0], ids[0], [None; 2]),
            Err(ContactError::SelfContact)
        );
        assert_eq!(contacts.len(), CONTACT_CAPACITY);
    }

    #[test]
    fn callback_failure_is_reported_without_releasing_pair() {
        let ids = actors(2);
        let mut world = World {
            fail_callback: true,
            ..World::default()
        };
        let pair = world
            .contacts
            .record_pair(ids[0], ids[1], [None; 2])
            .unwrap();
        assert_eq!(
            separate_pair(&mut world, pair[0]),
            Err(SeparationError::Host("callback failure"))
        );
        assert_eq!(world.contacts.len(), 2);
        assert_eq!(world.separated.len(), 1);
    }
}
