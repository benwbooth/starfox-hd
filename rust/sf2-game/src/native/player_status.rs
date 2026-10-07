//! Player-frame status services: shield display/warnings ($07:AF5B..AFE7),
//! contact-filter and heading-bank decay ($06:9195..9235), and transformation
//! cues ($06:8FE2..9074). These retain the actual live owners used by damage,
//! mode selection and flight pose, not snapshots of those services.

use super::path_control::PlayerTarget;
use super::path_fields::chase_byte;
use super::path_sound::AuthoredCue;
use super::player_storage::{self, PlayerStorageError};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore, SoundEvent};

#[cfg(test)]
#[path = "player_status_tests.rs"]
mod tests;

const DISPLAY_UPDATE_PENDING: u8 = 0x80;
const FULL_SHIELD_EXCEPTION: u8 = 40;
const LOW_SHIELD_BOUND: u8 = 13;
const CRITICAL_SHIELD_BOUND: u8 = 5;
const LOW_SHIELD_CUE: u8 = 22;
const CRITICAL_SHIELD_CUE: u8 = 23;
const TRANSFORM_COUNT: u8 = 0x3F;
const TRANSFORM_INPUT_INHIBITED: u8 = 0x40;
const REVERSE_TRANSFORM: u8 = 0x80;
const TRANSFORM_CUE_TIME: u8 = 61;
const SURFACE_CUE_TIME: u8 = 55;
const TRANSFORM_CUE: u8 = 30;
const REVERSE_TRANSFORM_CUE: u8 = 31;
const SURFACE_TRANSFORM_CUE: u8 = 169;
const ACTIVE_CARRY_MODE: u8 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerStatus {
    /// Last reserve observed by the shield-drop warning ($6C0A). Death
    /// preserves it; it is not the separately animated display at $6C38.
    pub shield_warning_history: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    MissingState(ObjectId),
    MissingPose(ObjectId),
    MissingModeSelection(ObjectId),
    MissingShieldCapacity,
    MissingViewMode,
    MissingCarryMode,
}

impl From<WorldInputError> for StatusError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<PlayerStorageError> for StatusError {
    fn from(error: PlayerStorageError) -> Self {
        Self::Storage(error)
    }
}

fn sound(world: &mut ScenePathWorld, owner: ObjectId, cue: u8) {
    let side = if world.primary_player == Some(owner) {
        PlayerTarget::Primary
    } else {
        PlayerTarget::Secondary
    };
    world
        .audio
        .queue(SoundEvent::Authored(AuthoredCue::new(cue, 0, side)));
}

fn reserve(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<u8, StatusError> {
    Ok(world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .reserve_shield)
}

/// Display changes set a pending-consumption bit. This routine does not
/// clear it: the separate display consumer owns that acknowledgement.
/// The warning uses actual shield loss even while the display is pending.
pub fn advance_shield(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<(), StatusError> {
    let displayed = player_storage::get(objects, resources, owner)?.retained_shield;
    if displayed & DISPLAY_UPDATE_PENDING == 0 {
        let shield = reserve(objects, world, owner)?;
        if shield != displayed {
            let next = if shield < displayed {
                displayed - 1
            } else {
                displayed.wrapping_add(1).min(
                    world
                        .active_shield_capacity
                        .ok_or(StatusError::MissingShieldCapacity)?,
                )
            };
            player_storage::get_mut(objects, resources, owner)?.retained_shield =
                next | DISPLAY_UPDATE_PENDING;
        }
    }
    warn_shield_loss(objects, world, owner)
}

/// The independently callable warning ($07:AF9B) preserves history on dead
/// actors. Its comparisons are unsigned, unlike the entry-prefix warning.
pub fn warn_shield_loss(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), StatusError> {
    if objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .hit_points
        == 0
    {
        return Ok(());
    }
    let shield = reserve(objects, world, owner)?;
    if shield != FULL_SHIELD_EXCEPTION {
        let previous = world
            .player(objects, owner)?
            .status
            .ok_or(StatusError::MissingState(owner))?
            .shield_warning_history;
        if shield < previous && shield < LOW_SHIELD_BOUND {
            sound(
                world,
                owner,
                if shield < CRITICAL_SHIELD_BOUND {
                    CRITICAL_SHIELD_CUE
                } else {
                    LOW_SHIELD_CUE
                },
            );
        }
    }
    world
        .player_mut(objects, owner)?
        .status
        .as_mut()
        .ok_or(StatusError::MissingState(owner))?
        .shield_warning_history = shield;
    Ok(())
}

/// The complete filter caller, including the following eighth-rate recovery
/// of the same heading-return bank later consumed by flight pose.
pub fn advance_filters(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), StatusError> {
    let clock = world.strategy_clock as u8;
    let record = world.player_mut(objects, owner)?;
    let hit = &mut record
        .contact
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit;
    let actor = objects.get_mut(owner).expect("validated player");
    hit.advance_recovery_filters(&mut actor.base.contacts, &mut actor.base.flags, clock);
    let paused = world
        .scripted_view_active()
        .ok_or(StatusError::MissingViewMode)?;
    let hit = &mut world
        .player_mut(objects, owner)?
        .contact
        .as_mut()
        .expect("validated contact owner")
        .hit;
    hit.advance_secondary_filter(
        &mut objects
            .get_mut(owner)
            .expect("validated player")
            .base
            .contacts,
        paused,
    );
    let pose = world
        .player_mut(objects, owner)?
        .pose
        .as_mut()
        .ok_or(StatusError::MissingPose(owner))?;
    pose.heading_return_bank = chase_byte(pose.heading_return_bank as u8, 0) as i8;
    Ok(())
}

/// The cue countdown and mode-request gate are the same byte. Arrival at
/// 61 clears the live request-inhibition bit; reverse transformation also
/// has a surface-only cue at 55. A zero low count preserves all upper bits.
pub fn advance_transform_cues(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), StatusError> {
    let state = world
        .player_mut(objects, owner)?
        .mode_selection
        .as_mut()
        .ok_or(StatusError::MissingModeSelection(owner))?;
    if state.cue_control & TRANSFORM_COUNT == 0 {
        return Ok(());
    }
    state.cue_control = state.cue_control.wrapping_sub(1);
    let value = state.cue_control;
    if value & REVERSE_TRANSFORM == 0 {
        if value & TRANSFORM_COUNT == TRANSFORM_CUE_TIME {
            state.cue_control &= !TRANSFORM_INPUT_INHIBITED;
            sound(world, owner, TRANSFORM_CUE);
        }
        return Ok(());
    }
    if value & TRANSFORM_COUNT == TRANSFORM_CUE_TIME {
        sound(world, owner, REVERSE_TRANSFORM_CUE);
        world
            .player_mut(objects, owner)?
            .mode_selection
            .as_mut()
            .expect("validated mode selection")
            .cue_control &= !TRANSFORM_INPUT_INHIBITED;
    }
    if world
        .player_carry_mode
        .ok_or(StatusError::MissingCarryMode)?
        == ACTIVE_CARRY_MODE
        && objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .extension
            .path_state
            .motion
            .carry_selected_player
        && value & TRANSFORM_COUNT == SURFACE_CUE_TIME
    {
        sound(world, owner, SURFACE_TRANSFORM_CUE);
    }
    Ok(())
}
