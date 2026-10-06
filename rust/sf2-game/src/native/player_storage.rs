//! Scene-owned player auxiliary storage (`$06:8260..82B4`).
//!
//! This allocation/clear/publication prefix is shared by the player entries.
//! It stops before their separate view-selection and object-formatting tail
//! (`$06:82B7`), global reset and next-visit mode initialization. None of those
//! services is implied by obtaining a player record here.

use super::path_runtime::{PathRuntime, PathRuntimeError};
use super::program_resources::{AllocationFailure, ProgramResources};
use super::program_state::ProgramData;
use super::scene_path_world::{PlayerPathRecords, ScenePathWorld, WorldInputError};
use super::{Angle, ObjectId, ObjectStore, Rotation, Vector3};

pub use super::path_score::PlayerScore;

const PLAYER_RECORD_COST: u16 = 472;
const PLAYER_GROUP: u8 = u8::MAX;
const PLAYER_OBJECT_HEALTH: u8 = 1;
const FINE_ANGLE_SHIFT: u32 = 8;

/// Published inputs read by this prefix, not inferred pilot profiles or
/// guessed defaults. Shield is copied before the later shared clamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerStorageInputs {
    pub pilot_code: u8,
    pub reserve_shield: u8,
    pub score: PlayerScore,
}

/// Additional domain fields of the player allocation. Disjoint player
/// services keep their existing records in ScenePathWorld, bound to this
/// exact allocation. Capacity is charged once for the complete record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerStorage {
    /// `$06:D9ED..D9FB` seeds the high bytes of fine pitch and yaw and the
    /// separate bank byte from the actor's current orientation.
    pub fine_pitch: u16,
    pub fine_yaw: u16,
    pub bank: Angle,
    /// Initial retained shield (`6C38`), distinct from live reserve `6C00`.
    pub retained_shield: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerStorageError {
    Runtime(PathRuntimeError),
    Allocation(AllocationFailure),
    World(WorldInputError),
    MissingStorage(ObjectId),
}

impl From<WorldInputError> for PlayerStorageError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

/// Read the canonical allocation. A still-live object whose programs were
/// released cannot expose a stale player payload through its prior binding.
pub fn get<'a>(
    objects: &ObjectStore,
    resources: &'a ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<&'a PlayerStorage, PlayerStorageError> {
    let storage = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .player_storage
        .ok_or(PlayerStorageError::MissingStorage(owner))?;
    match resources.get_owned(owner, storage) {
        Some(ProgramData::PlayerStorage(record)) => Ok(record),
        _ => Err(PlayerStorageError::MissingStorage(owner)),
    }
}

/// Replace the entire actor-owned resource chain, allocate a player record,
/// clear its supported semantic fields, then publish selection and copied
/// pilot/shield/score. Earlier release survives an allocation failure, exactly
/// as in the source; the scene wrapper latches the failure against retry.
pub fn replace(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &mut PathRuntime,
    owner: ObjectId,
    inputs: PlayerStorageInputs,
) -> Result<(), PlayerStorageError> {
    runtime
        .release_actor_programs(objects, owner)
        .map_err(PlayerStorageError::Runtime)?;
    world.release_player_bindings(owner);
    let actor = objects.get(owner).expect("release validated actor");
    let storage = PlayerStorage {
        fine_pitch: u16::from(actor.base.pitch.units()) << FINE_ANGLE_SHIFT,
        fine_yaw: u16::from(actor.base.yaw.units()) << FINE_ANGLE_SHIFT,
        bank: actor.base.roll,
        retained_shield: inputs.reserve_shield,
    };
    let resource = runtime
        .resources
        .allocate_owned(
            owner,
            PLAYER_RECORD_COST,
            ProgramData::PlayerStorage(storage),
        )
        .map_err(|error| PlayerStorageError::Allocation(error.reason))?;
    let actor = objects.get_mut(owner).expect("validated actor");
    actor.base.player_storage = Some(resource);
    // Player auxiliary ownership replaces the source actor's path field;
    // a previously imported actor program must not survive as a second use.
    actor.base.path = None;

    // The original clears the complete 472-byte player allocation, not just
    // selected gameplay fields. Every currently ported player service gets a
    // real zero-valued record; unrelated scene publications remain absent.
    let records = PlayerPathRecords {
        contact: Some(super::scene_contact::PlayerContactControl {
            hit: super::player_hit_control::PlayerHitControl {
                reserve_shield: inputs.reserve_shield,
                ..Default::default()
            },
            ..Default::default()
        }),
        protection: Some(Default::default()),
        auxiliary: Some(super::path_program::SelectedAuxiliaryState {
            mode: 0,
            action_flags: 0,
            stored_world_position: Vector3::default(),
            stored_rotation: Rotation::default(),
        }),
        charge: Some(Default::default()),
        rapid_aim: Some(Default::default()),
        action: Some(Default::default()),
        consumable: Some(Default::default()),
        target_control: Some(Default::default()),
        visit: Some(super::player_visit::PlayerVisitControl {
            pilot_code: inputs.pilot_code,
            ..Default::default()
        }),
        yaw_motion: Some(0),
        occupancy_exempt: Some(false),
        equipment: Some(Default::default()),
        score: Some(inputs.score),
        particles: Some(Default::default()),
        controlled_flags: Some(Default::default()),
        suppress_horizontal_follow: Some(false),
        carried: Some(Default::default()),
    };
    world.bind_player(objects, owner, records)?;
    world.bind_shots(
        objects,
        owner,
        super::path_shots::ActiveShots::from_count(0),
    )?;
    let actor = objects.get_mut(owner).expect("validated actor");
    actor.extension.spawn_group = PLAYER_GROUP;
    world.primary_player = Some(owner);
    actor.base.hit_points = PLAYER_OBJECT_HEALTH;
    Ok(())
}

#[cfg(test)]
#[path = "player_storage_tests.rs"]
mod tests;
