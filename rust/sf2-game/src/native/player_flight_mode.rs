//! Retained-pitch flight mode ($06:E2EE..E32B). The enclosing player strategy
//! owns input preparation, shoulder arbitration and position capture. This
//! mode retains input history, resets shared steering publications, applies
//! live controls, then runs the complete shared flight update exactly once.

use super::path_control::PlayerTarget;
use super::path_sound::AuthoredCue;
use super::player_flight::{self, FlightContext, FlightError, FlightResult};
use super::player_steering::{self, SteeringContext, SteeringError};
use super::player_vertical::{self, VerticalError, VerticalMode};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Button, ObjectId, ObjectStore, SoundEvent};

#[cfg(test)]
#[path = "player_flight_mode_tests.rs"]
mod tests;

const RETAINED_MODE_SELECTION_CUE: u8 = 54;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FlightModeContext {
    pub steering: SteeringContext,
    pub flight: FlightContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlightModeError {
    Vertical(VerticalError),
    Steering(SteeringError),
    Flight(FlightError),
    World(WorldInputError),
    MissingProcessedInput,
}

/// $06:E2EE uses the held/fine-pitch controller and hard limits. The adjacent
/// $06:E32C mode has additional surface/protection/transition owners and is
/// not represented by a guessed choice of the other vertical controller.
pub fn advance_retained_pitch(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    context: FlightModeContext,
) -> Result<FlightResult, FlightModeError> {
    player_vertical::retain_input(objects, world, owner).map_err(FlightModeError::Vertical)?;
    world
        .player_mut(objects, owner)
        .map_err(FlightModeError::World)?
        .steering
        .as_mut()
        .ok_or(FlightModeError::Steering(SteeringError::MissingSteering(
            owner,
        )))?
        .camera_bank_target = 0;
    world.player_pitch_target = Some(0);
    world.player_yaw_increment = Some(0);
    world.player_roll_increment = Some(0);
    player_steering::advance(objects, world, resources, owner, context.steering)
        .map_err(FlightModeError::Steering)?;
    player_vertical::advance(
        objects,
        world,
        resources,
        owner,
        VerticalMode::RetainedPitch,
    )
    .map_err(FlightModeError::Vertical)?;
    let result = player_flight::advance(objects, world, resources, owner, context.flight)
        .map_err(FlightModeError::Flight)?;
    if world
        .processed_player_input
        .ok_or(FlightModeError::MissingProcessedInput)?
        .pressed
        .contains(Button::Select)
    {
        // Unlike the adjacent mode's cue, this original call does not add
        // the current player's side bit.
        world.audio.queue(SoundEvent::Authored(AuthoredCue::new(
            RETAINED_MODE_SELECTION_CUE,
            0,
            PlayerTarget::Primary,
        )));
    }
    Ok(result)
}
