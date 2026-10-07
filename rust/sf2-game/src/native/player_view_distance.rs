//! Linked/external view transitions ($07:9AEF..9D35). Mode entry installs
//! the authored distance profile; ordinary camera visits consume shared menu
//! requests and advance the retained distance. Linked mode and transition
//! inhibition remain the charge/muzzle service's canonical shared bits.

use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore};

#[cfg(test)]
#[path = "player_view_distance_tests.rs"]
mod tests;

const MODE_FAMILY_MASK: u8 = 0xF0;
const FLIGHT_FAMILY: u8 = 0x10;
const ALTERNATE_FAMILY: u8 = 0x30;
const MENU_REQUEST: u8 = 0x20;
const MENU_LINKED_VIEW: u8 = 0x80;
const VIEW_SIDE_DISTANCE: i16 = -40;
const DISTANCE_STEP: i16 = 30;
const TRANSITION_SETTLED_DISTANCE: i16 = 50;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerViewDistance {
    /// Retained camera distance (6B67), before the per-visit offset.
    pub distance: i16,
    /// Authored throttle responses (6B69/6B6B), used by camera movement.
    pub boost_response: i16,
    pub brake_response: i16,
    pub linked_target: i16,
    pub external_target: i16,
    /// Profile words 6B71/6B73. All original profiles initialize these to
    /// zero; the recovered camera code has no readers of either word.
    pub reserved_profile: [i16; 2],
    /// Height correction subtracted by the camera pitch controller (6B75).
    pub pitch_height_offset: i16,
    /// 6B63 bit 20: capture the completed view change on the next visit.
    pub capture_pending: bool,
    /// 6B63 bit 04: a requested toggle, retained while transition is active.
    pub switch_requested: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewDistanceError {
    World(WorldInputError),
    MissingDistance(ObjectId),
    MissingFixedView,
    MissingViewOptions,
    MissingViewRequest,
}

impl From<WorldInputError> for ViewDistanceError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn distance_mut<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut PlayerViewDistance, ViewDistanceError> {
    world
        .player_mut(objects, owner)?
        .view_distance
        .as_mut()
        .ok_or(ViewDistanceError::MissingDistance(owner))
}

/// Complete profile initializer. It replaces only the eight distance/profile
/// words, preserving requests and charge state even during a mode change.
pub fn initialize(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ViewDistanceError> {
    let mode = world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode;
    // The seven-word source profiles at $07:9DC0/9DCE/9DDC.
    let (external, boost, brake, linked) = match mode & MODE_FAMILY_MASK {
        FLIGHT_FAMILY => (-240, -50, 40, 0),
        ALTERNATE_FAMILY => (-160, 0, 0, 0),
        _ => (-210, -20, 20, 20),
    };
    let state = distance_mut(objects, world, owner)?;
    state.distance = external;
    state.external_target = external;
    state.boost_response = boost;
    state.brake_response = brake;
    state.linked_target = linked;
    state.reserved_profile = [0; 2];
    state.pitch_height_offset = 0;
    Ok(())
}

fn request_capture(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
) -> Result<(), ViewDistanceError> {
    let view_id = world.fixed_players[0].ok_or(ViewDistanceError::MissingFixedView)?;
    let view = objects
        .get_mut(view_id)
        .ok_or(WorldInputError::MissingActor(view_id))?;
    // These are the canonical base flags consumed by view_blend.
    view.extension.path_state.motion.follow_player_displacement = true;
    view.extension.path_state.motion.generate_velocity_each_step = true;
    Ok(())
}

fn approach(current: i16, target: i16) -> i16 {
    let delta = current.wrapping_sub(target);
    if delta == 0 {
        current
    } else if delta < 0 {
        let advanced = current.wrapping_add(DISTANCE_STEP);
        if advanced.wrapping_sub(target) < 0 {
            advanced
        } else {
            target
        }
    } else {
        let advanced = current.wrapping_sub(DISTANCE_STEP);
        if advanced.wrapping_sub(target) >= 0 {
            advanced
        } else {
            target
        }
    }
}

/// Complete transition visit. The source's initial mode/carry/stance tests
/// all converge at $07:9B34 without changing state; they do not gate the
/// transition. Side filtering samples the OLD distance, before movement.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ViewDistanceError> {
    let state = distance_mut(objects, world, owner)?;
    if state.capture_pending {
        state.capture_pending = false;
        request_capture(objects, world)?;
    }
    let current = distance_mut(objects, world, owner)?.distance;
    objects
        .get_mut(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .flags
        .view_side_filter = current >= VIEW_SIDE_DISTANCE;

    let options_enabled = world
        .player_view_options_enabled
        .ok_or(ViewDistanceError::MissingViewOptions)?;
    let requested_view = if options_enabled {
        let control = world
            .scene
            .player_view_control
            .as_mut()
            .ok_or(ViewDistanceError::MissingViewRequest)?;
        if *control & MENU_REQUEST == 0 {
            None
        } else {
            *control &= !MENU_REQUEST;
            Some(*control & MENU_LINKED_VIEW != 0)
        }
    } else {
        Some(false)
    };
    let linked = world
        .player(objects, owner)?
        .charge
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?
        .linked_mode;
    if requested_view.is_some_and(|desired| desired != linked) {
        distance_mut(objects, world, owner)?.switch_requested = true;
    }

    let charge = world
        .player(objects, owner)?
        .charge
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
    if !charge.linked_muzzle_disabled && distance_mut(objects, world, owner)?.switch_requested {
        distance_mut(objects, world, owner)?.switch_requested = false;
        request_capture(objects, world)?;
        let charge = world.player_mut(objects, owner)?.charge.as_mut().unwrap();
        charge.linked_mode = !charge.linked_mode;
        charge.linked_muzzle_disabled = true;
        world.published_linked_view = Some(u8::from(charge.linked_mode));
    }
    let records = world.player(objects, owner)?;
    let ignored = records
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .ignores_contacts;
    let linked = records.charge.unwrap().linked_mode;
    let state = distance_mut(objects, world, owner)?;
    let target = if !ignored && linked {
        state.linked_target
    } else {
        state.external_target
    };
    state.distance = approach(state.distance, target);
    // Wrapped absolute value and subtraction preserve the half-word edge;
    // neither saturating arithmetic nor an unsigned distance test matches it.
    let residual = target.wrapping_sub(state.distance).wrapping_abs();
    if residual.wrapping_sub(TRANSITION_SETTLED_DISTANCE) <= 0 {
        let records = world.player_mut(objects, owner)?;
        let charge = records.charge.as_mut().unwrap();
        if charge.linked_muzzle_disabled {
            charge.linked_muzzle_disabled = false;
            records.view_distance.as_mut().unwrap().capture_pending = true;
        }
    }
    Ok(())
}
