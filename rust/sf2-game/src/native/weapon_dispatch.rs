//! Statically identified path-weapon variants ($0D:DBAA..DCBE).
//! These variants create one actor and install an already lowered path.
//! Unported player-auxiliary variants remain explicit selection errors.

use super::authored_paths;
use super::collision_pass::ExclusionGroups;
use super::weapon_creation::{self, CreationError};
use super::weapon_launch::{hostile_launch_needs_random, HostileLaunchCounts, LaunchParameters};
use super::{
    Angle, ObjectId, ObjectSpawnDefaults, ObjectStore, PathCursor, RandomState, ShapeId,
    OBJECT_CAPACITY,
};

const AUXILIARY_MODE_CLASS_MASK: u8 = 0xF0;
const VELOCITY_MODE_CLASS: u8 = 0x10;
const SLOW_LAUNCH_SPEED: u8 = 40;
const HOMING_LAUNCH_SPEED: u8 = 70;
const OCCUPANCY_LAUNCH_SPEED: u8 = 60;
const HEAVY_HEALTH: u8 = 120;
const HEAVY_ATTACK: u8 = 2;
const CHARGED_ATTACK: u8 = 10;
const HOSTILE_EXCLUSION: ExclusionGroups = ExclusionGroups::from_authored_class(0x50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathWeapon {
    PlayerOrHostileHeavy,
    PlayerChargedMesh,
    VariantGuided,
    DifficultyHoming,
    PrimaryMotionHoming,
    DoubledMotionHoming,
    OccupancySpeedSelected,
    OccupancyDefaultSpeed,
    OffsetGuided,
    PrimaryMotionSurfaceLimited,
}

impl PathWeapon {
    /// Complete native entries that this launch can install. Heavy weapons
    /// have separate player and hostile branches; both must exist in a
    /// catalog that accepts this creation service.
    pub const fn paths(self) -> &'static [PathCursor] {
        match self {
            Self::PlayerChargedMesh => &[authored_paths::AIMED_IMPACT_PROJECTILE],
            Self::PlayerOrHostileHeavy => &[
                authored_paths::PRIMARY_MOTION_GROUND_LIMITED,
                authored_paths::SURFACE_OR_GROUND_LIMITED,
            ],
            Self::VariantGuided => &[authored_paths::VARIANT_GUIDED_PROJECTILE],
            Self::DifficultyHoming => &[authored_paths::DIFFICULTY_HOMING_PROJECTILE],
            Self::PrimaryMotionHoming => &[authored_paths::PRIMARY_MOTION_HOMING_PROJECTILE],
            Self::DoubledMotionHoming => &[authored_paths::DOUBLED_MOTION_HOMING_PROJECTILE],
            Self::OccupancySpeedSelected | Self::OccupancyDefaultSpeed => {
                &[authored_paths::OCCUPANCY_SURFACE_LIMITED]
            }
            Self::OffsetGuided => &[authored_paths::OFFSET_GUIDED_PROJECTILE],
            Self::PrimaryMotionSurfaceLimited => &[authored_paths::PRIMARY_MOTION_SURFACE_LIMITED],
        }
    }
    /// The authored weapon selector is a gameplay byte, not a callable source
    /// address. Reject selectors whose creation services have not been ported.
    pub const fn from_selection(selection: u8) -> Option<Self> {
        Some(match selection {
            2 => Self::PlayerOrHostileHeavy,
            12 | 14 | 16 => Self::PlayerChargedMesh,
            18 => Self::VariantGuided,
            20 => Self::DifficultyHoming,
            22 => Self::PrimaryMotionHoming,
            24 => Self::DoubledMotionHoming,
            26 => Self::OccupancySpeedSelected,
            28 => Self::OccupancyDefaultSpeed,
            30 => Self::OffsetGuided,
            32 => Self::PrimaryMotionSurfaceLimited,
            _ => return None,
        })
    }
}

pub struct LaunchWorld<'a> {
    /// Published aiming pitch (1DF2), not caller pitch or a fresh target
    /// angle. Walker aim correction and ordinary flight both publish here.
    pub published_pitch: Option<Angle>,
    /// Live source player pointers, not the path's selected actor.
    pub primary: Option<ObjectId>,
    pub secondary: Option<ObjectId>,
    pub primary_auxiliary_mode: Option<u8>,
    pub hostile_counts: Option<&'a mut HostileLaunchCounts>,
    pub random: &'a mut RandomState,
}

/// Shared launcher state, retained across actor invocations. The fallback is
/// a real actor allocated by scene startup, not a null or fabricated handle.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WeaponState {
    pub published_pitch: Option<Angle>,
    pub parameters: LaunchParameters,
    pub hostile_counts: HostileLaunchCounts,
    pub fallback: Option<ObjectId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchError {
    MissingPublishedPitch,
    Creation(CreationError),
    MissingPrimary,
    MissingSecondary,
    MissingPrimaryAuxiliaryMode,
    MissingHostileCounts,
}

#[derive(Debug, Clone, Copy)]
pub struct LaunchRequest {
    pub weapon: PathWeapon,
    pub parameters: LaunchParameters,
    pub defaults: ObjectSpawnDefaults,
}

/// Install a complete path without executing it. Input faults are validated
/// before mutation; full-pool failure requires no player state and consumes
/// no random bytes, matching the source's allocation-before-classification.
pub fn launch(
    objects: &mut ObjectStore,
    caller: ObjectId,
    request: LaunchRequest,
    world: &mut LaunchWorld<'_>,
) -> Result<Option<ObjectId>, LaunchError> {
    if objects.get(caller).is_none() {
        return Err(LaunchError::Creation(CreationError::MissingActor(caller)));
    }
    if objects.len() == OBJECT_CAPACITY {
        return Ok(None);
    }
    let player_heavy = if request.weapon == PathWeapon::PlayerOrHostileHeavy {
        let primary = world.primary.ok_or(LaunchError::MissingPrimary)?;
        caller == primary || caller == world.secondary.ok_or(LaunchError::MissingSecondary)?
    } else {
        false
    };
    let charged = request.weapon == PathWeapon::PlayerChargedMesh;
    let published_pitch = if charged {
        Some(
            world
                .published_pitch
                .ok_or(LaunchError::MissingPublishedPitch)?,
        )
    } else {
        None
    };
    let caller_roll = objects
        .get(caller)
        .expect("validated weapon caller")
        .base
        .roll;
    let hostile = !player_heavy && !charged && request.weapon != PathWeapon::VariantGuided;
    let primary_yaw = if hostile {
        let primary = world.primary.ok_or(LaunchError::MissingPrimary)?;
        let actor = objects
            .get(primary)
            .ok_or(LaunchError::Creation(CreationError::MissingActor(primary)))?;
        if world.hostile_counts.is_none() {
            return Err(LaunchError::MissingHostileCounts);
        }
        Some(actor.base.yaw)
    } else {
        None
    };
    let selected_speed = match request.weapon {
        PathWeapon::DifficultyHoming | PathWeapon::OccupancySpeedSelected => {
            let mode = world
                .primary_auxiliary_mode
                .ok_or(LaunchError::MissingPrimaryAuxiliaryMode)?;
            Some(if mode & AUXILIARY_MODE_CLASS_MASK == VELOCITY_MODE_CLASS {
                SLOW_LAUNCH_SPEED
            } else if request.weapon == PathWeapon::DifficultyHoming {
                HOMING_LAUNCH_SPEED
            } else {
                OCCUPANCY_LAUNCH_SPEED
            })
        }
        _ => None,
    };
    let path_index =
        usize::from(request.weapon == PathWeapon::PlayerOrHostileHeavy && !player_heavy);
    let path = request.weapon.paths()[path_index];
    let create = if charged {
        weapon_creation::player_linked
    } else {
        weapon_creation::common
    };
    let Some(created) = create(objects, caller, request.parameters, request.defaults)
        .map_err(LaunchError::Creation)?
    else {
        return Ok(None);
    };
    let actor = objects.get_mut(created).expect("fresh weapon");
    actor.base.path = Some(path);
    if let Some(pitch) = published_pitch {
        actor.base.shape = ShapeId::PLAYER_CHARGED_LASER_LAUNCH;
        actor.base.roll = caller_roll;
        actor.base.pitch = pitch;
        actor.extension.reflection_shape = Some(actor.base.shape);
        actor.base.hit_points = HEAVY_HEALTH;
        actor.base.attack_power = CHARGED_ATTACK;
    }
    if let Some(speed) = selected_speed {
        super::path_motion::set_speed(actor, created, speed);
    }
    if request.weapon == PathWeapon::PlayerOrHostileHeavy {
        actor.base.hit_points = HEAVY_HEALTH;
        actor.base.attack_power = HEAVY_ATTACK;
    }
    if request.weapon == PathWeapon::VariantGuided {
        actor.base.flags.suppress_death_effects = false;
    }
    if let Some(primary_yaw) = primary_yaw {
        if hostile_launch_needs_random(primary_yaw, actor.base.yaw) {
            world
                .hostile_counts
                .as_deref_mut()
                .expect("validated launch counts")
                .classify_aligned_launch(
                    &mut actor.base.flags.collision_disabled,
                    world.random.next_byte(),
                );
        }
        actor.base.contacts.exclusion_groups = actor
            .base
            .contacts
            .exclusion_groups
            .union(HOSTILE_EXCLUSION);
    }
    Ok(Some(created))
}

#[cfg(test)]
#[path = "weapon_dispatch_tests.rs"]
mod tests;
