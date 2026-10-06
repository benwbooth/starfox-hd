//! Player contact callback (`$06:9707..9841` and turn gates at `$06:9842`).
//! This is shared by new, continuing, and separation contacts. Hull damage
//! remains the surrounding hit-response service's responsibility.

use super::collision_contacts::ContactError;
use super::hit_response::{HitContext, HitResponseHost, HitSide};
use super::path_protection::DeflectionProtection;
use super::player_hit_control::{self, ContactSound, ContactTurn, Impact, PlayerHitControl};
use super::{ObjectId, ObjectStore};

const PART_HIT_MASK: u8 = 0x07;
// Authored box channels to player feedback channels. Do not assign left/right
// anatomy from the bit ordering; shape-specific boxes own that interpretation.
const PART_FEEDBACK_REMAP: [(u8, u8); 3] = [(0x02, 0x80), (0x04, 0x40), (0x01, 0x20)];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerContactState {
    pub hit: PlayerHitControl,
    /// Player status 6B77 bit 01.
    pub active: bool,
    /// Player auxiliary 6A72 bit 10.
    pub ignores_contacts: bool,
    /// Only the low five bits of player protection 6C02 are tested.
    pub deflection_count: u8,
    /// Protection bit 40: projectile-only deflection with angular scatter.
    pub projectile_deflection: bool,
    /// Player mode high nibble 20 suppresses the contact turn.
    pub suppress_contact_turn: bool,
    /// Accumulated player-part feedback, distinct from object box flags.
    pub part_feedback: u8,
    pub turn: Option<ContactTurn>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ContactRules {
    /// Shared D7F4 is nonzero.
    pub enabled: bool,
    /// Shared 1D72 is nonzero.
    pub blocked: bool,
    /// Shared character/mode 1DE2 equals 9.
    pub suppress_turn: bool,
    pub strategy_clock: u8,
}

/// Implementations supply the same actors and context as HitResponseHost.
/// Reflection and sound are required world operations, not optional hooks.
pub trait PlayerContactHost: HitResponseHost {
    fn objects(&self) -> &ObjectStore;
    fn objects_mut(&mut self) -> &mut ObjectStore;
    // Read only when the source reaches each gate. The scene binds these to
    // the same auxiliary/protection/particle owners used by authored paths.
    fn player_hit(&self, owner: ObjectId) -> Result<&PlayerHitControl, Self::Error>;
    fn player_hit_mut(&mut self, owner: ObjectId) -> Result<&mut PlayerHitControl, Self::Error>;
    fn contact_active(&self, owner: ObjectId) -> Result<bool, Self::Error>;
    fn ignores_contacts(&self, owner: ObjectId) -> Result<bool, Self::Error>;
    fn protection(&self, owner: ObjectId) -> Result<DeflectionProtection, Self::Error>;
    fn suppress_contact_turn(&self, owner: ObjectId) -> Result<bool, Self::Error>;
    fn part_feedback(&mut self, owner: ObjectId) -> Result<&mut u8, Self::Error>;
    fn contact_turn(&mut self, owner: ObjectId) -> Result<&mut Option<ContactTurn>, Self::Error>;
    fn contacts_enabled(&self) -> Result<bool, Self::Error>;
    fn contacts_blocked(&self) -> Result<bool, Self::Error>;
    fn suppress_turn(&self) -> Result<bool, Self::Error>;
    fn strategy_clock(&self) -> u8;
    fn primary_player(&self) -> Option<ObjectId>;
    /// Other actor flag 25 bit 80, independent of projectile class.
    fn damages_player_parts(&self, other: ObjectId) -> Result<bool, Self::Error>;
    fn reflect_contacts(&mut self, owner: ObjectId) -> Result<(), Self::Error>;
    fn queue_contact_sound(
        &mut self,
        sound: ContactSound,
        side: HitSide,
    ) -> Result<(), Self::Error>;
    fn random_byte(&mut self) -> u8;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerContactError<E> {
    MissingPlayer(ObjectId),
    MissingActor(ObjectId),
    MissingContact,
    Contacts(ContactError),
    Host(E),
}

fn sound<H: PlayerContactHost>(
    host: &mut H,
    owner: ObjectId,
    cue: ContactSound,
) -> Result<(), PlayerContactError<H::Error>> {
    let side = if host.primary_player() == Some(owner) {
        HitSide::Primary
    } else {
        HitSide::Secondary
    };
    host.queue_contact_sound(cue, side)
        .map_err(PlayerContactError::Host)
}

fn finish<H: PlayerContactHost>(
    host: &mut H,
    owner: ObjectId,
    context: &mut HitContext,
    ignored: bool,
) -> Result<(), PlayerContactError<H::Error>> {
    if ignored {
        context.damage = 0;
        *host
            .hit_actor_mut(owner)
            .ok_or(PlayerContactError::MissingActor(owner))?
            .hit_marked = false;
    }
    host.player_hit_mut(owner)
        .map_err(PlayerContactError::Host)?
        .absorb_with_reserve(&mut context.damage);
    Ok(())
}

pub fn respond<H: PlayerContactHost>(
    host: &mut H,
    owner: ObjectId,
    context: &mut HitContext,
) -> Result<(), PlayerContactError<H::Error>> {
    if host
        .player_hit(owner)
        .map_err(PlayerContactError::Host)?
        .recovery
        != 0
        || !host
            .contact_active(owner)
            .map_err(PlayerContactError::Host)?
        || host
            .ignores_contacts(owner)
            .map_err(PlayerContactError::Host)?
    {
        return finish(host, owner, context, true);
    }

    // These first three gates precede resolving the contact. In particular,
    // a recovery-time separation callback need not inspect its other actor.
    let contact = context
        .current_contact
        .ok_or(PlayerContactError::MissingContact)?;
    let other_id = host
        .contacts()
        .get(contact)
        .ok_or(PlayerContactError::Contacts(ContactError::MissingContact(
            contact,
        )))?
        .other;
    if !host.contacts_enabled().map_err(PlayerContactError::Host)?
        || host.contacts_blocked().map_err(PlayerContactError::Host)?
    {
        return finish(host, owner, context, true);
    }

    let protection = host.protection(owner).map_err(PlayerContactError::Host)?;
    let timed_deflection = protection.remaining() != 0;
    // Do not inspect the other actor's class before the protection bit admits
    // it. The turn service has its own later class test.
    let deflect_projectile = !timed_deflection
        && protection.projectile_deflection()
        && host
            .hit_actor(other_id)
            .ok_or(PlayerContactError::MissingActor(other_id))?
            .credits_hit_side; // The same source class bit 31:08.
    if timed_deflection || deflect_projectile {
        if timed_deflection {
            host.player_hit_mut(owner)
                .map_err(PlayerContactError::Host)?
                .request_deflection_feedback();
        }
        if host
            .player_hit(owner)
            .map_err(PlayerContactError::Host)?
            .deflection_sound_due()
        {
            sound(host, owner, ContactSound::Deflection)?;
            let random = host.random_byte();
            host.player_hit_mut(owner)
                .map_err(PlayerContactError::Host)?
                .set_deflection_sound_cooldown(random);
        }
        host.reflect_contacts(owner)
            .map_err(PlayerContactError::Host)?;
        return finish(host, owner, context, true);
    }

    let actor = host
        .objects()
        .get(owner)
        .ok_or(PlayerContactError::MissingActor(owner))?;
    let hit_flags = actor.base.hit_flags;
    let position = actor.base.position;
    let yaw = actor.base.yaw;
    if host
        .damages_player_parts(other_id)
        .map_err(PlayerContactError::Host)?
        && hit_flags & PART_HIT_MASK != 0
    {
        let feedback = host
            .part_feedback(owner)
            .map_err(PlayerContactError::Host)?;
        for (source, destination) in PART_FEEDBACK_REMAP {
            if hit_flags & source != 0 {
                *feedback |= destination;
            }
        }
    }
    // Even actors which cannot damage parts consume the low box-hit bits.
    host.objects_mut()
        .get_mut(owner)
        .ok_or(PlayerContactError::MissingActor(owner))?
        .base
        .hit_flags &= !PART_HIT_MASK;
    if !host.suppress_turn().map_err(PlayerContactError::Host)?
        && !host
            .suppress_contact_turn(owner)
            .map_err(PlayerContactError::Host)?
        && !host
            .hit_actor(other_id)
            .ok_or(PlayerContactError::MissingActor(other_id))?
            .credits_hit_side
    {
        let other_position = host
            .objects()
            .get(other_id)
            .ok_or(PlayerContactError::MissingActor(other_id))?
            .base
            .position;
        *host.contact_turn(owner).map_err(PlayerContactError::Host)? = Some(
            player_hit_control::turn_from_contact(position, yaw, other_position),
        );
    }
    let clock = host.strategy_clock();
    host.player_hit_mut(owner)
        .map_err(PlayerContactError::Host)?
        .impact(Impact::Heavy, clock);
    // Cue selection sees original damage, before reserve absorption.
    sound(
        host,
        owner,
        player_hit_control::impact_sound(context.damage),
    )?;
    finish(host, owner, context, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision_contacts::{Contact, ContactHost, ContactId, ContactStore};
    use crate::hit_response::{self, HitActor, HitCallback};
    use crate::{Behavior, Object, ObjectKind, ShapeId, Vector3};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Event {
        Sound(ContactSound, HitSide),
        Random,
        Reflect(ObjectId),
        Strategy(ObjectId),
        Separate(ObjectId),
    }

    struct World {
        objects: ObjectStore,
        contacts: ContactStore,
        actors: Vec<HitActor>,
        player: PlayerContactState,
        rules: ContactRules,
        owner: ObjectId,
        other: ObjectId,
        primary: Option<ObjectId>,
        parts: bool,
        random: u8,
        events: Vec<Event>,
    }

    impl World {
        fn new() -> Self {
            let mut objects = ObjectStore::new();
            let owner = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::EnemyFlight,
                ))
                .unwrap();
            let other = objects
                .allocate(Object::new(
                    ObjectKind::Enemy,
                    ShapeId::EMPTY,
                    Behavior::EnemyFlight,
                ))
                .unwrap();
            objects.get_mut(other).unwrap().base.position = Vector3 {
                x: 100,
                y: 0,
                z: 200,
            };
            objects.get_mut(owner).unwrap().base.hit_flags = 0xFF;
            let mut contacts = ContactStore::default();
            contacts.record_pair(owner, other, [None, None]).unwrap();
            Self {
                objects,
                contacts,
                owner,
                other,
                primary: Some(owner),
                parts: true,
                actors: vec![
                    HitActor {
                        health: 40,
                        attack_power: 5,
                        hit_marked: true,
                        ..HitActor::default()
                    };
                    2
                ],
                player: PlayerContactState {
                    active: true,
                    ..PlayerContactState::default()
                },
                rules: ContactRules {
                    enabled: true,
                    strategy_clock: 1,
                    ..ContactRules::default()
                },
                random: 248,
                events: Vec::new(),
            }
        }

        fn context(&self, damage: u8) -> HitContext {
            HitContext {
                current_contact: self.contacts.first(self.owner),
                damage,
                other_parameter: 71,
            }
        }

        fn invoke(&mut self, damage: u8) -> HitContext {
            let mut context = self.context(damage);
            respond(self, self.owner, &mut context).unwrap();
            context
        }
    }

    impl ContactHost for World {
        type Error = String;
        fn contacts(&self) -> &ContactStore {
            &self.contacts
        }
        fn contacts_mut(&mut self) -> &mut ContactStore {
            &mut self.contacts
        }
        fn on_separation(&mut self, id: ContactId, contact: Contact) -> Result<(), Self::Error> {
            self.events.push(Event::Separate(contact.owner));
            if contact.owner == self.owner {
                let mut context = HitContext {
                    current_contact: Some(id),
                    damage: 0,
                    ..HitContext::default()
                };
                respond(self, contact.owner, &mut context).map_err(|e| format!("{e:?}"))?;
            }
            Ok(())
        }
    }

    impl HitResponseHost for World {
        fn hit_actor(&self, id: ObjectId) -> Option<HitActor> {
            self.actors.get(id.index()).copied()
        }
        fn hit_actor_mut(
            &mut self,
            id: ObjectId,
        ) -> Option<super::super::hit_response::HitActorMut<'_>> {
            self.actors.get_mut(id.index()).map(HitActor::as_mut)
        }
        fn has_hit_callback(&self, owner: ObjectId, _: HitCallback) -> Result<bool, Self::Error> {
            Ok(owner == self.owner)
        }
        fn run_hit_callback(
            &mut self,
            owner: ObjectId,
            _: ObjectId,
            _: HitCallback,
            context: &mut HitContext,
        ) -> Result<(), Self::Error> {
            respond(self, owner, context).map_err(|e| format!("{e:?}"))
        }
        fn strategies_paused(&self) -> bool {
            false
        }
        fn run_assigned_strategy(&mut self, owner: ObjectId) -> Result<ObjectId, Self::Error> {
            self.events.push(Event::Strategy(owner));
            Ok(owner)
        }
    }

    impl PlayerContactHost for World {
        fn objects(&self) -> &ObjectStore {
            &self.objects
        }
        fn objects_mut(&mut self) -> &mut ObjectStore {
            &mut self.objects
        }
        fn player_hit(&self, owner: ObjectId) -> Result<&PlayerHitControl, Self::Error> {
            (owner == self.owner)
                .then_some(&self.player.hit)
                .ok_or("missing player".into())
        }
        fn player_hit_mut(
            &mut self,
            owner: ObjectId,
        ) -> Result<&mut PlayerHitControl, Self::Error> {
            (owner == self.owner)
                .then_some(&mut self.player.hit)
                .ok_or("missing player".into())
        }
        fn contact_active(&self, _: ObjectId) -> Result<bool, Self::Error> {
            Ok(self.player.active)
        }
        fn ignores_contacts(&self, _: ObjectId) -> Result<bool, Self::Error> {
            Ok(self.player.ignores_contacts)
        }
        fn protection(&self, _: ObjectId) -> Result<DeflectionProtection, Self::Error> {
            Ok(DeflectionProtection::from_control(
                self.player.deflection_count & 0x1F
                    | if self.player.projectile_deflection {
                        0x40
                    } else {
                        0
                    },
            ))
        }
        fn suppress_contact_turn(&self, _: ObjectId) -> Result<bool, Self::Error> {
            Ok(self.player.suppress_contact_turn)
        }
        fn part_feedback(&mut self, _: ObjectId) -> Result<&mut u8, Self::Error> {
            Ok(&mut self.player.part_feedback)
        }
        fn contact_turn(&mut self, _: ObjectId) -> Result<&mut Option<ContactTurn>, Self::Error> {
            Ok(&mut self.player.turn)
        }
        fn contacts_enabled(&self) -> Result<bool, Self::Error> {
            Ok(self.rules.enabled)
        }
        fn contacts_blocked(&self) -> Result<bool, Self::Error> {
            Ok(self.rules.blocked)
        }
        fn suppress_turn(&self) -> Result<bool, Self::Error> {
            Ok(self.rules.suppress_turn)
        }
        fn strategy_clock(&self) -> u8 {
            self.rules.strategy_clock
        }
        fn primary_player(&self) -> Option<ObjectId> {
            self.primary
        }
        fn damages_player_parts(&self, other: ObjectId) -> Result<bool, Self::Error> {
            Ok(other == self.other && self.parts)
        }
        fn reflect_contacts(&mut self, owner: ObjectId) -> Result<(), Self::Error> {
            self.events.push(Event::Reflect(owner));
            Ok(())
        }
        fn queue_contact_sound(
            &mut self,
            sound: ContactSound,
            side: HitSide,
        ) -> Result<(), Self::Error> {
            self.events.push(Event::Sound(sound, side));
            Ok(())
        }
        fn random_byte(&mut self) -> u8 {
            self.events.push(Event::Random);
            self.random
        }
    }

    #[test]
    fn first_three_gates_do_not_require_a_contact_and_all_gates_clear_damage_and_mark() {
        for gate in 0..5 {
            let mut world = World::new();
            match gate {
                0 => world.player.hit.recovery = 0x80,
                1 => world.player.active = false,
                2 => world.player.ignores_contacts = true,
                3 => world.rules.enabled = false,
                4 => world.rules.blocked = true,
                _ => unreachable!(),
            }
            world.player.hit.reserve_shield = 0x80;
            let mut context = world.context(5);
            if gate < 3 {
                context.current_contact = None;
            }
            let owner = world.owner;
            respond(&mut world, owner, &mut context).unwrap();
            assert_eq!(context.damage, 0);
            assert_eq!(context.other_parameter, 71);
            assert!(!world.actors[owner.index()].hit_marked);
            // The reserve tail still executes with zero damage.
            assert_eq!(world.player.hit.reserve_shield, 0);
            assert!(world.events.is_empty());
            assert_eq!(world.objects.get(owner).unwrap().base.hit_flags, 0xFF);
        }
    }

    #[test]
    fn active_callback_resolves_contact_before_global_gates() {
        let mut world = World::new();
        world.rules.enabled = false;
        let owner = world.owner;
        assert_eq!(
            respond(&mut world, owner, &mut HitContext::default()),
            Err(PlayerContactError::MissingContact)
        );
    }

    #[test]
    fn deflection_mask_feedback_and_sound_random_order() {
        for count in 0..=u8::MAX {
            for projectile_only in [false, true] {
                for projectile in [false, true] {
                    let mut world = World::new();
                    world.player.deflection_count = count;
                    world.player.projectile_deflection = projectile_only;
                    world.actors[world.other.index()].credits_hit_side = projectile;
                    world.primary = None;
                    let context = world.invoke(5);
                    let timed = count & 0x1F != 0;
                    if timed || (projectile_only && projectile) {
                        assert_eq!(context.damage, 0);
                        assert_eq!(
                            world.events,
                            [
                                Event::Sound(ContactSound::Deflection, HitSide::Secondary),
                                Event::Random,
                                Event::Reflect(world.owner)
                            ]
                        );
                        assert_eq!(world.player.hit.feedback_duration, u8::from(timed));
                        assert_eq!(world.player.hit.feedback_flags, if timed { 8 } else { 0 });
                        assert_eq!(world.player.hit.deflection_sound_cooldown, 0);
                        assert_eq!(world.player.hit.recovery, 0);
                        assert_eq!(world.objects.get(world.owner).unwrap().base.hit_flags, 0xFF);
                    } else {
                        assert_eq!(context.damage, 5);
                        assert_eq!(world.player.hit.recovery, 10);
                    }
                }
            }
        }
    }

    #[test]
    fn deflection_cooldown_skips_sound_and_random_but_not_reflection() {
        let mut world = World::new();
        world.player.deflection_count = 1;
        world.player.hit.deflection_sound_cooldown = 4;
        world.invoke(20);
        assert_eq!(world.events, [Event::Reflect(world.owner)]);
        assert_eq!(world.player.hit.deflection_sound_cooldown, 4);
    }

    #[test]
    fn impact_consumes_part_bits_selectively_and_preserves_suppressed_turn() {
        for flags in 0..=u8::MAX {
            for parts in [false, true] {
                let mut world = World::new();
                world.parts = parts;
                world.objects.get_mut(world.owner).unwrap().base.hit_flags = flags;
                world.player.part_feedback = 3;
                world.invoke(5);
                let expected = 3 | if parts {
                    ((flags & 2) << 6) | ((flags & 4) << 4) | ((flags & 1) << 5)
                } else {
                    0
                };
                assert_eq!(world.player.part_feedback, expected);
                assert_eq!(
                    world.objects.get(world.owner).unwrap().base.hit_flags,
                    flags & 0xF8
                );
                assert_eq!(world.player.hit.bank_impulse, -30);
                assert!(world.player.turn.is_some());
                assert!(world.actors[world.owner.index()].hit_marked);
            }
        }
        for gate in 0..3 {
            let mut world = World::new();
            world.invoke(5);
            let prior = world.player.turn;
            world.player.hit.recovery = 0;
            world.objects.get_mut(world.other).unwrap().base.position.x = -100;
            match gate {
                0 => world.rules.suppress_turn = true,
                1 => world.player.suppress_contact_turn = true,
                2 => world.actors[world.other.index()].credits_hit_side = true,
                _ => unreachable!(),
            }
            world.invoke(5);
            assert_eq!(world.player.turn, prior);
        }
    }

    #[test]
    fn impact_sound_precedes_reserve_and_hull_damage_belongs_to_response_owner() {
        for reserve in [0, 1, 8] {
            let mut world = World::new();
            world.player.hit.reserve_shield = reserve;
            let owner = world.owner;
            let mut context = world.context(0);
            hit_response::respond(&mut world, owner, &mut context).unwrap();
            assert_eq!(
                world.actors[owner.index()].health,
                if reserve == 0 { 35 } else { 40 }
            );
            assert_eq!(
                world.player.hit.reserve_shield,
                if reserve == 8 { 3 } else { 0 }
            );
            assert_eq!(
                world.events,
                [
                    Event::Sound(ContactSound::HeavyImpact, HitSide::Primary),
                    Event::Strategy(owner)
                ]
            );
            world.events.clear();
            hit_response::respond(&mut world, owner, &mut context).unwrap();
            assert_eq!(context.damage, 0); // Continuing contact is rejected during recovery.
            assert_eq!(world.events, [Event::Strategy(owner)]);
            let contact = context.current_contact.unwrap();
            crate::collision_contacts::separate_pair(&mut world, contact).unwrap();
            assert!(world.contacts.is_empty());
            assert_eq!(
                &world.events[1..],
                [Event::Separate(owner), Event::Separate(world.other)]
            );
        }
    }
}
