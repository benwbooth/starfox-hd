//! Ordered player-strategy prefix (`$06:9C27..9D08`). Publishes pilot limits,
//! warnings, the parallel action and live equipment/motion before the separate
//! player-mode dispatcher. This is not the mode-specific movement strategy.

use super::path_control::PlayerTarget;
use super::path_motion::PublishedPlayerMotion;
use super::path_shots::ActiveShots;
use super::path_sound::AuthoredCue;
use super::player_action::{self, PlayerActionError};
use super::proximity_warning::{self, WarningControl, WarningError, WarningView};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{InputState, ObjectId, ObjectStore, SoundEvent, Vector3};

#[cfg(test)]
#[path = "player_visit_tests.rs"]
mod tests;

const SHIELD_CAPACITIES: [u8; 6] = [32, 32, 40, 40, 24, 24];
const CHARGE_THRESHOLDS: [u8; 6] = [25, 25, 35, 35, 10, 10];
const WARNING_DELAY: u8 = 10;
const LOW_SHIELD_BOUND: u8 = 13;
const CRITICAL_SHIELD_BOUND: u8 = 5;
const LOW_SHIELD_CUE: u8 = 22;
const CRITICAL_SHIELD_CUE: u8 = 23;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerVisitControl {
    /// Full pilot byte (6BFF). Codes outside the six-entry table use pilot
    /// zero's limits, but the proximity gate still sees the original byte.
    pub pilot_code: u8,
    /// Distinct 6B77 bits 20/40, not the displacement-suppression bit 04.
    pub warning_inhibited: bool,
    pub guarded_pilot_warning_ready: bool,
    /// Low-shield cue delay (6BE8), held at ten but wrapping elsewhere.
    pub shield_warning_clock: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PublishedConsumables {
    pub packed_count: u8,
    pub kind: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerVisitError {
    World(WorldInputError),
    MissingControl(ObjectId),
    MissingSpawnDefaults,
    Warning(WarningError),
    Action(PlayerActionError),
}

impl From<WorldInputError> for PlayerVisitError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn control(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<PlayerVisitControl, PlayerVisitError> {
    world
        .player(objects, owner)?
        .visit
        .ok_or(PlayerVisitError::MissingControl(owner))
}

/// Execute once for an admitted player strategy visit. Publications belong to
/// the caller, which need not be the primary or path-selected player. Earlier
/// side effects survive diagnostics; the scene wrapper prevents replay.
pub fn begin(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    input: InputState,
) -> Result<(), PlayerVisitError> {
    let player_control = control(objects, world, owner)?;
    let pilot = usize::from(player_control.pilot_code);
    let pilot = if pilot < SHIELD_CAPACITIES.len() {
        pilot
    } else {
        0
    };
    world.active_charge_threshold = Some(CHARGE_THRESHOLDS[pilot]);
    let capacity = SHIELD_CAPACITIES[pilot];
    world.active_shield_capacity = Some(capacity);
    let hit = &mut world
        .player_mut(objects, owner)?
        .contact
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit;
    // CMP/BPL clamps by the wrapped byte subtraction's sign, not carry.
    if (capacity.wrapping_sub(hit.reserve_shield) as i8) < 0 {
        hit.reserve_shield = capacity;
    }

    let paused = world
        .spawn_defaults
        .ok_or(PlayerVisitError::MissingSpawnDefaults)?
        .run_when_paused;
    if !paused {
        let mode = world
            .player(objects, owner)?
            .auxiliary
            .ok_or(WorldInputError::MissingAuxiliary(owner))?
            .mode;
        let warning = WarningControl {
            globally_disabled: false,
            movement_mode: mode,
            inhibited: player_control.warning_inhibited,
            pilot_code: player_control.pilot_code,
            guarded_pilot_ready: player_control.guarded_pilot_warning_ready,
        };
        let view = world.fixed_players[0]
            .and_then(|id| objects.get(id))
            .zip(world.primary_view_heading)
            .map(|(actor, bearing)| WarningView {
                position: actor.base.position,
                bearing,
            });
        proximity_warning::update_with_view(
            objects,
            owner,
            world.primary_player,
            warning,
            view,
            &mut world.audio,
        )
        .map_err(PlayerVisitError::Warning)?;
    }

    objects
        .get_mut(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .flags
        .view_side_filter = false;
    // No consumer can observe an invented position before the later position
    // store. If a prior snapshot exists, its position survives this clear.
    if let Some(motion) = &mut world.published_motion {
        motion.delta = Vector3::default();
    }
    let clock = &mut world
        .player_mut(objects, owner)?
        .visit
        .as_mut()
        .expect("validated visit control")
        .shield_warning_clock;
    if *clock != WARNING_DELAY {
        *clock = clock.wrapping_add(1);
        if *clock == WARNING_DELAY {
            let shield = world
                .player(objects, owner)?
                .contact
                .expect("validated contact owner")
                .hit
                .reserve_shield;
            if (shield.wrapping_sub(LOW_SHIELD_BOUND) as i8) < 0 {
                let cue = if (shield.wrapping_sub(CRITICAL_SHIELD_BOUND) as i8) < 0 {
                    CRITICAL_SHIELD_CUE
                } else {
                    LOW_SHIELD_CUE
                };
                world.audio.queue(SoundEvent::Authored(AuthoredCue::new(
                    cue,
                    0,
                    if world.primary_player == Some(owner) {
                        PlayerTarget::Primary
                    } else {
                        PlayerTarget::Secondary
                    },
                )));
            }
        }
    }
    if paused {
        world.bind_shots(objects, owner, ActiveShots::from_count(0))?;
    }
    player_action::advance(objects, world, owner, input).map_err(PlayerVisitError::Action)?;

    world.scene.active_shield = Some(
        world
            .player(objects, owner)?
            .contact
            .expect("validated contact owner")
            .hit
            .reserve_shield,
    );
    let equipment = world
        .player(objects, owner)?
        .equipment
        .ok_or(WorldInputError::MissingEquipment(owner))?;
    world.active_consumables = Some(PublishedConsumables {
        packed_count: equipment.packed_consumables,
        kind: equipment.consumable_type,
    });
    world.scene.active_weapon_level = Some(equipment.weapon_level);

    let actor = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    world.published_motion = Some(PublishedPlayerMotion {
        position: actor.base.position,
        delta: Vector3::default(),
    });
    let mode = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode;
    world.published_motion = Some(PublishedPlayerMotion::capture(
        actor,
        actor.extension.path_state.motion_delta,
        mode,
    ));
    let age = &mut objects
        .get_mut(owner)
        .expect("validated player")
        .extension
        .path_state
        .script_value;
    *age = age.saturating_add(1);
    Ok(())
}
