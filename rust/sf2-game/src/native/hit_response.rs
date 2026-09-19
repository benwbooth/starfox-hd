//! Contact hit response (`$03:A327..A469`).
//!
//! This service consumes the paired contact store and invokes real world
//! handlers. It does not invent collision geometry or substitute generic
//! damage for authored callbacks. The world must retain the shared callback
//! context across invocations, as callbacks can change its damage and cursor.

use super::collision_contacts::{ContactError, ContactHost, ContactId};
use super::{Object, ObjectId};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum HitSide {
    #[default]
    Primary,
    Secondary,
}

/// Named contact-response state. Comments identify source flags for the
/// eventual object-state adapter; no runtime encoded-field access is needed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HitActor {
    pub health: u8,
    pub attack_power: u8,
    /// Other actor's authored auxiliary byte, NOT the contact touch count.
    pub contact_parameter: u8,
    /// Skip this visit's entire contact list (source flag 25 bit 10).
    pub skip_contacts: bool,
    /// Do not set the hit marker (source flag 24 bit 08).
    pub suppress_hit_marker: bool,
    /// Source flag 20 bit 02, independent of the pending-contact bit.
    pub hit_marked: bool,
    /// Two actors with this flag do not damage one another (31 bit 80).
    pub mutually_non_damaging: bool,
    /// With the preceding flag, attribute the hit to a side (31 bit 08).
    pub credits_hit_side: bool,
    /// Source flag 23 bit 40 selects the secondary attribution.
    pub hit_side: HitSide,
    pub hit_by_primary: bool,
    pub hit_by_secondary: bool,
    /// Source class flag 31 bit 01 suppresses outgoing default damage.
    pub suppress_attack_damage: bool,
    /// Source flag 26 bit 01 enables the new-contact reaction latch.
    pub latch_new_contact: bool,
    /// Source flag 22 bit 02. Set even if no new-contact handler exists.
    pub new_contact_latched: bool,
    /// Source flag 26 bit 08 bypasses the assigned-strategy pause gate.
    pub run_when_paused: bool,
}

/// A short-lived borrow of the actual actor fields changed by hit response.
/// No health or contact flags are copied back after a world callback.
pub struct HitActorMut<'a> {
    pub health: &'a mut u8,
    pub hit_marked: &'a mut bool,
    pub hit_by_primary: &'a mut bool,
    pub hit_by_secondary: &'a mut bool,
    pub new_contact_latched: &'a mut bool,
    pub suppress_hit_marker: bool,
    pub mutually_non_damaging: bool,
}

impl HitActor {
    pub fn from_object(object: &Object) -> Self {
        let contact = &object.base.contacts;
        Self {
            health: object.base.hit_points,
            attack_power: object.base.attack_power,
            contact_parameter: contact.parameter,
            skip_contacts: contact.skip_contacts,
            suppress_hit_marker: contact.suppress_hit_marker,
            hit_marked: contact.hit_marked,
            mutually_non_damaging: contact.mutually_non_damaging,
            credits_hit_side: contact.credits_hit_side,
            hit_side: contact.hit_side,
            hit_by_primary: contact.hit_by_primary,
            hit_by_secondary: contact.hit_by_secondary,
            suppress_attack_damage: contact.suppress_attack_damage,
            latch_new_contact: contact.latch_new_contact,
            new_contact_latched: contact.new_contact_latched,
            run_when_paused: contact.run_when_paused,
        }
    }

    pub fn as_mut(&mut self) -> HitActorMut<'_> {
        HitActorMut {
            health: &mut self.health,
            hit_marked: &mut self.hit_marked,
            hit_by_primary: &mut self.hit_by_primary,
            hit_by_secondary: &mut self.hit_by_secondary,
            new_contact_latched: &mut self.new_contact_latched,
            suppress_hit_marker: self.suppress_hit_marker,
            mutually_non_damaging: self.mutually_non_damaging,
        }
    }
}

impl<'a> HitActorMut<'a> {
    pub fn from_object(object: &'a mut Object) -> Self {
        let contact = &mut object.base.contacts;
        Self {
            health: &mut object.base.hit_points,
            hit_marked: &mut contact.hit_marked,
            hit_by_primary: &mut contact.hit_by_primary,
            hit_by_secondary: &mut contact.hit_by_secondary,
            new_contact_latched: &mut contact.new_contact_latched,
            suppress_hit_marker: contact.suppress_hit_marker,
            mutually_non_damaging: contact.mutually_non_damaging,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitCallback {
    NewContact,
    ContinuingContact,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HitContext {
    pub current_contact: Option<ContactId>,
    pub damage: u8,
    /// Written only before a registered continuing-contact callback. A new
    /// callback sees the previously retained value, just as in the source.
    pub other_parameter: u8,
}

pub trait HitResponseHost: ContactHost {
    fn hit_actor(&self, id: ObjectId) -> Option<HitActor>;
    fn hit_actor_mut(&mut self, id: ObjectId) -> Option<HitActorMut<'_>>;
    fn has_hit_callback(&self, owner: ObjectId, kind: HitCallback) -> bool;
    fn run_hit_callback(
        &mut self,
        owner: ObjectId,
        other: ObjectId,
        kind: HitCallback,
        context: &mut HitContext,
    ) -> Result<(), Self::Error>;
    fn strategies_paused(&self) -> bool;
    /// Resolve the actor's CURRENT assigned strategy, after hit callbacks.
    fn run_assigned_strategy(&mut self, owner: ObjectId) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitError<E> {
    MissingActor(ObjectId),
    MissingCursor,
    Contacts(ContactError),
    Host(E),
}

/// Subtraction is a wrapping byte operation followed by a SIGN test. This
/// differs from saturating subtraction for damage or health above 127.
pub fn health_after_damage(health: u8, damage: u8) -> u8 {
    let remaining = health.wrapping_sub(damage);
    if (remaining as i8) < 0 {
        0
    } else {
        remaining
    }
}

fn actor<H: HitResponseHost>(host: &H, id: ObjectId) -> Result<HitActor, HitError<H::Error>> {
    host.hit_actor(id).ok_or(HitError::MissingActor(id))
}

pub fn respond<H: HitResponseHost>(
    host: &mut H,
    owner: ObjectId,
    context: &mut HitContext,
) -> Result<(), HitError<H::Error>> {
    let mut cursor = if actor(host, owner)?.skip_contacts {
        None
    } else {
        host.contacts().first(owner)
    };
    while let Some(id) = cursor {
        context.current_contact = Some(id);
        let entry = host
            .contacts()
            .get(id)
            .copied()
            .ok_or(HitError::Contacts(ContactError::MissingContact(id)))?;
        let other = actor(host, entry.other)?;
        let current = host
            .hit_actor_mut(owner)
            .ok_or(HitError::MissingActor(owner))?;
        if !current.suppress_hit_marker {
            *current.hit_marked = true;
        }
        if other.mutually_non_damaging && other.credits_hit_side {
            match other.hit_side {
                HitSide::Primary => *current.hit_by_primary = true,
                HitSide::Secondary => *current.hit_by_secondary = true,
            }
        }
        if !(current.mutually_non_damaging && other.mutually_non_damaging) {
            context.damage = if other.suppress_attack_damage {
                0
            } else {
                other.attack_power
            };
            let is_new = host
                .contacts_mut()
                .take_new_contact(id)
                .map_err(HitError::Contacts)?;
            if is_new && actor(host, owner)?.latch_new_contact {
                *host
                    .hit_actor_mut(owner)
                    .ok_or(HitError::MissingActor(owner))?
                    .new_contact_latched = true;
            }
            let kind = if is_new && host.has_hit_callback(owner, HitCallback::NewContact) {
                Some(HitCallback::NewContact)
            } else if host.has_hit_callback(owner, HitCallback::ContinuingContact) {
                Some(HitCallback::ContinuingContact)
            } else {
                None
            };
            if let Some(kind) = kind {
                if kind == HitCallback::ContinuingContact {
                    context.other_parameter = actor(host, entry.other)?.contact_parameter;
                }
                host.run_hit_callback(owner, entry.other, kind, context)
                    .map_err(HitError::Host)?;
            }
            let current = host
                .hit_actor_mut(owner)
                .ok_or(HitError::MissingActor(owner))?;
            *current.health = health_after_damage(*current.health, context.damage);
        }
        // Unlike cleanup/retirement, hit response reads next AFTER callbacks,
        // and follows the callback's current-contact context if it changed.
        let current = context.current_contact.ok_or(HitError::MissingCursor)?;
        cursor = host
            .contacts()
            .get(current)
            .ok_or(HitError::Contacts(ContactError::MissingContact(current)))?
            .next();
    }
    if actor(host, owner)?.run_when_paused || !host.strategies_paused() {
        host.run_assigned_strategy(owner).map_err(HitError::Host)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision_contacts::{Contact, ContactStore};
    use crate::{Behavior, Object, ObjectKind, ObjectStore, ShapeId};

    struct World {
        contacts: ContactStore,
        actors: Vec<HitActor>,
        ids: Vec<ObjectId>,
        new_handler: bool,
        continuing_handler: bool,
        callbacks: Vec<(HitCallback, u8, u8, bool)>,
        assigned: usize,
        paused: bool,
        replace_damage: Option<u8>,
        replace_health: Option<u8>,
        insert_contact: Option<ObjectId>,
    }

    impl World {
        fn new() -> Self {
            let mut objects = ObjectStore::new();
            let ids = (0..4)
                .map(|_| {
                    objects
                        .allocate(Object::new(
                            ObjectKind::Enemy,
                            ShapeId::EMPTY,
                            Behavior::EnemyFlight,
                        ))
                        .unwrap()
                })
                .collect();
            Self {
                contacts: ContactStore::default(),
                actors: vec![
                    HitActor {
                        health: 100,
                        attack_power: 3,
                        ..HitActor::default()
                    };
                    4
                ],
                ids,
                new_handler: false,
                continuing_handler: false,
                callbacks: vec![],
                assigned: 0,
                paused: false,
                replace_damage: None,
                replace_health: None,
                insert_contact: None,
            }
        }

        fn pair(&mut self, other: usize) -> ContactId {
            self.contacts
                .record_pair(self.ids[0], self.ids[other], [None; 2])
                .unwrap()[0]
        }

        fn respond(&mut self, context: &mut HitContext) {
            respond(self, self.ids[0], context).unwrap();
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
        fn on_separation(&mut self, _: ContactId, _: Contact) -> Result<(), Self::Error> {
            panic!("hit response must not separate contacts")
        }
    }

    impl HitResponseHost for World {
        fn hit_actor(&self, id: ObjectId) -> Option<HitActor> {
            self.actors.get(id.index()).copied()
        }
        fn hit_actor_mut(&mut self, id: ObjectId) -> Option<HitActorMut<'_>> {
            self.actors.get_mut(id.index()).map(HitActor::as_mut)
        }
        fn has_hit_callback(&self, _: ObjectId, kind: HitCallback) -> bool {
            match kind {
                HitCallback::NewContact => self.new_handler,
                HitCallback::ContinuingContact => self.continuing_handler,
            }
        }
        fn run_hit_callback(
            &mut self,
            owner: ObjectId,
            _: ObjectId,
            kind: HitCallback,
            context: &mut HitContext,
        ) -> Result<(), Self::Error> {
            self.callbacks.push((
                kind,
                context.damage,
                context.other_parameter,
                self.contacts
                    .get(context.current_contact.unwrap())
                    .unwrap()
                    .new_contact,
            ));
            if let Some(damage) = self.replace_damage {
                context.damage = damage;
            }
            if let Some(health) = self.replace_health {
                self.actors[owner.index()].health = health;
            }
            if let Some(other) = self.insert_contact.take() {
                self.contacts.record_pair(owner, other, [None; 2]).unwrap();
            }
            Ok(())
        }
        fn strategies_paused(&self) -> bool {
            self.paused
        }
        fn run_assigned_strategy(&mut self, _: ObjectId) -> Result<(), Self::Error> {
            self.assigned += 1;
            Ok(())
        }
    }

    #[test]
    fn signed_result_damage_matches_all_byte_pairs() {
        for health in 0..=u8::MAX {
            for damage in 0..=u8::MAX {
                let word_result = (i16::from(health) - i16::from(damage)).rem_euclid(256);
                assert_eq!(
                    health_after_damage(health, damage),
                    if word_result < 128 {
                        word_result as u8
                    } else {
                        0
                    }
                );
            }
        }
        assert_eq!(health_after_damage(0, 255), 1);
        assert_eq!(health_after_damage(200, 0), 0);
    }

    #[test]
    fn new_callback_replaces_continuing_and_sees_cleared_latch_and_retained_parameter() {
        let mut world = World::new();
        let pair = world.pair(1);
        world.new_handler = true;
        world.continuing_handler = true;
        world.actors[0].latch_new_contact = true;
        world.actors[1].contact_parameter = 45;
        let mut context = HitContext {
            other_parameter: 72,
            ..HitContext::default()
        };
        world.respond(&mut context);
        assert_eq!(
            world.callbacks,
            vec![(HitCallback::NewContact, 3, 72, false)]
        );
        assert_eq!(world.actors[0].health, 97);
        assert!(world.actors[0].new_contact_latched);
        world.respond(&mut context);
        assert_eq!(
            world.callbacks[1],
            (HitCallback::ContinuingContact, 3, 45, false)
        );
        assert_eq!(world.contacts.get(pair).unwrap().touches, 1);
        assert_eq!(world.assigned, 2);
    }

    #[test]
    fn absent_new_callback_falls_through_and_damage_uses_post_callback_health() {
        let mut world = World::new();
        world.pair(1);
        world.continuing_handler = true;
        world.replace_damage = Some(4);
        world.replace_health = Some(9);
        world.actors[1].contact_parameter = 23;
        world.respond(&mut HitContext::default());
        assert_eq!(
            world.callbacks,
            vec![(HitCallback::ContinuingContact, 3, 23, false)]
        );
        assert_eq!(world.actors[0].health, 5);
    }

    #[test]
    fn mutually_non_damaging_pair_still_marks_and_attributes_but_keeps_new_flag() {
        let mut world = World::new();
        let pair = world.pair(1);
        world.new_handler = true;
        world.actors[0].mutually_non_damaging = true;
        world.actors[0].latch_new_contact = true;
        world.actors[1].mutually_non_damaging = true;
        world.actors[1].credits_hit_side = true;
        world.actors[1].hit_side = HitSide::Secondary;
        world.respond(&mut HitContext::default());
        assert!(world.actors[0].hit_marked);
        assert!(world.actors[0].hit_by_secondary);
        assert!(!world.actors[0].hit_by_primary);
        assert!(!world.actors[0].new_contact_latched);
        assert_eq!(world.actors[0].health, 100);
        assert!(world.contacts.get(pair).unwrap().new_contact);
        assert!(world.callbacks.is_empty());
    }

    #[test]
    fn outgoing_damage_suppression_does_not_suppress_callback_or_new_latch() {
        let mut world = World::new();
        world.pair(1);
        world.actors[0].suppress_hit_marker = true;
        world.actors[0].latch_new_contact = true;
        world.actors[1].suppress_attack_damage = true;
        world.continuing_handler = true;
        world.respond(&mut HitContext::default());
        assert!(!world.actors[0].hit_marked);
        assert!(world.actors[0].new_contact_latched);
        assert_eq!(world.callbacks[0].1, 0);
        assert_eq!(world.actors[0].health, 100);
    }

    #[test]
    fn contact_skip_and_assigned_pause_are_independent_gates() {
        let mut world = World::new();
        let pair = world.pair(1);
        world.actors[0].skip_contacts = true;
        world.respond(&mut HitContext::default());
        assert_eq!(world.assigned, 1);
        assert!(world.contacts.get(pair).unwrap().new_contact);
        world.actors[0].skip_contacts = false;
        world.paused = true;
        world.respond(&mut HitContext::default());
        assert_eq!(world.actors[0].health, 97);
        assert_eq!(world.assigned, 1);
        world.actors[0].run_when_paused = true;
        world.respond(&mut HitContext::default());
        assert_eq!(world.assigned, 2);
    }

    #[test]
    fn callback_inserted_next_contact_is_processed_in_same_visit() {
        let mut world = World::new();
        world.pair(1);
        world.new_handler = true;
        world.insert_contact = Some(world.ids[2]);
        world.respond(&mut HitContext::default());
        assert_eq!(world.callbacks.len(), 2);
        assert_eq!(world.actors[0].health, 94);
    }
}
