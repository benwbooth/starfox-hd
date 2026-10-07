//! Free-flight visit preparation (`$06:869C..875E`). Camera installation,
//! contact admission, controller selection and shoulder arbitration run in
//! source order before the optional ground-pitch update. Movement, camera
//! publication and the remaining mode suffix belong to the enclosing visit.

use super::path_runtime::PathRuntime;
use super::player_camera_dispatch::{self, CameraDispatchError};
use super::player_camera_ground::{self, GroundCameraError};
use super::player_camera_tracking::TrackingStyle;
use super::player_input::{self, PlayerInputError};
use super::player_roll::{self, RollError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlightPreparationError {
    Camera(CameraDispatchError),
    World(WorldInputError),
    Input(PlayerInputError),
    Shoulders(RollError),
    GroundPitch(GroundCameraError),
}

pub fn prepare(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &mut PathRuntime,
    owner: ObjectId,
) -> Result<(), FlightPreparationError> {
    player_camera_dispatch::select_free_flight_camera(objects, world, owner)
        .map_err(FlightPreparationError::Camera)?;
    world
        .player_mut(objects, owner)
        .map_err(FlightPreparationError::World)?
        .contact
        .as_mut()
        .ok_or(FlightPreparationError::World(
            WorldInputError::MissingPlayerContact(owner),
        ))?
        .ignores_contacts = false;
    // This is the same event consumed by authored hit-event triggers, not
    // the collision pass's new-contact observation or damage attribution.
    objects
        .get_mut(owner)
        .ok_or(FlightPreparationError::World(
            WorldInputError::MissingActor(owner),
        ))?
        .extension
        .path_state
        .conditions
        .hit_event_pending = true;
    player_input::prepare(objects, world, owner).map_err(FlightPreparationError::Input)?;
    player_roll::prepare_shoulders(objects, world, owner)
        .map_err(FlightPreparationError::Shoulders)?;
    if world
        .player(objects, owner)
        .map_err(FlightPreparationError::World)?
        .camera_dispatch
        .ok_or(FlightPreparationError::Camera(
            CameraDispatchError::MissingState(owner),
        ))?
        .style
        != Some(TrackingStyle::Normal)
    {
        player_camera_ground::advance_pitch(objects, world, runtime, owner)
            .map_err(FlightPreparationError::GroundPitch)?;
    }
    Ok(())
}
