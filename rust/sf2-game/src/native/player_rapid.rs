//! Rapid-fire tail of the player weapon service (`$07:D7E4..D8A2`) and
//! flight launcher (`$07:D988..DAB1`). The enclosing strategy still owns
//! the preceding aim publication and consumable service, then charged fire.

use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::weapon_dispatch::{self, LaunchError, LaunchRequest, LaunchWorld, PathWeapon};
use super::weapon_launch::{LaunchParameters, MuzzleOffset};
use super::weapon_rapid::RapidWeapon;
use super::{Angle, Button, InputState, ObjectId, ObjectSpawnDefaults, ObjectStore, Vector3};

#[cfg(test)]
#[path = "player_rapid_tests.rs"]
mod tests;

const QUEUE_INCREMENT: u8 = 16;
const QUEUE_ADMISSION: u8 = 64;
const QUEUE_MASK: u8 = 0xF0;
const DELAY_MASK: u8 = 0x0F;
const SUCCESS_DELAY: u8 = 1;
const LINKED_MUZZLE_DISTANCE: i8 = 20;
const ALTERNATE_PITCH_OFFSET: i8 = -4;

/// Independently retained player aiming state, not a fresh target candidate.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RapidAim {
    /// Barrel-roll step (6ADD).
    pub roll_step: Angle,
    /// Retained aim point 6B9C/9E/A0, published by $07:AA14.
    pub retained_aim: Vector3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RapidError {
    World(WorldInputError),
    MissingWeapons,
    MissingSpawnDefaults,
    MissingFixedView,
    MissingActor(ObjectId),
    Launch(LaunchError),
}

fn control<'a>(
    world: &'a mut ScenePathWorld,
    objects: &ObjectStore,
    owner: ObjectId,
) -> Result<&'a mut u8, RapidError> {
    Ok(&mut world
        .player_mut(objects, owner)
        .map_err(RapidError::World)?
        .charge
        .as_mut()
        .ok_or(RapidError::World(WorldInputError::MissingPlayerCharge(
            owner,
        )))?
        .rapid_control)
}

fn defaults(world: &ScenePathWorld) -> Result<ObjectSpawnDefaults, RapidError> {
    world.spawn_defaults().ok_or(RapidError::MissingSpawnDefaults)
}

fn dispatch(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    weapon: RapidWeapon,
    defaults: ObjectSpawnDefaults,
) -> Result<Option<ObjectId>, RapidError> {
    let caller_inputs = world.caller_weapon_inputs(objects, owner);
    let weapons = world.weapons.as_mut().ok_or(RapidError::MissingWeapons)?;
    weapon_dispatch::launch(
        objects,
        resources,
        owner,
        LaunchRequest {
            weapon: PathWeapon::Rapid(weapon),
            parameters: weapons.parameters,
            defaults,
        },
        &mut LaunchWorld {
            caller_inputs,
            fallback: weapons.fallback,
            published_pitch: weapons.published_pitch,
            primary: world.primary_player,
            secondary: world.secondary_player,
            primary_auxiliary_mode: None,
            hostile_counts: Some(&mut weapons.hostile_counts),
            random: &mut world.random,
        },
    )
    .map_err(RapidError::Launch)
}

/// The helper's no-weapon return retains its non-null auxiliary/view cursor.
/// Its enclosing flight launcher tests that cursor, not its failure flag;
/// therefore these exceptional levels consume a queue entry without a shot.
fn flight_variant(level: u8) -> Option<RapidWeapon> {
    if (level.wrapping_sub(1) as i8) < 0 {
        return None;
    }
    Some(match level {
        2 => RapidWeapon::Upgraded,
        3 => RapidWeapon::Maximum,
        _ => RapidWeapon::Basic,
    })
}

fn flight_helper(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    level: u8,
    distance: i8,
    defaults: ObjectSpawnDefaults,
) -> Result<bool, RapidError> {
    let parameters = &mut world
        .weapons
        .as_mut()
        .ok_or(RapidError::MissingWeapons)?
        .parameters;
    parameters.pitch_offset = 0;
    parameters.yaw_offset = 0;
    parameters.target = None;
    let Some(weapon) = flight_variant(level) else {
        return Ok(true);
    };
    parameters.muzzle = MuzzleOffset {
        x: 0,
        y: 0,
        z: distance,
    };
    Ok(dispatch(objects, world, resources, owner, weapon, defaults)?.is_some())
}

fn launch_flight(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<bool, RapidError> {
    let level = world
        .player(objects, owner)
        .map_err(RapidError::World)?
        .equipment
        .ok_or(RapidError::World(WorldInputError::MissingEquipment(owner)))?
        .weapon_level;
    world.scene.active_weapon_level = Some(level);
    // Shared initializer mode 1B84 bit 02, not strategy pause or 1D72.
    if world.scripted_view_active().ok_or(RapidError::MissingSpawnDefaults)? {
        return Ok(false);
    }
    let defaults = defaults(world)?;
    let charge = world
        .player(objects, owner)
        .map_err(RapidError::World)?
        .charge
        .ok_or(RapidError::World(WorldInputError::MissingPlayerCharge(
            owner,
        )))?;
    if !charge.linked_mode || charge.linked_muzzle_disabled {
        return flight_helper(objects, world, resources, owner, level, 0, defaults);
    }
    let distance = if matches!(level, 1 | 2) {
        -LINKED_MUZZLE_DISTANCE
    } else {
        LINKED_MUZZLE_DISTANCE
    };
    let view = world.fixed_players[0].ok_or(RapidError::MissingFixedView)?;
    let position = objects
        .get(view)
        .ok_or(RapidError::MissingActor(view))?
        .base
        .position;
    let actor = objects
        .get_mut(owner)
        .ok_or(RapidError::MissingActor(owner))?;
    let original = (actor.base.position.x, actor.base.position.z);
    actor.base.position.x = position.x;
    actor.base.position.z = position.z;
    // On an ordinary allocation rejection, restore the source origin. A
    // diagnostic fault retains earlier writes and must not be retried.
    let result = flight_helper(objects, world, resources, owner, level, distance, defaults)?;
    let actor = objects.get_mut(owner).expect("live rapid caller");
    (actor.base.position.x, actor.base.position.z) = original;
    Ok(result)
}

/// One visit to the rapid tail, after aiming/consumable work. Inputs must be
/// the source-processed player controls. The scene owner latches any error.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    input: InputState,
) -> Result<(), RapidError> {
    let mode = world
        .player(objects, owner)
        .map_err(RapidError::World)?
        .auxiliary
        .ok_or(RapidError::World(WorldInputError::MissingAuxiliary(owner)))?
        .mode;
    if mode & 0xF0 != 0x10 {
        if !input.pressed.contains(Button::B) {
            return Ok(());
        }
        if world
            .action_gate
            .ok_or(RapidError::World(WorldInputError::MissingActionGate))?
            .code
            != 0
        {
            return Ok(());
        }
        if world.scripted_view_active().ok_or(RapidError::MissingSpawnDefaults)? {
            return Ok(());
        }
        let defaults = defaults(world)?;
        world
            .weapons
            .as_mut()
            .ok_or(RapidError::MissingWeapons)?
            .parameters = LaunchParameters {
            pitch_offset: ALTERNATE_PITCH_OFFSET,
            ..Default::default()
        };
        dispatch(
            objects,
            world,
            resources,
            owner,
            RapidWeapon::Alternate,
            defaults,
        )?;
        return Ok(());
    }
    if input.pressed.contains(Button::B) {
        let value = control(world, objects, owner)?;
        if (value.wrapping_sub(QUEUE_ADMISSION) as i8) < 0 {
            *value = value.wrapping_add(QUEUE_INCREMENT);
        }
    }
    let value = *control(world, objects, owner)?;
    if value & DELAY_MASK != 0 {
        *control(world, objects, owner)? = value.wrapping_sub(1);
    } else if value & QUEUE_MASK != 0 && launch_flight(objects, world, resources, owner)? {
        let value = control(world, objects, owner)?;
        *value = (*value & QUEUE_MASK).wrapping_sub(QUEUE_INCREMENT) | SUCCESS_DELAY;
    }
    Ok(())
}
