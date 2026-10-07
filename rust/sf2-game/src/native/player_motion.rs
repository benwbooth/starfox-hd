//! Complete flight translation ($06:EE0A..F00F). Base-speed velocity,
//! thrust-plus-base displacement and retained collision sliding are distinct
//! source owners; neither the reserved aiming actor nor carried motion is
//! replaced by an unobservable temporary object.

use super::path_motion;
use super::player_storage::{self, PlayerStorageError};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::surface_motion::{
    self, SurfaceMotionError, SurfaceMotionInputs, SurfaceMotionResult, SurfaceTilt,
};
use super::{Angle, ObjectId, ObjectStore, Vector3};

#[cfg(test)]
#[path = "player_motion_tests.rs"]
mod tests;

const HORIZONTAL_X: u8 = 0x80;
const VERTICAL: u8 = 0x40;
const HORIZONTAL_Z: u8 = 0x20;
const ACTIVE_CARRY_MODE: u8 = 1;
const PROTECTED_SPEED: u8 = 20;
const PROTECTED_CLEARANCE: i16 = 300;
const PROTECTED_THRUST: i8 = 100;
const CONSTRAINED_CONFIGURATION: u8 = 9;
const SURFACE_OBSTRUCTION: u8 = 0x10;
const ORDINARY_VELOCITY_SCALE: i16 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerMotion {
    /// Last walker collision-query height (6AF7). Flight translation does
    /// not replace it; the free-flight surface prefix consumes it later.
    pub surface_height: i16,
    /// 6AC7/C9/CB, captured before the mode-specific player update.
    pub previous_position: Vector3,
    /// 6AAD, signed sideways impulse from contact turn and terrain response.
    pub lateral_impulse: i8,
    /// Retained horizontal response (6B11/13), not base velocity.
    pub surface_velocity: [i16; 2],
    /// Shared with map-cell/view coverage and contact feedback (6BE6).
    /// Movement sets bit 10 on obstruction and leaves every other bit,
    /// and prior events, intact.
    pub contact_flags: u8,
}

/// Scene publication (1D6F/71) copied into/out of the moving player's
/// actor contact only by the constrained branch. Contact material flags
/// are deliberately not part of this shared relationship.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerSurfaceSupport {
    pub object: Option<ObjectId>,
    pub group: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MotionContext {
    /// Required only when this configuration enters the surface service.
    pub inherited_surface_tilt: Option<SurfaceTilt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotionError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    Surface(SurfaceMotionError),
    MissingSpeed(ObjectId),
    MissingVertical(ObjectId),
    MissingMotion(ObjectId),
    MissingCarryMode,
    MissingEnvironmentPlane,
    MissingViewMode,
    MissingProxy,
    MissingSurfaceSupport,
    MissingSurfaceMode,
    MissingSurfaceTilt,
}

impl From<WorldInputError> for MotionError {
    fn from(value: WorldInputError) -> Self {
        Self::World(value)
    }
}
impl From<PlayerStorageError> for MotionError {
    fn from(value: PlayerStorageError) -> Self {
        Self::Storage(value)
    }
}
impl From<SurfaceMotionError> for MotionError {
    fn from(value: SurfaceMotionError) -> Self {
        Self::Surface(value)
    }
}

/// `$06:9E25..9E36`: caller-owned history, not the position after movement.
pub fn capture_position(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), MotionError> {
    let position = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .position;
    world
        .player_mut(objects, owner)?
        .motion
        .as_mut()
        .ok_or(MotionError::MissingMotion(owner))?
        .previous_position = position;
    Ok(())
}

/// Source helper $06:EEEF: publish the unfiltered result on the reserved
/// actor, then copy/filter the moving actor's axes. The proxy's speed and
/// pose writes precede the fine-pitch and movement-permission dependencies.
fn velocity(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    resources: &ProgramResources<ProgramData>,
    owner: ObjectId,
    speed: u8,
) -> Result<Vector3, MotionError> {
    let proxy = world
        .weapons
        .as_ref()
        .and_then(|weapons| weapons.fallback)
        .ok_or(MotionError::MissingProxy)?;
    objects
        .get_mut(proxy)
        .ok_or(WorldInputError::MissingActor(proxy))?
        .base
        .speed = speed;
    let actor = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    let (pitch, yaw, roll) = (actor.base.pitch, actor.base.yaw, actor.base.roll);
    let actor = objects.get_mut(proxy).expect("validated motion proxy");
    actor.base.pitch = pitch;
    actor.base.yaw = yaw;
    actor.base.roll = roll;
    let pitch = Angle::from_units(
        (player_storage::get(objects, resources, owner)?.fine_pitch >> u8::BITS) as u8,
    );
    let actor = objects.get_mut(proxy).expect("validated motion proxy");
    actor.base.pitch = pitch;
    actor.base.velocity =
        path_motion::direction_velocity(pitch, yaw, speed, ORDINARY_VELOCITY_SCALE);
    let generated = actor.base.velocity;
    objects
        .get_mut(owner)
        .expect("validated moving actor")
        .base
        .velocity = generated;
    let axes = world
        .player(objects, owner)?
        .vertical
        .ok_or(MotionError::MissingVertical(owner))?
        .motion_axes;
    let actor = objects.get_mut(owner).expect("validated moving actor");
    if axes & HORIZONTAL_X == 0 {
        actor.base.velocity.x = 0;
    }
    if axes & VERTICAL == 0 {
        actor.base.velocity.y = 0;
    }
    if axes & HORIZONTAL_Z == 0 {
        actor.base.velocity.z = 0;
    }
    Ok(actor.base.velocity)
}

pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &ProgramResources<ProgramData>,
    owner: ObjectId,
    context: MotionContext,
) -> Result<Option<SurfaceMotionResult>, MotionError> {
    let protected = world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .hold_secondary_protection;
    if protected {
        let carry_mode = world
            .player_carry_mode
            .ok_or(MotionError::MissingCarryMode)?;
        let actor = objects.get_mut(owner).expect("validated moving actor");
        if carry_mode != ACTIVE_CARRY_MODE
            || !actor.extension.path_state.motion.carry_selected_player
        {
            actor.base.speed = PROTECTED_SPEED;
        }
        let record = world.player_mut(objects, owner)?;
        record
            .vertical
            .as_mut()
            .ok_or(MotionError::MissingVertical(owner))?
            .motion_axes &= !(HORIZONTAL_X | HORIZONTAL_Z);
        record
            .speed
            .as_mut()
            .ok_or(MotionError::MissingSpeed(owner))?
            .thrust = 0;
        let threshold = world
            .environment_plane_height
            .ok_or(MotionError::MissingEnvironmentPlane)?
            .wrapping_add(PROTECTED_CLEARANCE);
        if threshold.wrapping_sub(
            objects
                .get(owner)
                .expect("validated moving actor")
                .base
                .position
                .y,
        ) < 0
        {
            world
                .player_mut(objects, owner)?
                .speed
                .as_mut()
                .expect("validated speed")
                .thrust = PROTECTED_THRUST;
        }
    }
    if world
        .scripted_view_active()
        .ok_or(MotionError::MissingViewMode)?
    {
        return Ok(None);
    }
    let thrust = world
        .player(objects, owner)?
        .speed
        .ok_or(MotionError::MissingSpeed(owner))?
        .thrust;
    let mut displacement = velocity(objects, world, resources, owner, thrust as u8)?;
    if thrust < 0 {
        displacement.y = 0;
    }
    world.player_mut(objects, owner)?.flight_displacement = Some(displacement);
    let speed = objects
        .get(owner)
        .expect("validated moving actor")
        .base
        .speed;
    let base_velocity = velocity(objects, world, resources, owner, speed)?;
    path_motion::integrate(&mut displacement, base_velocity);
    world.player_mut(objects, owner)?.flight_displacement = Some(displacement);
    let configuration = world
        .scene
        .player_configuration
        .ok_or(WorldInputError::MissingPlayerConfiguration)?;
    if configuration != CONSTRAINED_CONFIGURATION {
        let actor = objects.get_mut(owner).expect("validated moving actor");
        path_motion::integrate(&mut actor.base.position, displacement);
        actor.extension.path_state.motion_delta = Vector3::default();
        actor.extension.path_state.platform_carry.saved_position = Vector3::default();
        return Ok(None);
    }

    let actor = objects.get_mut(owner).expect("validated moving actor");
    actor.base.position.y = actor.base.position.y.wrapping_add(displacement.y);
    actor.extension.path_state.motion_delta.x = displacement.x;
    actor.extension.path_state.motion_delta.z = displacement.z;
    let sliding = world
        .player(objects, owner)?
        .motion
        .ok_or(MotionError::MissingMotion(owner))?
        .surface_velocity;
    let actor = objects.get_mut(owner).expect("validated moving actor");
    actor.base.velocity = Vector3 {
        x: sliding[0],
        y: 0,
        z: sliding[1],
    };
    let support = world
        .player_surface_support
        .ok_or(MotionError::MissingSurfaceSupport)?;
    actor.extension.surface_contact.supporting_object = support.object;
    actor.extension.surface_contact.group = support.group;
    let search = world
        .surface_mode
        .ok_or(MotionError::MissingSurfaceMode)?
        .search();
    let tilt = context
        .inherited_surface_tilt
        .ok_or(MotionError::MissingSurfaceTilt)?;
    let result = surface_motion::advance(
        objects,
        owner,
        SurfaceMotionInputs {
            search,
            strategy_tick: world.strategy_clock as u8,
            gravity: None,
            inherited_tilt: tilt,
        },
    )?;
    let actor = objects.get(owner).expect("validated moving actor");
    world.player_surface_support = Some(PlayerSurfaceSupport {
        object: actor.extension.surface_contact.supporting_object,
        group: actor.extension.surface_contact.group,
    });
    let sliding = [actor.base.velocity.x, actor.base.velocity.z];
    world
        .player_mut(objects, owner)?
        .motion
        .as_mut()
        .expect("validated motion")
        .surface_velocity = sliding;
    objects
        .get_mut(owner)
        .expect("validated moving actor")
        .base
        .velocity = base_velocity;
    if result.obstructed {
        world
            .player_mut(objects, owner)?
            .motion
            .as_mut()
            .expect("validated motion")
            .contact_flags |= SURFACE_OBSTRUCTION;
    }
    Ok(Some(result))
}
