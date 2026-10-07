//! Indexed scene entry ($06:A8D2..A987). Only complete path/action pairs
//! may be installed. Unsupported entries fault after the source allocation
//! prefix; they never become an empty path or the scene-nine empty action.
use super::player_action::{AuthoredSceneAction, PlayerAction};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId, OBJECT_CAPACITY};

const SKIP_FIRST: u8 = 254;
const SKIP_SECOND: u8 = 255;
const RESTORE_SAVED_SCENE: u8 = 22;
const AUTHORED_SCENE_COUNT: u8 = 30;
const SCENE_NINE: u8 = 9;
const SCENE_NINE_COMPANION_SEED: u16 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneInstallError {
    World(WorldInputError),
    MissingSelection,
    MissingSavedSelection(ObjectId),
    MissingSpawnDefaults,
    ObjectPoolExhausted,
    MissingAction(ObjectId),
    /// The fresh actor remains owned by the faulted scene for diagnosis.
    UnsupportedScene {
        selection: u8,
        table_index: u8,
        actor: ObjectId,
    },
}

impl From<WorldInputError> for SceneInstallError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

pub fn install(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    player: ObjectId,
) -> Result<Option<ObjectId>, SceneInstallError> {
    let mut selection = world
        .scene_selection
        .ok_or(SceneInstallError::MissingSelection)?;
    if matches!(selection, SKIP_FIRST | SKIP_SECOND) {
        return Ok(None);
    }
    if selection == RESTORE_SAVED_SCENE {
        selection = world
            .player(objects, player)?
            .saved_scene_selection
            .ok_or(SceneInstallError::MissingSavedSelection(player))?;
        world.scene_selection = Some(selection);
        // The source does NOT repeat the sentinel test after substitution.
    }
    if objects.len() == OBJECT_CAPACITY {
        return Err(SceneInstallError::ObjectPoolExhausted);
    }
    let defaults = world
        .spawn_defaults()
        .ok_or(SceneInstallError::MissingSpawnDefaults)?;
    let mut actor = Object::new_authored(
        ObjectKind::Effect,
        ShapeId::EMPTY,
        Behavior::FollowPath,
        defaults,
    );
    // New actors already have the zero world pose explicitly written here.
    actor.extension.path_state.needs_path_initialization = true;
    let head = objects.active_ids().first().copied();
    let created = objects
        .allocate_after(head, actor)
        .ok_or(SceneInstallError::ObjectPoolExhausted)?;
    let table_index = if selection < AUTHORED_SCENE_COUNT {
        selection
    } else {
        0
    };
    if table_index != SCENE_NINE {
        return Err(SceneInstallError::UnsupportedScene {
            selection,
            table_index,
            actor: created,
        });
    }
    let action = world
        .player_mut(objects, player)?
        .action
        .as_mut()
        .ok_or(SceneInstallError::MissingAction(player))?;
    // Unlike conditional action replacement, indexed entry always resets
    // these two counters, even when the action identity did not change.
    // The source also zeroes 1DA5, but that is only the interpreter's
    // per-visit scratch copy of 6C16 ($0D:BCF4), reloaded before any read.
    action.action = Some(PlayerAction::Scene(AuthoredSceneAction::Scene9));
    action.elapsed = 0;
    action.auxiliary_counter = SCENE_NINE_COMPANION_SEED;
    let actor = objects.get_mut(created).expect("fresh scene actor");
    actor.base.path = Some(super::authored_paths::SCENE_NINE);
    actor.base.hit_points = 1;
    actor.base.attack_power = 1;
    actor.base.flags.collision_disabled = true;
    Ok(Some(created))
}

#[cfg(test)]
#[path = "scene_install_tests.rs"]
mod tests;
