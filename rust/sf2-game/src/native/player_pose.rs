//! Complete flight-pose composition ($06:ECB0..EE09). Steering owns its
//! targets and lean terms; roll owns its impulse; charge owns linked mode.
//! Fine orientation and retained bank stay in the canonical player storage.
//! This service publishes actor angles without moving or aiming the actor.

use super::path_fields::chase_word;
use super::player_storage::{self, PlayerStorageError};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Angle, ObjectId, ObjectStore};

const SUBMODE_MASK: u8 = 0x0F;
const FAST_PITCH_SUBMODE: u8 = 1;
const MODE_FAMILY_MASK: u8 = 0xF0;
const FLIGHT_FAMILY: u8 = 0x10;
const ACTIVE: u8 = 0x01;
const HEADING_LOCKED: u8 = 0x04;
const QUARTER_RESPONSE: i16 = 4;
const FINE_ANGLE_SHIFT: u32 = u8::BITS;

/// Retained terms shared with steering and ambient motion. These are not
/// duplicate actor angles, sampled input or already-composed output values.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerPose {
    /// 6ACF: additive pitch from terrain/input lean.
    pub pitch_lean: i8,
    /// 6AD0/1: fine turning lean; pose uses its high byte in visible yaw.
    pub turning_lean: u16,
    /// 6AD4: decaying additive yaw, quarter-chased toward zero here.
    pub yaw_trim: i8,
    /// 6AD5: current sample from the ambient bank waveform.
    pub ambient_bank: i8,
    /// 6AD7: directional-input bank, also adjusted by terrain clearance.
    pub steering_bank: i8,
    /// 6AD8/9: signed shoulder bank; pose uses its low byte, not the high.
    pub shoulder_bank: i16,
    /// 6ADA: bank while returning to a locked heading.
    pub heading_return_bank: i8,
    /// 6ADE: additional yaw, omitted while heading is locked.
    pub yaw_offset: i8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoseError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    MissingPose(ObjectId),
    MissingPitchTarget,
    MissingYawIncrement,
    MissingRoll(ObjectId),
}

impl From<WorldInputError> for PoseError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

impl From<PlayerStorageError> for PoseError {
    fn from(error: PlayerStorageError) -> Self {
        Self::Storage(error)
    }
}

fn quarter_step(delta: i16) -> i16 {
    if delta == 0 {
        0
    } else if delta < 0 {
        delta.min(-QUARTER_RESPONSE) / QUARTER_RESPONSE
    } else {
        delta.max(QUARTER_RESPONSE) / QUARTER_RESPONSE
    }
}

/// Helpers $7F:2782/$7F:2567 wrap the subtraction before interpreting its
/// sign, then truncate each signed half. The minimum nonzero step is one.
pub(crate) fn quarter_byte(current: u8, target: u8) -> u8 {
    current.wrapping_add(quarter_step(i16::from(target.wrapping_sub(current) as i8)) as u8)
}

pub(crate) fn quarter_word(current: u16, target: u16) -> u16 {
    current.wrapping_add(quarter_step(target.wrapping_sub(current) as i16) as u16)
}

/// Inactive-roll bank recovery ($06:EDCE..EDE6). Keep the two signed
/// rounding steps separate; arithmetic right shifts stall negative banks.
pub(crate) fn recover_bank(value: i8) -> i8 {
    let half = value / 2;
    half + half / 2
}

/// Compose pitch, yaw and roll in source order. A missing later service
/// preserves the already-completed angle/trim writes; the scene wrapper
/// latches that failure, so retry cannot integrate yaw twice.
pub fn compose(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<(), PoseError> {
    let pose = world
        .player_mut(objects, owner)?
        .pose
        .as_mut()
        .ok_or(PoseError::MissingPose(owner))?;
    pose.yaw_trim = quarter_byte(pose.yaw_trim as u8, 0) as i8;
    let pose = *pose;
    let auxiliary = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    let target = world
        .player_pitch_target
        .ok_or(PoseError::MissingPitchTarget)?;
    let storage = player_storage::get_mut(objects, resources, owner)?;
    storage.fine_pitch = if auxiliary.mode & SUBMODE_MASK == FAST_PITCH_SUBMODE {
        quarter_word(storage.fine_pitch, target)
    } else {
        // $7F:25A3 and the path helper share wrapped eighth-chase semantics.
        chase_word(storage.fine_pitch, target)
    };
    let fine_pitch = storage.fine_pitch;
    let linked_pitch = auxiliary.action_flags & ACTIVE != 0
        && world
            .player(objects, owner)?
            .charge
            .ok_or(WorldInputError::MissingPlayerCharge(owner))?
            .linked_mode;
    let pitch = if linked_pitch {
        let reversed = (auxiliary.stored_rotation.pitch.units() as i8).wrapping_neg();
        // These two source halves are arithmetic shifts (round down), unlike
        // the chase and bank-recovery halves, which truncate toward zero.
        reversed.wrapping_add(reversed >> 2) as u8
    } else {
        ((fine_pitch >> FINE_ANGLE_SHIFT) as u8).wrapping_add(pose.pitch_lean as u8)
    };
    objects.get_mut(owner).expect("validated player").base.pitch = Angle::from_units(pitch);

    let locked_heading = auxiliary.action_flags & HEADING_LOCKED != 0;
    if !locked_heading {
        let increment = world
            .player_yaw_increment
            .ok_or(PoseError::MissingYawIncrement)?;
        world.player_mut(objects, owner)?.yaw_motion = Some(increment);
        let storage = player_storage::get_mut(objects, resources, owner)?;
        storage.fine_yaw = storage.fine_yaw.wrapping_add(increment);
    }
    let fine_yaw = player_storage::get(objects, resources, owner)?.fine_yaw;
    let yaw = ((fine_yaw >> FINE_ANGLE_SHIFT) as u8)
        .wrapping_add((pose.turning_lean >> FINE_ANGLE_SHIFT) as u8)
        .wrapping_add(pose.yaw_trim as u8)
        .wrapping_add(if locked_heading {
            0
        } else {
            pose.yaw_offset as u8
        });
    objects.get_mut(owner).expect("validated player").base.yaw = Angle::from_units(yaw);

    let ignores_contacts = world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .ignores_contacts;
    if ignores_contacts || auxiliary.mode & MODE_FAMILY_MASK != FLIGHT_FAMILY {
        let actor = objects.get_mut(owner).expect("validated player");
        actor.base.roll = Angle::from_units(((actor.base.roll.units() as i8) / 2) as u8);
    } else {
        let impulse = world
            .player(objects, owner)?
            .roll
            .ok_or(PoseError::MissingRoll(owner))?
            .impulse;
        let storage = player_storage::get_mut(objects, resources, owner)?;
        let bank = storage.bank.units() as i8;
        let recovered = if impulse == 0 {
            recover_bank(bank)
        } else {
            bank
        };
        storage.bank = Angle::from_units(recovered.wrapping_add(impulse) as u8);
        let roll = storage
            .bank
            .units()
            .wrapping_add(pose.ambient_bank as u8)
            .wrapping_add(pose.shoulder_bank as u8)
            .wrapping_add(pose.steering_bank as u8)
            .wrapping_add(pose.heading_return_bank as u8);
        objects.get_mut(owner).expect("validated player").base.roll = Angle::from_units(roll);
    }
    Ok(())
}

#[cfg(test)]
#[path = "player_pose_tests.rs"]
mod tests;
