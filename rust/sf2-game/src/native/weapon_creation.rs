//! Shared authored weapon allocation and formatting ($0D:E017..E0CD).
//! Allocation inserts immediately after the caller and scopes pressure
//! retirement to its suffix. No path is executed during creation.

use super::collision_pass::ExclusionGroups;
use super::hit_response::HitSide;
use super::path_control::PlayerTarget;
use super::weapon_launch::{format_pose, LaunchParameters};
use super::{
    Behavior, Object, ObjectId, ObjectKind, ObjectSpawnDefaults, ObjectStore, Rotation, ShapeId,
};

const INITIAL_HEALTH: u8 = 1;
const INITIAL_ATTACK: u8 = 1;
const PLAYER_WEAPON_GROUP: u8 = 255;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationError {
    MissingActor(ObjectId),
}

/// $0D:E017. Pool exhaustion is an ordinary failed allocation with no actor
/// changes. The caller decides whether to use the world's reserved fallback.
/// A successful result still needs a variant-specific path and other settings.
pub fn common(
    objects: &mut ObjectStore,
    caller: ObjectId,
    parameters: LaunchParameters,
    defaults: ObjectSpawnDefaults,
) -> Result<Option<ObjectId>, CreationError> {
    let source = objects
        .get(caller)
        .ok_or(CreationError::MissingActor(caller))?;
    let pose = format_pose(
        source.base.position,
        Rotation {
            pitch: source.base.pitch,
            yaw: source.base.yaw,
            roll: source.base.roll,
        },
        parameters,
    );
    let source_speed = source.base.speed;
    let fresh = Object::new_authored(
        ObjectKind::Projectile,
        ShapeId::EMPTY,
        Behavior::FollowPath,
        defaults,
    );
    let Some(created) = objects.allocate_weapon_after(caller, fresh) else {
        return Ok(None);
    };
    let weapon = objects.get_mut(created).expect("fresh weapon");
    weapon.base.hit_points = INITIAL_HEALTH;
    weapon.base.attack_power = INITIAL_ATTACK;
    // $03:AB1D assigns the entire actor flag byte: this also clears HOLD.
    weapon.base.flags.weapon_launch_formatted = true;
    weapon.extension.path_state.hold_latched = false;
    weapon.base.contacts.weapon_formatted = true;
    weapon.base.position = pose.position;
    weapon.base.pitch = pose.rotation.pitch;
    weapon.base.yaw = pose.rotation.yaw;
    weapon.base.roll = pose.rotation.roll;
    // Formatter tail $03:AC11. These are aliases of the later path fields,
    // not additional private launch-only copies of heading and speed.
    weapon.base.child_number = pose.rotation.pitch.units();
    weapon.extension.path_state.repeat_counter = pose.rotation.yaw.units();
    weapon.base.wait_timer = source_speed;
    weapon.base.linked_object = Some(caller);
    weapon.base.flags.general_search_eligible = true;
    weapon.base.flags.exclude_from_shape_footprint_search = true;
    weapon.base.flags.suppress_death_effects = true;
    weapon.extension.path_state.animation.shape.initialize(0);
    weapon.base.flags.maximum_draw_distance = true;
    weapon.extension.path_state.needs_path_initialization = true;
    objects
        .get_mut(caller)
        .expect("validated firing actor")
        .base
        .linked_object = Some(created);
    Ok(Some(created))
}

/// $0D:E0A4. Player-linked weapons attach for source queries, but are not
/// inserted into a child chain. Target selection follows the caller's hit
/// attribution side, not its path-selected player. No mesh is allocated here.
pub fn player_linked(
    objects: &mut ObjectStore,
    caller: ObjectId,
    parameters: LaunchParameters,
    defaults: ObjectSpawnDefaults,
) -> Result<Option<ObjectId>, CreationError> {
    let Some(created) = common(objects, caller, parameters, defaults)? else {
        return Ok(None);
    };
    let side = objects
        .get(caller)
        .expect("validated firing actor")
        .base
        .contacts
        .hit_side;
    let weapon = objects.get_mut(created).expect("fresh weapon");
    weapon.extension.spawn_group = PLAYER_WEAPON_GROUP;
    weapon.base.contacts.exclusion_groups = weapon
        .base
        .contacts
        .exclusion_groups
        .union(ExclusionGroups::MUTUALLY_NON_DAMAGING_CLASS);
    weapon.base.contacts.mutually_non_damaging = true;
    weapon.base.attachment = Some(caller);
    if side == HitSide::Secondary {
        weapon.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
    }
    Ok(Some(created))
}

#[cfg(test)]
#[path = "weapon_creation_tests.rs"]
mod tests;
