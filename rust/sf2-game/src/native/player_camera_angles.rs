//! Retained flight-camera pitch ($07:86B7..88BE) and final pose publication
//! ($07:968B..9720). The full angle's high bytes remain in the existing
//! selected-auxiliary rotation, shared with paths and player pose composition.

use super::path_fields::chase_word;
use super::scene_path_world::{PlayerPathRecords, ScenePathWorld, WorldInputError};
use super::view_transition::FixedViewAngles;
use super::{Angle, Button, ObjectId, ObjectStore, Rotation};

#[cfg(test)]
#[path = "player_camera_angles_tests.rs"]
mod tests;

const FINE_SHIFT: u32 = u8::BITS;
const UPPER_LIMIT: u8 = 0x04;
const LOWER_LIMIT: u8 = 0x08;
const PITCH_UP: u8 = 0x01;
const PITCH_DOWN: u8 = 0x08;
const UP_TRACKING_PAIR: u8 = 0x03;
const DOWN_TRACKING_PAIR: u8 = 0x18;
const HEIGHT_TRACKING: u8 = 0x80;
const SMALL_INCREMENT: i16 = 128;
const TRACKING_INCREMENT: i16 = 448;
const LINKED_INCREMENT: i16 = 512;
const RECOIL_MULTIPLIER: i16 = 2;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CameraAngleFractions {
    pub pitch: u8,
    pub yaw: u8,
    pub roll: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CameraPitchProfile {
    /// Map-configured camera limits (6BFD/6BFE), not the separate player
    /// pitch limits at 6BF9/6BFA. Each byte is expanded to a fine angle.
    pub up: i8,
    pub down: i8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCameraAngles {
    /// Low bytes of 6B31/33/35; high bytes have one existing owner in
    /// SelectedAuxiliaryState.stored_rotation. Never retain a second pose.
    pub fractions: CameraAngleFractions,
    /// Height-following control (6B30), produced by camera height tracking.
    pub height_control: u8,
    /// Retained pitch increment (6B3F), separate from pitch recoil (6B3B).
    pub pitch_increment: i16,
    /// Heading lean added at publication (6B3D), not the stored view yaw.
    pub yaw_offset: i16,
    pub profile: CameraPitchProfile,
}

impl PlayerCameraAngles {
    pub fn capture(self, rotation: Rotation) -> FixedViewAngles {
        FixedViewAngles {
            pitch: u16::from_le_bytes([self.fractions.pitch, rotation.pitch.units()]),
            yaw: u16::from_le_bytes([self.fractions.yaw, rotation.yaw.units()]),
            roll: u16::from_le_bytes([self.fractions.roll, rotation.roll.units()]),
        }
    }

    pub fn write(&mut self, rotation: &mut Rotation, angles: FixedViewAngles) {
        let [pitch_fraction, pitch] = angles.pitch.to_le_bytes();
        let [yaw_fraction, yaw] = angles.yaw.to_le_bytes();
        let [roll_fraction, roll] = angles.roll.to_le_bytes();
        self.fractions = CameraAngleFractions {
            pitch: pitch_fraction,
            yaw: yaw_fraction,
            roll: roll_fraction,
        };
        *rotation = Rotation {
            pitch: Angle::from_units(pitch),
            yaw: Angle::from_units(yaw),
            roll: Angle::from_units(roll),
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraAnglesError {
    World(WorldInputError),
    MissingAngles(ObjectId),
    MissingVertical(ObjectId),
    MissingProcessedInput,
    MissingFixedView,
}
impl From<WorldInputError> for CameraAnglesError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn angles(
    records: &PlayerPathRecords,
    owner: ObjectId,
) -> Result<FixedViewAngles, CameraAnglesError> {
    let camera = records
        .camera_angles
        .ok_or(CameraAnglesError::MissingAngles(owner))?;
    let auxiliary = records
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    Ok(camera.capture(auxiliary.stored_rotation))
}

fn set_pitch(
    records: &mut PlayerPathRecords,
    owner: ObjectId,
    pitch: u16,
) -> Result<(), CameraAnglesError> {
    let mut current = angles(records, owner)?;
    current.pitch = pitch;
    records.camera_angles.as_mut().unwrap().write(
        &mut records.auxiliary.as_mut().unwrap().stored_rotation,
        current,
    );
    Ok(())
}

fn step_increment(current: i16, target: i16) -> i16 {
    let difference = current.wrapping_sub(target);
    if difference == 0 {
        current
    } else if difference < 0 {
        let next = current.wrapping_add(SMALL_INCREMENT);
        if next.wrapping_sub(target) < 0 {
            next
        } else {
            target
        }
    } else {
        let next = current.wrapping_sub(SMALL_INCREMENT);
        if next.wrapping_sub(target) >= 0 {
            next
        } else {
            target
        }
    }
}

fn fine(value: i8) -> i16 {
    (i16::from(value) << FINE_SHIFT) as i16
}

/// Advances only the retained camera pitch. Ordinary and linked camera input
/// have distinct branches; neither writes player pitch or the fixed view.
pub fn advance_pitch(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), CameraAnglesError> {
    let charge = world
        .player(objects, owner)?
        .charge
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
    let linked = charge.linked_mode && !charge.linked_muzzle_disabled;
    let target;
    if !linked {
        world
            .player_mut(objects, owner)?
            .camera_angles
            .as_mut()
            .ok_or(CameraAnglesError::MissingAngles(owner))?
            .pitch_increment = 0;
        let records = world.player(objects, owner)?;
        let camera = records.camera_angles.unwrap();
        let limits = records
            .vertical
            .ok_or(CameraAnglesError::MissingVertical(owner))?
            .limit_flags;
        let control = camera.height_control;
        let (target_byte, increment) = if limits & UPPER_LIMIT != 0 && control & PITCH_UP != 0 {
            (camera.profile.up >> 1, TRACKING_INCREMENT)
        } else if limits & UPPER_LIMIT == 0
            && control & HEIGHT_TRACKING == 0
            && limits & LOWER_LIMIT != 0
            && control & PITCH_DOWN != 0
        {
            (camera.profile.down >> 1, -TRACKING_INCREMENT)
        } else if control & DOWN_TRACKING_PAIR == DOWN_TRACKING_PAIR {
            (camera.profile.down, -TRACKING_INCREMENT)
        } else if control & UP_TRACKING_PAIR == UP_TRACKING_PAIR {
            (camera.profile.up, TRACKING_INCREMENT)
        } else if control & (PITCH_UP | PITCH_DOWN) != 0 {
            let increment = if control & PITCH_UP != 0 {
                SMALL_INCREMENT
            } else {
                -SMALL_INCREMENT
            };
            let records = world.player_mut(objects, owner)?;
            records.camera_angles.as_mut().unwrap().pitch_increment = increment;
            let current = angles(records, owner)?.pitch;
            set_pitch(records, owner, chase_word(current, 0))?;
            return Ok(());
        } else {
            // The source's final recovery reads the zero written above.
            return Ok(());
        };
        target = fine(target_byte);
        world
            .player_mut(objects, owner)?
            .camera_angles
            .as_mut()
            .unwrap()
            .pitch_increment = increment;
    } else {
        let input = world
            .processed_player_input
            .ok_or(CameraAnglesError::MissingProcessedInput)?;
        let direction = if input.held.contains(Button::Up) {
            Some(false)
        } else if input.held.contains(Button::Down) {
            Some(true)
        } else {
            None
        };
        let Some(up) = direction else {
            let camera = world
                .player_mut(objects, owner)?
                .camera_angles
                .as_mut()
                .ok_or(CameraAnglesError::MissingAngles(owner))?;
            camera.pitch_increment = step_increment(camera.pitch_increment, 0);
            return Ok(());
        };
        let records = world.player(objects, owner)?;
        let camera = records
            .camera_angles
            .ok_or(CameraAnglesError::MissingAngles(owner))?;
        let limits = records
            .vertical
            .ok_or(CameraAnglesError::MissingVertical(owner))?
            .limit_flags;
        target = if up {
            fine(camera.profile.up)
        } else {
            fine(camera.profile.down)
        };
        let restricted = limits & if up { UPPER_LIMIT } else { LOWER_LIMIT } != 0;
        let target = if restricted { target >> 1 } else { target };
        let increment_target = if up {
            LINKED_INCREMENT
        } else {
            -LINKED_INCREMENT
        };
        let records = world.player_mut(objects, owner)?;
        let camera = records.camera_angles.as_mut().unwrap();
        camera.pitch_increment = step_increment(camera.pitch_increment, increment_target);
        return integrate_pitch(records, owner, target);
    }
    integrate_pitch(world.player_mut(objects, owner)?, owner, target)
}

fn integrate_pitch(
    records: &mut PlayerPathRecords,
    owner: ObjectId,
    target: i16,
) -> Result<(), CameraAnglesError> {
    let current = angles(records, owner)?.pitch;
    let candidate = current.wrapping_add(records.camera_angles.unwrap().pitch_increment as u16);
    // Source compares magnitudes as unsigned, using the candidate's sign
    // to select the side. A single signed min/max changes wrap boundaries.
    let use_candidate = if (candidate as i16) < 0 {
        candidate >= target as u16
    } else {
        candidate < target as u16
    };
    set_pitch(
        records,
        owner,
        if use_candidate {
            candidate
        } else {
            chase_word(current, target as u16)
        },
    )
}

fn fixed_view(world: &ScenePathWorld) -> Result<ObjectId, CameraAnglesError> {
    world.fixed_players[0].ok_or(CameraAnglesError::MissingFixedView)
}

/// The two consecutive publication calls in the normal camera. Recoil has
/// already advanced in their caller. The roll publication is the full angle,
/// not its byte-angle observation or the later continuity-adjusted result.
pub fn publish(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), CameraAnglesError> {
    let original = angles(world.player(objects, owner)?, owner)?;
    let view_id = fixed_view(world)?;
    let view = objects
        .get_mut(view_id)
        .ok_or(WorldInputError::MissingActor(view_id))?;
    let mut output = FixedViewAngles::capture(view);
    output.pitch = original.pitch;
    output.write_to(view);
    let records = world.player(objects, owner)?;
    let recoil = records
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .camera_pitch_recoil;
    output.pitch = output
        .pitch
        .wrapping_add(recoil.wrapping_mul(RECOIL_MULTIPLIER) as u16);
    output.yaw = original
        .yaw
        .wrapping_add(records.camera_angles.unwrap().yaw_offset as u16);
    output.roll = original.roll;
    output.write_to(objects.get_mut(view_id).unwrap());
    world.published_camera_roll = Some(output.roll);
    let view = objects.get_mut(view_id).unwrap();
    view.base.view_rear_distance = 0;
    let position = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .stored_world_position;
    objects.get_mut(view_id).unwrap().base.position = position;
    Ok(())
}

/// Scripted/action-gated dispatcher branch ($07:96DC). It samples the view
/// directly and needs no player storage, pose, input or recoil owner.
pub fn publish_existing_roll(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
) -> Result<(), CameraAnglesError> {
    let view_id = fixed_view(world)?;
    let view = objects
        .get(view_id)
        .ok_or(WorldInputError::MissingActor(view_id))?;
    world.published_camera_roll = Some(FixedViewAngles::capture(view).roll);
    Ok(())
}
