//! Surface-camera placement ($07:812C..81F0) and its full retained height
//! controller ($07:90A3..9487). Ground protection/plane flags keep their
//! existing owners; only the additional return transition is owned here.

use super::path_fields::chase_word;
use super::path_runtime::PathRuntime;
use super::player_camera_position::{self, CameraPositionError};
use super::player_camera_tracking::{self, CameraTrackingError, TrackingStyle};
use super::player_steering::half_word;
use super::player_storage::{self, PlayerStorageError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::view_transition::FixedViewAngles;
use super::{ObjectId, ObjectStore};

const CARRY_ENABLED: u8 = 1;
const FAMILY_MASK: u8 = 0xF0;
const FLIGHT_FAMILY: u8 = 0x10;
const SURFACE_MODE_MASK: u8 = 7;
const SPECIAL_SURFACE_MODE: u8 = 2;
const STANCE_MASK: u8 = 7;
const PLANE_MARGIN: i16 = 16;
const LINKED_CLEARANCE: i16 = 35;
const SUPPORT_CLEARANCE: i16 = 100;
const STANCE_CLEARANCE: i16 = 25;
const QUERY_CLEARANCE: i16 = 70;
const SPECIAL_CONFIGURATION: u8 = 9;
const SPECIAL_HEIGHT_LIMIT: i16 = -250;
const FAST_PLANE_STEP: i16 = 20;
const PLANE_STEP: i16 = 4;
const YAW_STEP: i16 = 256;
const HEIGHT_CORRECTION_LIMIT: i16 = 50;
const QUARTER_DIVISOR: i16 = 4;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCameraSurface {
    /// 6B7D bit 20: crossing back below the environment plane. Bits 80/40
    /// live in contact/consumable; bits 10/08 in the ground-camera owner.
    pub returning_below_plane: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceCameraError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    Position(CameraPositionError),
    Tracking(CameraTrackingError),
    MissingSurfaceCamera(ObjectId),
    MissingGround(ObjectId),
    MissingAngles(ObjectId),
    MissingConsumable(ObjectId),
    MissingSurface(ObjectId),
    MissingMotion(ObjectId),
    MissingBoundary(ObjectId),
    MissingTracking(ObjectId),
    MissingCarryMode,
    MissingSurfaceMode,
    MissingEnvironmentPlane,
    MissingConfiguration,
}
impl From<WorldInputError> for SurfaceCameraError {
    fn from(value: WorldInputError) -> Self {
        Self::World(value)
    }
}
impl From<PlayerStorageError> for SurfaceCameraError {
    fn from(value: PlayerStorageError) -> Self {
        Self::Storage(value)
    }
}
impl From<CameraPositionError> for SurfaceCameraError {
    fn from(value: CameraPositionError) -> Self {
        Self::Position(value)
    }
}
impl From<CameraTrackingError> for SurfaceCameraError {
    fn from(value: CameraTrackingError) -> Self {
        Self::Tracking(value)
    }
}

fn height(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<i16, SurfaceCameraError> {
    Ok(world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .stored_world_position
        .y)
}
fn set_height(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    height: i16,
) -> Result<(), SurfaceCameraError> {
    world
        .player_mut(objects, owner)?
        .auxiliary
        .as_mut()
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .stored_world_position
        .y = height;
    Ok(())
}
fn carried_height(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<i16, SurfaceCameraError> {
    Ok(world
        .player(objects, owner)?
        .camera_ground
        .ok_or(SurfaceCameraError::MissingGround(owner))?
        .carried_target_height)
}
fn environment(world: &ScenePathWorld) -> Result<i16, SurfaceCameraError> {
    world
        .environment_plane_height
        .ok_or(SurfaceCameraError::MissingEnvironmentPlane)
}
fn carried(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<bool, SurfaceCameraError> {
    Ok(world
        .player_carry_mode
        .ok_or(SurfaceCameraError::MissingCarryMode)?
        == CARRY_ENABLED
        && objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .extension
            .path_state
            .motion
            .carry_selected_player)
}
fn support(objects: &ObjectStore, owner: ObjectId) -> Result<bool, SurfaceCameraError> {
    Ok(objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .extension
        .surface_contact
        .supporting_object
        .is_some())
}
fn quarter(current: i16, target: i16) -> i16 {
    let difference = target.wrapping_sub(current);
    let step = if difference < 0 {
        difference.min(-QUARTER_DIVISOR) / QUARTER_DIVISOR
    } else if difference > 0 {
        difference.max(QUARTER_DIVISOR) / QUARTER_DIVISOR
    } else {
        0
    };
    current.wrapping_add(step)
}
fn approach(current: i16, target: i16, step: i16) -> i16 {
    match current.wrapping_sub(target) {
        0 => current,
        difference if difference < 0 => {
            let next = current.wrapping_add(step);
            if next.wrapping_sub(target) < 0 {
                next
            } else {
                target
            }
        }
        _ => {
            let next = current.wrapping_sub(step);
            if next.wrapping_sub(target) >= 0 {
                next
            } else {
                target
            }
        }
    }
}
fn clear_plane_follow(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), SurfaceCameraError> {
    let records = world.player_mut(objects, owner)?;
    let ground = records
        .camera_ground
        .as_mut()
        .ok_or(SurfaceCameraError::MissingGround(owner))?;
    ground.hold_pitch = false;
    ground.follow_environment_plane = false;
    records
        .consumable
        .as_mut()
        .ok_or(SurfaceCameraError::MissingConsumable(owner))?
        .recovery_blocked = false;
    Ok(())
}

/// Bounded half-chase helper ($06:B9BA). First bring the current value
/// within fifty of the target, then half-chase. This is not a speed clamp.
fn bounded_half(current: i16, target: i16) -> i16 {
    let difference = target.wrapping_sub(current);
    let current = if difference >= HEIGHT_CORRECTION_LIMIT {
        target.wrapping_sub(HEIGHT_CORRECTION_LIMIT)
    } else if difference < -HEIGHT_CORRECTION_LIMIT {
        target.wrapping_add(HEIGHT_CORRECTION_LIMIT)
    } else {
        current
    };
    half_word(current as u16, target as u16) as i16
}
fn finish_height(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    target: i16,
) -> Result<(), SurfaceCameraError> {
    let current = height(objects, world, owner)?;
    let target = bounded_half(current, target);
    let charge = world
        .player(objects, owner)?
        .charge
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
    let value = if charge.linked_mode && !charge.linked_muzzle_disabled {
        target
    } else {
        chase_word(current as u16, target as u16) as i16
    };
    set_height(objects, world, owner, value)
}
fn query_height(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<i16, SurfaceCameraError> {
    let records = world.player(objects, owner)?;
    let query = records
        .motion
        .ok_or(SurfaceCameraError::MissingMotion(owner))?
        .surface_height;
    let plane = records
        .surface
        .ok_or(SurfaceCameraError::MissingSurface(owner))?
        .plane_height;
    let mut target = query.wrapping_add(plane);
    if support(objects, owner)? {
        target = target.wrapping_sub(SUPPORT_CLEARANCE);
        if world
            .scene
            .player_configuration
            .ok_or(SurfaceCameraError::MissingConfiguration)?
            == SPECIAL_CONFIGURATION
            && SPECIAL_HEIGHT_LIMIT.wrapping_sub(target) >= 0
        {
            target = SPECIAL_HEIGHT_LIMIT;
        }
    }
    Ok(target.wrapping_sub(QUERY_CLEARANCE))
}
fn cross_below_plane(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), SurfaceCameraError> {
    let actor_height = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .position
        .y;
    world
        .player_mut(objects, owner)?
        .camera_tracking
        .as_mut()
        .ok_or(SurfaceCameraError::MissingTracking(owner))?
        .anchor_height = actor_height;
    let target = environment(world)?.wrapping_sub(PLANE_MARGIN);
    let current = height(objects, world, owner)?;
    if current != target {
        return set_height(objects, world, owner, approach(current, target, PLANE_STEP));
    }
    let records = world.player_mut(objects, owner)?;
    records
        .camera_surface
        .as_mut()
        .ok_or(SurfaceCameraError::MissingSurfaceCamera(owner))?
        .returning_below_plane = false;
    records
        .contact
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .hold_secondary_protection = false;
    Ok(())
}

pub fn advance_height(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    style: TrackingStyle,
    auxiliary_camera: bool,
) -> Result<(), SurfaceCameraError> {
    let records = world.player(objects, owner)?;
    let protection = records
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .hold_secondary_protection;
    let recovery = records
        .consumable
        .ok_or(SurfaceCameraError::MissingConsumable(owner))?
        .recovery_blocked;
    let crossing = records
        .camera_surface
        .ok_or(SurfaceCameraError::MissingSurfaceCamera(owner))?
        .returning_below_plane;
    let ground = records
        .camera_ground
        .ok_or(SurfaceCameraError::MissingGround(owner))?;
    if !protection
        && !recovery
        && !crossing
        && !ground.hold_pitch
        && records
            .charge
            .ok_or(WorldInputError::MissingPlayerCharge(owner))?
            .linked_mode
    {
        let mut target = ground.carried_target_height.wrapping_sub(LINKED_CLEARANCE);
        if world
            .surface_mode
            .ok_or(SurfaceCameraError::MissingSurfaceMode)?
            .flags
            & SURFACE_MODE_MASK
            != SPECIAL_SURFACE_MODE
        {
            let plane = records
                .surface
                .ok_or(SurfaceCameraError::MissingSurface(owner))?
                .plane_height;
            if plane.wrapping_sub(target) < 0 {
                target = plane.wrapping_sub(LINKED_CLEARANCE);
            }
        }
        if objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .base
            .flags
            .standing_on_surface
        {
            clear_plane_follow(objects, world, owner)?;
        }
        return finish_height(objects, world, owner, target);
    }
    if ground.follow_environment_plane {
        let value = half_word(
            height(objects, world, owner)? as u16,
            ground.carried_target_height as u16,
        ) as i16;
        set_height(objects, world, owner, value)?;
        if objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .base
            .flags
            .standing_on_surface
        {
            clear_plane_follow(objects, world, owner)?;
        }
    }
    let records = world.player(objects, owner)?;
    let mut hold = records
        .camera_ground
        .ok_or(SurfaceCameraError::MissingGround(owner))?
        .hold_pitch;
    if !hold
        && records
            .consumable
            .ok_or(SurfaceCameraError::MissingConsumable(owner))?
            .recovery_blocked
    {
        if !carried(objects, world, owner)? {
            let query = world
                .player(objects, owner)?
                .motion
                .ok_or(SurfaceCameraError::MissingMotion(owner))?
                .surface_height;
            if query.wrapping_sub(environment(world)?) >= 0 {
                return Ok(());
            }
            return finish_height(objects, world, owner, query_height(objects, world, owner)?);
        }
        let target = environment(world)?.wrapping_sub(PLANE_MARGIN);
        let current = height(objects, world, owner)?;
        if current != target {
            return set_height(
                objects,
                world,
                owner,
                approach(current, target, FAST_PLANE_STEP),
            );
        }
        world
            .player_mut(objects, owner)?
            .camera_ground
            .as_mut()
            .unwrap()
            .hold_pitch = true;
        hold = true;
    }
    if hold {
        let target = environment(world)?.wrapping_add(PLANE_MARGIN);
        let current = height(objects, world, owner)?;
        if current != target {
            return set_height(objects, world, owner, approach(current, target, PLANE_STEP));
        }
        clear_plane_follow(objects, world, owner)?;
    }
    if world
        .player(objects, owner)?
        .camera_surface
        .ok_or(SurfaceCameraError::MissingSurfaceCamera(owner))?
        .returning_below_plane
    {
        return cross_below_plane(objects, world, owner);
    }
    let protection = world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?;
    if protection.hit.hold_secondary_protection {
        if protection.ignores_contacts {
            let value = quarter(
                height(objects, world, owner)?,
                carried_height(objects, world, owner)?,
            );
            return set_height(objects, world, owner, value);
        }
        let actor_height = objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .base
            .position
            .y;
        set_height(objects, world, owner, actor_height)?;
        let target = environment(world)?.wrapping_add(PLANE_MARGIN);
        if target.wrapping_sub(actor_height) >= 0 {
            set_height(objects, world, owner, target)?;
        }
        if carried(objects, world, owner)? {
            return Ok(());
        }
        if target.wrapping_sub(height(objects, world, owner)?) < 0 {
            return Ok(());
        }
        set_height(objects, world, owner, target)?;
        world
            .player_mut(objects, owner)?
            .camera_surface
            .as_mut()
            .unwrap()
            .returning_below_plane = true;
        return cross_below_plane(objects, world, owner);
    }
    let family = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode
        & FAMILY_MASK;
    if family == FLIGHT_FAMILY {
        let target = objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .base
            .position
            .y;
        let value = chase_word(height(objects, world, owner)? as u16, target as u16) as i16;
        return set_height(objects, world, owner, value);
    }
    let follow_carried;
    if carried(objects, world, owner)? {
        follow_carried = true;
    } else {
        let stance = world
            .player(objects, owner)?
            .motion
            .ok_or(SurfaceCameraError::MissingMotion(owner))?
            .walker_contact_control
            & STANCE_MASK;
        if stance >= 1 {
            let mut target = world
                .player(objects, owner)?
                .surface
                .ok_or(SurfaceCameraError::MissingSurface(owner))?
                .plane_height
                .wrapping_sub(STANCE_CLEARANCE);
            if support(objects, owner)? {
                target = target.wrapping_sub(SUPPORT_CLEARANCE);
            }
            return finish_height(objects, world, owner, target);
        }
        follow_carried = world
            .surface_mode
            .ok_or(SurfaceCameraError::MissingSurfaceMode)?
            .flags
            & SURFACE_MODE_MASK
            == SPECIAL_SURFACE_MODE
            && !support(objects, owner)?;
    }
    if follow_carried {
        let target = carried_height(objects, world, owner)?;
        let target = player_camera_tracking::advance_height(
            objects,
            world,
            owner,
            target,
            style,
            auxiliary_camera,
        )?;
        let value = quarter(height(objects, world, owner)?, target);
        return set_height(objects, world, owner, value);
    }
    finish_height(objects, world, owner, query_height(objects, world, owner)?)
}

/// The source surface caller writes yaw/roll before position; only retained
/// X/Z use the prepared vector. Height is owned by its separate controller.
pub fn prepare(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &PathRuntime,
    owner: ObjectId,
    style: TrackingStyle,
    auxiliary_camera: bool,
) -> Result<(), SurfaceCameraError> {
    let yaw = player_storage::get(objects, &runtime.resources, owner)?
        .fine_yaw
        .wrapping_neg();
    let records = world.player_mut(objects, owner)?;
    let camera = records
        .camera_angles
        .as_mut()
        .ok_or(SurfaceCameraError::MissingAngles(owner))?;
    let auxiliary = records
        .auxiliary
        .as_mut()
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    let mut angles: FixedViewAngles = camera.capture(auxiliary.stored_rotation);
    angles.yaw = yaw.wrapping_sub(camera.yaw_difference as u16);
    camera.write(&mut auxiliary.stored_rotation, angles);
    camera.yaw_difference = approach(camera.yaw_difference, 0, YAW_STEP);
    camera.yaw_offset = chase_word(camera.yaw_offset as u16, 0) as i16;
    angles.roll = half_word(angles.roll, 0);
    camera.write(&mut auxiliary.stored_rotation, angles);
    let returned = records
        .boundary
        .ok_or(SurfaceCameraError::MissingBoundary(owner))?
        .return_position;
    let prepared =
        player_camera_position::advance_distance(objects, world, owner, returned, style)?;
    let prepared = player_camera_position::advance_lateral(objects, world, owner, prepared)?;
    let auxiliary = world
        .player_mut(objects, owner)?
        .auxiliary
        .as_mut()
        .unwrap();
    auxiliary.stored_world_position.x = prepared.x;
    auxiliary.stored_world_position.z = prepared.z;
    advance_height(objects, world, owner, style, auxiliary_camera)
}
