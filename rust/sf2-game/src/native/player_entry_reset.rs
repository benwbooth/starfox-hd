//! Player-entry reset through the action-gate branch (`$06:83F1..846B`).
//! This composes the complete world and movement resets before stopping the
//! retained action, hiding the craft and clearing launch counters. The next
//! entry phase still owns action-gate dispatch and strategy installation.

use super::player_motion_reset::{self, MotionResetError};
use super::positional_audio::PositionalAudio;
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::scene_world_reset::{self, RegionSelection, WorldResetError};
use super::{ObjectId, ObjectStore, ShapeId};

const CRUISE_SOUND: u8 = 4;
const ENTRY_HANDOFF_FLAGS: u8 = 0x50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryResetError {
    WorldReset(WorldResetError),
    Motion(MotionResetError),
    World(WorldInputError),
    MissingAction(ObjectId),
    MissingHandoff,
    MissingCameraDispatch(ObjectId),
    MissingWeaponState,
}

pub fn reset(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    positional: &mut PositionalAudio,
    owner: ObjectId,
) -> Result<(), EntryResetError> {
    scene_world_reset::clear(objects, world, RegionSelection::Clear)
        .map_err(EntryResetError::WorldReset)?;
    player_motion_reset::prepare_scene_entry(objects, world, resources, owner)
        .map_err(EntryResetError::Motion)?;
    finish(objects, world, positional, owner)
}

/// The final reset block (`$06:8413..846B`). Action termination retains
/// total update count. Silencing positional output retains its selected
/// identity and distance; pending nearest-actor selection is independent.
pub fn finish(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    positional: &mut PositionalAudio,
    owner: ObjectId,
) -> Result<(), EntryResetError> {
    world
        .player_mut(objects, owner)
        .map_err(EntryResetError::World)?
        .action
        .as_mut()
        .ok_or(EntryResetError::MissingAction(owner))?
        .stop();
    objects
        .get_mut(owner)
        .ok_or(EntryResetError::World(WorldInputError::MissingActor(owner)))?
        .base
        .shape = ShapeId::EMPTY;
    world.engine_sound_control = Some(super::player_engine_sound::EngineSoundControl::from_bits(
        CRUISE_SOUND,
    ));
    positional.silence_published();
    world
        .handoff
        .as_mut()
        .ok_or(EntryResetError::MissingHandoff)?
        .player_flags &= !ENTRY_HANDOFF_FLAGS;
    objects
        .get_mut(owner)
        .expect("validated player")
        .base
        .flags
        .collision_disabled = true;
    world
        .player_mut(objects, owner)
        .map_err(EntryResetError::World)?
        .camera_dispatch
        .as_mut()
        .ok_or(EntryResetError::MissingCameraDispatch(owner))?
        .style = None;
    // These are hostile-launch statistics, NOT the retained player-motion
    // publication. Resetting motion here would erase another service's data.
    world
        .weapons
        .as_mut()
        .ok_or(EntryResetError::MissingWeaponState)?
        .hostile_counts = Default::default();
    Ok(())
}

#[cfg(test)]
#[path = "player_entry_reset_tests.rs"]
mod tests;
