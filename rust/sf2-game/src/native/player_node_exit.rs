//! Player-owned node-exit admission and objective clearing ($06:A045..A0A5).
//! The new presentation is inserted after the active-list head and runs only
//! when the live scheduler reaches it. Admission does not execute its path,
//! copy the player's pose, attach it, or publish the path's last-spawn value.

use super::scene_path_world::ScenePathWorld;
use super::{Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId, OBJECT_CAPACITY};

const PRESENTATION_REQUESTED: u8 = 0x80;
const PRESENTATION_CREATED: u8 = 0x40;
const CLEAR_OBJECTIVES: u8 = 1;
const COMPANION_BYTE: u16 = 0xFF00;

/// Independent publications owned by scene setup and player mission control.
/// Keep unobserved inputs absent: neither a zero request nor completion can
/// be inferred from the current objective count or actor pool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NodeExitState {
    /// Request/creation flags ($1E08). Admission changes only the creation bit.
    pub presentation_flags: Option<u8>,
    /// Completion selector ($1E17). Exactly one clears the objective low byte;
    /// other nonzero values retain it and have distinct later consumers.
    pub completion_code: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeExitError {
    MissingObjectives,
    MissingPresentationFlags,
    MissingCompletionCode,
    MissingSpawnDefaults,
    ObjectPoolExhausted,
}

/// Complete this branch before the mission-completion action selection. The
/// objective-clear request is observed AFTER creation, including on a visit
/// that both creates the presentation and clears the final objective.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
) -> Result<Option<ObjectId>, NodeExitError> {
    let remaining = world
        .objective_counts
        .as_ref()
        .ok_or(NodeExitError::MissingObjectives)?
        .remaining_word as u8;
    let mut created = None;
    if remaining != 0 {
        let flags = world
            .node_exit
            .presentation_flags
            .ok_or(NodeExitError::MissingPresentationFlags)?;
        if flags & (PRESENTATION_REQUESTED | PRESENTATION_CREATED) == PRESENTATION_REQUESTED {
            // The source's nominal allocation-rejection branch is unreachable:
            // a full strategy pool enters its fatal handler before returning.
            if objects.len() == OBJECT_CAPACITY {
                return Err(NodeExitError::ObjectPoolExhausted);
            }
            let defaults = world
                .spawn_defaults()
                .ok_or(NodeExitError::MissingSpawnDefaults)?;
            let mut actor = Object::new_authored(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::FollowPath,
                defaults,
            );
            actor.base.hit_points = 1;
            actor.base.attack_power = 1;
            actor.base.flags.collision_disabled = true;
            actor.base.path = Some(super::authored_paths::NODE_EXIT_PRESENTATION);
            actor.extension.path_state.needs_path_initialization = true;
            let head = objects.active_ids().first().copied();
            created = Some(
                objects
                    .allocate_after(head, actor)
                    .ok_or(NodeExitError::ObjectPoolExhausted)?,
            );
            world.node_exit.presentation_flags = Some(flags | PRESENTATION_CREATED);
        }
    }
    if world
        .node_exit
        .completion_code
        .ok_or(NodeExitError::MissingCompletionCode)?
        == CLEAR_OBJECTIVES
    {
        world
            .objective_counts
            .as_mut()
            .expect("validated objective owner")
            .remaining_word &= COMPANION_BYTE;
    }
    Ok(created)
}

#[cfg(test)]
#[path = "player_node_exit_tests.rs"]
mod tests;
