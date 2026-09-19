//! Ordered world retirement (`$7F:335A..33B1`). The actor remains live while
//! contact callbacks run. Scene detachment is deferred; actor-slot recycling
//! happens only after contacts, relationships, and owned programs are released.

use super::collision_contacts::{self, ContactHost, SeparationError};
use super::scene_proxy::{SceneProxyError, SceneProxyStore};
use super::{Object, ObjectId, ObjectStore};

pub trait RetirementHost: ContactHost {
    fn objects(&self) -> &ObjectStore;
    fn objects_and_proxies_mut(&mut self) -> (&mut ObjectStore, &mut SceneProxyStore);
    /// Release owned path/callback allocations and clear registrations and
    /// loop state (`$7F:19A7`). Contact callbacks must remain registered until
    /// separation finishes. This operation must not free the actor itself.
    fn release_actor_programs(&mut self, owner: ObjectId) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetirementError<E> {
    MissingActor(ObjectId),
    SceneProxy(SceneProxyError),
    Separation(SeparationError<E>),
    Host(E),
}

pub fn retire<H: RetirementHost>(
    host: &mut H,
    owner: ObjectId,
) -> Result<Object, RetirementError<H::Error>> {
    if host.objects().get(owner).is_none() {
        return Err(RetirementError::MissingActor(owner));
    }
    {
        let (objects, proxies) = host.objects_and_proxies_mut();
        proxies
            .retire_actor(objects, owner)
            .map_err(RetirementError::SceneProxy)?;
    }
    collision_contacts::retire_owner(host, owner).map_err(RetirementError::Separation)?;
    {
        let (objects, _) = host.objects_and_proxies_mut();
        if objects.get(owner).is_none() {
            return Err(RetirementError::MissingActor(owner));
        }
        objects.detach_relationships(owner);
    }
    host.release_actor_programs(owner)
        .map_err(RetirementError::Host)?;
    host.objects_and_proxies_mut()
        .0
        .remove_detached(owner)
        .ok_or(RetirementError::MissingActor(owner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision_contacts::{Contact, ContactId, ContactStore};
    use crate::hit_response::HitCallback;
    use crate::{Behavior, ObjectKind, PathCursor, PathId, ShapeId, OBJECT_CAPACITY};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Event {
        Separate(ObjectId),
        ReleasePrograms(ObjectId),
    }

    struct World {
        objects: ObjectStore,
        proxies: SceneProxyStore,
        contacts: ContactStore,
        programs: Vec<Vec<HitCallback>>,
        owner: ObjectId,
        other: ObjectId,
        child: ObjectId,
        events: Vec<Event>,
        fail_separation: bool,
        allocation_during_callback: Vec<Option<ObjectId>>,
    }

    fn effect() -> Object {
        Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Effect)
    }

    impl World {
        fn new() -> Self {
            let mut objects = ObjectStore::new();
            let owner = objects.allocate(effect()).unwrap();
            let other = objects.allocate(effect()).unwrap();
            let child = objects.allocate(effect()).unwrap();
            objects.get_mut(owner).unwrap().base.first_child = Some(child);
            let value = objects.get_mut(child).unwrap();
            value.base.attachment = Some(owner);
            value.extension.parent = Some(owner);
            value.base.flags.remove_with_parent = true;
            objects.get_mut(other).unwrap().base.linked_object = Some(owner);
            while objects.len() < OBJECT_CAPACITY {
                objects.allocate(effect()).unwrap();
            }
            let mut proxies = SceneProxyStore::default();
            proxies
                .capture_actor(
                    &mut objects,
                    owner,
                    PathCursor {
                        path: PathId::from_catalog_index(2),
                        command_index: 4,
                    },
                )
                .unwrap()
                .unwrap();
            let mut contacts = ContactStore::default();
            contacts.record_pair(owner, other, [None, None]).unwrap();
            Self {
                objects,
                proxies,
                contacts,
                owner,
                other,
                child,
                programs: vec![
                    vec![HitCallback::NewContact, HitCallback::ContinuingContact];
                    OBJECT_CAPACITY
                ],
                events: Vec::new(),
                fail_separation: false,
                allocation_during_callback: Vec::new(),
            }
        }
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
            self.events.push(Event::Separate(contact.owner));
            // Both callbacks see a live actor and programs, detached proxy,
            // still-linked child and interaction owner, and no recycled slot.
            assert!(self.objects.get(self.owner).is_some());
            assert_eq!(
                self.objects.get(self.owner).unwrap().extension.scene_proxy,
                None
            );
            assert!(!self.programs[self.owner.index()].is_empty());
            let proxy = self.proxies.get(self.proxies.active_ids()[0]).unwrap();
            assert_eq!(proxy.owner, None);
            assert!(proxy.flags.actor_retired());
            assert_eq!(
                self.objects.get(self.child).unwrap().extension.parent,
                Some(self.owner)
            );
            assert_eq!(
                self.objects.get(self.other).unwrap().base.linked_object,
                Some(self.owner)
            );
            assert_eq!(self.contacts.len(), 2);
            self.allocation_during_callback
                .push(self.objects.allocate(effect()));
            if self.fail_separation {
                Err("separation failed")
            } else {
                Ok(())
            }
        }
    }

    impl RetirementHost for World {
        fn objects(&self) -> &ObjectStore {
            &self.objects
        }
        fn objects_and_proxies_mut(&mut self) -> (&mut ObjectStore, &mut SceneProxyStore) {
            (&mut self.objects, &mut self.proxies)
        }
        fn release_actor_programs(&mut self, owner: ObjectId) -> Result<(), Self::Error> {
            self.events.push(Event::ReleasePrograms(owner));
            assert!(self.objects.get(owner).is_some());
            assert!(self.contacts.is_empty());
            assert_eq!(self.objects.get(self.child).unwrap().extension.parent, None);
            assert_eq!(
                self.objects.get(self.other).unwrap().base.linked_object,
                None
            );
            self.programs[owner.index()].clear();
            Ok(())
        }
    }

    #[test]
    fn world_retirement_observes_source_order_and_only_then_reuses_actor_slot() {
        let mut world = World::new();
        let owner = world.owner;
        let old_lifetime = world.objects.lifetime_id(owner).unwrap();
        retire(&mut world, owner).unwrap();
        assert_eq!(
            world.events,
            [
                Event::Separate(owner),
                Event::Separate(world.other),
                Event::ReleasePrograms(owner)
            ]
        );
        assert_eq!(world.allocation_during_callback, [None, None]);
        assert_eq!(world.objects.len(), OBJECT_CAPACITY - 1);
        assert!(
            world
                .objects
                .get(world.child)
                .unwrap()
                .base
                .flags
                .remove_after_tick
        );
        assert_eq!(world.proxies.len(), 1);
        assert!(world.programs[owner.index()].is_empty());
        let replacement = world.objects.allocate(effect()).unwrap();
        assert_eq!(replacement, owner);
        assert_ne!(world.objects.lifetime_id(replacement), Some(old_lifetime));
    }

    #[test]
    fn callback_failure_keeps_identity_and_later_resources_owned() {
        let mut world = World::new();
        world.fail_separation = true;
        let owner = world.owner;
        assert_eq!(
            retire(&mut world, owner),
            Err(RetirementError::Separation(SeparationError::Host(
                "separation failed"
            )))
        );
        assert_eq!(world.events, [Event::Separate(owner)]);
        assert_eq!(world.objects.len(), OBJECT_CAPACITY);
        assert_eq!(world.contacts.len(), 2);
        assert!(!world.programs[owner.index()].is_empty());
        assert_eq!(
            world.objects.get(world.child).unwrap().extension.parent,
            Some(owner)
        );
    }

    #[test]
    fn missing_actor_has_no_retirement_side_effects() {
        let mut world = World::new();
        let owner = world.owner;
        retire(&mut world, owner).unwrap();
        world.events.clear();
        assert_eq!(
            retire(&mut world, owner),
            Err(RetirementError::MissingActor(owner))
        );
        assert!(world.events.is_empty());
        assert_eq!(world.proxies.len(), 1);
    }
}
