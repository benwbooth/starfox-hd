//! Action-only player strategies ($06:8362..83F0 and $06:84BE..84EE).
//! One installs a persistent active phase after resetting movement once;
//! the other repeats that reset and the primary palette service each visit.
//! Both sample the visiting player's raw input and run its real action.

use super::hit_response::HitSide;
use super::player_action::{self, PlayerActionError};
use super::player_motion_reset::{self, MotionResetError};
use super::player_palette::{self, PaletteError};
use super::positional_audio::PositionalAudio;
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Behavior, ObjectId, ObjectStore, ShapeId};

const CRUISE_SOUND: u8 = 4;
const ENTRY_HANDOFF_FLAGS: u8 = 0x50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionWaitPhase {
    Initialize,
    Active,
    /// `$06:8362`: no successor is installed. Movement is prepared again on
    /// every strategy visit; action timing and positional audio are retained.
    Resetting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionWaitError {
    World(WorldInputError),
    MotionReset(MotionResetError),
    Action(PlayerActionError),
    Palette(PaletteError),
    MissingAction(ObjectId),
    MissingHandoff,
    MissingController(HitSide),
    WrongBehavior,
}

impl From<WorldInputError> for ActionWaitError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

/// The active phase still samples input while scripted view suppresses the
/// action dispatcher. Neither controller remapping nor queued injection is
/// used here. Initialization retains the action identity, companion counter
/// and total age; it clears only the elapsed counter before that first visit.
pub fn step(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    positional: &mut PositionalAudio,
    owner: ObjectId,
) -> Result<(), ActionWaitError> {
    let phase = match objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .behavior
    {
        Behavior::PlayerActionWait(phase) => phase,
        _ => return Err(ActionWaitError::WrongBehavior),
    };
    if phase == ActionWaitPhase::Resetting {
        world.engine_sound_control = Some(
            super::player_engine_sound::EngineSoundControl::from_bits(CRUISE_SOUND),
        );
        world
            .handoff
            .as_mut()
            .ok_or(ActionWaitError::MissingHandoff)?
            .player_flags &= !ENTRY_HANDOFF_FLAGS;
        objects
            .get_mut(owner)
            .expect("validated waiting player")
            .base
            .flags
            .collision_disabled = true;
        player_motion_reset::prepare_scene_entry(objects, world, resources, owner)
            .map_err(ActionWaitError::MotionReset)?;
    } else if phase == ActionWaitPhase::Initialize {
        objects
            .get_mut(owner)
            .expect("validated waiting player")
            .base
            .shape = ShapeId::EMPTY;
        player_motion_reset::prepare_scene_entry(objects, world, resources, owner)
            .map_err(ActionWaitError::MotionReset)?;
        world
            .player_mut(objects, owner)?
            .action
            .as_mut()
            .ok_or(ActionWaitError::MissingAction(owner))?
            .elapsed = 0;
        objects
            .get_mut(owner)
            .expect("validated waiting player")
            .base
            .shape = ShapeId::EMPTY;
        world.engine_sound_control = Some(
            super::player_engine_sound::EngineSoundControl::from_bits(CRUISE_SOUND),
        );
        positional.silence_published();
        world
            .handoff
            .as_mut()
            .ok_or(ActionWaitError::MissingHandoff)?
            .player_flags = 0;
        objects
            .get_mut(owner)
            .expect("validated waiting player")
            .base
            .behavior = Behavior::PlayerActionWait(ActionWaitPhase::Active);
    }
    objects
        .get_mut(owner)
        .expect("validated waiting player")
        .base
        .flags
        .collision_disabled = true;
    if phase == ActionWaitPhase::Resetting {
        return advance_action_tail(objects, world, owner);
    }
    sample_and_advance(objects, world, owner)
}

/// Shared tail (`$06:84BE..84EE`): sample the visiting player's raw input,
/// run its real action, then the primary palette service. Entered both from
/// the resetting wait and from the scene-entry wait ($06:84A2).
pub fn advance_action_tail(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ActionWaitError> {
    sample_and_advance(objects, world, owner)?;
    player_palette::advance_primary(objects, world).map_err(ActionWaitError::Palette)
}

fn sample_and_advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ActionWaitError> {
    let side = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .contacts
        .hit_side;
    let controller = match side {
        HitSide::Primary => 0,
        HitSide::Secondary => 1,
    };
    let input =
        world.controller_inputs[controller].ok_or(ActionWaitError::MissingController(side))?;
    world.processed_player_input = Some(input);
    player_action::advance(objects, world, owner, input).map_err(ActionWaitError::Action)
}

#[cfg(test)]
#[path = "player_action_wait_tests.rs"]
mod tests;
