//! Map-installed auxiliary camera tasks ($07:9DF6..A325). These run after
//! ordinary camera publication and share the fixed view's real path/history
//! fields. They do not integrate the velocity they copy from player motion.

use super::path_runtime::PathRuntime;
use super::path_scene_state::CameraTrackingTarget;
use super::player_steering::half_word;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::view_blend::ViewBlendControl;
use super::view_transition::FixedViewAngles;
use super::{Angle, ObjectId, ObjectStore, Vector3};
use sf_core::aim_angle::{sf2_atan16, sf2_xz_angle_distance};
use sf_core::snes_trig::{rotate_16xz, rotate_8xz};

const FINE_SHIFT: u32 = 8;
const FAMILY_MASK: u8 = 0xF0;
const FLIGHT_FAMILY: u8 = 0x10;
const SURFACE_KIND_MASK: u8 = 0x07;
const FIXED_ORBIT_CONFIGURATION: u8 = 9;
const FLIGHT_FORWARD: i16 = -200;
const FLIGHT_HEIGHT: i16 = -50;
const SURFACE_FORWARD: i16 = -160;
const SURFACE_HEIGHT: i16 = -40;
const INITIAL_RETREAT: i16 = -80;
const RETREAT_STEP: i16 = 5;
const PLANE_CEILING: i16 = -16;
const INITIAL_PITCH: i16 = -2560;
const FOCUS_TURN_DELAY: u8 = 15;
const EARLY_TIMER_MASK: u16 = 0xFFFE;
const EARLY_FOCUS_HEIGHT: i16 = 20;
const EARLY_FOCUS_FORWARD: i16 = -120;
const LATE_FOCUS_FORWARD: i16 = -150;
const HANDOFF_FORWARD: i8 = 60;
const HANDOFF_HEIGHT: i16 = -240;
const HANDOFF_HORIZONTAL_SCALE: i16 = 16;
const HANDOFF_CHASE_DIVISOR: i16 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbitStyle {
    /// Orbit the published player position while turning toward scene focus.
    EncounterFocus,
    /// Fixed distance/height, independent of the player's flight family.
    FlightDistance,
    /// Flight/ground-specific distance and height.
    ModeDistance,
    /// Increasing retreat in flight; ground still uses its fixed distance.
    Retreat,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AuxiliaryCameraTask {
    #[default]
    None,
    Handoff,
    Initialize(OrbitStyle),
    Orbit(OrbitStyle),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCameraAuxiliary {
    /// Typed task selector (6A9D/F), installed by scene control.
    pub task: AuxiliaryCameraTask,
    /// Retained retreat distance (6B54), including in non-flight modes.
    pub retreat_distance: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuxiliaryCameraError {
    World(WorldInputError),
    MissingState(ObjectId),
    MissingFixedView,
    MissingProxy,
    MissingTrackingTarget,
    MissingMotion(ObjectId),
    MissingConfiguration,
    MissingSurfaceMode,
    MissingHandoff,
    MissingFocus,
    MissingPublishedMotion,
    MissingEncounterTimer,
}
impl From<WorldInputError> for AuxiliaryCameraError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn state_mut<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut PlayerCameraAuxiliary, AuxiliaryCameraError> {
    world
        .player_mut(objects, owner)?
        .camera_auxiliary
        .as_mut()
        .ok_or(AuxiliaryCameraError::MissingState(owner))
}
fn view_id(world: &ScenePathWorld) -> Result<ObjectId, AuxiliaryCameraError> {
    world.fixed_players[0].ok_or(AuxiliaryCameraError::MissingFixedView)
}
fn proxy_id(world: &ScenePathWorld) -> Result<ObjectId, AuxiliaryCameraError> {
    world
        .weapons
        .and_then(|weapons| weapons.fallback)
        .ok_or(AuxiliaryCameraError::MissingProxy)
}

/// The six map installers ($0D:C75C..C831). Clearing also requests both
/// continuity captures; the handoff installer alone leaves them untouched.
pub fn install(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    task: AuxiliaryCameraTask,
) -> Result<(), AuxiliaryCameraError> {
    state_mut(objects, world, owner)?.task = task;
    if task != AuxiliaryCameraTask::Handoff {
        let view = view_id(world)?;
        let view = objects
            .get_mut(view)
            .ok_or(WorldInputError::MissingActor(view))?;
        let mut control = ViewBlendControl::capture(view);
        control.capture_position = true;
        control.capture_rotation = true;
        control.write_to(view);
    }
    Ok(())
}

fn initialize(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    style: OrbitStyle,
) -> Result<(), AuxiliaryCameraError> {
    if style == OrbitStyle::Retreat {
        state_mut(objects, world, owner)?.retreat_distance = INITIAL_RETREAT;
    }
    world.camera_tracking = Some(CameraTrackingTarget { actor: Some(owner) });
    let view = view_id(world)?;
    let view = objects
        .get_mut(view)
        .ok_or(WorldInputError::MissingActor(view))?;
    view.extension.path_state.script_parameter = FixedViewAngles::capture(view)
        .heading()
        .units()
        .wrapping_neg();
    view.extension.path_state.script_value = 0;
    state_mut(objects, world, owner)?.task = AuxiliaryCameraTask::Orbit(style);
    Ok(())
}

fn copy_velocity(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<(), AuxiliaryCameraError> {
    let motion = world
        .player(objects, owner)?
        .flight_displacement
        .ok_or(AuxiliaryCameraError::MissingMotion(owner))?;
    let view = view_id(world)?;
    objects
        .get_mut(view)
        .ok_or(WorldInputError::MissingActor(view))?
        .base
        .velocity = motion;
    Ok(())
}

/// Restore the complete previous orientation before aiming. The pitch's
/// preliminary half is an arithmetic shift; the subsequent chase rounds
/// toward zero and guarantees progress. These are distinct operations.
fn aim(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    runtime: &mut PathRuntime,
) -> Result<(), AuxiliaryCameraError> {
    let view_id = view_id(world)?;
    let view = objects
        .get_mut(view_id)
        .ok_or(WorldInputError::MissingActor(view_id))?;
    let previous = view.extension.path_state.motion_delta;
    let angles = FixedViewAngles {
        pitch: previous.x as u16,
        yaw: previous.y as u16,
        roll: previous.z as u16,
    };
    angles.write_to(view);
    let origin = view.base.position;
    let target = world
        .camera_tracking
        .and_then(|tracking| tracking.actor)
        .ok_or(AuxiliaryCameraError::MissingTrackingTarget)?;
    runtime.steering.unchanged_axes = 0;
    let target = objects
        .get(target)
        .ok_or(WorldInputError::MissingActor(target))?
        .base
        .position;
    let dx = target.x.wrapping_sub(origin.x);
    let dy = target.y.wrapping_sub(origin.y);
    let dz = target.z.wrapping_sub(origin.z);
    let pitch = sf2_atan16(dy, sf2_xz_angle_distance(dx, dz)).wrapping_neg() as i16 >> 1;
    FixedViewAngles {
        pitch: half_word(angles.pitch, pitch as u16),
        yaw: half_word(angles.yaw, sf2_atan16(dx, dz)),
        roll: 0,
    }
    .write_to(objects.get_mut(view_id).expect("validated fixed view"));
    Ok(())
}

fn orbit_position(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    style: OrbitStyle,
) -> Result<(), AuxiliaryCameraError> {
    let mut forward = if style == OrbitStyle::Retreat {
        let state = state_mut(objects, world, owner)?;
        state.retreat_distance = state.retreat_distance.wrapping_sub(RETREAT_STEP);
        state.retreat_distance
    } else {
        FLIGHT_FORWARD
    };
    let mut vertical = FLIGHT_HEIGHT;
    if style != OrbitStyle::FlightDistance {
        let mode = world
            .player(objects, owner)?
            .auxiliary
            .ok_or(WorldInputError::MissingAuxiliary(owner))?
            .mode;
        if mode & FAMILY_MASK != FLIGHT_FAMILY {
            forward = SURFACE_FORWARD;
            vertical = SURFACE_HEIGHT;
        }
    }
    let proxy = proxy_id(world)?;
    let view_id = view_id(world)?;
    let configuration = world
        .scene
        .player_configuration
        .ok_or(AuxiliaryCameraError::MissingConfiguration)?;
    let view = objects
        .get_mut(view_id)
        .ok_or(WorldInputError::MissingActor(view_id))?;
    if configuration != FIXED_ORBIT_CONFIGURATION {
        view.extension.path_state.script_parameter =
            view.extension.path_state.script_parameter.wrapping_sub(1);
    }
    let yaw = view.extension.path_state.script_parameter;
    let target = objects
        .get_mut(proxy)
        .ok_or(WorldInputError::MissingActor(proxy))?;
    target.base.yaw = Angle::from_units(yaw);
    target.base.pitch = Angle::ZERO;
    let (x, z) = rotate_16xz(yaw, 0, forward);
    let origin = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .position;
    let position = Vector3 {
        x: origin.x.wrapping_add(x),
        y: origin.y.wrapping_add(vertical),
        z: origin.z.wrapping_add(z),
    };
    objects
        .get_mut(proxy)
        .expect("validated proxy")
        .base
        .position = position;
    let view = objects.get_mut(view_id).expect("validated fixed view");
    view.base.position = position;
    let mut angles = FixedViewAngles::capture(view);
    angles.pitch = INITIAL_PITCH as u16;
    angles.write_to(view);
    let surface = world
        .surface_mode
        .ok_or(AuxiliaryCameraError::MissingSurfaceMode)?;
    if surface.flags & SURFACE_KIND_MASK != 0
        && view.base.position.y.wrapping_sub(PLANE_CEILING) >= 0
    {
        view.base.position.y = PLANE_CEILING;
    }
    Ok(())
}

fn focus_position(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<(), AuxiliaryCameraError> {
    let proxy_id = proxy_id(world)?;
    let focus = world
        .camera_focus
        .ok_or(AuxiliaryCameraError::MissingFocus)?
        .position;
    objects
        .get_mut(proxy_id)
        .ok_or(WorldInputError::MissingActor(proxy_id))?
        .base
        .position = focus;
    let origin = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .position;
    let desired = ((sf2_atan16(
        focus.x.wrapping_sub(origin.x),
        focus.z.wrapping_sub(origin.z),
    ) >> FINE_SHIFT) as u8)
        .wrapping_neg();
    objects.get_mut(proxy_id).expect("validated proxy").base.yaw = Angle::from_units(desired);
    let view_id = view_id(world)?;
    let view = objects
        .get_mut(view_id)
        .ok_or(WorldInputError::MissingActor(view_id))?;
    let orbit = &mut view.extension.path_state.script_parameter;
    if ((view.extension.path_state.script_value as u8).wrapping_sub(FOCUS_TURN_DELAY) as i8) >= 0 {
        let difference = orbit.wrapping_sub(desired) as i8;
        if difference < 0 {
            *orbit = orbit.wrapping_add(1);
        } else if difference > 0 {
            *orbit = orbit.wrapping_sub(1);
        }
    }
    let yaw = *orbit;
    let proxy = objects.get_mut(proxy_id).expect("validated proxy");
    proxy.base.yaw = Angle::from_units(yaw);
    proxy.base.pitch = Angle::ZERO;
    proxy.base.roll = Angle::ZERO;
    let published = world
        .published_motion
        .ok_or(AuxiliaryCameraError::MissingPublishedMotion)?
        .position;
    proxy.base.position = published;
    let timer = world
        .encounter_timer_steps
        .ok_or(AuxiliaryCameraError::MissingEncounterTimer)?;
    let (vertical, forward) = if timer & EARLY_TIMER_MASK == 0 {
        (EARLY_FOCUS_HEIGHT, EARLY_FOCUS_FORWARD)
    } else {
        (FLIGHT_HEIGHT, LATE_FOCUS_FORWARD)
    };
    let (x, z) = rotate_16xz(yaw, 0, forward);
    objects
        .get_mut(view_id)
        .expect("validated fixed view")
        .base
        .position = Vector3 {
        x: published.x.wrapping_add(x),
        y: published.y.wrapping_add(vertical),
        z: published.z.wrapping_add(z),
    };
    Ok(())
}

fn slow_chase(current: i16, target: i16) -> i16 {
    let difference = target.wrapping_sub(current);
    let step = if difference < 0 {
        difference.min(-HANDOFF_CHASE_DIVISOR) / HANDOFF_CHASE_DIVISOR
    } else if difference > 0 {
        difference.max(HANDOFF_CHASE_DIVISOR) / HANDOFF_CHASE_DIVISOR
    } else {
        0
    };
    current.wrapping_add(step)
}

fn handoff(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &mut PathRuntime,
) -> Result<(), AuxiliaryCameraError> {
    let proxy_id = proxy_id(world)?;
    let handoff = world.handoff.ok_or(AuxiliaryCameraError::MissingHandoff)?;
    let proxy = objects
        .get_mut(proxy_id)
        .ok_or(WorldInputError::MissingActor(proxy_id))?;
    proxy.base.position = Vector3 {
        x: handoff.x,
        y: FLIGHT_HEIGHT,
        z: handoff.z,
    };
    let mut angles = FixedViewAngles::capture(proxy);
    angles.yaw = handoff.heading_word;
    angles.write_to(proxy);
    let (x, z) = rotate_8xz(proxy.base.yaw.units(), 0, HANDOFF_FORWARD);
    proxy.base.position.x = proxy.base.position.x.wrapping_add(x);
    proxy.base.position.z = proxy.base.position.z.wrapping_add(z);
    world.camera_tracking = Some(CameraTrackingTarget {
        actor: Some(proxy_id),
    });
    aim(objects, world, runtime)?;
    // Position targets the handoff anchor, not the already-offset aim proxy.
    let proxy = objects.get_mut(proxy_id).expect("validated proxy");
    proxy.base.position = Vector3 {
        x: handoff.x,
        y: 0,
        z: handoff.z,
    };
    proxy.base.yaw = Angle::from_units(handoff.heading_word as u8);
    proxy.base.pitch = Angle::ZERO;
    proxy.base.roll = Angle::ZERO;
    let (x, z) = rotate_8xz(proxy.base.yaw.units(), 0, HANDOFF_FORWARD);
    let target = Vector3 {
        x: handoff
            .x
            .wrapping_add(x.wrapping_mul(HANDOFF_HORIZONTAL_SCALE)),
        y: HANDOFF_HEIGHT,
        z: handoff
            .z
            .wrapping_add(z.wrapping_mul(HANDOFF_HORIZONTAL_SCALE)),
    };
    let view_id = view_id(world)?;
    let view = objects
        .get_mut(view_id)
        .ok_or(WorldInputError::MissingActor(view_id))?;
    let previous = view.extension.path_state.platform_carry.saved_position;
    view.base.position = Vector3 {
        x: slow_chase(previous.x, target.x),
        y: slow_chase(previous.y, target.y),
        z: slow_chase(previous.z, target.z),
    };
    let mut angles = FixedViewAngles::capture(view);
    angles.pitch = ((angles.pitch as i16) >> 1) as u16;
    angles.write_to(view);
    Ok(())
}

/// Complete footer, including the word counter increment even when no task
/// is installed. Initialization resets that counter before its first update.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &mut PathRuntime,
    owner: ObjectId,
) -> Result<(), AuxiliaryCameraError> {
    let view = view_id(world)?;
    let view = objects
        .get_mut(view)
        .ok_or(WorldInputError::MissingActor(view))?;
    view.extension.path_state.script_value = view.extension.path_state.script_value.wrapping_add(1);
    let task = state_mut(objects, world, owner)?.task;
    let style = match task {
        AuxiliaryCameraTask::None => return Ok(()),
        AuxiliaryCameraTask::Handoff => return handoff(objects, world, runtime),
        AuxiliaryCameraTask::Initialize(style) => {
            initialize(objects, world, owner, style)?;
            style
        }
        AuxiliaryCameraTask::Orbit(style) => style,
    };
    if style == OrbitStyle::EncounterFocus {
        copy_velocity(objects, world, owner)?;
        focus_position(objects, world, owner)?;
        aim(objects, world, runtime)?;
    } else {
        orbit_position(objects, world, owner, style)?;
        aim(objects, world, runtime)?;
        copy_velocity(objects, world, owner)?;
    }
    Ok(())
}
