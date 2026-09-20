//! Contact-list reflection (`$07:F1AE..F2ED`), shared by players and armor.
//! Incoming shots are collision-disabled, not retired. A reflected heavy shot
//! is formatted at the reflector then moved to the incoming shot's position.

use super::collision_contacts::ContactStore;
use super::weapon_dispatch::{
    self, LaunchError, LaunchRequest, LaunchWorld, PathWeapon, WeaponState,
};
use super::{ObjectId, ObjectSpawnDefaults, ObjectStore, RandomState, Rotation, OBJECT_CAPACITY};

const REFLECTED_SPRITE_SOURCE_SPEED: u8 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReflectionRules {
    /// Scene mode 1AA6 bit 02: continue after a successful reflection.
    pub process_all: bool,
    /// Caller-owned player protection bit 40; never the selected pilot's flag.
    /// Required only when the reflector is one of the two live players.
    pub player_scatter: Option<bool>,
    pub owner: ObjectId,
}

pub struct ReflectionWorld<'a> {
    pub contacts: Option<&'a ContactStore>,
    pub rules: Option<ReflectionRules>,
    pub weapons: Option<&'a mut WeaponState>,
    pub defaults: Option<ObjectSpawnDefaults>,
    pub primary: Option<ObjectId>,
    pub secondary: Option<ObjectId>,
    pub random: &'a mut RandomState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectionError {
    MissingActor(ObjectId),
    MissingContacts,
    MissingRules,
    WrongRulesOwner {
        expected: ObjectId,
        supplied: ObjectId,
    },
    MissingPlayerScatter,
    MissingWeaponState,
    MissingSpawnDefaults,
    MissingFallback,
    Launch(LaunchError),
}

pub fn reflect_contacts(
    objects: &mut ObjectStore,
    owner: ObjectId,
    world: &mut ReflectionWorld<'_>,
) -> Result<(), ReflectionError> {
    let reflector = objects
        .get(owner)
        .ok_or(ReflectionError::MissingActor(owner))?;
    if !reflector.base.contacts.skip_contacts {
        return Ok(());
    }
    let contacts = world.contacts.ok_or(ReflectionError::MissingContacts)?;
    let Some(mut cursor) = contacts.first(owner) else {
        return Ok(());
    };
    let rules = world.rules.ok_or(ReflectionError::MissingRules)?;
    if rules.owner != owner {
        return Err(ReflectionError::WrongRulesOwner {
            expected: owner,
            supplied: rules.owner,
        });
    }
    loop {
        // ContactStore owns its links and cannot expose a missing live entry.
        let contact = *contacts.get(cursor).expect("live reflection contact");
        let incoming = objects
            .get(contact.other)
            .ok_or(ReflectionError::MissingActor(contact.other))?;
        if incoming.base.contacts.credits_hit_side && !incoming.base.flags.collision_disabled {
            let player = world.primary == Some(owner) || world.secondary == Some(owner);
            let scatter = if player {
                rules
                    .player_scatter
                    .ok_or(ReflectionError::MissingPlayerScatter)?
            } else {
                true
            };
            let defaults = world
                .defaults
                .ok_or(ReflectionError::MissingSpawnDefaults)?;
            let weapons = world
                .weapons
                .as_deref_mut()
                .ok_or(ReflectionError::MissingWeaponState)?;
            // The launcher's ordinary full-pool branch reads neither live
            // player pointer, but reflection still edits its reserved fallback.
            if objects.len() == OBJECT_CAPACITY {
                let fallback = weapons.fallback.ok_or(ReflectionError::MissingFallback)?;
                objects
                    .get(fallback)
                    .ok_or(ReflectionError::MissingActor(fallback))?;
            } else {
                let primary = world
                    .primary
                    .ok_or(ReflectionError::Launch(LaunchError::MissingPrimary))?;
                if owner != primary {
                    let secondary = world
                        .secondary
                        .ok_or(ReflectionError::Launch(LaunchError::MissingSecondary))?;
                    if owner != secondary {
                        objects
                            .get(primary)
                            .ok_or(ReflectionError::MissingActor(primary))?;
                    }
                }
            }
            let rotation = Rotation {
                pitch: incoming.base.pitch,
                yaw: incoming.base.yaw,
                roll: incoming.base.roll,
            };
            let yaw = objects.get(owner).expect("validated reflector").base.yaw;
            objects
                .get_mut(contact.other)
                .expect("validated incoming shot")
                .base
                .flags
                .collision_disabled = true;
            let scatter = scatter.then(|| [world.random.next_byte(), world.random.next_byte()]);
            weapons.parameters =
                super::weapon_launch::reflection_parameters(rotation, yaw, scatter);
            let reflected = weapon_dispatch::launch(
                objects,
                owner,
                LaunchRequest {
                    weapon: PathWeapon::PlayerOrHostileHeavy,
                    parameters: weapons.parameters,
                    defaults,
                },
                &mut LaunchWorld {
                    caller_inputs: None,
                    fallback: weapons.fallback,
                    published_pitch: weapons.published_pitch,
                    primary: world.primary,
                    secondary: world.secondary,
                    primary_auxiliary_mode: None,
                    hostile_counts: Some(&mut weapons.hostile_counts),
                    random: world.random,
                },
            )
            .map_err(ReflectionError::Launch)?
            .or(weapons.fallback)
            .ok_or(ReflectionError::MissingFallback)?;
            // Sample after creation: allocation and reciprocal-link updates
            // precede the source's incoming shape/material/position reads.
            let incoming = objects
                .get(contact.other)
                .ok_or(ReflectionError::MissingActor(contact.other))?
                .clone();
            let shot = objects
                .get_mut(reflected)
                .ok_or(ReflectionError::MissingActor(reflected))?;
            shot.base.shape = incoming
                .extension
                .reflection_shape
                .unwrap_or(incoming.base.shape);
            shot.extension.material_set = incoming.extension.material_set;
            shot.base.position = incoming.base.position;
            if incoming.base.flags.scaled_sprite {
                super::path_appearance::set_sprite(
                    shot,
                    incoming.extension.depth_offset as u8,
                    incoming.extension.texture_scroll_x,
                );
                // This changes the incoming byte directly; it does not
                // regenerate velocity or set the reflected weapon's speed.
                objects
                    .get_mut(contact.other)
                    .expect("validated incoming sprite")
                    .base
                    .speed = REFLECTED_SPRITE_SOURCE_SPEED;
            }
            if !rules.process_all {
                break;
            }
        }
        let Some(next) = contact.next() else {
            break;
        };
        cursor = next;
    }
    Ok(())
}

#[cfg(test)]
#[path = "weapon_reflection_tests.rs"]
mod tests;
