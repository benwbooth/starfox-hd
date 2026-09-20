//! Player rapid-shot creation ($0D:DCFD..DE37). Admission precedes allocation;
//! live count increments belong to the installed paths, not the launch call.

use super::path_shots::ActiveShots;
use super::weapon_dispatch::{LaunchError, LaunchWorld};
use super::weapon_launch::LaunchParameters;
use super::{
    authored_paths, weapon_creation, Angle, ObjectId, ObjectSpawnDefaults, ObjectStore, PathCursor,
    ShapeId, Vector3, OBJECT_CAPACITY,
};

const BASIC_SHAPE: ShapeId = ShapeId::from_catalog_index(358);
const UPGRADED_SHAPE: ShapeId = ShapeId::from_catalog_index(360);
const MAXIMUM_SHAPE: ShapeId = ShapeId::from_catalog_index(363);
const ALTERNATE_SPRITE: ShapeId = ShapeId::from_catalog_index(25);
const WEAPON_LEVEL_MASK: u8 = 0x03;
const SPRITE_LEVEL: u8 = 1;
const SPRITE_ATTACK: u8 = 2;
const MESH_ATTACK: u8 = 4;
const RAPID_LAUNCH_FRAME: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RapidWeapon {
    Basic,
    Upgraded,
    Maximum,
    Alternate,
}

impl RapidWeapon {
    pub const fn paths(self) -> &'static [PathCursor] {
        match self {
            Self::Alternate => &[authored_paths::ALTERNATE_RAPID_IMPACT_PROJECTILE],
            _ => &[authored_paths::RAPID_IMPACT_PROJECTILE],
        }
    }
}

/// Fresh observations of the FIRING actor's auxiliary record, not the path's
/// selected player or the global active-pilot equipment snapshot. Optional
/// fields are required only after the source branch actually reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallerWeaponInputs {
    pub owner: ObjectId,
    pub active_shots: Option<ActiveShots>,
    /// Full 6C06 equipment byte. Alternate creation masks its low two bits.
    pub weapon_level: Option<u8>,
    /// Player barrel-roll step (6ADD), zeroed for the basic rapid shot.
    pub roll_step: Option<Angle>,
    /// Retained aim point (6B9C/9E/A0), published at $07:AA14. This is not
    /// the target-candidate position (6BCC/CE/D0) or a fresh target lookup.
    pub retained_aim: Option<Vector3>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RapidLaunchError {
    MissingCallerInputs,
    WrongCallerInputs {
        expected: ObjectId,
        supplied: ObjectId,
    },
    MissingShotCount,
    MissingWeaponLevel,
    MissingRollStep,
    MissingRetainedAim,
    MissingAimProxy,
    MissingAimProxyActor(ObjectId),
}

pub(super) fn launch(
    objects: &mut ObjectStore,
    resources: &mut super::program_resources::ProgramResources<super::program_state::ProgramData>,
    caller: ObjectId,
    weapon: RapidWeapon,
    parameters: LaunchParameters,
    defaults: ObjectSpawnDefaults,
    world: &mut LaunchWorld<'_>,
) -> Result<Option<ObjectId>, LaunchError> {
    let inputs = world
        .caller_inputs
        .ok_or(RapidLaunchError::MissingCallerInputs)?;
    if inputs.owner != caller {
        return Err(RapidLaunchError::WrongCallerInputs {
            expected: caller,
            supplied: inputs.owner,
        }
        .into());
    }
    let level = if weapon == RapidWeapon::Alternate {
        let level = inputs
            .weapon_level
            .ok_or(RapidLaunchError::MissingWeaponLevel)?
            & WEAPON_LEVEL_MASK;
        if level == 0 {
            return Ok(None);
        }
        level
    } else {
        0
    };
    if !inputs
        .active_shots
        .ok_or(RapidLaunchError::MissingShotCount)?
        .admits_launch()
    {
        return Ok(None);
    }
    // Unlike the charged/simple launchers, both rapid admission gates above
    // execute even when the object pool is full. Later observations do not.
    if objects.len() == OBJECT_CAPACITY {
        return Ok(None);
    }
    let pitch = if weapon == RapidWeapon::Alternate {
        Some(
            world
                .published_pitch
                .ok_or(LaunchError::MissingPublishedPitch)?,
        )
    } else {
        None
    };
    let roll_step = match weapon {
        RapidWeapon::Upgraded | RapidWeapon::Maximum => {
            inputs.roll_step.ok_or(RapidLaunchError::MissingRollStep)?
        }
        _ => Angle::ZERO,
    };
    let aim = if weapon != RapidWeapon::Alternate {
        let position = inputs
            .retained_aim
            .ok_or(RapidLaunchError::MissingRetainedAim)?;
        let proxy = world.fallback.ok_or(RapidLaunchError::MissingAimProxy)?;
        objects
            .get(proxy)
            .ok_or(RapidLaunchError::MissingAimProxyActor(proxy))?;
        Some((proxy, position))
    } else {
        None
    };
    let caller_roll = objects
        .get(caller)
        .expect("validated rapid caller")
        .base
        .roll;
    let Some(created) = weapon_creation::player_linked(objects, caller, parameters, defaults)
        .map_err(LaunchError::Creation)?
    else {
        return Ok(None);
    };
    let actor = objects.get_mut(created).expect("fresh rapid weapon");
    actor.base.path = Some(weapon.paths()[0]);
    actor.base.shape = match weapon {
        RapidWeapon::Basic => BASIC_SHAPE,
        RapidWeapon::Upgraded => UPGRADED_SHAPE,
        RapidWeapon::Maximum => MAXIMUM_SHAPE,
        RapidWeapon::Alternate if level == SPRITE_LEVEL => ALTERNATE_SPRITE,
        RapidWeapon::Alternate => MAXIMUM_SHAPE,
    };
    if let Some(pitch) = pitch {
        actor.base.pitch = pitch;
        if level == SPRITE_LEVEL {
            super::path_appearance::set_sprite(actor, 0, 0);
            actor.base.attack_power = SPRITE_ATTACK;
        } else {
            actor.base.attack_power = MESH_ATTACK;
        }
    } else {
        actor.extension.relative_rotation.roll = roll_step;
        actor.base.roll = caller_roll;
        actor
            .extension
            .path_state
            .animation
            .shape
            .initialize(RAPID_LAUNCH_FRAME);
    }
    if let Some((proxy, position)) = aim {
        // The source visibly replaces the reserved actor's full position
        // before using it for horizontal facing; retain this side effect.
        objects
            .get_mut(proxy)
            .expect("validated aim proxy")
            .base
            .position = position;
        let actor = objects.get_mut(created).expect("fresh rapid weapon");
        actor.base.yaw = Angle::from_units(sf_core::aim_angle::sf2_yaw_to_target(
            position.x.wrapping_sub(actor.base.position.x),
            position.z.wrapping_sub(actor.base.position.z),
        ));
    }
    let actor = objects.get_mut(created).expect("fresh rapid weapon");
    actor.extension.auxiliary.set(resources, created,
        super::actor_auxiliary::AuxiliaryRecord::ReflectionShape(actor.base.shape))
        .map_err(LaunchError::Auxiliary)?;
    Ok(Some(created))
}

impl From<RapidLaunchError> for LaunchError {
    fn from(error: RapidLaunchError) -> Self {
        Self::Rapid(error)
    }
}

#[cfg(test)]
#[path = "weapon_rapid_tests.rs"]
mod tests;
