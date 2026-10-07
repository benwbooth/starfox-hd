//! Exit-presentation shield installer and strategy ($06:F953..FA03).
//! Admission reads the primary player's protection; attachment and pose use
//! the current actor. This is a scheduled effect, not a path or a countdown.

use super::path_protection::{DeflectionProtection, LinkedEffectActivity};
use super::path_relationships::{self, RelationshipError};
use super::scene_path_world::WorldInputError;
use super::{
    Behavior, Object, ObjectId, ObjectKind, ObjectSpawnDefaults, ObjectStore, ShapeId,
    OBJECT_CAPACITY,
};

const SHIELD_SHAPE: ShapeId = ShapeId::from_catalog_index(84);
const CHILD_NUMBER: u8 = 12;
const SPIN_PITCH: i8 = 8;
const SPIN_ROLL: i8 = 6;
const SHIELD_FRAME: u8 = 0x89;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitShieldError {
    World(WorldInputError),
    Relationships(RelationshipError),
    MissingPrimaryProtection,
    MissingSpawnDefaults,
    MissingSpawnActivity,
    MissingAttachment,
    MissingActionGate,
    ObjectPoolExhausted,
    WrongBehavior,
}

impl From<WorldInputError> for ExitShieldError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<RelationshipError> for ExitShieldError {
    fn from(error: RelationshipError) -> Self {
        Self::Relationships(error)
    }
}

/// Every admitted invocation allocates a fresh numbered child; no reuse or
/// action-gate check occurs at birth. Full allocation is a fatal source path.
pub fn install(
    objects: &mut ObjectStore,
    owner: ObjectId,
    primary_protection: Option<DeflectionProtection>,
    defaults: Option<ObjectSpawnDefaults>,
    activity: Option<&mut LinkedEffectActivity>,
) -> Result<Option<ObjectId>, ExitShieldError> {
    if primary_protection
        .ok_or(ExitShieldError::MissingPrimaryProtection)?
        .remaining()
        == 0
    {
        return Ok(None);
    }
    if objects.len() == OBJECT_CAPACITY {
        return Err(ExitShieldError::ObjectPoolExhausted);
    }
    let defaults = defaults.ok_or(ExitShieldError::MissingSpawnDefaults)?;
    let fresh = Object::new_authored(
        ObjectKind::Effect,
        SHIELD_SHAPE,
        Behavior::ExitShield,
        defaults,
    );
    let head = objects.active_ids().first().copied();
    let child = objects
        .allocate_after(head, fresh)
        .ok_or(ExitShieldError::ObjectPoolExhausted)?;
    path_relationships::attach_fresh_child(objects, owner, child, CHILD_NUMBER)?;
    super::player_effect::format(objects, owner, child)?;
    let actor = objects.get_mut(child).expect("fresh exit shield");
    actor.base.behavior = Behavior::ExitShield;
    actor.extension.path_state.needs_path_initialization = false;
    activity
        .ok_or(ExitShieldError::MissingSpawnActivity)?
        .recent_spawn = 1;
    actor.base.flags.collision_disabled = true;
    Ok(Some(child))
}

/// Spin precedes parent observations. Hidden/empty parents skip both frame
/// publication and gate observation; retirement is deferred to scene cleanup.
pub fn step(
    objects: &mut ObjectStore,
    owner: ObjectId,
    action_gate: Option<u8>,
) -> Result<(), ExitShieldError> {
    let actor = objects
        .get_mut(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    if actor.base.behavior != Behavior::ExitShield {
        return Err(ExitShieldError::WrongBehavior);
    }
    actor.base.flags.maximum_draw_distance = true;
    actor.base.flags.collision_disabled = true;
    actor.extension.relative_rotation.pitch = actor
        .extension
        .relative_rotation
        .pitch
        .wrapping_add(SPIN_PITCH);
    actor.extension.relative_rotation.roll = actor
        .extension
        .relative_rotation
        .roll
        .wrapping_add(SPIN_ROLL);
    let parent = actor
        .base
        .attachment
        .ok_or(ExitShieldError::MissingAttachment)?;
    let parent = objects
        .get(parent)
        .ok_or(WorldInputError::MissingActor(parent))?;
    if parent.base.flags.visible && parent.base.shape != ShapeId::EMPTY {
        objects
            .get_mut(owner)
            .expect("validated shield")
            .extension
            .path_state
            .animation
            .shape
            .initialize(SHIELD_FRAME);
        if action_gate.ok_or(ExitShieldError::MissingActionGate)? != 0 {
            return Ok(());
        }
    }
    objects
        .get_mut(owner)
        .expect("validated shield")
        .base
        .flags
        .remove_after_tick = true;
    Ok(())
}

#[cfg(test)]
#[path = "exit_shield_tests.rs"]
mod tests;
