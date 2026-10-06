//! Aim publication preceding consumable/rapid fire (`$07:D6CC..D78A`) and
//! the separately scheduled retained forward-point update (`$07:A941..AA2B`).

use super::path_steering::SteeringState;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::target_search::{self, AimWindow, TargetSearchError};
use super::{Angle, ObjectId, ObjectStore, Vector3};
use sf_core::aim_angle::{sf2_atan16, sf2_xz_angle_distance};

const WALKER_MODE: u8 = 0x20;
const MODE_MASK: u8 = 0xF0;
const WALKER_TARGET_WINDOW: AimWindow = AimWindow {
    minimum_distance: 0,
    maximum_distance: 7_000,
    yaw_half_width: 10,
    pitch_half_width: 50,
};
const PREDICTION_TICKS: i16 = 4;
const PITCH_LIMIT: i8 = 25;
const FINE_TO_COARSE_SHIFT: u32 = u8::BITS;
const RETAINED_FORWARD_DISTANCE: i8 = 75;
const RETAINED_WORLD_SCALE: i16 = 128;
const RETAINED_YAW_LEAD: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponAimError {
    World(WorldInputError),
    Search(TargetSearchError),
    MissingWeapons,
    MissingProxy,
    MissingActor(ObjectId),
    MissingPublishedPitch,
    MissingYawMotion(ObjectId),
    MissingRetainedAim(ObjectId),
}

/// Project a point ahead of the craft for subsequent rapid launches. The
/// source calls this separately from firing; do not update it implicitly
/// inside the launcher or replace it with the current Walker candidate.
pub fn retain_forward_point(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), WeaponAimError> {
    let weapons = world
        .weapons
        .as_ref()
        .ok_or(WeaponAimError::MissingWeapons)?;
    let proxy = weapons.fallback.ok_or(WeaponAimError::MissingProxy)?;
    let yaw = objects
        .get(owner)
        .ok_or(WeaponAimError::MissingActor(owner))?
        .base
        .yaw;
    objects
        .get_mut(proxy)
        .ok_or(WeaponAimError::MissingActor(proxy))?
        .base
        .yaw = yaw;
    let pitch = weapons
        .published_pitch
        .ok_or(WeaponAimError::MissingPublishedPitch)?;
    objects
        .get_mut(proxy)
        .expect("validated aim proxy")
        .base
        .pitch = pitch;
    let yaw_motion = world
        .player(objects, owner)
        .map_err(WeaponAimError::World)?
        .yaw_motion
        .ok_or(WeaponAimError::MissingYawMotion(owner))?;
    let lead = ((yaw_motion >> FINE_TO_COARSE_SHIFT) as u8).wrapping_mul(RETAINED_YAW_LEAD);
    let proxy_actor = objects.get_mut(proxy).expect("validated aim proxy");
    proxy_actor.base.yaw = yaw.wrapping_add(lead as i8);
    // Roll acts on a zero lateral/vertical pair. Preserve the live proxy's
    // roll and each byte-quantized rotation; the forward value starts at 75.
    let (x, y, z) = sf_core::snes_trig::strat_roffs_full(
        proxy_actor.base.roll.units(),
        proxy_actor.base.pitch.units(),
        proxy_actor.base.yaw.units(),
        0,
        0,
        RETAINED_FORWARD_DISTANCE,
    );
    let origin = objects
        .get(owner)
        .expect("validated aim owner")
        .base
        .position;
    let position = Vector3 {
        x: origin.x.wrapping_add(x.wrapping_mul(RETAINED_WORLD_SCALE)),
        y: origin.y.wrapping_add(y.wrapping_mul(RETAINED_WORLD_SCALE)),
        z: origin.z.wrapping_add(z.wrapping_mul(RETAINED_WORLD_SCALE)),
    };
    objects
        .get_mut(proxy)
        .expect("validated aim proxy")
        .base
        .position = position;
    world
        .player_mut(objects, owner)
        .map_err(WeaponAimError::World)?
        .rapid_aim
        .as_mut()
        .ok_or(WeaponAimError::MissingRetainedAim(owner))?
        .retained_aim = position;
    Ok(())
}

fn walker_pitch(fine: u16) -> Angle {
    // Each signed division truncates separately, before adding and taking
    // the high byte. A single multiply by three/four is not equivalent.
    let half = fine as i16 / 2;
    let quarter = half / 2;
    let coarse = ((half + quarter) >> FINE_TO_COARSE_SHIFT) as i8;
    Angle::from_units(coarse.clamp(-PITCH_LIMIT, PITCH_LIMIT) as u8)
}

pub fn publish(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    steering: &mut SteeringState,
    owner: ObjectId,
) -> Result<(), WeaponAimError> {
    world
        .weapons
        .as_mut()
        .ok_or(WeaponAimError::MissingWeapons)?
        .published_pitch = Some(Angle::ZERO);
    let mode = world
        .player(objects, owner)
        .map_err(WeaponAimError::World)?
        .auxiliary
        .ok_or(WeaponAimError::World(WorldInputError::MissingAuxiliary(
            owner,
        )))?
        .mode;
    if mode & MODE_MASK != WALKER_MODE {
        let pitch = objects
            .get(owner)
            .ok_or(WeaponAimError::MissingActor(owner))?
            .base
            .pitch;
        world
            .weapons
            .as_mut()
            .expect("validated weapons")
            .published_pitch = Some(pitch);
        return Ok(());
    }
    let Some(target) = target_search::nearest(objects, owner, WALKER_TARGET_WINDOW)
        .map_err(WeaponAimError::Search)?
    else {
        return Ok(());
    };
    let proxy = world
        .weapons
        .as_ref()
        .expect("validated weapons")
        .fallback
        .ok_or(WeaponAimError::MissingProxy)?;
    let candidate = &objects.get(target).expect("live search result").base;
    // The prediction stores are followed by $7F:2BBE copying the original
    // candidate position over them. Only a self-alias retains the prediction.
    let position = if proxy == target {
        Vector3 {
            x: candidate
                .position
                .x
                .wrapping_add(candidate.velocity.x.wrapping_mul(PREDICTION_TICKS)),
            y: candidate
                .position
                .y
                .wrapping_add(candidate.velocity.y.wrapping_mul(PREDICTION_TICKS)),
            z: candidate
                .position
                .z
                .wrapping_add(candidate.velocity.z.wrapping_mul(PREDICTION_TICKS)),
        }
    } else {
        candidate.position
    };
    objects
        .get_mut(proxy)
        .ok_or(WeaponAimError::MissingActor(proxy))?
        .base
        .position = position;
    steering.unchanged_axes = 0;
    // Re-read the owner after the proxy copy: those identities can alias.
    let origin = objects
        .get(owner)
        .expect("validated aim owner")
        .base
        .position;
    let distance = sf2_xz_angle_distance(
        position.x.wrapping_sub(origin.x),
        position.z.wrapping_sub(origin.z),
    );
    let fine = sf2_atan16(position.y.wrapping_sub(origin.y), distance);
    world
        .weapons
        .as_mut()
        .expect("validated weapons")
        .published_pitch = Some(walker_pitch(fine));
    Ok(())
}

#[cfg(test)]
#[path = "player_weapon_aim_tests.rs"]
mod tests;
