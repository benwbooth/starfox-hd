//! Horizontal flight control ($06:E4B1..E7C8), including the complete turning
//! lean producer ($06:E8F5..E9E9). This publishes pose terms and a fine yaw
//! increment; it does not integrate movement or replace the enclosing mode.

use super::path_fields::{chase_byte, chase_word};
use super::player_pose::{quarter_byte, quarter_word};
use super::player_storage::{self, PlayerStorageError};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Angle, Button, ObjectId, ObjectStore};

const MODE_FAMILY_MASK: u8 = 0xF0;
const FLIGHT_FAMILY: u8 = 0x10;
const SPECIAL_TURN_FAMILY: u8 = 0x30;
const HEADING_LOCKED: u8 = 0x04;
const BRAKING: u8 = 0x20;
const SHARP_TURN: u8 = 0x08;
const DIRECTION_MASK: u16 = Button::Left as u16 | Button::Right as u16;
const SHOULDER_MASK: u16 = Button::LeftShoulder as u16 | Button::RightShoulder as u16;
const DIRECTION_AGE_SHIFT: u32 = 2;
const DIRECTION_AGE_LIMIT_BIT: u8 = 0x20;
const DIRECTION_AGE_LIMIT: u8 = 31;
const CAMERA_BANK_TARGET: i8 = 6;
const DIRECTIONAL_BANK: i8 = 30;
const SHOULDER_BANK_STEP: i16 = 10;
const SHOULDER_BANK_LIMIT: i16 = 64;
const NORMAL_RESPONSE: u16 = 640;
const SPECIAL_TURN_LEAN: u8 = 8;
const LOCKED_TURN_LEAN: u8 = 24;
const FINE_ANGLE_SHIFT: u32 = u8::BITS;
const LATERAL_ANGLE_MULTIPLIER: u8 = 4;
const LATERAL_SCALE: i32 = 40;
const HALF_RESPONSE: i16 = 2;
const NORMAL_LEAN: [u8; 6] = [24, 16, 8, 8, 24, 24];
const NORMAL_YAW_STEP: [u16; 6] = [512, 400, 544, 544, 384, 384];
const SHOULDER_YAW_STEP: [u16; 6] = [768, 656, 800, 800, 768, 640];
const BRAKING_YAW_STEP: u16 = 960;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerSteering {
    /// 6B17: direction-edge metadata and saturating held age. Left adds
    /// a second increment after the common age update, including on edges.
    pub direction_age: u8,
    /// 6AD2: retained step used to approach the current turning lean.
    pub turn_response: u16,
    /// 6AC0: signed camera-bank target from horizontal input. $07:867C
    /// expands it to a fine angle; $07:96CC publishes the resulting view roll.
    pub camera_bank_target: i8,
    /// 6AAE: heading used while the action locks fine yaw.
    pub locked_heading: Angle,
    /// 6B15: signed lateral offset from turning lean, not world velocity.
    pub lateral_offset: i16,
}

/// Special-turn and locked-heading branches do not initialize their response
/// target. Their caller must provide that value. It is an invocation input,
/// not a retained scratch area or a guessed zero. Neutral input needs none.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SteeringContext {
    pub inherited_response_target: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteeringError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    MissingSteering(ObjectId),
    MissingPose(ObjectId),
    MissingRoll(ObjectId),
    MissingVisit(ObjectId),
    MissingProcessedInput,
    MissingInheritedResponseTarget,
}

impl From<WorldInputError> for SteeringError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

impl From<PlayerStorageError> for SteeringError {
    fn from(error: PlayerStorageError) -> Self {
        Self::Storage(error)
    }
}

fn pilot_index(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<usize, SteeringError> {
    let code = world
        .player(objects, owner)?
        .visit
        .ok_or(SteeringError::MissingVisit(owner))?
        .pilot_code as usize;
    Ok(if code < NORMAL_LEAN.len() { code } else { 0 })
}

/// Wrapped half chase ($7F:2534), with minimum progress and signed division
/// toward zero. In particular the half-turn tie follows the negative delta.
pub(crate) fn half_word(current: u16, target: u16) -> u16 {
    let delta = target.wrapping_sub(current) as i16;
    let step = if delta == 0 {
        0
    } else if delta < 0 {
        delta.min(-HALF_RESPONSE) / HALF_RESPONSE
    } else {
        delta.max(HALF_RESPONSE) / HALF_RESPONSE
    };
    current.wrapping_add(step as u16)
}

fn approach_lean(current: u16, target: u16, response: u16) -> u16 {
    let difference = current.wrapping_sub(target) as i16;
    if difference == 0 {
        current
    } else if difference < 0 {
        let next = current.wrapping_add(response);
        if next.wrapping_sub(target) as i16 >= 0 {
            target
        } else {
            next
        }
    } else {
        let next = current.wrapping_sub(response);
        if (next.wrapping_sub(target) as i16) < 0 {
            target
        } else {
            next
        }
    }
}

fn advance_lean(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    context: SteeringContext,
) -> Result<(), SteeringError> {
    let auxiliary = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    let special = auxiliary.mode & MODE_FAMILY_MASK == SPECIAL_TURN_FAMILY;
    let locked = auxiliary.action_flags & HEADING_LOCKED != 0;
    let magnitude = if special {
        SPECIAL_TURN_LEAN
    } else {
        let pilot = pilot_index(objects, world, owner)?;
        if locked {
            LOCKED_TURN_LEAN
        } else {
            NORMAL_LEAN[pilot]
        }
    };
    let input = world
        .processed_player_input
        .ok_or(SteeringError::MissingProcessedInput)?;
    if input.held.bits() & DIRECTION_MASK == 0 {
        world
            .player_mut(objects, owner)?
            .steering
            .as_mut()
            .ok_or(SteeringError::MissingSteering(owner))?
            .turn_response = 0;
        // The original deliberately leaves turning lean unchanged here.
        return Ok(());
    }
    let magnitude = u16::from(magnitude) << FINE_ANGLE_SHIFT;
    let target = if input.held.contains(Button::Left) {
        magnitude
    } else {
        magnitude.wrapping_neg()
    };
    let response_target = if special || locked {
        context
            .inherited_response_target
            .ok_or(SteeringError::MissingInheritedResponseTarget)?
    } else {
        NORMAL_RESPONSE
    };
    let records = world.player_mut(objects, owner)?;
    let steering = records
        .steering
        .as_mut()
        .ok_or(SteeringError::MissingSteering(owner))?;
    steering.turn_response = quarter_word(steering.turn_response, response_target);
    let pose = records
        .pose
        .as_mut()
        .ok_or(SteeringError::MissingPose(owner))?;
    pose.turning_lean = approach_lean(pose.turning_lean, target, steering.turn_response);
    Ok(())
}

/// All horizontal-control writes, in source order. Missing later owners keep
/// the completed prefix. The scene wrapper latches errors against retries.
pub fn advance(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    context: SteeringContext,
) -> Result<(), SteeringError> {
    advance_lean(objects, world, owner, context)?;
    let input = world
        .processed_player_input
        .ok_or(SteeringError::MissingProcessedInput)?;
    let direction = input.held.bits() & DIRECTION_MASK;
    let pressed = input.pressed.bits() & DIRECTION_MASK;
    let left = input.held.contains(Button::Left);
    let shoulders_held = input.held.bits() & SHOULDER_MASK != 0;
    let steering = world
        .player_mut(objects, owner)?
        .steering
        .as_mut()
        .ok_or(SteeringError::MissingSteering(owner))?;
    if pressed != 0 {
        steering.direction_age = (pressed >> DIRECTION_AGE_SHIFT) as u8;
    } else if direction != 0 {
        steering.direction_age = steering.direction_age.wrapping_add(1);
        if steering.direction_age & DIRECTION_AGE_LIMIT_BIT != 0 {
            steering.direction_age = DIRECTION_AGE_LIMIT;
        }
    }
    steering.camera_bank_target = if left {
        steering.direction_age = steering.direction_age.wrapping_add(1);
        -CAMERA_BANK_TARGET
    } else if direction != 0 {
        CAMERA_BANK_TARGET
    } else {
        0
    };
    if direction != 0 {
        let braking = if shoulders_held {
            world
                .player(objects, owner)?
                .auxiliary
                .ok_or(WorldInputError::MissingAuxiliary(owner))?
                .action_flags
                & BRAKING
                != 0
        } else {
            false
        };
        let pilot = pilot_index(objects, world, owner)?;
        let step = if braking {
            BRAKING_YAW_STEP
        } else if shoulders_held {
            SHOULDER_YAW_STEP[pilot]
        } else {
            NORMAL_YAW_STEP[pilot]
        };
        world.player_yaw_increment = Some(if left { step } else { step.wrapping_neg() });
        if braking {
            world
                .player_mut(objects, owner)?
                .auxiliary
                .as_mut()
                .expect("validated auxiliary")
                .action_flags |= SHARP_TURN;
        }
    }
    // Neutral input does not reset the shared yaw increment. That publication
    // is cleared by the enclosing flight visit before calling steering.
    let ignores = world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .ignores_contacts;
    let bank_target = if ignores || shoulders_held || direction == 0 {
        0
    } else if left {
        DIRECTIONAL_BANK
    } else {
        -DIRECTIONAL_BANK
    };
    let pose = world
        .player_mut(objects, owner)?
        .pose
        .as_mut()
        .ok_or(SteeringError::MissingPose(owner))?;
    pose.steering_bank = chase_byte(pose.steering_bank as u8, bank_target as u8) as i8;
    let shoulder_step = if ignores {
        None
    } else {
        let shoulders = world
            .player(objects, owner)?
            .roll
            .ok_or(SteeringError::MissingRoll(owner))?
            .shoulders;
        if shoulders.left_selected() {
            Some(SHOULDER_BANK_STEP)
        } else if shoulders.right_selected() {
            Some(-SHOULDER_BANK_STEP)
        } else {
            None
        }
    };
    let records = world.player_mut(objects, owner)?;
    let pose = records.pose.as_mut().expect("validated pose");
    let bank = if let Some(step) = shoulder_step {
        pose.shoulder_bank.wrapping_add(step)
    } else {
        let bank = if ignores {
            half_word(pose.shoulder_bank as u16, 0)
        } else {
            pose.shoulder_bank as u16
        };
        chase_word(bank, 0) as i16
    };
    pose.shoulder_bank = bank.clamp(-SHOULDER_BANK_LIMIT, SHOULDER_BANK_LIMIT);
    if ignores {
        records
            .steering
            .as_mut()
            .expect("validated steering")
            .camera_bank_target = 0;
    }
    let auxiliary = records
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    if auxiliary.action_flags & HEADING_LOCKED != 0 {
        let yaw = player_storage::get(objects, resources, owner)?.fine_yaw;
        let records = world.player_mut(objects, owner)?;
        let heading = records
            .steering
            .as_ref()
            .expect("validated steering")
            .locked_heading
            .units();
        let target_bank = heading
            .wrapping_sub((yaw >> FINE_ANGLE_SHIFT) as u8)
            .wrapping_mul(2);
        let pose = records.pose.as_mut().expect("validated pose");
        pose.heading_return_bank = quarter_byte(pose.heading_return_bank as u8, target_bank) as i8;
        player_storage::get_mut(objects, resources, owner)?.fine_yaw =
            half_word(yaw, u16::from(heading) << FINE_ANGLE_SHIFT);
        pose.turning_lean = half_word(pose.turning_lean, 0);
    }
    let records = world.player_mut(objects, owner)?;
    let steering = records.steering.as_mut().expect("validated steering");
    if direction == 0 {
        steering.lateral_offset = half_word(steering.lateral_offset as u16, 0) as i16;
    } else if auxiliary.mode & MODE_FAMILY_MASK == FLIGHT_FAMILY {
        let lean = records.pose.as_ref().expect("validated pose").turning_lean;
        let angle = ((lean >> FINE_ANGLE_SHIFT) as u8).wrapping_mul(LATERAL_ANGLE_MULTIPLIER);
        // $07:E0DA multiplies a signed byte sine by 40, then takes bits 8..23.
        // Negative results round down; division toward zero is not equivalent.
        steering.lateral_offset = ((i32::from(sf_core::snes_trig::SINTAB[usize::from(angle)])
            * LATERAL_SCALE)
            >> FINE_ANGLE_SHIFT) as i16;
    }
    Ok(())
}

#[cfg(test)]
#[path = "player_steering_tests.rs"]
mod tests;
