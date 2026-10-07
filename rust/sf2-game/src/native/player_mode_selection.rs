//! Mode-request arbitration and transformation entry ($06:98CD..9A0F).
//! New controller requests are gated separately from already-pending requests.
//! The selected strategy phase runs later; this service does not complete a
//! transformation or change the current mode speculatively.

use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Button, ObjectId, ObjectStore};

#[cfg(test)]
#[path = "player_mode_selection_tests.rs"]
mod tests;

const FAMILY_MASK: u8 = 0xF0;
const REQUEST_MASK: u8 = 0x0F;
const ACTIVE_CARRY_MODE: u8 = 1;
const SURFACE_CARRY: u8 = 0x02;
const TRANSITION_ACTIVE: u8 = 0x08;
const TRANSITION_QUEUED: u8 = 0x10;
const WALKER_REQUEST: u8 = 4;
const WALKER_FAMILY: u8 = 0x20;
const ALTERNATE_FAMILY: u8 = 0x30;
const ENTER_WALKER: u8 = 9;
const LEAVE_WALKER: u8 = 11;
const ENTER_ALTERNATE: u8 = 13;
const LEAVE_ALTERNATE: u8 = 15;
// Original five-row mode-family / entry-phase table ($06:9A10).
const REQUEST_PROFILES: [(u8, u8); 5] = [(0, 0), (0x10, 5), (0x10, 7), (0x30, 3), (0x20, 0)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeRequest {
    FreeFlight,
    RetainedPitch,
    Walker,
}

impl ModeRequest {
    pub const fn selector(self) -> u8 {
        match self {
            Self::FreeFlight => 1,
            Self::RetainedPitch => 2,
            Self::Walker => WALKER_REQUEST,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerModeSelection {
    /// Pending selector low nibble and retained high flags (6AA1).
    pub requested: u8,
    /// Transition flags (6BEC), immediately before the retained return X.
    pub transition_control: u8,
    /// Surface/carry flags (6B64), shared with surface-effect movement.
    pub surface_control: u8,
    /// Only the new-request gate at 6B9B bit 40. It does not suppress
    /// already-pending transformation work.
    pub request_inhibited: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeSelectionError {
    World(WorldInputError),
    MissingSelection(ObjectId),
    MissingCarryMode,
    MissingProcessedInput,
    UnsupportedPendingMode(u8),
}

impl From<WorldInputError> for ModeSelectionError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn selection(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<PlayerModeSelection, ModeSelectionError> {
    world
        .player(objects, owner)?
        .mode_selection
        .ok_or(ModeSelectionError::MissingSelection(owner))
}

/// True is the original transformation-entry cue request. A same-family
/// mode switch publishes its initializer phase but deliberately returns false.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    request: ModeRequest,
) -> Result<bool, ModeSelectionError> {
    let mut state = selection(objects, world, owner)?;
    if !state.request_inhibited {
        let carried = world
            .player_carry_mode
            .ok_or(ModeSelectionError::MissingCarryMode)?
            == ACTIVE_CARRY_MODE
            && objects
                .get(owner)
                .ok_or(WorldInputError::MissingActor(owner))?
                .extension
                .path_state
                .motion
                .carry_selected_player;
        if carried == (state.surface_control & SURFACE_CARRY != 0) {
            let pressed = world
                .processed_player_input
                .ok_or(ModeSelectionError::MissingProcessedInput)?
                .pressed;
            if pressed.contains(Button::Select)
                && state.transition_control & (TRANSITION_ACTIVE | TRANSITION_QUEUED) == 0
            {
                state.requested = (state.requested & FAMILY_MASK) | request.selector();
                world
                    .player_mut(objects, owner)?
                    .mode_selection
                    .as_mut()
                    .unwrap()
                    .requested = state.requested;
            }
        }
    }
    let pending = state.requested & REQUEST_MASK;
    if pending == 0 {
        return Ok(false);
    }
    let current = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode;
    if pending == current & REQUEST_MASK {
        return Ok(false);
    }
    let (family, entry) = *REQUEST_PROFILES
        .get(usize::from(pending))
        .ok_or(ModeSelectionError::UnsupportedPendingMode(pending))?;
    if family == current & FAMILY_MASK {
        world
            .player_mut(objects, owner)?
            .mode_selection
            .as_mut()
            .unwrap()
            .requested &= FAMILY_MASK;
        objects
            .get_mut(owner)
            .expect("validated player")
            .base
            .behavior_phase = entry;
        return Ok(false);
    }
    if state.transition_control & TRANSITION_ACTIVE != 0 {
        return Ok(false);
    }
    if state.transition_control & TRANSITION_QUEUED != 0 {
        world
            .player_mut(objects, owner)?
            .mode_selection
            .as_mut()
            .unwrap()
            .transition_control =
            (state.transition_control & !TRANSITION_QUEUED) | TRANSITION_ACTIVE;
    }
    let phase = match current & FAMILY_MASK {
        WALKER_FAMILY => LEAVE_WALKER,
        ALTERNATE_FAMILY => LEAVE_ALTERNATE,
        _ if pending == WALKER_REQUEST => ENTER_WALKER,
        _ => ENTER_ALTERNATE,
    };
    objects
        .get_mut(owner)
        .expect("validated player")
        .base
        .behavior_phase = phase;
    Ok(true)
}
