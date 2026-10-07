//! Player corridor limits ($07:E2F3..E5C7) and their real region producer
//! ($07:F893..F95E). The shared proxy is observable even on a rejected probe.

use super::path_fields::chase_word;
use super::player_storage::{self, PlayerStorageError};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Angle, ObjectId, ObjectStore, Vector3};

#[cfg(test)]
#[path = "player_boundary_tests.rs"]
mod tests;

const CORRIDOR_ACTIVE: u8 = 0x04;
const MODE_FAMILY_MASK: u8 = 0xF0;
const LEVEL_FLIGHT_MODE: u8 = 0x10;
const LEVEL_HEIGHT: i16 = -40;
const LEVEL_LOOKAHEAD: i16 = 400;
const DIAGONAL_WIDTH_SCALE: i32 = 362;
const WIDTH_FRACTION_BITS: u32 = 8;
const OCTANT_MASK: u8 = 0xE0;
const EAST_WEST: u8 = 64;
const WEST_EAST: u8 = 192;
const SUM_FORWARD: u8 = 32;
const SUM_REVERSE: u8 = 160;
const DIFFERENCE_FORWARD: u8 = 224;
const DIFFERENCE_REVERSE: u8 = 96;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerBoundary {
    /// Region center and half extents (6AAF..6AB8), installed by live actors.
    /// Heading is shared with steering.locked_heading, not duplicated here.
    pub center: Vector3,
    pub half_width: i16,
    pub half_height: i16,
    /// Last usable movement position (6BED/EF/F1). Corridor correction only
    /// replaces X/Z; occupancy constraints also consume/publish this history.
    pub return_position: Vector3,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Corridor {
    pub center: Vector3,
    pub half_width: i16,
    /// A negative half height bypasses vertical admission altogether.
    pub half_height: i16,
    pub heading: Angle,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RegionInputs {
    pub heading_offset: Angle,
    pub half_width: i16,
    pub half_height: i16,
    pub activation_radius: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    MissingBoundary(ObjectId),
    MissingSteering(ObjectId),
    MissingProxy,
}
impl From<WorldInputError> for BoundaryError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<PlayerStorageError> for BoundaryError {
    fn from(error: PlayerStorageError) -> Self {
        Self::Storage(error)
    }
}

/// $07:E3D6: signed wrapped comparisons, including the source's asymmetric
/// equality cases and fallback to X limits for non-octant headings. Diagonal
/// corrections halve AFTER word negation, rounding negative values downward.
pub fn correct_proxy(
    objects: &mut ObjectStore,
    proxy: ObjectId,
    corridor: Corridor,
) -> Result<bool, BoundaryError> {
    let actor = objects
        .get_mut(proxy)
        .ok_or(WorldInputError::MissingActor(proxy))?;
    actor.base.contacts.hit_marked = false;
    let position = &mut actor.base.position;
    let width = corridor.half_width;
    let span = ((i32::from(width) * DIAGONAL_WIDTH_SCALE) >> WIDTH_FRACTION_BITS) as i16;
    let center = corridor.center;
    let delta = match corridor.heading.units() {
        EAST_WEST | WEST_EAST => {
            let upper = center.z.wrapping_add(width).wrapping_sub(position.z);
            let lower = center.z.wrapping_sub(width).wrapping_sub(position.z);
            if upper < 0 {
                Some([0, upper])
            } else if lower >= 0 {
                Some([0, lower])
            } else {
                None
            }
        }
        SUM_FORWARD | SUM_REVERSE => {
            let upper = center.x.wrapping_add(center.z).wrapping_add(span);
            let lower = center.x.wrapping_add(center.z).wrapping_sub(span);
            let sum = position.x.wrapping_add(position.z);
            let above = sum.wrapping_sub(upper);
            let below = sum.wrapping_sub(lower);
            let excess = if corridor.heading.units() == SUM_FORWARD {
                if above >= 0 {
                    Some(above)
                } else if below < 0 {
                    Some(below)
                } else {
                    None
                }
            } else if below < 0 {
                Some(below)
            } else if above >= 0 {
                Some(above)
            } else {
                None
            };
            excess.map(|value| {
                let half = value.wrapping_neg() >> 1;
                [half, half]
            })
        }
        DIFFERENCE_FORWARD | DIFFERENCE_REVERSE => {
            let x_limit = center.x.wrapping_sub(center.z).wrapping_add(span);
            let z_limit = center.z.wrapping_sub(center.x).wrapping_add(span);
            let (x_excess, z_excess, negative) = if corridor.heading.units() == DIFFERENCE_FORWARD {
                (
                    position.x.wrapping_sub(position.z).wrapping_sub(x_limit),
                    position.z.wrapping_sub(position.x).wrapping_sub(z_limit),
                    false,
                )
            } else {
                (
                    position.x.wrapping_sub(position.z).wrapping_add(z_limit),
                    position.z.wrapping_sub(position.x).wrapping_add(x_limit),
                    true,
                )
            };
            if (x_excess < 0) == negative {
                let half = x_excess >> 1;
                Some([half.wrapping_neg(), half])
            } else if (z_excess < 0) == negative {
                let half = z_excess >> 1;
                Some([half, half.wrapping_neg()])
            } else {
                None
            }
        }
        _ => {
            let upper = center.x.wrapping_add(width).wrapping_sub(position.x);
            let lower = center.x.wrapping_sub(width).wrapping_sub(position.x);
            if upper < 0 {
                Some([upper, 0])
            } else if lower >= 0 {
                Some([lower, 0])
            } else {
                None
            }
        }
    };
    if let Some([x, z]) = delta {
        position.x = position.x.wrapping_add(x);
        position.z = position.z.wrapping_add(z);
        Ok(true)
    } else {
        Ok(false)
    }
}

fn proxy(world: &ScenePathWorld) -> Result<ObjectId, BoundaryError> {
    world
        .weapons
        .as_ref()
        .and_then(|weapons| weapons.fallback)
        .ok_or(BoundaryError::MissingProxy)
}

fn copy_to_proxy(
    objects: &mut ObjectStore,
    owner: ObjectId,
    proxy: ObjectId,
) -> Result<(), BoundaryError> {
    let position = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .position;
    objects
        .get_mut(proxy)
        .ok_or(WorldInputError::MissingActor(proxy))?
        .base
        .position = position;
    Ok(())
}

/// $07:E384: vertical admission followed by the same destructive proxy probe.
/// The owner itself is not corrected. A boundary equality that produces a zero
/// correction still rejects admission because the original reports correction.
pub fn contains(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
    corridor: Corridor,
) -> Result<bool, BoundaryError> {
    let height = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .position
        .y;
    if corridor.half_height >= 0
        && (corridor
            .center
            .y
            .wrapping_sub(corridor.half_height)
            .wrapping_sub(height)
            >= 0
            || corridor
                .center
                .y
                .wrapping_add(corridor.half_height)
                .wrapping_sub(height)
                < 0)
    {
        return Ok(false);
    }
    let proxy = proxy(world)?;
    copy_to_proxy(objects, owner, proxy)?;
    Ok(!correct_proxy(objects, proxy, corridor)?)
}

pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<bool, BoundaryError> {
    let auxiliary = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?;
    if auxiliary.action_flags & CORRIDOR_ACTIVE == 0 {
        return Ok(false);
    }
    if auxiliary.mode & MODE_FAMILY_MASK == LEVEL_FLIGHT_MODE {
        let height = objects
            .get(owner)
            .expect("validated player")
            .base
            .position
            .y;
        player_storage::get_mut(objects, resources, owner)?.fine_pitch =
            sf_core::aim_angle::sf2_atan16(LEVEL_HEIGHT.wrapping_sub(height), LEVEL_LOOKAHEAD);
        objects
            .get_mut(owner)
            .expect("validated player")
            .base
            .position
            .y = chase_word(height as u16, LEVEL_HEIGHT as u16) as i16;
    }
    let record = world.player(objects, owner)?;
    let boundary = record
        .boundary
        .ok_or(BoundaryError::MissingBoundary(owner))?;
    let heading = record
        .steering
        .ok_or(BoundaryError::MissingSteering(owner))?
        .locked_heading;
    let proxy = proxy(world)?;
    copy_to_proxy(objects, owner, proxy)?;
    if !correct_proxy(
        objects,
        proxy,
        Corridor {
            center: boundary.center,
            half_width: boundary.half_width,
            half_height: boundary.half_height,
            heading,
        },
    )? {
        return Ok(false);
    }
    let position = objects.get(proxy).expect("validated proxy").base.position;
    objects
        .get_mut(owner)
        .expect("validated player")
        .base
        .position = position;
    let history = &mut world
        .player_mut(objects, owner)?
        .boundary
        .as_mut()
        .expect("validated boundary")
        .return_position;
    history.x = position.x;
    history.z = position.z;
    Ok(true)
}

/// $07:F893: an already-active player skips containment, but not the anchor's
/// signed wrapped radius checks. Successful installation shares the action
/// flag and heading with actual displacement/steering consumers.
pub fn install_region(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    anchor: ObjectId,
    owner: ObjectId,
    inputs: RegionInputs,
) -> Result<bool, BoundaryError> {
    let actor = objects
        .get(anchor)
        .ok_or(WorldInputError::MissingActor(anchor))?;
    let center = actor.base.position;
    let heading = Angle::from_units(
        (actor.base.yaw.units() & OCTANT_MASK).wrapping_add(inputs.heading_offset.units()),
    );
    let active = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .action_flags
        & CORRIDOR_ACTIVE
        != 0;
    if !active
        && !contains(
            objects,
            world,
            owner,
            Corridor {
                center,
                heading,
                half_width: inputs.half_width,
                half_height: inputs.half_height,
            },
        )?
    {
        return Ok(false);
    }
    let position = objects.get(owner).expect("validated player").base.position;
    if center
        .x
        .wrapping_sub(position.x)
        .wrapping_abs()
        .wrapping_sub(inputs.activation_radius)
        >= 0
        || center
            .z
            .wrapping_sub(position.z)
            .wrapping_abs()
            .wrapping_sub(inputs.activation_radius)
            >= 0
    {
        return Ok(false);
    }
    let record = world.player_mut(objects, owner)?;
    record
        .auxiliary
        .as_mut()
        .expect("validated auxiliary")
        .action_flags |= CORRIDOR_ACTIVE;
    record
        .steering
        .as_mut()
        .ok_or(BoundaryError::MissingSteering(owner))?
        .locked_heading = heading;
    let boundary = record
        .boundary
        .as_mut()
        .ok_or(BoundaryError::MissingBoundary(owner))?;
    boundary.center = center;
    boundary.half_width = inputs.half_width;
    boundary.half_height = inputs.half_height;
    Ok(true)
}
