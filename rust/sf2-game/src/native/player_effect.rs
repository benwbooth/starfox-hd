//! Shared player-effect formatting (`$07:BEF5..BF41`). Allocation, child
//! attachment, path selection and caller-specific overrides stay separate.

use super::collision_pass::ExclusionGroups;
use super::scene_path_world::WorldInputError;
use super::{Behavior, ObjectId, ObjectStore};

const EFFECT_HEALTH: u8 = 1;
const EFFECT_ATTACK: u8 = 1;

pub(super) fn format(
    objects: &mut ObjectStore,
    owner: ObjectId,
    effect: ObjectId,
) -> Result<(), WorldInputError> {
    let effect_record = objects
        .get_mut(effect)
        .ok_or(WorldInputError::MissingActor(effect))?;
    effect_record.base.behavior = Behavior::FollowPath;
    effect_record.extension.path_state.needs_path_initialization = true;
    effect_record.extension.spawn_group = u8::MAX;
    effect_record.base.hit_points = EFFECT_HEALTH;
    effect_record.base.attack_power = EFFECT_ATTACK;
    effect_record.base.flags.general_search_eligible = true;
    let player = &objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base;
    let (position, pitch, yaw, roll) = (player.position, player.pitch, player.yaw, player.roll);
    let effect_record = objects.get_mut(effect).expect("validated effect");
    effect_record.base.position = position;
    effect_record.base.pitch = pitch;
    effect_record.base.yaw = yaw;
    effect_record.base.roll = roll;
    effect_record.base.contacts.run_when_paused = true;
    effect_record.base.contacts.exclusion_groups = effect_record
        .base
        .contacts
        .exclusion_groups
        .without(ExclusionGroups::PATH_SPAWN);
    effect_record.base.flags.collision_disabled = true;
    Ok(())
}
