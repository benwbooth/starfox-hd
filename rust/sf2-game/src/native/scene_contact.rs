//! Native contact registrations and scene bindings. `$06:8597..85D6`
//! installs the player handler in the actor's ordinary counted auxiliary
//! table, in new/continuing/separation order. No parallel callback registry.

use super::actor_auxiliary::{AuxiliaryError, AuxiliaryKind, AuxiliaryRecord};
use super::collision_contacts::{Contact, ContactId};
use super::hit_response::{HitCallback, HitContext, HitSide};
use super::path_control::PlayerTarget;
use super::path_protection::DeflectionProtection;
use super::path_sound::AuthoredCue;
use super::player_contact::{self, PlayerContactHost};
use super::player_hit_control::{ContactSound, ContactTurn, PlayerHitControl};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::WorldInputError;
use super::scene_strategy::{SceneActors, SceneCallbacks, SceneError};
use super::weapon_reflection::{self, ReflectionRules, ReflectionWorld};
use super::{ObjectId, ObjectStore, SoundEvent};

const PLAYER_ACTIVE: u8 = 0x01;
const NO_CONTACT_TURN_MODE: u8 = 0x20;
const SPECIAL_PLAYER_CONFIGURATION: u8 = 9;
const HEAVY_IMPACT_SOUND: u8 = 18;
const LIGHT_IMPACT_SOUND: u8 = 19;
const DEFLECTION_SOUND: u8 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContactCallback {
    Player,
}

/// Contact-owned player fields. Activity, mode, protection and part feedback
/// deliberately remain with their existing auxiliary/path owners.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerContactControl {
    pub hit: PlayerHitControl,
    /// Auxiliary 6A72 bit 10; independent of controlled flags at 6B65.
    pub ignores_contacts: bool,
    pub turn: Option<ContactTurn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationError {
    MissingActor(ObjectId),
    Auxiliary(AuxiliaryError),
}

pub fn install_player(
    objects: &mut ObjectStore,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<(), RegistrationError> {
    let actor = objects
        .get_mut(owner)
        .ok_or(RegistrationError::MissingActor(owner))?;
    for record in [
        AuxiliaryRecord::NewContact(ContactCallback::Player),
        AuxiliaryRecord::ContinuingContact(ContactCallback::Player),
        AuxiliaryRecord::Separation(ContactCallback::Player),
    ] {
        actor
            .extension
            .auxiliary
            .set(resources, owner, record)
            .map_err(RegistrationError::Auxiliary)?;
    }
    Ok(())
}

fn kind(callback: HitCallback) -> AuxiliaryKind {
    match callback {
        HitCallback::NewContact => AuxiliaryKind::NewContact,
        HitCallback::ContinuingContact => AuxiliaryKind::ContinuingContact,
    }
}

fn lookup<C: SceneCallbacks>(
    host: &SceneActors<'_, C>,
    owner: ObjectId,
    kind: AuxiliaryKind,
) -> Result<Option<ContactCallback>, SceneError<C::Error>> {
    let actor = host
        .objects
        .get(owner)
        .ok_or(SceneError::MissingActor(owner))?;
    Ok(
        match actor
            .extension
            .auxiliary
            .find(&host.execution.paths.runtime.resources, owner, kind)
            .map_err(SceneError::Auxiliary)?
        {
            Some(
                AuxiliaryRecord::NewContact(callback)
                | AuxiliaryRecord::ContinuingContact(callback)
                | AuxiliaryRecord::Separation(callback),
            ) => Some(callback),
            None => None,
            _ => unreachable!("contact-kind lookup returned a different typed record"),
        },
    )
}

pub fn has_callback<C: SceneCallbacks>(
    host: &SceneActors<'_, C>,
    owner: ObjectId,
    callback: HitCallback,
) -> Result<bool, SceneError<C::Error>> {
    Ok(lookup(host, owner, kind(callback))?.is_some())
}

fn invoke<C: SceneCallbacks>(
    host: &mut SceneActors<'_, C>,
    owner: ObjectId,
    callback: ContactCallback,
    context: &mut HitContext,
) -> Result<(), SceneError<C::Error>> {
    match callback {
        ContactCallback::Player => player_contact::respond(host, owner, context)
            .map_err(|error| SceneError::PlayerContact(Box::new(error))),
    }
}

pub fn hit<C: SceneCallbacks>(
    host: &mut SceneActors<'_, C>,
    owner: ObjectId,
    callback: HitCallback,
    context: &mut HitContext,
) -> Result<(), SceneError<C::Error>> {
    let callback =
        lookup(host, owner, kind(callback))?.ok_or(SceneError::MissingContactCallback(owner))?;
    invoke(host, owner, callback, context)
}

pub fn separate<C: SceneCallbacks>(
    host: &mut SceneActors<'_, C>,
    contact: ContactId,
    entry: Contact,
) -> Result<(), SceneError<C::Error>> {
    // `$7F:3F5B` publishes the cursor even without a registered callback.
    // It does NOT initialize damage or the continuing-contact parameter.
    host.execution.hit_context.current_contact = Some(contact);
    let Some(callback) = lookup(host, entry.owner, AuxiliaryKind::Separation)? else {
        return Ok(());
    };
    let mut context = std::mem::take(&mut host.execution.hit_context);
    let result = invoke(host, entry.owner, callback, &mut context);
    host.execution.hit_context = context;
    result
}

impl<C: SceneCallbacks> PlayerContactHost for SceneActors<'_, C> {
    fn objects(&self) -> &ObjectStore {
        self.objects
    }
    fn objects_mut(&mut self) -> &mut ObjectStore {
        self.objects
    }
    fn player_hit(&self, owner: ObjectId) -> Result<&PlayerHitControl, Self::Error> {
        Ok(&self
            .world
            .player(self.objects, owner)
            .map_err(SceneError::World)?
            .contact
            .as_ref()
            .ok_or(SceneError::World(WorldInputError::MissingPlayerContact(
                owner,
            )))?
            .hit)
    }
    fn player_hit_mut(&mut self, owner: ObjectId) -> Result<&mut PlayerHitControl, Self::Error> {
        Ok(&mut self
            .world
            .player_mut(self.objects, owner)
            .map_err(SceneError::World)?
            .contact
            .as_mut()
            .ok_or(SceneError::World(WorldInputError::MissingPlayerContact(
                owner,
            )))?
            .hit)
    }
    fn contact_active(&self, owner: ObjectId) -> Result<bool, Self::Error> {
        let auxiliary = self
            .world
            .player(self.objects, owner)
            .map_err(SceneError::World)?
            .auxiliary
            .ok_or(SceneError::World(WorldInputError::MissingAuxiliary(owner)))?;
        Ok(auxiliary.action_flags & PLAYER_ACTIVE != 0)
    }
    fn ignores_contacts(&self, owner: ObjectId) -> Result<bool, Self::Error> {
        Ok(self
            .world
            .player(self.objects, owner)
            .map_err(SceneError::World)?
            .contact
            .as_ref()
            .ok_or(SceneError::World(WorldInputError::MissingPlayerContact(
                owner,
            )))?
            .ignores_contacts)
    }
    fn protection(&self, owner: ObjectId) -> Result<DeflectionProtection, Self::Error> {
        self.world
            .player(self.objects, owner)
            .map_err(SceneError::World)?
            .protection
            .ok_or(SceneError::World(WorldInputError::MissingPlayerProtection(
                owner,
            )))
    }
    fn suppress_contact_turn(&self, owner: ObjectId) -> Result<bool, Self::Error> {
        let auxiliary = self
            .world
            .player(self.objects, owner)
            .map_err(SceneError::World)?
            .auxiliary
            .ok_or(SceneError::World(WorldInputError::MissingAuxiliary(owner)))?;
        Ok(auxiliary.mode & 0xF0 == NO_CONTACT_TURN_MODE)
    }
    fn part_feedback(&mut self, owner: ObjectId) -> Result<&mut u8, Self::Error> {
        Ok(&mut self
            .world
            .player_mut(self.objects, owner)
            .map_err(SceneError::World)?
            .particles
            .as_mut()
            .ok_or(SceneError::World(WorldInputError::MissingPlayerParticles(
                owner,
            )))?
            .flags)
    }
    fn contact_turn(&mut self, owner: ObjectId) -> Result<&mut Option<ContactTurn>, Self::Error> {
        Ok(&mut self
            .world
            .player_mut(self.objects, owner)
            .map_err(SceneError::World)?
            .contact
            .as_mut()
            .ok_or(SceneError::World(WorldInputError::MissingPlayerContact(
                owner,
            )))?
            .turn)
    }
    fn contacts_enabled(&self) -> Result<bool, Self::Error> {
        self.world
            .contacts_enabled
            .ok_or(SceneError::World(WorldInputError::MissingContactEnable))
    }
    fn contacts_blocked(&self) -> Result<bool, Self::Error> {
        Ok(self
            .world
            .action_gate
            .ok_or(SceneError::World(WorldInputError::MissingActionGate))?
            .code
            != 0)
    }
    fn suppress_turn(&self) -> Result<bool, Self::Error> {
        Ok(self
            .world
            .scene
            .player_configuration
            .ok_or(SceneError::World(
                WorldInputError::MissingPlayerConfiguration,
            ))?
            == SPECIAL_PLAYER_CONFIGURATION)
    }
    fn strategy_clock(&self) -> u8 {
        self.world.strategy_clock as u8
    }
    fn primary_player(&self) -> Option<ObjectId> {
        self.world.primary_player
    }
    fn damages_player_parts(&self, other: ObjectId) -> Result<bool, Self::Error> {
        Ok(self
            .objects
            .get(other)
            .ok_or(SceneError::MissingActor(other))?
            .base
            .contacts
            .damages_player_parts)
    }
    fn reflect_contacts(&mut self, owner: ObjectId) -> Result<(), Self::Error> {
        let actor = self
            .objects
            .get(owner)
            .ok_or(SceneError::MissingActor(owner))?;
        if !actor.base.contacts.skip_contacts || self.world.contacts.first(owner).is_none() {
            return Ok(());
        }
        // The reflector service diagnoses these observations only if a live
        // eligible incoming shot actually needs them.
        let scatter = self
            .world
            .player(self.objects, owner)
            .ok()
            .and_then(|record| record.protection)
            .map(DeflectionProtection::projectile_deflection);
        let rules = self
            .world
            .reflect_all_contacts
            .map(|process_all| ReflectionRules {
                process_all,
                player_scatter: scatter,
                owner,
            });
        let defaults = self.world.spawn_defaults();
        weapon_reflection::reflect_contacts(
            self.objects,
            &mut self.execution.paths.runtime.resources,
            owner,
            &mut ReflectionWorld {
                contacts: Some(&self.world.contacts),
                rules,
                weapons: self.world.weapons.as_mut(),
                defaults,
                primary: self.world.primary_player,
                secondary: self.world.secondary_player,
                random: &mut self.world.random,
            },
        )
        .map_err(SceneError::Reflection)
    }
    fn queue_contact_sound(
        &mut self,
        sound: ContactSound,
        side: HitSide,
    ) -> Result<(), Self::Error> {
        let cue = match sound {
            ContactSound::LightImpact => LIGHT_IMPACT_SOUND,
            ContactSound::HeavyImpact => HEAVY_IMPACT_SOUND,
            ContactSound::Deflection => DEFLECTION_SOUND,
        };
        let target = match side {
            HitSide::Primary => PlayerTarget::Primary,
            HitSide::Secondary => PlayerTarget::Secondary,
        };
        self.world
            .audio
            .queue(SoundEvent::Authored(AuthoredCue::new(cue, 0, target)));
        Ok(())
    }
    fn random_byte(&mut self) -> u8 {
        self.world.random.next_byte()
    }
}

#[cfg(test)]
#[path = "scene_contact_tests.rs"]
mod tests;
