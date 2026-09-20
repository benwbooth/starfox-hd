//! Authored death marking (`$7F:8B94`), before the callback movement tail.
//! This requests later death processing; it does not retire slots, release
//! resources, unlink children, or manufacture an explosion effect.

use super::path_relationships::RelationshipError;
use super::{ObjectId, ObjectStore, OBJECT_CAPACITY};

pub const FRIEND_HEALTH_SLOTS: usize = 5;
const INITIAL_FRIEND_HEALTH: u8 = 40;

/// Retained friend-health records, not the active pilots' health. The
/// source initializes six bytes at $03:82BB (the first is the old Fox
/// record). Its inherited friend commands address the following five with
/// a one-based actor selector. No current-pilot identity is inferred here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FriendHealth {
    pub remaining: [u8; FRIEND_HEALTH_SLOTS],
}

impl Default for FriendHealth {
    fn default() -> Self {
        Self {
            remaining: [INITIAL_FRIEND_HEALTH; FRIEND_HEALTH_SLOTS],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathError {
    Relationships(RelationshipError),
    MissingFriendHealth,
    /// Beyond the proven health records the source would overwrite other
    /// globals. Such aliases are not representable as friend health.
    InvalidFriendSelector(u8),
}

/// Diagnose invalid native state before changing either actors or health.
/// Only the direct sibling chain is marked, not grandchildren. The source
/// owner flag gates traversal independently of the refresh-suppression flag.
pub fn mark_for_death(
    objects: &mut ObjectStore,
    owner: ObjectId,
    friends: Option<&mut FriendHealth>,
) -> Result<(), DeathError> {
    let error = DeathError::Relationships;
    let actor = objects
        .get(owner)
        .ok_or_else(|| error(RelationshipError::MissingActor(owner)))?;
    let selector = actor.extension.path_state.repeat_counter;
    if usize::from(selector) > FRIEND_HEALTH_SLOTS {
        return Err(DeathError::InvalidFriendSelector(selector));
    }
    if selector != 0 && friends.is_none() {
        return Err(DeathError::MissingFriendHealth);
    }
    let mut affected = Vec::new();
    let mut visited = [false; OBJECT_CAPACITY];
    visited[owner.index()] = true;
    let mut next = if actor.extension.path_state.motion.refresh_child_chain {
        actor.base.first_child
    } else {
        None
    };
    while let Some(child) = next {
        if visited[child.index()] {
            return Err(error(RelationshipError::ChildCycle(child)));
        }
        visited[child.index()] = true;
        let actor = objects
            .get(child)
            .ok_or_else(|| error(RelationshipError::MissingActor(child)))?;
        next = actor.base.next_sibling;
        affected.push(child);
    }
    if selector != 0 {
        friends.expect("validated friend-health record").remaining[usize::from(selector - 1)] = 0;
    }
    affected.push(owner);
    for id in affected {
        let actor = objects.get_mut(id).expect("validated death-chain actor");
        actor.base.flags.suppress_death_effects = true;
        actor.base.hit_points = 0;
    }
    Ok(())
}
