//! Height-following part of the live flight camera ($07:8D66..90A2).
//! Its retained height flags also drive player_camera_angles. The prepared
//! camera height is an invocation value, not a second world-space position.

use super::path_fields::chase_word;
use super::player_steering::half_word;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Button, ObjectId, ObjectStore};
use sf_core::snes_trig::mulslog;

#[cfg(test)]
#[path = "player_camera_tracking_tests.rs"]
mod tests;

const FAMILY_MASK: u8 = 0xF0;
const ALTERNATE_FAMILY: u8 = 0x30;
const WALKER_FAMILY: u8 = 0x20;
const ACTIVE: u8 = 0x01;
const UP_REQUEST: u8 = 0x01;
const DOWN_REQUEST: u8 = 0x08;
const FOLLOWING_UP: u8 = 0x02;
const FOLLOWING_DOWN: u8 = 0x10;
const UP_REQUEST_AND_DIRECTION: u8 = 0x05;
const DOWN_REQUEST_AND_DIRECTION: u8 = 0x28;
const HEIGHT_LOCKED: u8 = 0x80;
const CONTROL_TAGS: u8 = 0xC0;
const NEUTRAL_CONTROLS: u8 = 0xE4;
const CARRY_ENABLED: u8 = 1;
const LINKED_VERTICAL_OFFSET: i16 = -20;
const HEIGHT_FOLLOW_THRESHOLD: i16 = 30;
const HEIGHT_RECOVERY_STEP: i16 = 5;
const TERRAIN_CLEARANCE: i16 = 100;
const CARRIED_HEIGHT_SCALE: i32 = 126;
const CARRIED_HEIGHT_CORRECTION_SHIFT: u32 = 5;

/// Semantic camera modes that share the tracking helpers. These are not
/// source addresses; the camera dispatcher owns selecting the actual mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingStyle {
    Normal,
    ProjectionCorrected,
    Surface,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCameraTracking {
    /// Retained height anchor and difference (6B45/6B47).
    pub anchor_height: i16,
    pub height_difference: i16,
    /// Smoothed vertical offset (6B4E), used before distance projection.
    pub vertical_offset: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraTrackingError {
    World(WorldInputError),
    MissingTracking(ObjectId),
    MissingAngles(ObjectId),
    MissingVertical(ObjectId),
    MissingProcessedInput,
    MissingCarryMode,
}
impl From<WorldInputError> for CameraTrackingError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn tracking_mut<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut PlayerCameraTracking, CameraTrackingError> {
    world
        .player_mut(objects, owner)?
        .camera_tracking
        .as_mut()
        .ok_or(CameraTrackingError::MissingTracking(owner))
}

/// Complete height-following helper. The override flag describes the real
/// caller's active auxiliary camera, not a guessed default or callback result.
pub fn advance_height(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    prepared_height: i16,
    style: TrackingStyle,
    override_active: bool,
) -> Result<i16, CameraTrackingError> {
    let auxiliary = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    let family = auxiliary.mode & FAMILY_MASK;
    if family == ALTERNATE_FAMILY {
        return Ok(prepared_height);
    }
    if auxiliary.action_flags & ACTIVE == 0 {
        let tracking = tracking_mut(objects, world, owner)?;
        tracking.vertical_offset = chase_word(tracking.vertical_offset as u16, 0) as i16;
        return Ok(prepared_height);
    }
    let contacts = world
        .contacts_enabled()
        .ok_or(WorldInputError::MissingContactEnable)?;
    let reset_to_actor = if !contacts {
        true
    } else {
        let charge = world
            .player(objects, owner)?
            .charge
            .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
        charge.linked_muzzle_disabled || charge.linked_mode
    };
    if reset_to_actor {
        let tracking = tracking_mut(objects, world, owner)?;
        tracking.height_difference = 0;
        tracking.vertical_offset = LINKED_VERTICAL_OFFSET;
        let actor = objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?;
        let height = actor.base.position.y;
        let records = world.player_mut(objects, owner)?;
        records.auxiliary.as_mut().unwrap().stored_world_position.y = height;
        records.camera_tracking.as_mut().unwrap().anchor_height = height;
        let angles = records
            .camera_angles
            .as_mut()
            .ok_or(CameraTrackingError::MissingAngles(owner))?;
        angles.height_control &= CONTROL_TAGS;
        angles.height_control |= if (actor.base.pitch.units() as i8) < 0 {
            UP_REQUEST_AND_DIRECTION
        } else {
            DOWN_REQUEST_AND_DIRECTION
        };
        return Ok(prepared_height);
    }
    if family == WALKER_FAMILY {
        if world
            .player_carry_mode
            .ok_or(CameraTrackingError::MissingCarryMode)?
            == CARRY_ENABLED
            && objects
                .get(owner)
                .ok_or(WorldInputError::MissingActor(owner))?
                .extension
                .path_state
                .motion
                .carry_selected_player
        {
            let scaled = mulslog(i32::from(prepared_height), CARRIED_HEIGHT_SCALE) as i16;
            return Ok(scaled.wrapping_sub(scaled >> CARRIED_HEIGHT_CORRECTION_SHIFT));
        }
        return Ok(prepared_height);
    }
    if override_active {
        let tracking = tracking_mut(objects, world, owner)?;
        tracking.height_difference = 0;
        tracking.vertical_offset = 0;
        let height = objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .base
            .position
            .y;
        let records = world.player_mut(objects, owner)?;
        records.auxiliary.as_mut().unwrap().stored_world_position.y = height;
        records.camera_tracking.as_mut().unwrap().anchor_height = height;
        return Ok(prepared_height);
    }
    let input = world
        .processed_player_input
        .ok_or(CameraTrackingError::MissingProcessedInput)?;
    let actor_pitch = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .pitch
        .units() as i8;
    let records = world.player_mut(objects, owner)?;
    let tracking = records
        .camera_tracking
        .as_mut()
        .ok_or(CameraTrackingError::MissingTracking(owner))?;
    let camera = records
        .camera_angles
        .as_mut()
        .ok_or(CameraTrackingError::MissingAngles(owner))?;
    let up = input.held.contains(Button::Up);
    let down = input.held.contains(Button::Down);
    if up
        && (actor_pitch >= 0
            || tracking
                .height_difference
                .wrapping_add(HEIGHT_FOLLOW_THRESHOLD)
                >= 0)
    {
        if camera.height_control & DOWN_REQUEST == 0 {
            camera.height_control &= CONTROL_TAGS | DOWN_REQUEST_AND_DIRECTION;
        }
        camera.height_control |= DOWN_REQUEST_AND_DIRECTION;
    } else if !up
        && down
        && (actor_pitch < 0
            || tracking
                .height_difference
                .wrapping_sub(HEIGHT_FOLLOW_THRESHOLD)
                < 0)
    {
        if camera.height_control & UP_REQUEST == 0 {
            camera.height_control &= CONTROL_TAGS | UP_REQUEST_AND_DIRECTION;
        }
        camera.height_control |= UP_REQUEST_AND_DIRECTION;
    } else {
        camera.height_control &= NEUTRAL_CONTROLS;
    }
    let mut recover_height = false;
    if style != TrackingStyle::ProjectionCorrected && camera.height_control & UP_REQUEST == 0 {
        let lower_offset = records
            .vertical
            .ok_or(CameraTrackingError::MissingVertical(owner))?
            .profile
            .lower_height_offset;
        recover_height = prepared_height
            .wrapping_add(lower_offset)
            .wrapping_add(TERRAIN_CLEARANCE)
            >= 0;
        if recover_height {
            camera.height_control &= !(FOLLOWING_UP | FOLLOWING_DOWN);
            tracking.height_difference = if tracking.height_difference > 0 {
                (tracking.height_difference - HEIGHT_RECOVERY_STEP).max(0)
            } else if tracking.height_difference < 0 {
                (tracking.height_difference + HEIGHT_RECOVERY_STEP).min(0)
            } else {
                0
            };
            tracking.anchor_height = prepared_height.wrapping_sub(tracking.height_difference);
            camera.height_control |= HEIGHT_LOCKED;
        } else {
            camera.height_control &= !HEIGHT_LOCKED;
        }
    }
    let difference;
    if recover_height || camera.height_control & (FOLLOWING_UP | FOLLOWING_DOWN) != 0 {
        tracking.anchor_height = prepared_height.wrapping_sub(tracking.height_difference);
        difference = tracking.height_difference;
    } else {
        difference = prepared_height.wrapping_sub(tracking.anchor_height);
        tracking.height_difference = difference;
        if camera.height_control & DOWN_REQUEST_AND_DIRECTION != 0 {
            if difference >= HEIGHT_FOLLOW_THRESHOLD {
                tracking.anchor_height = half_word(
                    tracking.anchor_height as u16,
                    prepared_height.wrapping_sub(HEIGHT_FOLLOW_THRESHOLD) as u16,
                ) as i16;
                if camera.height_control & DOWN_REQUEST != 0 {
                    camera.height_control |= FOLLOWING_DOWN;
                }
            }
        } else if difference < -HEIGHT_FOLLOW_THRESHOLD {
            tracking.anchor_height = half_word(
                tracking.anchor_height as u16,
                prepared_height.wrapping_add(HEIGHT_FOLLOW_THRESHOLD) as u16,
            ) as i16;
            if camera.height_control & UP_REQUEST != 0 {
                camera.height_control |= FOLLOWING_UP;
            }
        }
    }
    tracking.vertical_offset = (difference >> 1)
        .wrapping_sub(difference >> 4)
        .wrapping_neg();
    Ok(prepared_height)
}
