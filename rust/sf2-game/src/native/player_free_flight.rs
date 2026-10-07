//! Complete free-flight mode ($06:E32C..E399), including its surface-effect
//! prefix and pending Walker request. Input preparation, shoulder arbitration
//! and previous-position capture belong to the enclosing player strategy.

use super::path_control::PlayerTarget;
use super::path_sound::AuthoredCue;
use super::player_flight::FlightResult;
use super::player_flight_mode::{self, FlightModeContext, FlightModeError};
use super::player_mode_selection::{self, ModeRequest, ModeSelectionError};
use super::player_surface_effect::{self, SurfaceEffectError, SurfaceEffectInputs};
use super::player_surface_prepare::{self, SurfacePreparationError};
use super::player_vertical::VerticalMode;
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore, SoundEvent};

const TRANSFORMATION_CUE: u8 = 28;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FreeFlightContext {
    pub mode: FlightModeContext,
    pub protected_effect: SurfaceEffectInputs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreeFlightError {
    World(WorldInputError),
    SurfacePreparation(SurfacePreparationError),
    SurfaceEffect(SurfaceEffectError),
    Controls(FlightModeError),
    ModeSelection(ModeSelectionError),
    MissingContact(ObjectId),
}

pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    mut context: FreeFlightContext,
) -> Result<FlightResult, FreeFlightError> {
    if let Some(height) = player_surface_prepare::prepare(objects, world, owner)
        .map_err(FreeFlightError::SurfacePreparation)?
    {
        context.protected_effect.vertical = Some(height as i8);
    }
    if world
        .player(objects, owner)
        .map_err(FreeFlightError::World)?
        .contact
        .ok_or(FreeFlightError::MissingContact(owner))?
        .hit
        .hold_secondary_protection
    {
        if let Some(effect) =
            player_surface_effect::spawn(objects, world, owner, context.protected_effect)
                .map_err(FreeFlightError::SurfaceEffect)?
        {
            // This is the effect's rotated vertical offset, not its final
            // world position or the surface plane. A skipped effect leaves
            // the caller's steering input untouched.
            context.mode.steering.inherited_response_target = Some(effect.steering_response_target);
        }
    }
    let result = player_flight_mode::advance_controls(
        objects,
        world,
        resources,
        owner,
        context.mode,
        VerticalMode::Flight,
    )
    .map_err(FreeFlightError::Controls)?;
    if player_mode_selection::advance(objects, world, owner, ModeRequest::Walker)
        .map_err(FreeFlightError::ModeSelection)?
    {
        let target = if world.primary_player == Some(owner) {
            PlayerTarget::Primary
        } else {
            PlayerTarget::Secondary
        };
        world.audio.queue(SoundEvent::Authored(AuthoredCue::new(
            TRANSFORMATION_CUE,
            0,
            target,
        )));
    }
    Ok(result)
}
