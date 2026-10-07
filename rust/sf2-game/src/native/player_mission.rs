//! Encounter progress/forced-retreat admission ($06:9FAD..A044) and the
//! later layout-advance consumer ($06:A0F9..A108).
//! This runs after player effects/attachment publication, not before the
//! parallel action's visit. Exit-controller creation remains the next branch;
//! admission does not run a newly installed action or the final aim update.

use super::path_sound::MusicControlRequest;
use super::player_action::PlayerAction;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore};

#[cfg(test)]
#[path = "player_mission_tests.rs"]
mod tests;

const SPECIAL_CONFIGURATION: u8 = 9;
const ASTROPOLIS_LOCATION: u16 = 11;
const TRANSITION_PROGRESS: u8 = 254;
const COMPLETE_PROGRESS: u8 = 255;
const TRANSITION_REQUESTED: u8 = 0x80;
const COMPLETION_REQUESTED: u8 = 0x40;
const ACTION_TRIGGER: u8 = 0x01;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerMissionControl {
    /// Per-player mission flags ($6A71). Progress publications set their
    /// respective high bit once; unrelated flags remain unchanged.
    pub flags: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionAdmission {
    /// Continue at the real exit-controller branch ($06:A045).
    ContinueExitControl,
    /// Skip exit creation and proceed to the retained forward aim ($06:A316).
    /// The parallel retreat action first runs on the next player visit.
    ForcedRetreatInstalled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionError {
    World(WorldInputError),
    MissingConfiguration,
    MissingLocation,
    MissingCoordination,
    MissingMissionControl(ObjectId),
    MissingObjectiveState,
    MissingSceneInhibition,
    MissingAction(ObjectId),
    MissingHandoff,
    MissingLayout,
}

impl From<WorldInputError> for MissionError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

pub fn advance_admission(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<MissionAdmission, MissionError> {
    if world
        .scene
        .player_configuration
        .ok_or(MissionError::MissingConfiguration)?
        == SPECIAL_CONFIGURATION
    {
        let location = world
            .scene
            .encounter_location
            .ok_or(MissionError::MissingLocation)?;
        let progress = world
            .coordination
            .ok_or(MissionError::MissingCoordination)?
            .progress;
        let request = if location != ASTROPOLIS_LOCATION && progress == TRANSITION_PROGRESS {
            Some((
                TRANSITION_REQUESTED,
                MusicControlRequest::EncounterProgressTransition,
            ))
        } else if progress == COMPLETE_PROGRESS {
            Some((
                COMPLETION_REQUESTED,
                MusicControlRequest::EncounterProgressComplete,
            ))
        } else {
            None
        };
        if let Some((flag, request)) = request {
            let control = world
                .player_mut(objects, owner)?
                .mission
                .as_mut()
                .ok_or(MissionError::MissingMissionControl(owner))?;
            if control.flags & flag == 0 {
                control.flags |= flag;
                world.audio.request_music_control(request);
            }
        }
    }
    if !world
        .contacts_enabled()
        .ok_or(MissionError::MissingObjectiveState)?
    {
        return Ok(MissionAdmission::ContinueExitControl);
    }
    if !world
        .reticle_inhibited
        .ok_or(MissionError::MissingSceneInhibition)?
    {
        return Ok(MissionAdmission::ContinueExitControl);
    }
    let action = world
        .player_mut(objects, owner)?
        .action
        .as_mut()
        .ok_or(MissionError::MissingAction(owner))?;
    if action.action.is_some() {
        return Ok(MissionAdmission::ContinueExitControl);
    }
    action.install(PlayerAction::ForcedRetreat);
    world
        .player_mut(objects, owner)?
        .auxiliary
        .as_mut()
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .action_flags &= !ACTION_TRIGGER;
    Ok(MissionAdmission::ForcedRetreatInstalled)
}

/// Consume the path-owned layout-advance bit ($06:A0F9..A108), after mission
/// completion selection and before exit-controller handoff. Clearing precedes
/// the wrapped byte increment; no other handoff bits or layout bytes change.
pub fn consume_layout_advance(world: &mut ScenePathWorld) -> Result<(), MissionError> {
    const ADVANCE_LAYOUT: u8 = 0x01;
    let flags = &mut world
        .handoff
        .as_mut()
        .ok_or(MissionError::MissingHandoff)?
        .player_flags;
    if *flags & ADVANCE_LAYOUT != 0 {
        *flags &= !ADVANCE_LAYOUT;
        let layout = world
            .scene
            .encounter_layout
            .as_mut()
            .ok_or(MissionError::MissingLayout)?;
        *layout = layout.wrapping_add(1);
    }
    Ok(())
}
