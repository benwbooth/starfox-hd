//! Camera position helpers ($07:88BF..8D65): lateral following, rotated
//! view distance and boost/brake displacement. Their retained fields belong
//! to the player; the prepared position is local to one camera invocation.

use super::path_fields::chase_word;
use super::player_camera_tracking::TrackingStyle;
use super::player_pose::quarter_word;
use super::player_steering::half_word;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore, Vector3};
use sf_core::snes_trig::{rotate_16xz, rotate_16yz};

#[cfg(test)]
#[path = "player_camera_position_tests.rs"]
mod tests;

const FAMILY_MASK: u8 = 0xF0;
const FLIGHT_FAMILY: u8 = 0x10;
const ACTIVE: u8 = 0x01;
const HEADING_LOCKED: u8 = 0x04;
const BOOST: u8 = 0x40;
const BRAKE: u8 = 0x20;
const OBSTRUCTION_FLAGS: u8 = 0xC0;
const PLAYER_OBSTRUCTED: u8 = 0x80;
const SHOULDER_BUTTONS: u16 = 0x0030;
const CARRY_ENABLED: u8 = 1;
const LATERAL_RECOVERY_STEP: i16 = 10;
const LATERAL_LIMIT: i16 = 100;
const OBSTRUCTION_DECAY_DIVISOR: i16 = 4;
const PROTECTED_DISTANCE: i16 = -400;
const OBSTRUCTED_OFFSET: i16 = 100;
const CARRIED_OFFSET: i16 = -100;
const SUPPORTED_OFFSET: i16 = -200;
const SURFACE_RESPONSE_DIVISOR: i16 = 32;
const IMPULSE_RESPONSE_DIVISOR: i16 = 16;
const BOOST_IMPULSE_TARGET: i16 = 70;
const BRAKE_IMPULSE_TARGET: i16 = 20;
const DOUBLED_OFFSET_SCALE: i16 = 2;
const PROJECTED_HALF_SHIFT: u32 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCameraPosition {
    /// Smoothed lateral position (6B4A) and its retained partner (6B4C).
    /// The partner is reset here but is not otherwise interpreted.
    pub lateral_offset: i16,
    pub secondary_lateral_offset: i16,
    /// Integrated steering/impact input (6AE7), distinct from its smoothing.
    pub lateral_accumulator: i16,
    /// Shared surface-distance and boost/brake response (6B52).
    pub longitudinal_offset: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraPositionError {
    World(WorldInputError),
    Tracking(super::player_camera_tracking::CameraTrackingError),
    MissingPosition(ObjectId),
    MissingExemption(ObjectId),
    MissingTracking(ObjectId),
    MissingDistance(ObjectId),
    MissingMotion(ObjectId),
    MissingSteering(ObjectId),
    MissingAmbient(ObjectId),
    MissingProcessedInput,
    MissingCarryMode,
    MissingProxy,
}
impl From<WorldInputError> for CameraPositionError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<super::player_camera_tracking::CameraTrackingError> for CameraPositionError {
    fn from(error: super::player_camera_tracking::CameraTrackingError) -> Self {
        Self::Tracking(error)
    }
}

fn position_mut<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut PlayerCameraPosition, CameraPositionError> {
    world
        .player_mut(objects, owner)?
        .camera_position
        .as_mut()
        .ok_or(CameraPositionError::MissingPosition(owner))
}

fn reset_lateral(position: &mut PlayerCameraPosition) {
    position.lateral_offset = 0;
    position.secondary_lateral_offset = 0;
    position.lateral_accumulator = 0;
}

/// Slow source chases use a wrapped signed difference, minimum progress,
/// and negative division toward zero. They are not arithmetic shifts.
fn slow_chase(current: i16, target: i16, divisor: i16) -> i16 {
    let difference = target.wrapping_sub(current);
    let step = if difference < 0 {
        difference.min(-divisor) / divisor
    } else if difference > 0 {
        difference.max(divisor) / divisor
    } else {
        0
    };
    current.wrapping_add(step)
}

pub fn advance_lateral(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    mut prepared: Vector3,
) -> Result<Vector3, CameraPositionError> {
    let charge = world
        .player(objects, owner)?
        .charge
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
    if charge.linked_muzzle_disabled || charge.linked_mode {
        reset_lateral(position_mut(objects, world, owner)?);
        return Ok(prepared);
    }
    let auxiliary = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    if auxiliary.action_flags & ACTIVE == 0 {
        reset_lateral(position_mut(objects, world, owner)?);
        return Ok(prepared);
    }
    if auxiliary.mode & FAMILY_MASK != FLIGHT_FAMILY {
        reset_lateral(position_mut(objects, world, owner)?);
    } else {
        let mut integrate = true;
        if auxiliary.action_flags & HEADING_LOCKED == 0 {
            let motion = world
                .player(objects, owner)?
                .motion
                .ok_or(CameraPositionError::MissingMotion(owner))?;
            if motion.contact_flags & OBSTRUCTION_FLAGS != 0 {
                position_mut(objects, world, owner)?.lateral_accumulator /=
                    OBSTRUCTION_DECAY_DIVISOR;
                integrate = false;
            } else {
                let held = world
                    .processed_player_input
                    .ok_or(CameraPositionError::MissingProcessedInput)?
                    .held
                    .bits();
                if held & SHOULDER_BUTTONS != 0 {
                    let accumulator = &mut position_mut(objects, world, owner)?.lateral_accumulator;
                    *accumulator = if *accumulator > 0 {
                        (*accumulator - LATERAL_RECOVERY_STEP).max(0)
                    } else if *accumulator < 0 {
                        (*accumulator + LATERAL_RECOVERY_STEP).min(0)
                    } else {
                        0
                    };
                    integrate = false;
                }
            }
        }
        if integrate {
            let steering = world
                .player(objects, owner)?
                .steering
                .ok_or(CameraPositionError::MissingSteering(owner))?
                .lateral_offset;
            let impulse = world
                .player(objects, owner)?
                .motion
                .ok_or(CameraPositionError::MissingMotion(owner))?
                .lateral_impulse;
            let state = position_mut(objects, world, owner)?;
            state.lateral_accumulator = state
                .lateral_accumulator
                .wrapping_add(steering)
                .wrapping_add(i16::from(impulse))
                .clamp(-LATERAL_LIMIT, LATERAL_LIMIT);
        }
        let state = position_mut(objects, world, owner)?;
        state.lateral_offset = quarter_word(
            state.lateral_offset as u16,
            state.lateral_accumulator as u16,
        ) as i16;
    }
    let offset = position_mut(objects, world, owner)?.lateral_offset;
    let (x, z) = rotate_16xz(
        auxiliary.stored_rotation.yaw.units().wrapping_neg(),
        offset,
        0,
    );
    prepared.x = prepared.x.wrapping_add(x >> PROJECTED_HALF_SHIFT);
    prepared.z = prepared.z.wrapping_add(z >> PROJECTED_HALF_SHIFT);
    Ok(prepared)
}

/// Surface cameras reuse the longitudinal response that boost/brake owns;
/// ordinary and projection-corrected cameras read only the view distance.
pub fn advance_distance(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    mut prepared: Vector3,
    style: TrackingStyle,
) -> Result<Vector3, CameraPositionError> {
    if style == TrackingStyle::Surface {
        let hold = world
            .player(objects, owner)?
            .contact
            .ok_or(WorldInputError::MissingPlayerContact(owner))?
            .hit
            .hold_secondary_protection;
        if hold {
            let distance = world
                .player_mut(objects, owner)?
                .view_distance
                .as_mut()
                .ok_or(CameraPositionError::MissingDistance(owner))?;
            distance.distance =
                quarter_word(distance.distance as u16, PROTECTED_DISTANCE as u16) as i16;
        } else {
            let suppressed = world
                .player(objects, owner)?
                .occupancy_exempt
                .ok_or(CameraPositionError::MissingExemption(owner))?;
            let linked = if suppressed {
                false
            } else {
                let charge = world
                    .player(objects, owner)?
                    .charge
                    .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
                charge.linked_mode && !charge.linked_muzzle_disabled
            };
            let obstructed = if suppressed || linked {
                false
            } else {
                world
                    .player(objects, owner)?
                    .motion
                    .ok_or(CameraPositionError::MissingMotion(owner))?
                    .contact_flags
                    & PLAYER_OBSTRUCTED
                    != 0
            };
            if obstructed {
                let state = position_mut(objects, world, owner)?;
                state.longitudinal_offset =
                    quarter_word(state.longitudinal_offset as u16, OBSTRUCTED_OFFSET as u16) as i16;
            } else {
                let carry_mode = world
                    .player_carry_mode
                    .ok_or(CameraPositionError::MissingCarryMode)?;
                let actor = objects
                    .get(owner)
                    .ok_or(WorldInputError::MissingActor(owner))?;
                let target = if carry_mode == CARRY_ENABLED
                    && actor.extension.path_state.motion.carry_selected_player
                {
                    CARRIED_OFFSET
                } else {
                    let charge = world
                        .player(objects, owner)?
                        .charge
                        .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
                    if charge.linked_mode && !charge.linked_muzzle_disabled {
                        0
                    } else if actor.extension.surface_contact.supporting_object.is_none() {
                        0
                    } else if world
                        .player(objects, owner)?
                        .contact
                        .ok_or(WorldInputError::MissingPlayerContact(owner))?
                        .ignores_contacts
                    {
                        0
                    } else {
                        SUPPORTED_OFFSET
                    }
                };
                let state = position_mut(objects, world, owner)?;
                state.longitudinal_offset =
                    slow_chase(state.longitudinal_offset, target, SURFACE_RESPONSE_DIVISOR);
            }
        }
    }
    let mut distance = world
        .player(objects, owner)?
        .view_distance
        .ok_or(CameraPositionError::MissingDistance(owner))?
        .distance;
    if style == TrackingStyle::Surface {
        distance = distance.wrapping_add(position_mut(objects, world, owner)?.longitudinal_offset);
    }
    let records = world.player(objects, owner)?;
    let rotation = records
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .stored_rotation;
    let vertical = records
        .camera_tracking
        .ok_or(CameraPositionError::MissingTracking(owner))?
        .vertical_offset;
    let (y, z) = rotate_16yz(
        rotation.pitch.units().wrapping_neg(),
        vertical.wrapping_mul(DOUBLED_OFFSET_SCALE),
        distance,
    );
    let (x, z) = rotate_16xz(rotation.yaw.units().wrapping_neg(), 0, z);
    prepared.x = prepared.x.wrapping_add(x >> PROJECTED_HALF_SHIFT);
    prepared.y = prepared.y.wrapping_add(y >> PROJECTED_HALF_SHIFT);
    prepared.z = prepared.z.wrapping_add(z >> PROJECTED_HALF_SHIFT);
    Ok(prepared)
}

/// Apply boost/brake displacement to the retained camera position via the
/// actual shared proxy. Linked view still copies that proxy while preserving
/// the charge impulse and its timer; it does not project its decaying offset.
pub fn advance_boost(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), CameraPositionError> {
    let charge = world
        .player(objects, owner)?
        .charge
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
    let distance;
    if charge.linked_mode {
        let state = position_mut(objects, world, owner)?;
        state.longitudinal_offset = chase_word(state.longitudinal_offset as u16, 0) as i16;
        distance = 0;
    } else {
        let action = world
            .player(objects, owner)?
            .auxiliary
            .ok_or(WorldInputError::MissingAuxiliary(owner))?
            .action_flags;
        let mut impulse_target = BOOST_IMPULSE_TARGET;
        let target = if action & (BOOST | BRAKE) != 0 {
            let profile = world
                .player(objects, owner)?
                .view_distance
                .ok_or(CameraPositionError::MissingDistance(owner))?;
            let response = if action & BOOST != 0 {
                profile.boost_response
            } else {
                impulse_target = BRAKE_IMPULSE_TARGET;
                profile.brake_response
            };
            response.wrapping_add(charge.speed_impulse)
        } else {
            0
        };
        let charge = world.player_mut(objects, owner)?.charge.as_mut().unwrap();
        if charge.speed_impulse_ticks != 0 {
            charge.speed_impulse_ticks -= 1;
            charge.speed_impulse =
                half_word(charge.speed_impulse as u16, impulse_target as u16) as i16;
        } else {
            charge.speed_impulse = slow_chase(charge.speed_impulse, 0, IMPULSE_RESPONSE_DIVISOR);
        }
        let impulse = charge.speed_impulse;
        let state = position_mut(objects, world, owner)?;
        state.longitudinal_offset =
            chase_word(state.longitudinal_offset as u16, target as u16) as i16;
        distance = state
            .longitudinal_offset
            .wrapping_mul(DOUBLED_OFFSET_SCALE)
            .wrapping_add(impulse);
    }
    let proxy = world
        .weapons
        .as_ref()
        .and_then(|weapons| weapons.fallback)
        .ok_or(CameraPositionError::MissingProxy)?;
    let position = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .stored_world_position;
    objects
        .get_mut(proxy)
        .ok_or(WorldInputError::MissingActor(proxy))?
        .base
        .position = position;
    // A malformed alias still observes the copied actor before rotation,
    // matching the real shared record rather than a discarded temporary.
    let actor = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    let (y, z) = rotate_16yz(actor.base.pitch.units(), 0, distance);
    let (x, z) = rotate_16xz(actor.base.yaw.units(), 0, z);
    let position = &mut objects.get_mut(proxy).unwrap().base.position;
    position.x = position.x.wrapping_add(x);
    position.z = position.z.wrapping_add(z);
    position.y = position.y.wrapping_add(y);
    let position = *position;
    world
        .player_mut(objects, owner)?
        .auxiliary
        .as_mut()
        .unwrap()
        .stored_world_position = position;
    Ok(())
}

/// Complete common-camera position prefix ($07:84EC..852F), in the actual
/// caller order. Pitch/yaw/roll are the next phase, not fabricated here.
/// Return the invocation's prepared position, which deliberately differs
/// from the retained position after boost/brake projection through the proxy.
pub fn prepare(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    style: TrackingStyle,
    auxiliary_camera: bool,
) -> Result<Vector3, CameraPositionError> {
    let mut prepared = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .position;
    prepared.y = super::player_camera_tracking::advance_height(
        objects,
        world,
        owner,
        prepared.y,
        style,
        auxiliary_camera,
    )?;
    prepared = advance_distance(objects, world, owner, prepared, style)?;
    prepared = advance_lateral(objects, world, owner, prepared)?;
    let offset = world
        .player(objects, owner)?
        .ambient
        .ok_or(CameraPositionError::MissingAmbient(owner))?
        .retained_offset;
    prepared.y = prepared.y.wrapping_add(offset);
    world
        .player_mut(objects, owner)?
        .auxiliary
        .as_mut()
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .stored_world_position = prepared;
    advance_boost(objects, world, owner)?;
    Ok(prepared)
}
