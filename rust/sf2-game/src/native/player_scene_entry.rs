//! Indexed-scene player strategy: entry (`$06:83F1..84A1`) and the per-visit
//! wait (`$06:84A2..84BD`) that continues into the shared action tail. The
//! entry visit falls through into its first wait; no visit is skipped.
//! A closed gate leaves the scene: the wait's exit prefix (`$06:84EF..8524`)
//! joins the entry's exit at `$06:8525`, which installs the player flight
//! strategy (`$06:9C27`). The caller then runs that strategy in the same
//! visit, as the source's closing `JMP $9C27` does.

use super::actor_auxiliary::{AuxiliaryError, AuxiliaryRecord, DeathHandler};
use super::path_control::PlayerTarget;
use super::path_sound::AuthoredCue;
use super::player_action_wait::{self, ActionWaitError};
use super::player_camera_auxiliary::AuxiliaryCameraTask;
use super::player_camera_dispatch::{self, CameraDispatchError};
use super::player_entry_reset::{self, EntryResetError};
use super::positional_audio::PositionalAudio;
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_contact::{self, RegistrationError};
use super::scene_install::{self, SceneInstallError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::view_blend::{self, ViewBlendError};
use super::{Behavior, ObjectId, ObjectStore, SoundEvent};

/// Entry retries every visit while no scene is selected. The installer's
/// other skip sentinel (254) is NOT tested here and still installs the wait.
const NO_SCENE_SELECTED: u8 = u8::MAX;
/// Shared mode word bit 10 ($1B84), cleared on every wait visit.
const SCENE_ENTRY_MODE: u16 = 0x0010;
/// Auxiliary action bit 01 (6B77), set by both exit paths.
const ACTION_ACTIVE: u8 = 0x01;
/// The indexed scene whose wait exit carries the selected player (1D73).
const CARRIED_EXIT_SCENE: u8 = 0x1D;
/// Reticle position bytes 1E30/1E31 restart at the screen centre.
const RETICLE_CENTRE: u8 = 0x80;
/// `$06:9D5E`: flight-phase pair (ordinary, 1DE3 bit 80 variant) of each
/// player-configuration record. Configuration 8 shares record 0. Each record's
/// second word pair (1E56, 1E54) has no recovered reader and is not modeled.
const CONFIGURATION_PHASES: [(u8, u8); 10] = [
    (5, 0),
    (5, 0),
    (5, 0),
    (7, 3),
    (7, 3),
    (7, 3),
    (5, 0),
    (7, 3),
    (5, 0),
    (5, 0),
];
const CONFIGURATION_VARIANT: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneEntryPhase {
    Enter,
    /// `$06:845C`: the scene-player initializer's entry point, after the
    /// reset prefix. Only the hostile-launch statistics are cleared first.
    ClearLaunchCounts,
    Wait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryOutcome {
    /// Strategy unchanged; the complete entry repeats on the next visit.
    Retry,
    /// The wait is installed, with the scene actor when one was created.
    Waiting(Option<ObjectId>),
    /// The gate was closed at entry or while waiting. The exit prefix ran;
    /// the caller completes the hand-over with [`enter_flight`].
    GateClosed,
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
    MissingWeaponState,
    MissingAuxiliary(ObjectId),
    MissingCameraTracking(ObjectId),
    MissingCameraAuxiliary(ObjectId),
    MissingModeSelection(ObjectId),
    MissingEquipment(ObjectId),
    MissingRestoreCue,
    MissingPublishedConsumables,
    MissingPublishedWeaponLevel,
    MissingSceneGate,
    MissingConfiguration,
    MissingConfigurationVariant,
    WrongBehavior,
    Blend(ViewBlendError),
    Auxiliary(AuxiliaryError),
    Registration(RegistrationError),
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
            if let EntryOutcome::Waiting(_) = outcome {
                // The source falls through into the wait in the same visit.
                if wait(objects, world, resources, owner)? {
                    exit_wait(objects, world, owner)?;
                    return Ok(Some(EntryOutcome::GateClosed));
                }
            }
            Ok(Some(outcome))
        }
        SceneEntryPhase::ClearLaunchCounts => {
            world
                .weapons
                .as_mut()
                .ok_or(SceneEntryError::MissingWeaponState)?
                .hostile_counts = Default::default();
            let outcome = begin(objects, world, owner)?;
            if let EntryOutcome::Waiting(_) = outcome {
                if wait(objects, world, resources, owner)? {
                    exit_wait(objects, world, owner)?;
                    return Ok(Some(EntryOutcome::GateClosed));
                }
            }
            Ok(Some(outcome))
        }
        SceneEntryPhase::Wait => {
            if wait(objects, world, resources, owner)? {
                exit_wait(objects, world, owner)?;
                return Ok(Some(EntryOutcome::GateClosed));
            }
            Ok(None)
        }
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
        return Ok(EntryOutcome::GateClosed);
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
/// True is a closed gate: the source leaves for [`exit_wait`] before the tail.
pub fn wait(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<bool, SceneEntryError> {
    player_camera_dispatch::advance_projection(objects, world, owner)
        .map_err(SceneEntryError::Camera)?;
    world
        .view_transition_mode
        .as_mut()
        .ok_or(SceneEntryError::MissingViewMode)?
        .flags &= !SCENE_ENTRY_MODE;
    if gate_closed(world)? {
        return Ok(true);
    }
    player_action_wait::advance_action_tail(objects, world, resources, owner)
        .map_err(SceneEntryError::Action)?;
    Ok(false)
}

/// `$06:84EF..8524`: the wait's own exit prefix, before the shared exit.
pub fn exit_wait(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), SceneEntryError> {
    auxiliary_mut(objects, world, owner)?.action_flags |= ACTION_ACTIVE;
    if world.scene_selection.ok_or(SceneEntryError::MissingSelection)? == CARRIED_EXIT_SCENE {
        let view = world.fixed_players[0].ok_or(SceneEntryError::MissingFixedView)?;
        objects
            .get_mut(view)
            .ok_or(WorldInputError::MissingActor(view))?
            .extension
            .path_state
            .motion
            .carry_selected_player = true;
    }
    let height = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .position
        .y;
    world
        .player_mut(objects, owner)?
        .camera_tracking
        .as_mut()
        .ok_or(SceneEntryError::MissingCameraTracking(owner))?
        .anchor_height = height;
    view_blend::advance(objects, world).map_err(SceneEntryError::Blend)?;
    // `$7F:6E09` queues the complete word 1CE3; its high byte has no writer.
    let cue = world
        .audio
        .restore_cue_id()
        .ok_or(SceneEntryError::MissingRestoreCue)?;
    world.audio.queue(SoundEvent::Authored(AuthoredCue::new(cue, 0, PlayerTarget::Primary)));
    Ok(())
}

/// `$06:8525..8624`: the shared scene exit. Publishes the reticle, restores
/// published equipment, installs the death and contact handlers again and
/// installs the flight strategy at the configuration's phase. The caller
/// runs that strategy next (`$06:8625 JMP $9C27`).
pub fn enter_flight(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<(), SceneEntryError> {
    world.reticle_enabled = Some(true);
    world.target_reticle.horizontal = Some(RETICLE_CENTRE);
    world.target_reticle.vertical = Some(RETICLE_CENTRE);
    // `$06:D9FF`: equipment from the shared publications 1DD2..1DD4.
    let level = world
        .scene
        .active_weapon_level
        .ok_or(SceneEntryError::MissingPublishedWeaponLevel)?;
    let consumables = world
        .active_consumables
        .ok_or(SceneEntryError::MissingPublishedConsumables)?;
    let equipment = world
        .player_mut(objects, owner)?
        .equipment
        .as_mut()
        .ok_or(SceneEntryError::MissingEquipment(owner))?;
    equipment.weapon_level = level;
    equipment.consumable_type = consumables.kind;
    equipment.packed_consumables = consumables.packed_count;
    world
        .view_transition_mode
        .as_mut()
        .ok_or(SceneEntryError::MissingViewMode)?
        .flags &= !SCENE_ENTRY_MODE;
    world
        .scene_gate_flags
        .as_mut()
        .ok_or(SceneEntryError::MissingSceneGate)?
        .hud_ready = true;
    auxiliary_mut(objects, world, owner)?.action_flags |= ACTION_ACTIVE;
    let actor = objects
        .get_mut(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    actor.base.flags.visible = true;
    actor.base.contacts.run_when_paused = true;
    let mut auxiliary = std::mem::take(&mut actor.extension.auxiliary);
    let registered = auxiliary.set(
        resources,
        owner,
        AuxiliaryRecord::DeathHandler(DeathHandler::ScenePlayer),
    );
    let actor = objects.get_mut(owner).expect("validated scene player");
    actor.extension.auxiliary = auxiliary;
    registered.map_err(SceneEntryError::Auxiliary)?;
    actor.base.flags.collision_disabled = false;
    actor.base.flags.general_search_eligible = false;
    actor.base.behavior = Behavior::PlayerFlight;
    scene_contact::install_player(objects, resources, owner)
        .map_err(SceneEntryError::Registration)?;
    world
        .player_mut(objects, owner)?
        .camera_auxiliary
        .as_mut()
        .ok_or(SceneEntryError::MissingCameraAuxiliary(owner))?
        .task = AuxiliaryCameraTask::None;
    // The bound reads 1DE2/1DE3 as one word, so a variant selects record 0.
    let configuration = world
        .scene
        .player_configuration
        .ok_or(SceneEntryError::MissingConfiguration)?;
    let variant = world
        .scene
        .player_configuration_variant
        .ok_or(SceneEntryError::MissingConfigurationVariant)?;
    let record = if variant == 0 && usize::from(configuration) < CONFIGURATION_PHASES.len() {
        usize::from(configuration)
    } else {
        0
    };
    let (ordinary, alternate) = CONFIGURATION_PHASES[record];
    let phase = if variant & CONFIGURATION_VARIANT != 0 {
        alternate
    } else {
        ordinary
    };
    world
        .player_mut(objects, owner)?
        .mode_selection
        .as_mut()
        .ok_or(SceneEntryError::MissingModeSelection(owner))?
        .configured_phase = phase;
    objects.get_mut(owner).expect("validated scene player").base.behavior_phase = phase;
    Ok(())
}

fn auxiliary_mut<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut super::path_program::SelectedAuxiliaryState, SceneEntryError> {
    world
        .player_mut(objects, owner)?
        .auxiliary
        .as_mut()
        .ok_or(SceneEntryError::MissingAuxiliary(owner))
}

fn gate_closed(world: &ScenePathWorld) -> Result<bool, SceneEntryError> {
    Ok(world
        .action_gate
        .ok_or(SceneEntryError::MissingActionGate)?
        .code
        == 0)
}
