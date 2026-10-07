//! Parallel player-action stream, revisited in authored order each update
//! (`$0D:BCD0..BDD9`). This is distinct from the player's actor path.
//! Only source-complete streams have native identities; scene entry must not
//! replace other streams with an empty or assumed-success action.
//! The interpreter's globals 1D9E/1DA0 (stream pointer), 1DA1, 1DA3 (slot)
//! and 1DA5 (copy of 6C16) are per-visit scratch, reloaded at
//! `$0D:BCEA..BD08` before any read, so they carry no state between visits.

use super::player_camera_auxiliary::{self, AuxiliaryCameraError, AuxiliaryCameraTask, OrbitStyle};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Button, InputState, ObjectId, ObjectStore};

#[cfg(test)]
#[path = "player_action_tests.rs"]
mod tests;

const EARLY_TRIGGER_START: u16 = 2;
const EARLY_TRIGGER_END: u16 = 11;
const DETONATION_TIME: u16 = 12;
const RESTORATION_TIME: u16 = 14;
const COMPLETION_TIME: u16 = 40;
const SPECIAL_CONFIGURATION: u8 = 9;
const RETREAT_CAMERA_TIME: u16 = 8;
const SCRIPTED_PROTECTION: u8 = 63;
const ACTION_TRIGGER: u8 = 0x01;
const SCENE_THREE_EXIT_TIME: u16 = 180;
const SCENE_FOUR_EXIT_TIME: u16 = 144;
const SCENE_FIVE_EXIT_TIME: u16 = 227;
const SCENE_TWENTY_FIVE_EXIT_TIME: u16 = 124;
pub const SCENE_PALETTE_COLORS: usize = 128;

/// Source-complete entries of the indexed scene/controller table. The scene
/// numbers identify authored records, not inferred cinematic roles. Other
/// entries cannot be substituted with Scene9's genuinely empty stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredSceneAction {
    /// Record 3, `$0D:C4F3`: request the next scene at update 180.
    Scene3,
    /// Record 4, `$0D:BEB4`: request the next scene at update 144.
    Scene4,
    /// Record 5, `$0D:BED3`: disable projection correction, then exit at 227.
    Scene5,
    /// Record 9, `$0D:C191`: a non-null, empty action that still advances time.
    Scene9,
    /// Record 25, `$0D:BEBB`: request the next scene at update 124.
    Scene25,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerAction {
    /// `$0D:BDDA`, installed by the triggered consumable.
    TriggeredProjectile,
    /// `$0D:BF63`, forced retreat when the scene inhibits ordinary play.
    ForcedRetreat,
    Scene(AuthoredSceneAction),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerActionState {
    pub action: Option<PlayerAction>,
    /// Stream-local time (6C16), writable by individual services.
    pub elapsed: u16,
    /// Companion counter (6C18), cleared by installation and termination.
    /// Its other producers are not part of this stream.
    pub auxiliary_counter: u16,
    /// Update count (6C1A), retained by stream installation and termination.
    pub total_updates: u16,
}

impl PlayerActionState {
    /// The original installer resets only when the selected stream changes.
    pub fn install_triggered_projectile(&mut self) {
        self.install(PlayerAction::TriggeredProjectile);
    }

    pub fn install(&mut self, action: PlayerAction) {
        if self.action != Some(action) {
            self.action = Some(action);
            self.elapsed = 0;
            self.auxiliary_counter = 0;
        }
    }

    pub(super) fn stop(&mut self) {
        self.action = None;
        self.elapsed = 0;
        self.auxiliary_counter = 0;
    }

    fn finish_visit(&mut self) {
        // Overflow suppresses BOTH stores; elapsed remains at the maximum.
        let next = self.elapsed.wrapping_add(1);
        if next != 0 {
            self.elapsed = next;
            self.total_updates = self.total_updates.wrapping_add(1);
        }
    }
}

/// Canonical shared player-service byte (1E0D). Each service changes only
/// its own bits; the protection path also observes the minimum override.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerServiceFlags(u8);

impl PlayerServiceFlags {
    const MINIMUM_PROTECTION: u8 = 0x01;
    const RESTORE_PALETTE: u8 = 0x10;

    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
    pub const fn bits(self) -> u8 {
        self.0
    }
    pub const fn minimum_protection(self) -> bool {
        self.0 & Self::MINIMUM_PROTECTION != 0
    }
    pub const fn palette_restoration_requested(self) -> bool {
        self.0 & Self::RESTORE_PALETTE != 0
    }
    fn request_palette_restoration(&mut self) {
        self.0 |= Self::RESTORE_PALETTE;
    }
}

/// Scene's live and saved packed palette. Snapshotting copies complete color
/// words, including the unused top bit; it neither edits colors nor requests
/// a display refresh (`$07:EBB8`). Rendering converts these at presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenePalette {
    pub colors: [u16; SCENE_PALETTE_COLORS],
    pub saved_colors: [u16; SCENE_PALETTE_COLORS],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionTiming {
    Always,
    At(u16),
    Interval { start: u16, end: u16 },
}

impl ActionTiming {
    fn applies(self, elapsed: u16) -> bool {
        match self {
            Self::Always => true,
            Self::At(at) => elapsed == at,
            Self::Interval { start, end } => (start..end).contains(&elapsed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionService {
    CheckEarlyDetonation,
    SavePalette,
    Stop,
    RequestRestoration,
    Detonate,
    DisableViewOptions,
    RefreshProtection,
    ClearActionTrigger,
    RequestRetreatTransition,
    RequestRetreatAudio,
    InstallRetreatCamera,
    RequestSceneExit,
    DisableProjectionCorrection,
}

// Preserve the source order, including Stop before the earlier-time events.
const TRIGGERED_SERVICES: [(ActionTiming, ActionService); 5] = [
    (
        ActionTiming::Interval {
            start: EARLY_TRIGGER_START,
            end: EARLY_TRIGGER_END,
        },
        ActionService::CheckEarlyDetonation,
    ),
    (ActionTiming::At(0), ActionService::SavePalette),
    (ActionTiming::At(COMPLETION_TIME), ActionService::Stop),
    (
        ActionTiming::At(RESTORATION_TIME),
        ActionService::RequestRestoration,
    ),
    (ActionTiming::At(DETONATION_TIME), ActionService::Detonate),
];

const RETREAT_SERVICES: [(ActionTiming, ActionService); 6] = [
    (ActionTiming::At(0), ActionService::DisableViewOptions),
    (ActionTiming::Always, ActionService::RefreshProtection),
    (ActionTiming::At(0), ActionService::ClearActionTrigger),
    (ActionTiming::At(0), ActionService::RequestRetreatTransition),
    (ActionTiming::At(0), ActionService::RequestRetreatAudio),
    (
        ActionTiming::At(RETREAT_CAMERA_TIME),
        ActionService::InstallRetreatCamera,
    ),
];

const SCENE_THREE_SERVICES: [(ActionTiming, ActionService); 1] = [(
    ActionTiming::At(SCENE_THREE_EXIT_TIME), ActionService::RequestSceneExit,
)];
const SCENE_FOUR_SERVICES: [(ActionTiming, ActionService); 1] = [(
    ActionTiming::At(SCENE_FOUR_EXIT_TIME), ActionService::RequestSceneExit,
)];
const SCENE_FIVE_SERVICES: [(ActionTiming, ActionService); 2] = [
    (ActionTiming::At(0), ActionService::DisableProjectionCorrection),
    (ActionTiming::At(SCENE_FIVE_EXIT_TIME), ActionService::RequestSceneExit),
];
const SCENE_TWENTY_FIVE_SERVICES: [(ActionTiming, ActionService); 1] = [(
    ActionTiming::At(SCENE_TWENTY_FIVE_EXIT_TIME), ActionService::RequestSceneExit,
)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerActionError {
    World(WorldInputError),
    Camera(AuxiliaryCameraError),
    MissingSpawnDefaults,
    MissingPlayerAction(ObjectId),
    MissingConfiguration,
    MissingPalette,
    MissingProjectileTrigger,
    MissingServiceFlags,
    MissingSceneTransition,
    MissingCinematicSignals,
    MissingCameraDispatch(ObjectId),
}

impl From<WorldInputError> for PlayerActionError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn state<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut PlayerActionState, PlayerActionError> {
    world
        .player_mut(objects, owner)?
        .action
        .as_mut()
        .ok_or(PlayerActionError::MissingPlayerAction(owner))
}

pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    input: InputState,
) -> Result<(), PlayerActionError> {
    if world
        .scripted_view_active()
        .ok_or(PlayerActionError::MissingSpawnDefaults)?
    {
        return Ok(());
    }
    let player = state(objects, world, owner)?;
    let mut decision_time = player.elapsed;
    let Some(action) = player.action else {
        return Ok(());
    };
    let services: &[_] = match action {
        PlayerAction::TriggeredProjectile => &TRIGGERED_SERVICES,
        PlayerAction::ForcedRetreat => &RETREAT_SERVICES,
        PlayerAction::Scene(scene) => match scene {
            AuthoredSceneAction::Scene3 => &SCENE_THREE_SERVICES,
            AuthoredSceneAction::Scene4 => &SCENE_FOUR_SERVICES,
            AuthoredSceneAction::Scene5 => &SCENE_FIVE_SERVICES,
            AuthoredSceneAction::Scene9 => &[],
            AuthoredSceneAction::Scene25 => &SCENE_TWENTY_FIVE_SERVICES,
        },
    };
    for &(timing, service) in services {
        if !timing.applies(decision_time) {
            continue;
        }
        match service {
            ActionService::CheckEarlyDetonation => {
                let triggered = world
                    .projectile_trigger
                    .as_ref()
                    .ok_or(PlayerActionError::MissingProjectileTrigger)?
                    .activation
                    != 0;
                if triggered || input.pressed.contains(Button::X) {
                    state(objects, world, owner)?.elapsed = DETONATION_TIME;
                    decision_time = DETONATION_TIME;
                }
            }
            ActionService::SavePalette => {
                if world
                    .scene
                    .player_configuration
                    .ok_or(PlayerActionError::MissingConfiguration)?
                    != SPECIAL_CONFIGURATION
                {
                    let palette = world
                        .palette
                        .as_mut()
                        .ok_or(PlayerActionError::MissingPalette)?;
                    palette.saved_colors = palette.colors;
                }
            }
            ActionService::Stop => state(objects, world, owner)?.stop(),
            ActionService::RequestRestoration => world
                .player_service_flags
                .as_mut()
                .ok_or(PlayerActionError::MissingServiceFlags)?
                .request_palette_restoration(),
            ActionService::Detonate => {
                world
                    .projectile_trigger
                    .as_mut()
                    .ok_or(PlayerActionError::MissingProjectileTrigger)?
                    .activation = 1
            }
            ActionService::DisableViewOptions => world.player_view_options_enabled = Some(false),
            ActionService::RefreshProtection => {
                world
                    .player_mut(objects, owner)?
                    .contact
                    .as_mut()
                    .ok_or(WorldInputError::MissingPlayerContact(owner))?
                    .hit
                    .secondary_protection = SCRIPTED_PROTECTION;
            }
            ActionService::ClearActionTrigger => {
                world
                    .player_mut(objects, owner)?
                    .auxiliary
                    .as_mut()
                    .ok_or(WorldInputError::MissingAuxiliary(owner))?
                    .action_flags &= !ACTION_TRIGGER;
            }
            ActionService::RequestRetreatTransition => world
                .scene_transition
                .as_mut()
                .ok_or(PlayerActionError::MissingSceneTransition)?
                .request_forced_retreat(),
            ActionService::RequestRetreatAudio => world
                .audio
                .request_music_control(super::path_sound::MusicControlRequest::ForcedRetreat),
            ActionService::InstallRetreatCamera => player_camera_auxiliary::install(
                objects,
                world,
                owner,
                AuxiliaryCameraTask::Initialize(OrbitStyle::Retreat),
            )
            .map_err(PlayerActionError::Camera)?,
            ActionService::RequestSceneExit => world
                .cinematic_signals
                .as_mut()
                .ok_or(PlayerActionError::MissingCinematicSignals)?
                .exit_requested = true,
            ActionService::DisableProjectionCorrection => world
                .player_mut(objects, owner)?
                .camera_dispatch
                .as_mut()
                .ok_or(PlayerActionError::MissingCameraDispatch(owner))?
                .projection_correction_disabled = true,
        }
    }
    // A stop service clears the stored time, not this visit's decision time.
    // The final increment therefore leaves elapsed one even after termination.
    state(objects, world, owner)?.finish_visit();
    Ok(())
}
