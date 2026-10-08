//! Scene-player initializer (`$06:82F9..832B`, with `$06:832C..8361`): the
//! strategy a map installs for the player of an indexed scene. Its first
//! visit formats the player and installs the scene entry at `$06:845C`.
//! The secondary-side entry (`$06:82ED`) differs only in the hit side.

use super::actor_auxiliary::{AuxiliaryError, AuxiliaryRecord, DeathHandler};
use super::hit_response::HitSide;
use super::path_runtime::PathRuntime;
use super::player_scene_entry::SceneEntryPhase;
use super::player_scene_reset::{self, PlayerSceneResetError};
use super::player_storage::{self, PlayerStorageError, PlayerStorageInputs};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Behavior, ObjectId, ObjectStore};

const SCENE_PLAYER_CONFIGURATION: u8 = 0;
const SPRITE_SIZE: u8 = 1;
const PROVISIONAL_HEALTH: u8 = u8::MAX;
const PROVISIONAL_ATTACK: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneInitError {
    World(WorldInputError),
    Auxiliary(AuxiliaryError),
    Storage(PlayerStorageError),
    Reset(PlayerSceneResetError),
    MissingPilot,
    MissingShield,
    MissingScore,
    MissingReflectionPolicy,
    MissingTargetSelection(ObjectId),
}

impl From<WorldInputError> for SceneInitError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

pub fn initialize(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    runtime: &mut PathRuntime,
    owner: ObjectId,
    side: HitSide,
) -> Result<(), SceneInitError> {
    let actor = objects
        .get_mut(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    actor.base.contacts.hit_side = side;
    actor.base.contacts.run_when_paused = true;
    world.scene.player_configuration = Some(SCENE_PLAYER_CONFIGURATION);
    let actor = objects.get_mut(owner).expect("validated scene player");
    actor.extension.texture_scroll_x = SPRITE_SIZE;
    // `$06:832C`: provisional health and attack are replaced by the shared
    // storage entry below, which also releases the death-routine record
    // registered here; the registration still costs its allocation first.
    actor.base.flags.collision_disabled = true;
    actor.base.hit_points = PROVISIONAL_HEALTH;
    actor.base.attack_power = PROVISIONAL_ATTACK;
    actor.base.behavior = Behavior::PlayerSceneEntry(SceneEntryPhase::ClearLaunchCounts);
    let mut auxiliary = std::mem::take(&mut actor.extension.auxiliary);
    let registered = auxiliary.set(
        &mut runtime.resources,
        owner,
        AuxiliaryRecord::DeathHandler(DeathHandler::ScenePlayer),
    );
    objects.get_mut(owner).expect("validated scene player").extension.auxiliary = auxiliary;
    registered.map_err(SceneInitError::Auxiliary)?;
    let inputs = PlayerStorageInputs {
        pilot_code: world.scene.active_pilot.ok_or(SceneInitError::MissingPilot)?,
        reserve_shield: world.scene.active_shield.ok_or(SceneInitError::MissingShield)?,
        score: world.published_score.ok_or(SceneInitError::MissingScore)?,
    };
    player_storage::initialize(objects, world, runtime, owner, inputs)
        .map_err(SceneInitError::Storage)?;
    player_scene_reset::reset_services(objects, world, owner).map_err(SceneInitError::Reset)?;
    player_scene_reset::reset_background_base(runtime);
    let reflect_all = world
        .reflect_all_contacts
        .ok_or(SceneInitError::MissingReflectionPolicy)?;
    world
        .player_mut(objects, owner)?
        .target_selection
        .as_mut()
        .ok_or(SceneInitError::MissingTargetSelection(owner))?
        .initialize(reflect_all);
    Ok(())
}
