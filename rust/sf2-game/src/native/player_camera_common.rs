//! Common camera orientation and its full position/orientation caller
//! ($07:84EC..86B6). Stored fine yaw shares the player's real allocation;
//! all three camera-angle high bytes remain authored-path-visible.

use super::path_fields::chase_word;
use super::path_runtime::PathRuntime;
use super::player_camera_angles::{self, CameraAnglesError};
use super::player_camera_ground::{self, GroundCameraError};
use super::player_camera_position::{self, CameraPositionError};
use super::player_camera_tracking::TrackingStyle;
use super::player_storage::{self, PlayerStorageError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Button, ObjectId, ObjectStore};

#[cfg(test)]
#[path = "player_camera_common_tests.rs"]
mod tests;

const FAMILY_MASK: u8 = 0xF0;
const TERRAIN_PITCH_FAMILY: u8 = 0x30;
const HEADING_LOCKED: u8 = 0x04;
const LINKED_LEAN_THRESHOLD: u16 = 4096;
const LATERAL_LEAN_THRESHOLD: i16 = 90;
const LEAN_QUARTER_SHIFT: u32 = 2;
const LEAN_EIGHTH_SHIFT: u32 = 3;
const FINE_SHIFT: u32 = 8;
const ANGLE_STEP: u16 = 256;
const ROLL_RECOVERY_DIVISOR: i16 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonCameraError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    Position(CameraPositionError),
    Angles(CameraAnglesError),
    Ground(GroundCameraError),
    MissingPose(ObjectId),
    MissingPosition(ObjectId),
    MissingAngles(ObjectId),
    MissingSteering(ObjectId),
    MissingProcessedInput,
}
impl From<WorldInputError> for CommonCameraError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<PlayerStorageError> for CommonCameraError {
    fn from(error: PlayerStorageError) -> Self {
        Self::Storage(error)
    }
}
impl From<CameraPositionError> for CommonCameraError {
    fn from(error: CameraPositionError) -> Self {
        Self::Position(error)
    }
}
impl From<CameraAnglesError> for CommonCameraError {
    fn from(error: CameraAnglesError) -> Self {
        Self::Angles(error)
    }
}
impl From<GroundCameraError> for CommonCameraError {
    fn from(error: GroundCameraError) -> Self {
        Self::Ground(error)
    }
}

fn approach_angle(current: u16, target: u16) -> u16 {
    let difference = current.wrapping_sub(target) as i16;
    if difference == 0 {
        current
    } else if difference < 0 {
        let next = current.wrapping_add(ANGLE_STEP);
        if (next.wrapping_sub(target) as i16) < 0 {
            next
        } else {
            target
        }
    } else {
        let next = current.wrapping_sub(ANGLE_STEP);
        if (next.wrapping_sub(target) as i16) >= 0 {
            next
        } else {
            target
        }
    }
}

/// Full yaw/roll tail. The yaw difference is genuinely zero in the source:
/// it subtracts the value just stored, not a retained previous yaw.
pub fn advance_yaw_roll(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &PathRuntime,
    owner: ObjectId,
) -> Result<(), CommonCameraError> {
    let action = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .action_flags;
    if action & HEADING_LOCKED == 0 {
        let camera = world
            .player_mut(objects, owner)?
            .camera_angles
            .as_mut()
            .ok_or(CommonCameraError::MissingAngles(owner))?;
        camera.yaw_offset = chase_word(camera.yaw_offset as u16, 0) as i16;
    } else {
        let linked = world
            .player(objects, owner)?
            .charge
            .ok_or(WorldInputError::MissingPlayerCharge(owner))?
            .linked_mode;
        let target = if linked {
            let lean = world
                .player(objects, owner)?
                .pose
                .ok_or(CommonCameraError::MissingPose(owner))?
                .turning_lean as i16;
            let magnitude = if lean < 0 {
                lean.wrapping_neg() as u16
            } else {
                lean as u16
            };
            if (magnitude.wrapping_sub(LINKED_LEAN_THRESHOLD) as i16) >= 0 {
                (lean >> LEAN_QUARTER_SHIFT).wrapping_neg()
            } else {
                0
            }
        } else {
            let offset = world
                .player(objects, owner)?
                .camera_position
                .ok_or(CommonCameraError::MissingPosition(owner))?
                .lateral_offset;
            let required = if offset >= LATERAL_LEAN_THRESHOLD {
                Some(Button::Left)
            } else if offset < -LATERAL_LEAN_THRESHOLD {
                Some(Button::Right)
            } else {
                None
            };
            let admitted = if let Some(button) = required {
                world
                    .processed_player_input
                    .ok_or(CommonCameraError::MissingProcessedInput)?
                    .held
                    .contains(button)
            } else {
                false
            };
            if admitted {
                let lean = world
                    .player(objects, owner)?
                    .pose
                    .ok_or(CommonCameraError::MissingPose(owner))?
                    .turning_lean as i16;
                (lean >> LEAN_QUARTER_SHIFT)
                    .wrapping_add(lean >> LEAN_EIGHTH_SHIFT)
                    .wrapping_neg()
            } else {
                0
            }
        };
        let camera = world
            .player_mut(objects, owner)?
            .camera_angles
            .as_mut()
            .ok_or(CommonCameraError::MissingAngles(owner))?;
        camera.yaw_offset = approach_angle(camera.yaw_offset as u16, target as u16) as i16;
    }
    let yaw = player_storage::get(objects, &runtime.resources, owner)?
        .fine_yaw
        .wrapping_neg();
    {
        let records = world.player_mut(objects, owner)?;
        let camera = records
            .camera_angles
            .as_mut()
            .ok_or(CommonCameraError::MissingAngles(owner))?;
        let auxiliary = records
            .auxiliary
            .as_mut()
            .ok_or(WorldInputError::MissingAuxiliary(owner))?;
        let mut angles = camera.capture(auxiliary.stored_rotation);
        angles.yaw = yaw;
        camera.write(&mut auxiliary.stored_rotation, angles);
        camera.yaw_difference = 0;
    }
    let ignores_contacts = world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .ignores_contacts;
    let target = if ignores_contacts {
        None
    } else {
        Some(
            u16::from(
                world
                    .player(objects, owner)?
                    .steering
                    .ok_or(CommonCameraError::MissingSteering(owner))?
                    .camera_bank_target as u8,
            ) << FINE_SHIFT,
        )
    };
    let records = world.player_mut(objects, owner)?;
    let camera = records
        .camera_angles
        .as_mut()
        .ok_or(CommonCameraError::MissingAngles(owner))?;
    let auxiliary = records
        .auxiliary
        .as_mut()
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    let mut angles = camera.capture(auxiliary.stored_rotation);
    angles.roll = if let Some(target) = target {
        approach_angle(angles.roll, target)
    } else {
        ((angles.roll as i16) / ROLL_RECOVERY_DIVISOR) as u16
    };
    camera.write(&mut auxiliary.stored_rotation, angles);
    Ok(())
}

pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &mut PathRuntime,
    owner: ObjectId,
    style: TrackingStyle,
    auxiliary_camera: bool,
) -> Result<(), CommonCameraError> {
    player_camera_position::prepare(objects, world, owner, style, auxiliary_camera)?;
    let family = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode
        & FAMILY_MASK;
    if family == TERRAIN_PITCH_FAMILY {
        player_camera_ground::advance_pitch(objects, world, runtime, owner)?;
    } else {
        player_camera_angles::advance_pitch(objects, world, owner)?;
    }
    advance_yaw_roll(objects, world, runtime, owner)
}
