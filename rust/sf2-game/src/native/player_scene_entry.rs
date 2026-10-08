//! Indexed-scene player strategy: entry (`$06:83F1..84A1`) and the per-visit
//! wait (`$06:84A2..84BD`) that continues into the shared action tail. The
//! entry visit falls through into its first wait; no visit is skipped.
//! Both closed-gate exits (`$06:8525`, `$06:84EF`) are separate, unported
//! strategies; they fault here after the writes the source performs first.

use super::player_action_wait::{self, ActionWaitError};
use super::player_camera_dispatch::{self, CameraDispatchError};
use super::player_entry_reset::{self, EntryResetError};
use super::positional_audio::PositionalAudio;
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_install::{self, SceneInstallError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Behavior, ObjectId, ObjectStore};

/// Entry retries every visit while no scene is selected. The installer's
/// other skip sentinel (254) is NOT tested here and still installs the wait.
const NO_SCENE_SELECTED: u8 = u8::MAX;
/// Shared mode word bit 10 ($1B84), cleared on every wait visit.
const SCENE_ENTRY_MODE: u16 = 0x0010;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneEntryPhase {
    Enter,
    Wait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryOutcome {
    /// Strategy unchanged; the complete entry repeats on the next visit.
    Retry,
    /// The wait is installed, with the scene actor when one was created.
    Waiting(Option<ObjectId>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneEntryError {
    World(WorldInputError),
    Reset(EntryResetError),
    Install(SceneInstallError),
    Camera(CameraDispatchError),
    Action(ActionWaitError),
    MissingActionGate,
    MissingSelection,
    MissingFixedView,
    MissingViewMode,
    WrongBehavior,
    /// `$06:8525`: gate closed at entry; the fixed view is already marked.
    UnportedEntryExit,
    /// `$06:84EF`: gate closed while waiting, after projection and mode.
    UnportedWaitExit,
}

impl From<WorldInputError> for SceneEntryError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

pub fn step(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    positional: &mut PositionalAudio,
    owner: ObjectId,
) -> Result<Option<EntryOutcome>, SceneEntryError> {
    let phase = match objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .behavior
    {
        Behavior::PlayerSceneEntry(phase) => phase,
        _ => return Err(SceneEntryError::WrongBehavior),
    };
    match phase {
        SceneEntryPhase::Enter => {
            player_entry_reset::reset(objects, world, resources, positional, owner)
                .map_err(SceneEntryError::Reset)?;
            let outcome = begin(objects, world, owner)?;
            if outcome != EntryOutcome::Retry {
                // The source falls through into the wait in the same visit.
                wait(objects, world, owner)?;
            }
            Ok(Some(outcome))
        }
        SceneEntryPhase::Wait => wait(objects, world, owner).map(|()| None),
    }
}

/// `$06:846C..84A1`, after the entry reset and before the first wait.
pub fn begin(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<EntryOutcome, SceneEntryError> {
    if gate_closed(world)? {
        // Fixed-view byte 21 bit 20, the same bit as the carry gate.
        let view = world.fixed_players[0].ok_or(SceneEntryError::MissingFixedView)?;
        objects
            .get_mut(view)
            .ok_or(WorldInputError::MissingActor(view))?
            .extension
            .path_state
            .motion
            .carry_selected_player = true;
        return Err(SceneEntryError::UnportedEntryExit);
    }
    if world.scene_selection.ok_or(SceneEntryError::MissingSelection)? == NO_SCENE_SELECTED {
        return Ok(EntryOutcome::Retry);
    }
    let created =
        scene_install::install(objects, world, owner).map_err(SceneEntryError::Install)?;
    objects
        .get_mut(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .behavior = Behavior::PlayerSceneEntry(SceneEntryPhase::Wait);
    world.audio.save_restore_cue();
    Ok(EntryOutcome::Waiting(created))
}

/// `$06:84A2..84BD`, then the shared tail. The projection service has its own
/// scripted-view and map gates; the mode bit is cleared on every visit.
pub fn wait(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), SceneEntryError> {
    player_camera_dispatch::advance_projection(objects, world, owner)
        .map_err(SceneEntryError::Camera)?;
    world
        .view_transition_mode
        .as_mut()
        .ok_or(SceneEntryError::MissingViewMode)?
        .flags &= !SCENE_ENTRY_MODE;
    if gate_closed(world)? {
        return Err(SceneEntryError::UnportedWaitExit);
    }
    player_action_wait::advance_action_tail(objects, world, owner)
        .map_err(SceneEntryError::Action)
}

fn gate_closed(world: &ScenePathWorld) -> Result<bool, SceneEntryError> {
    Ok(world
        .action_gate
        .ok_or(SceneEntryError::MissingActionGate)?
        .code
        == 0)
}
