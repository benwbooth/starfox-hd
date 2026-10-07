//! Complete player-camera dispatch ($07:8000..812B), including the retained
//! projection-height publication ($07:9489..950D). Continuity remains the next
//! enclosing-player call; an auxiliary task runs before that capture/decay.

use super::path_runtime::PathRuntime;
use super::player_camera_angles::{self, CameraAnglesError};
use super::player_camera_auxiliary::{self, AuxiliaryCameraError, AuxiliaryCameraTask};
use super::player_camera_common::{self, CommonCameraError};
use super::player_camera_surface::{self, SurfaceCameraError};
use super::player_camera_tracking::TrackingStyle;
use super::player_view_distance::{self, ViewDistanceError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::view_blend::{self, ViewBlendControl, ViewBlendError};
use super::{ObjectId, ObjectStore};

const HIDE_HORIZON: u8 = 0x04;
const ACTIVE_CARRY_MODE: u8 = 1;
const HEIGHT_MIDPOINT_DIVISOR: i16 = 2;
const HEIGHT_RESPONSE_SHIFT: u32 = 4;
const HEIGHT_RESPONSE_LIMIT: i16 = 160;
const PLANE_CLEARANCE: i16 = 10;
const CAMERA_INSTALL_PENDING: u8 = 0x08;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCameraDispatch {
    /// Source primary task selector (6A9A/C), absent after storage zeroing.
    pub style: Option<TrackingStyle>,
    /// Map-owned 6B65 bit 40, distinct from path-controlled bits 08 and 10.
    pub projection_correction_disabled: bool,
    /// Surface-camera 6B63 bit 02, sharing its byte with real linked/view flags.
    pub outside_occupied_world: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameraDispatchError {
    World(WorldInputError),
    Angles(CameraAnglesError),
    Common(CommonCameraError),
    Surface(SurfaceCameraError),
    Distance(ViewDistanceError),
    Auxiliary(AuxiliaryCameraError),
    Blend(ViewBlendError),
    MissingState(ObjectId),
    MissingAuxiliary(ObjectId),
    MissingModeSelection(ObjectId),
    MissingExemption(ObjectId),
    MissingViewMode,
    MissingActionGate,
    MissingCarryMode,
    MissingFixedView,
    MissingOccupancy,
    MissingHeightLimits,
    MissingEnvironmentPlane,
}
impl From<WorldInputError> for CameraDispatchError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<CameraAnglesError> for CameraDispatchError {
    fn from(error: CameraAnglesError) -> Self {
        Self::Angles(error)
    }
}
impl From<CommonCameraError> for CameraDispatchError {
    fn from(error: CommonCameraError) -> Self {
        Self::Common(error)
    }
}
impl From<SurfaceCameraError> for CameraDispatchError {
    fn from(error: SurfaceCameraError) -> Self {
        Self::Surface(error)
    }
}
impl From<ViewDistanceError> for CameraDispatchError {
    fn from(error: ViewDistanceError) -> Self {
        Self::Distance(error)
    }
}
impl From<AuxiliaryCameraError> for CameraDispatchError {
    fn from(error: AuxiliaryCameraError) -> Self {
        Self::Auxiliary(error)
    }
}

/// The actual enclosing player's paired camera calls ($06:9A26..9A2E).
/// Continuity still runs when the dispatcher intentionally bypasses its tasks.
pub fn advance_with_continuity(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &mut PathRuntime,
    owner: ObjectId,
) -> Result<(), CameraDispatchError> {
    advance(objects, world, runtime, owner)?;
    view_blend::advance(objects, world).map_err(CameraDispatchError::Blend)
}

/// Normal-flight plane limit ($07:9D36..9D79), called AFTER continuity.
/// The saved continuity position is deliberately not rewritten by this clamp.
pub fn clamp_normal_to_plane(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<(), CameraDispatchError> {
    if state(objects, world, owner)?.style != Some(TrackingStyle::Normal) {
        return Ok(());
    }
    let view = world.fixed_players[0].ok_or(CameraDispatchError::MissingFixedView)?;
    let plane = world
        .environment_plane_height
        .ok_or(CameraDispatchError::MissingEnvironmentPlane)?;
    let ceiling = plane.wrapping_sub(PLANE_CLEARANCE);
    let view = objects
        .get_mut(view)
        .ok_or(WorldInputError::MissingActor(view))?;
    if ceiling.wrapping_sub(view.base.position.y) < 0 {
        view.base.position.y = ceiling;
    }
    Ok(())
}

fn state(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<PlayerCameraDispatch, CameraDispatchError> {
    world
        .player(objects, owner)?
        .camera_dispatch
        .ok_or(CameraDispatchError::MissingState(owner))
}

/// The free-flight strategy's actual camera installer ($06:869C..8719).
/// Auxiliary cameras and secondary protection defer replacement. An already
/// normal task bypasses all installation work, including profile reset.
pub fn select_free_flight_camera(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), CameraDispatchError> {
    if state(objects, world, owner)?.style == Some(TrackingStyle::Normal) {
        return Ok(());
    }
    if world
        .player(objects, owner)?
        .camera_auxiliary
        .ok_or(CameraDispatchError::MissingAuxiliary(owner))?
        .task
        != AuxiliaryCameraTask::None
    {
        return Ok(());
    }
    if world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .hold_secondary_protection
    {
        return Ok(());
    }
    let view = world.fixed_players[0].ok_or(CameraDispatchError::MissingFixedView)?;
    let view = objects
        .get_mut(view)
        .ok_or(WorldInputError::MissingActor(view))?;
    let mut control = ViewBlendControl::capture(view);
    control.capture_position = true;
    control.capture_rotation = true;
    control.write_to(view);
    player_view_distance::initialize(objects, world, owner)?;
    world
        .player_mut(objects, owner)?
        .mode_selection
        .as_mut()
        .ok_or(CameraDispatchError::MissingModeSelection(owner))?
        .surface_control &= !CAMERA_INSTALL_PENDING;
    world
        .player_mut(objects, owner)?
        .camera_dispatch
        .as_mut()
        .ok_or(CameraDispatchError::MissingState(owner))?
        .style = Some(TrackingStyle::Normal);
    Ok(())
}

/// The map can inhibit this correction without resetting its previous value.
/// The midpoint rounds toward zero; the wrapped displacement divides with
/// signed floor before clamping. Do not interchange those roundings.
pub fn advance_projection(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), CameraDispatchError> {
    if world
        .view_transition_mode
        .ok_or(CameraDispatchError::MissingViewMode)?
        .active()
    {
        return Ok(());
    }
    if state(objects, world, owner)?.projection_correction_disabled {
        return Ok(());
    }
    let view = world.fixed_players[0].ok_or(CameraDispatchError::MissingFixedView)?;
    let (top, bottom) = world
        .camera_height_limits
        .ok_or(CameraDispatchError::MissingHeightLimits)?;
    let midpoint = top.wrapping_add(bottom) / HEIGHT_MIDPOINT_DIVISOR;
    let height = objects
        .get(view)
        .ok_or(WorldInputError::MissingActor(view))?
        .base
        .position
        .y;
    let correction = height.wrapping_sub(midpoint) >> HEIGHT_RESPONSE_SHIFT;
    world.camera_projection_offset =
        Some(correction.clamp(-HEIGHT_RESPONSE_LIMIT, HEIGHT_RESPONSE_LIMIT));
    Ok(())
}

fn recoil(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), CameraDispatchError> {
    world
        .player_mut(objects, owner)?
        .contact
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .advance_pitch_recoil();
    Ok(())
}

/// Includes all three installed primary tasks, both early gates and the real
/// auxiliary footer. A missing primary task is distinct from an absent record
/// and bypasses even the footer's otherwise-unconditional counter increment.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &mut PathRuntime,
    owner: ObjectId,
) -> Result<(), CameraDispatchError> {
    if world
        .view_transition_mode
        .ok_or(CameraDispatchError::MissingViewMode)?
        .active()
    {
        player_camera_angles::publish_existing_roll(objects, world)?;
        return Ok(());
    }
    if world
        .action_gate
        .ok_or(CameraDispatchError::MissingActionGate)?
        .code
        != 0
    {
        player_camera_angles::publish_existing_roll(objects, world)?;
        player_camera_auxiliary::advance(objects, world, runtime, owner)?;
        return Ok(());
    }
    let Some(style) = state(objects, world, owner)?.style else {
        return Ok(());
    };
    match style {
        TrackingStyle::Normal => {
            let flags = world
                .player(objects, owner)?
                .mode_selection
                .ok_or(CameraDispatchError::MissingModeSelection(owner))?
                .surface_control;
            if flags & HIDE_HORIZON != 0 {
                world.horizon_disabled = Some(true);
            }
        }
        TrackingStyle::Surface => {
            if world
                .player_carry_mode
                .ok_or(CameraDispatchError::MissingCarryMode)?
                == ACTIVE_CARRY_MODE
                && objects
                    .get(owner)
                    .ok_or(WorldInputError::MissingActor(owner))?
                    .extension
                    .path_state
                    .motion
                    .carry_selected_player
            {
                world.horizon_disabled = Some(true);
            }
        }
        TrackingStyle::ProjectionCorrected => {}
    }
    let auxiliary = world
        .player(objects, owner)?
        .camera_auxiliary
        .ok_or(CameraDispatchError::MissingAuxiliary(owner))?
        .task
        != AuxiliaryCameraTask::None;
    if style == TrackingStyle::Surface {
        // This precedes surface preparation. In the other two modes it runs
        // after publication, so the same request takes effect one visit later.
        player_view_distance::advance(objects, world, owner)?;
        player_camera_surface::prepare(objects, world, runtime, owner, style, auxiliary)?;
        recoil(objects, world, owner)?;
        let suppressed = world
            .player(objects, owner)?
            .occupancy_exempt
            .ok_or(CameraDispatchError::MissingExemption(owner))?;
        let outside = if suppressed {
            false
        } else {
            let view = world.fixed_players[0].ok_or(CameraDispatchError::MissingFixedView)?;
            let position = objects
                .get(view)
                .ok_or(WorldInputError::MissingActor(view))?
                .base
                .position;
            !world
                .occupancy
                .as_ref()
                .ok_or(CameraDispatchError::MissingOccupancy)?
                .contains(position)
        };
        world
            .player_mut(objects, owner)?
            .camera_dispatch
            .as_mut()
            .ok_or(CameraDispatchError::MissingState(owner))?
            .outside_occupied_world = outside;
        player_camera_angles::publish(objects, world, owner)?;
    } else {
        player_camera_common::advance(objects, world, runtime, owner, style, auxiliary)?;
        recoil(objects, world, owner)?;
        player_camera_angles::publish(objects, world, owner)?;
        player_view_distance::advance(objects, world, owner)?;
        // $07:9488 is literally RTL and has no source-visible work.
        if style == TrackingStyle::ProjectionCorrected {
            advance_projection(objects, world, owner)?;
        }
    }
    player_camera_auxiliary::advance(objects, world, runtime, owner)?;
    Ok(())
}
