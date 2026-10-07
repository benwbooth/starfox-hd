//! Contact-turn publication and the flight recoil tail ($06:9842..98A0,
//! $06:AA6F..AB04, $06:E273..E2D0). Shared response fields live with their
//! real pose, charge and translation consumers, not a second hit snapshot.

use super::path_motion;
use super::player_hit_control::{self, ContactTurn, Impact};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore};

#[cfg(test)]
#[path = "player_impact_tests.rs"]
mod tests;

const CONSTRAINED_CONFIGURATION: u8 = 9;
const MODE_FAMILY_MASK: u8 = 0xF0;
const NO_CONTACT_TURN_MODE: u8 = 0x20;
const QUARTER_TURN: u8 = 64;
const HORIZONTAL_SAMPLE_ADVANCE: u8 = 1;
const LATERAL_DECAY: i8 = 20;
const CHARGE_DECAY: u8 = 0x40;
const CHARGING: u8 = 0x80;
const HIT_BANK: i8 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImpactError {
    World(WorldInputError),
    MissingPose(ObjectId),
    MissingMotion(ObjectId),
}

impl From<WorldInputError> for ImpactError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

/// Publication order is observable when a later native owner is missing.
/// The target overwrites only the low byte of the actor's shared motion phase.
pub fn publish_turn(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    turn: ContactTurn,
) -> Result<(), ImpactError> {
    let actor = objects
        .get_mut(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    actor.extension.path_state.motion_phase =
        (actor.extension.path_state.motion_phase & 0xFF00) | u16::from(turn.target_yaw.units());
    world
        .player_mut(objects, owner)?
        .pose
        .as_mut()
        .ok_or(ImpactError::MissingPose(owner))?
        .yaw_trim = turn.yaw_impulse;
    world
        .player_mut(objects, owner)?
        .motion
        .as_mut()
        .ok_or(ImpactError::MissingMotion(owner))?
        .lateral_impulse = turn.lateral_impulse;
    Ok(())
}

/// Complete turn helper, including its independent configuration, movement
/// family and other-actor class gates. Suppressed calls leave all owners alone.
pub fn turn_from_actor(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    other: ObjectId,
) -> Result<(), ImpactError> {
    if world
        .scene
        .player_configuration
        .ok_or(WorldInputError::MissingPlayerConfiguration)?
        == CONSTRAINED_CONFIGURATION
    {
        return Ok(());
    }
    if world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode
        & MODE_FAMILY_MASK
        == NO_CONTACT_TURN_MODE
    {
        return Ok(());
    }
    let other = objects
        .get(other)
        .ok_or(WorldInputError::MissingActor(other))?;
    if other.base.contacts.credits_hit_side {
        return Ok(());
    }
    let actor = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    let turn = player_hit_control::turn_from_contact(
        actor.base.position,
        actor.base.yaw,
        other.base.position,
    );
    publish_turn(objects, world, owner, turn)
}

/// Light hits can reject before charge/pose are observed. Admitted impacts
/// first update recovery, feedback and pitch recoil, then the SAME charge
/// control and heading-return bank read by their subsequent services.
pub fn impact(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    impact: Impact,
) -> Result<bool, ImpactError> {
    if !world
        .player_mut(objects, owner)?
        .contact
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .begin_impact(impact)
    {
        return Ok(false);
    }
    let charge = world
        .player_mut(objects, owner)?
        .charge
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
    charge.control = (charge.control | CHARGE_DECAY) & !CHARGING;
    let bank = if world.strategy_clock & 1 == 0 {
        HIT_BANK
    } else {
        -HIT_BANK
    };
    world
        .player_mut(objects, owner)?
        .pose
        .as_mut()
        .ok_or(ImpactError::MissingPose(owner))?
        .heading_return_bank = bank;
    Ok(true)
}

/// This runs AFTER ordinary flight translation. The source replaces only
/// horizontal velocity and integrates all three axes, including retained Y.
/// Its single horizontal product must not be replaced by pitch-zero flight
/// generation, which introduces another rounding step through cos(0).
pub fn advance_recoil(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ImpactError> {
    use sf_core::snes_trig::{mulslog_mac8, COSTAB, SINTAB};
    let impulse = world
        .player(objects, owner)?
        .motion
        .ok_or(ImpactError::MissingMotion(owner))?
        .lateral_impulse;
    if impulse == 0 {
        return Ok(());
    }
    let actor = objects.get_mut(owner).expect("validated player");
    let heading = actor.base.yaw.units().wrapping_add(QUARTER_TURN);
    actor.extension.path_state.motion_phase = u16::from_le_bytes([heading, impulse as u8]);
    // This horizontal-only helper advances the table index by one after
    // negating the heading; ordinary flight's generator does not.
    let direction = usize::from(
        heading
            .wrapping_neg()
            .wrapping_add(HORIZONTAL_SAMPLE_ADVANCE),
    );
    actor.base.velocity.x = i16::from(mulslog_mac8(impulse, SINTAB[direction]));
    actor.base.velocity.z = i16::from(mulslog_mac8(impulse, COSTAB[direction]));
    path_motion::integrate(&mut actor.base.position, actor.base.velocity);
    world
        .player_mut(objects, owner)?
        .motion
        .as_mut()
        .expect("validated motion")
        .lateral_impulse = if impulse > 0 {
        (impulse - LATERAL_DECAY).max(0)
    } else {
        (impulse + LATERAL_DECAY).min(0)
    };
    Ok(())
}
