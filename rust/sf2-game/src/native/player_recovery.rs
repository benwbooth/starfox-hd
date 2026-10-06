//! Shared shield-request consumption (`$06:9F36..9F53`) and its numbered
//! feedback installer (`$07:D0C6..D11C`). The consumable's healing emitter is
//! a different actor; requests can also come from pickups and other paths.

use super::path_relationships::{self, RelationshipError};
use super::player_hit_control::ShieldRecoveryRequest;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{
    authored_paths, Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId, OBJECT_CAPACITY,
};

const FEEDBACK_NUMBER: u8 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    World(WorldInputError),
    Relationships(RelationshipError),
    MissingRequest,
    MissingShieldCapacity,
    MissingSpawnDefaults,
    ObjectPoolExhausted,
}

impl From<WorldInputError> for RecoveryError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<RelationshipError> for RecoveryError {
    fn from(error: RelationshipError) -> Self {
        Self::Relationships(error)
    }
}

fn install_feedback(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<(), RecoveryError> {
    // An existing child is not reformatted or restarted, including when it
    // has already been marked for removal but is still linked this visit.
    if path_relationships::find_direct_child(objects, owner, FEEDBACK_NUMBER)?.is_some() {
        return Ok(());
    }
    if objects.len() == OBJECT_CAPACITY {
        return Err(RecoveryError::ObjectPoolExhausted);
    }
    let defaults = world
        .spawn_defaults()
        .ok_or(RecoveryError::MissingSpawnDefaults)?;
    let effect = Object::new_authored(
        ObjectKind::Effect,
        ShapeId::EMPTY,
        Behavior::FollowPath,
        defaults,
    );
    let head = objects.active_ids().first().copied();
    let Some(effect) = objects.allocate_after(head, effect) else {
        return Err(RecoveryError::ObjectPoolExhausted);
    };
    path_relationships::attach_fresh_child(objects, owner, effect, FEEDBACK_NUMBER)?;
    objects
        .get_mut(effect)
        .expect("allocated recovery feedback")
        .base
        .path = Some(authored_paths::PRIMARY_TARGET_FOLLOWER);
    super::player_effect::format(objects, owner, effect)?;
    Ok(())
}

/// Clear the shared request before any player/capacity reads. Recovery goes
/// to this caller, not implicitly the primary player. Pool exhaustion faults
/// after the shield change; it never cancels or requeues that completed write.
/// Returns whether a nonzero request was consumed, not allocation success.
pub fn consume(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<bool, RecoveryError> {
    let amount = std::mem::take(
        &mut world
            .shield_recovery
            .as_mut()
            .ok_or(RecoveryError::MissingRequest)?
            .amount,
    );
    if amount == 0 {
        return Ok(false);
    }
    // Preserve source read order: caller shield precedes published capacity.
    world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?;
    let capacity = world
        .active_shield_capacity
        .ok_or(RecoveryError::MissingShieldCapacity)?;
    let hit = &mut world
        .player_mut(objects, owner)?
        .contact
        .as_mut()
        .expect("validated contact owner")
        .hit;
    ShieldRecoveryRequest { amount }.consume(hit, capacity);
    install_feedback(objects, world, owner)?;
    Ok(true)
}
