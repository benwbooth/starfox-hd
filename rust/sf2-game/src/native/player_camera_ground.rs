//! Terrain/Walker camera pitch ($07:81F1..84DB) and the real shared-proxy
//! target builder ($07:9721..97F4). The fixed view is an existing actor;
//! target geometry observes that live pose rather than a newly prepared one.

use super::path_runtime::PathRuntime;
use super::player_steering::half_word;
use super::player_storage::{self, PlayerStorageError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Angle, ObjectId, ObjectStore, Vector3};
use sf_core::aim_angle::{sf2_atan16, sf2_xz_angle_distance};
use sf_core::snes_trig::rotate_8xz;

#[cfg(test)]
#[path = "player_camera_ground_tests.rs"]
mod tests;

const FAMILY_MASK: u8 = 0xF0;
const FLIGHT_FAMILY: u8 = 0x10;
const WALKER_FAMILY: u8 = 0x20;
const CARRY_ENABLED: u8 = 1;
const PLANE_MARGIN: i16 = 16;
const HEIGHT_TARGET_MARGIN: i16 = 30;
const HEIGHT_RESPONSE: i16 = 3;
const LINKED_HEIGHT_MARGIN: i16 = 15;
const AIM_FORWARD: i8 = 100;
const FINE_SHIFT: u32 = 8;
const FRACTION_MASK: u16 = 0x00FF;
const PITCH_LIMIT: i8 = 40;
const SURFACE_MODE_MASK: u8 = 0x07;
const SPECIAL_SURFACE_MODE: u8 = 2;
const SPECIAL_MATERIAL: u8 = 1;
const WALKER_PITCH_FOLLOW: u8 = 0x08;
const WALKER_STANCE_MASK: u8 = 0x07;
const STANDING_STANCE_LIMIT: u8 = 2;
const FRAME_RESPONSE_DIVISOR: i16 = 8;
const FRAME_MASK: u8 = 0x7F;

// The source indexes with all seven retained animation bits, without a
// sixteen-frame clamp. Preserve all 128 readable bias bytes, including the
// contiguous values after the authored sixteen-frame table. These are data
// values only; no source instructions are decoded or executed by gameplay.
const FRAME_PITCH_BIAS: [u8; 128] = [
    128, 128, 0, 0, 0, 0, 128, 128, 128, 128, 0, 0, 0, 0, 128, 128, 90, 8, 226, 32, 194, 16, 180,
    43, 194, 32, 181, 12, 141, 194, 29, 181, 14, 141, 196, 29, 181, 16, 141, 198, 29, 226, 32, 32,
    102, 141, 32, 122, 138, 32, 191, 136, 194, 32, 173, 196, 29, 24, 121, 226, 106, 141, 196, 29,
    153, 195, 106, 173, 194, 29, 153, 193, 106, 173, 198, 29, 153, 197, 106, 226, 32, 32, 211, 139,
    185, 160, 106, 41, 240, 201, 48, 208, 4, 92, 66, 133, 7, 32, 183, 134, 128, 4, 34, 241, 129, 7,
    90, 180, 43, 185, 119, 107, 122, 137, 4, 208, 4, 92, 30, 134, 7, 194, 32, 169, 0, 0, 141, 174,
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCameraGround {
    /// Independent parts of 6B7D: 10 resets pitch; 08 follows the plane.
    /// Protection-hold 80 and recovery-blocked 40 keep their existing owners.
    pub hold_pitch: bool,
    pub follow_environment_plane: bool,
    /// 6B5A/5C: retained height offset and its target adjustment.
    pub height_offset: i16,
    pub height_target_adjustment: i16,
    /// Carried camera target height (6BF3), not return_position.y (6BEF).
    pub carried_target_height: i16,
    /// Fine pitch added after ordinary following (6B37).
    pub animation_pitch: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraAim {
    pub pitch: u16,
    pub yaw: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroundCameraError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    MissingGround(ObjectId),
    MissingAngles(ObjectId),
    MissingBoundary(ObjectId),
    MissingDistance(ObjectId),
    MissingConsumable(ObjectId),
    MissingSurface(ObjectId),
    MissingMotion(ObjectId),
    MissingEnvironmentPlane,
    MissingCarryMode,
    MissingSurfaceMode,
    MissingProxy,
    MissingFixedView,
}
impl From<WorldInputError> for GroundCameraError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<PlayerStorageError> for GroundCameraError {
    fn from(error: PlayerStorageError) -> Self {
        Self::Storage(error)
    }
}

fn ground_mut<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut PlayerCameraGround, GroundCameraError> {
    world
        .player_mut(objects, owner)?
        .camera_ground
        .as_mut()
        .ok_or(GroundCameraError::MissingGround(owner))
}

fn pitch(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<u16, GroundCameraError> {
    let records = world.player(objects, owner)?;
    let angles = records
        .camera_angles
        .ok_or(GroundCameraError::MissingAngles(owner))?;
    let rotation = records
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .stored_rotation;
    Ok(angles.capture(rotation).pitch)
}

fn set_pitch(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    value: u16,
) -> Result<(), GroundCameraError> {
    let records = world.player_mut(objects, owner)?;
    let angles = records
        .camera_angles
        .as_mut()
        .ok_or(GroundCameraError::MissingAngles(owner))?;
    let auxiliary = records
        .auxiliary
        .as_mut()
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    let mut rotation = angles.capture(auxiliary.stored_rotation);
    rotation.pitch = value;
    angles.write(&mut auxiliary.stored_rotation, rotation);
    Ok(())
}

fn approach_height(current: i16, target: i16) -> i16 {
    let difference = current.wrapping_sub(target);
    if difference == 0 {
        current
    } else if difference < 0 {
        let next = current.wrapping_add(HEIGHT_RESPONSE);
        if next.wrapping_sub(target) < 0 {
            next
        } else {
            target
        }
    } else {
        let next = current.wrapping_sub(HEIGHT_RESPONSE);
        if next.wrapping_sub(target) >= 0 {
            next
        } else {
            target
        }
    }
}

/// The aim helper intentionally uses only signed bytes of its offset input.
/// Flight copies the actual player pose and suppresses forward offset; Walker
/// uses retained return X/Z, carried target height and canonical fine yaw.
pub fn aim_target(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    runtime: &mut PathRuntime,
    owner: ObjectId,
    lateral: i8,
    vertical: i8,
    forward: i8,
) -> Result<CameraAim, GroundCameraError> {
    let family = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode
        & FAMILY_MASK;
    let proxy = world
        .weapons
        .as_ref()
        .and_then(|weapons| weapons.fallback)
        .ok_or(GroundCameraError::MissingProxy)?;
    let forward = if family == WALKER_FAMILY {
        let records = world.player(objects, owner)?;
        let returned = records
            .boundary
            .ok_or(GroundCameraError::MissingBoundary(owner))?
            .return_position;
        let height = records
            .camera_ground
            .ok_or(GroundCameraError::MissingGround(owner))?
            .carried_target_height;
        objects
            .get_mut(proxy)
            .ok_or(WorldInputError::MissingActor(proxy))?
            .base
            .position = Vector3 {
            x: returned.x,
            y: height,
            z: returned.z,
        };
        let yaw =
            (player_storage::get(objects, &runtime.resources, owner)?.fine_yaw >> FINE_SHIFT) as u8;
        objects.get_mut(proxy).unwrap().base.yaw = Angle::from_units(yaw);
        forward
    } else {
        let actor = objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?;
        let position = actor.base.position;
        let rotation = (actor.base.pitch, actor.base.yaw, actor.base.roll);
        let target = objects
            .get_mut(proxy)
            .ok_or(WorldInputError::MissingActor(proxy))?;
        target.base.position = position;
        (target.base.pitch, target.base.yaw, target.base.roll) = rotation;
        0
    };
    let target = objects.get_mut(proxy).unwrap();
    let (x, z) = rotate_8xz(target.base.yaw.units(), lateral, forward);
    target.base.position.x = target.base.position.x.wrapping_add(x);
    target.base.position.z = target.base.position.z.wrapping_add(z);
    target.base.position.y = target.base.position.y.wrapping_add(i16::from(vertical));
    let target = target.base.position;
    runtime.steering.unchanged_axes = 0;
    let view = world.fixed_players[0].ok_or(GroundCameraError::MissingFixedView)?;
    let origin = objects
        .get(view)
        .ok_or(WorldInputError::MissingActor(view))?
        .base
        .position;
    let dx = target.x.wrapping_sub(origin.x);
    let dy = target.y.wrapping_sub(origin.y);
    let dz = target.z.wrapping_sub(origin.z);
    Ok(CameraAim {
        pitch: sf2_atan16(dy, sf2_xz_angle_distance(dx, dz)).wrapping_neg(),
        yaw: sf2_atan16(dx, dz),
    })
}

pub fn advance_pitch(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &mut PathRuntime,
    owner: ObjectId,
) -> Result<(), GroundCameraError> {
    let protection = world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .hold_secondary_protection;
    if protection || ground_mut(objects, world, owner)?.hold_pitch {
        ground_mut(objects, world, owner)?.animation_pitch = 0;
        set_pitch(objects, world, owner, 0)?;
        return Ok(());
    }
    let plane_follow = ground_mut(objects, world, owner)?.follow_environment_plane;
    let recovery_blocked = if plane_follow {
        false
    } else {
        world
            .player(objects, owner)?
            .consumable
            .ok_or(GroundCameraError::MissingConsumable(owner))?
            .recovery_blocked
    };
    let mut height = if plane_follow || recovery_blocked {
        let plane = world
            .environment_plane_height
            .ok_or(GroundCameraError::MissingEnvironmentPlane)?;
        let actor_height = objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .base
            .position
            .y;
        let offset = ground_mut(objects, world, owner)?.height_offset;
        plane
            .wrapping_sub(PLANE_MARGIN)
            .wrapping_sub(actor_height)
            .wrapping_add(offset)
            .min(0)
    } else {
        let state = ground_mut(objects, world, owner)?;
        state.height_offset = approach_height(
            state.height_offset,
            state
                .height_target_adjustment
                .wrapping_sub(HEIGHT_TARGET_MARGIN),
        );
        let offset = state.height_offset;
        let pitch_height = world
            .player(objects, owner)?
            .view_distance
            .ok_or(GroundCameraError::MissingDistance(owner))?
            .pitch_height_offset;
        offset.wrapping_sub(pitch_height)
    };
    let auxiliary = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    if auxiliary.mode & FAMILY_MASK != FLIGHT_FAMILY {
        let carry = world
            .player_carry_mode
            .ok_or(GroundCameraError::MissingCarryMode)?;
        if carry == CARRY_ENABLED
            && objects
                .get(owner)
                .ok_or(WorldInputError::MissingActor(owner))?
                .extension
                .path_state
                .motion
                .carry_selected_player
        {
            let extra = auxiliary
                .stored_world_position
                .y
                .wrapping_sub(ground_mut(objects, world, owner)?.carried_target_height);
            if extra >= 0 {
                height = height.wrapping_add(extra);
            }
        }
    }
    if world
        .player(objects, owner)?
        .charge
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?
        .linked_mode
    {
        height = height.wrapping_sub(LINKED_HEIGHT_MARGIN);
    }
    let target = aim_target(objects, world, runtime, owner, 0, height as i8, AIM_FORWARD)?.pitch;
    let high = ((target >> FINE_SHIFT) as i8).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    let target = target & FRACTION_MASK | (u16::from(high as u8) << FINE_SHIFT);
    let special = world
        .surface_mode
        .ok_or(GroundCameraError::MissingSurfaceMode)?
        .flags
        & SURFACE_MODE_MASK
        == SPECIAL_SURFACE_MODE
        && world
            .player(objects, owner)?
            .surface
            .ok_or(GroundCameraError::MissingSurface(owner))?
            .material
            == SPECIAL_MATERIAL;
    let should_follow = if special {
        !world
            .player(objects, owner)?
            .consumable
            .ok_or(GroundCameraError::MissingConsumable(owner))?
            .recovery_blocked
            || (target as i16) < 0
    } else {
        world
            .player(objects, owner)?
            .motion
            .ok_or(GroundCameraError::MissingMotion(owner))?
            .walker_contact_control
            & WALKER_PITCH_FOLLOW
            != 0
            || objects
                .get(owner)
                .ok_or(WorldInputError::MissingActor(owner))?
                .base
                .flags
                .standing_on_surface
    };
    if should_follow {
        let current = pitch(objects, world, owner)?;
        set_pitch(objects, world, owner, half_word(current, target))?;
    }
    let stance = world
        .player(objects, owner)?
        .motion
        .ok_or(GroundCameraError::MissingMotion(owner))?
        .walker_contact_control
        & WALKER_STANCE_MASK;
    let actor = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    let animation_pitch = if stance < STANDING_STANCE_LIMIT && actor.base.flags.standing_on_surface
    {
        let frame = actor.extension.path_state.animation.shape.packed() & FRAME_MASK;
        let target = i16::from(FRAME_PITCH_BIAS[usize::from(frame)]);
        let current = ground_mut(objects, world, owner)?.animation_pitch;
        let difference = current.wrapping_sub(target);
        let step = if difference < 0 {
            difference.min(-FRAME_RESPONSE_DIVISOR) / FRAME_RESPONSE_DIVISOR
        } else if difference > 0 {
            difference.max(FRAME_RESPONSE_DIVISOR) / FRAME_RESPONSE_DIVISOR
        } else {
            0
        };
        // This inline blend is target + (current - target)/8, NOT the usual
        // current + (target - current)/8 chase used by camera height/offsets.
        target.wrapping_add(step)
    } else {
        0
    };
    ground_mut(objects, world, owner)?.animation_pitch = animation_pitch;
    let current = pitch(objects, world, owner)?;
    set_pitch(
        objects,
        world,
        owner,
        current.wrapping_add(animation_pitch as u16),
    )
}
