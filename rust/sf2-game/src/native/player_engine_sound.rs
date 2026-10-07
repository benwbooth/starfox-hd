//! Player engine-sound control ($06:9236..9456). The shared sound byte is
//! independent of every actor's extension counter. This prepares the live
//! audio request; it neither queues one-shot cues nor advances audio playback.
//!
//! The enclosing caller supports the source-complete action identities in
//! `PlayerAction`. The special-scene action must gain its original sound
//! exemption when that action is ported; it cannot currently be installed.

use super::player_action::PlayerAction;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore};

const MODE_FAMILY: u8 = 0xF0;
const SURFACE_FAMILY: u8 = 0x20;
const WALKER_FAMILY: u8 = 0x30;
const PRESERVED_HIGH_BIT: u8 = 0x80;
const FLIGHT_RESET_MASK: u8 = 0x93;
const FLIGHT_MOTION_RESET_MASK: u8 = 0xEC;
const BOOST: u8 = 0x40;
const BRAKE: u8 = 0x20;
const CRUISE_SOUND: u8 = 0x04;
const BOOST_SOUND: u8 = 0x08;
const BRAKE_SOUND: u8 = 0x0C;
const VERTICAL_MOTION_SOUND: u8 = 0x10;
const PROTECTED_OR_CARRIED_SOUND: u8 = 0x40;
const PILOT_SPEED: [u8; 6] = [40, 42, 32, 30, 48, 50];
const VERTICAL_SPEED_SHIFT: u32 = 3;
const ROLL_THRESHOLD: i8 = 10;
const TURN_THRESHOLDS: [i16; 3] = [512, 768, 960];
const STRIDING: u8 = 0x80;
const WALKER_TURNING: u8 = 0x08;
const STRIDE_SOUND: u8 = 0x2C;
const WALKER_TURN_SOUND: u8 = 0x34;
const ACTIVE_CARRY_MODE: u8 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EngineSoundControl(u8);

impl EngineSoundControl {
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
    pub const fn bits(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineSoundError {
    World(WorldInputError),
    MissingAction(ObjectId),
    MissingObjectiveState,
    MissingSoundControl,
    MissingVisit(ObjectId),
    MissingMotion(ObjectId),
    MissingRoll(ObjectId),
    MissingYawMotion(ObjectId),
    MissingCarryMode,
}

impl From<WorldInputError> for EngineSoundError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

/// Complete mode-specific helper ($06:9267). Reads the same live contact,
/// boost/brake, roll, yaw and Walker controls as their gameplay producers.
/// Only the caller publishes the result, after all required inputs succeed.
pub fn compose(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
    previous: EngineSoundControl,
) -> Result<EngineSoundControl, EngineSoundError> {
    let record = world.player(objects, owner)?;
    let auxiliary = record
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    let ignores_contacts = || {
        record
            .contact
            .map(|contact| contact.ignores_contacts)
            .ok_or(WorldInputError::MissingPlayerContact(owner))
    };
    let mut bits = previous.bits();
    match auxiliary.mode & MODE_FAMILY {
        WALKER_FAMILY => {
            if ignores_contacts()? {
                return Ok(EngineSoundControl::default());
            }
            let motion = record
                .motion
                .ok_or(EngineSoundError::MissingMotion(owner))?;
            // The source briefly masks the old byte, then replaces it in
            // both branches. No previous sound bits survive this selection.
            bits = if motion.walker_stride_control & STRIDING != 0 {
                STRIDE_SOUND
            } else {
                0
            };
            if motion.walker_turn_control & WALKER_TURNING != 0 {
                bits |= WALKER_TURN_SOUND;
            }
        }
        SURFACE_FAMILY => {
            bits &= PRESERVED_HIGH_BIT;
            if world
                .player_carry_mode
                .ok_or(EngineSoundError::MissingCarryMode)?
                == ACTIVE_CARRY_MODE
                && objects
                    .get(owner)
                    .expect("validated player")
                    .extension
                    .path_state
                    .motion
                    .carry_selected_player
            {
                bits |= PROTECTED_OR_CARRIED_SOUND;
            }
        }
        _ => {
            let protection = record
                .contact
                .ok_or(WorldInputError::MissingPlayerContact(owner))?
                .hit
                .hold_secondary_protection;
            if protection || ignores_contacts()? {
                bits &= PRESERVED_HIGH_BIT;
            } else {
                bits &= FLIGHT_RESET_MASK;
                bits |= if auxiliary.action_flags & BOOST != 0 {
                    BOOST_SOUND
                } else if auxiliary.action_flags & BRAKE != 0 {
                    BRAKE_SOUND
                } else {
                    CRUISE_SOUND
                };
                bits &= FLIGHT_MOTION_RESET_MASK;
                let velocity = objects
                    .get(owner)
                    .expect("validated player")
                    .base
                    .velocity
                    .y;
                let speed = velocity.wrapping_abs();
                let pilot = usize::from(
                    record
                        .visit
                        .ok_or(EngineSoundError::MissingVisit(owner))?
                        .pilot_code,
                );
                let threshold = i16::from(
                    PILOT_SPEED.get(pilot).copied().unwrap_or(PILOT_SPEED[0])
                        >> VERTICAL_SPEED_SHIFT,
                );
                if speed.wrapping_sub(threshold) >= 0 {
                    bits |= VERTICAL_MOTION_SOUND;
                }
                let turn = (record
                    .yaw_motion
                    .ok_or(EngineSoundError::MissingYawMotion(owner))?
                    as i16)
                    .wrapping_abs();
                let roll = record
                    .roll
                    .ok_or(EngineSoundError::MissingRoll(owner))?
                    .impulse
                    .wrapping_abs();
                // All source comparisons branch on the wrapped difference's
                // sign, not a mathematical absolute value or unsigned carry.
                let band = if roll.wrapping_sub(ROLL_THRESHOLD) >= 0 {
                    TURN_THRESHOLDS.len() as u8
                } else {
                    TURN_THRESHOLDS
                        .iter()
                        .filter(|&&threshold| {
                            let delta = turn.wrapping_sub(threshold);
                            delta > 0
                        })
                        .count() as u8
                };
                bits |= band;
            }
            if protection {
                bits |= PROTECTED_OR_CARRIED_SOUND;
            }
        }
    }
    Ok(EngineSoundControl(bits))
}

/// Original objective-low-byte gate, followed by an atomic publication to
/// the shared sound owner ($1CE5). Keep the action match exhaustive so newly
/// implemented action streams cannot silently inherit this sound policy.
pub fn advance(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), EngineSoundError> {
    let action = world
        .player(objects, owner)?
        .action
        .ok_or(EngineSoundError::MissingAction(owner))?
        .action;
    match action {
        None | Some(PlayerAction::TriggeredProjectile) => {}
    }
    if !world
        .contacts_enabled()
        .ok_or(EngineSoundError::MissingObjectiveState)?
    {
        return Ok(());
    }
    let previous = world
        .engine_sound_control
        .ok_or(EngineSoundError::MissingSoundControl)?;
    let next = compose(objects, world, owner, previous)?;
    world.engine_sound_control = Some(next);
    Ok(())
}

#[cfg(test)]
#[path = "player_engine_sound_tests.rs"]
mod tests;
