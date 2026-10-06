//! Primary-player target retention inside the display service
//! (`$07:A50A..A66B`, called by `$07:A326`). This boundary begins after
//! reticle positioning and ends before display drawing. It is not an actor
//! strategy and does not choose its own frame cadence.

use super::path_control::PlayerTarget;
use super::path_sound::AuthoredCue;
use super::path_target::{PublishedHomingTarget, TargetSelection};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore, SoundEvent};

const PLAYER_ACTIVE: u8 = 0x01;
const PATH_REQUESTED: u8 = 0x08;
const SELECTION_LOCKED: u8 = 0x10;
const RETENTION_SUPPRESSED: u8 = 0x20;
const MARKER_STYLE_MASK: u8 = 0xF0;
const RETICLE_OFFSET: u8 = 24;
const WINDOW_HALF_WIDTH: u8 = 32;
const WINDOW_WIDTH: u8 = 64;
const RETENTION_GRACE: u8 = 10;
const ACQUISITION_CUE: u8 = 59;

/// Fields distinct from candidate selection. The forced owner remains in
/// TargetSelection: the next candidate scan must observe this same value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TargetLock {
    pub previous_candidate: Option<ObjectId>,
    /// A nonzero byte advances with wrapping; zero acquires immediately.
    pub acquisition_clock: u8,
    pub grace_remaining: u8,
    /// Separate from TargetSelection's display-status byte.
    pub marker_style: u8,
}

/// Shared screen point published by reticle positioning. Axes are separate:
/// one reset writes only horizontal, and an out-of-window horizontal point
/// bypasses the vertical read. No default center is manufactured here.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TargetReticle {
    pub horizontal: Option<u8>,
    pub vertical: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetLockError {
    World(WorldInputError),
    MissingUpgrade,
    MissingScriptedViewMode,
    MissingLock(ObjectId),
    MissingHorizontalReticle,
    MissingVerticalReticle,
}

impl From<WorldInputError> for TargetLockError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn selection<'a>(
    objects: &ObjectStore,
    world: &'a ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a TargetSelection, TargetLockError> {
    world
        .player(objects, owner)?
        .target_selection
        .as_ref()
        .ok_or(WorldInputError::MissingTargetSelection(owner).into())
}

fn lock_mut<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut TargetLock, TargetLockError> {
    world
        .player_mut(objects, owner)?
        .target_lock
        .as_mut()
        .ok_or(TargetLockError::MissingLock(owner))
}

fn finish(objects: &ObjectStore, world: &mut ScenePathWorld, owner: ObjectId) {
    world
        .player_mut(objects, owner)
        .expect("validated player")
        .target_selection
        .as_mut()
        .expect("validated target selection")
        .control_flags &= !(PATH_REQUESTED | SELECTION_LOCKED);
}

fn cancel(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), TargetLockError> {
    let candidate = selection(objects, world, owner)?.candidate;
    lock_mut(objects, world, owner)?.previous_candidate = candidate;
    let player = world.player_mut(objects, owner)?;
    player.target_selection.as_mut().unwrap().forced_owner = None;
    let lock = player.target_lock.as_mut().unwrap();
    lock.acquisition_clock = 0;
    lock.grace_remaining = 0;
    // Cancellation alone does NOT clear the shared projectile publication.
    finish(objects, world, owner);
    Ok(())
}

fn inside_window(reticle: u8, candidate: u8) -> bool {
    let centered = reticle
        .wrapping_sub(RETICLE_OFFSET)
        .wrapping_sub(candidate)
        .wrapping_add(WINDOW_HALF_WIDTH);
    (centered as i8) >= 0 && (centered.wrapping_sub(WINDOW_WIDTH) as i8) < 0
}

/// Run once at the enclosing display service's retention boundary. Select
/// the actual primary player, not a path-selected player or view subject.
/// Missing inputs are lazy and preserve already-executed source writes.
/// The enclosing scene must latch errors before attempting another visit.
pub fn update(objects: &ObjectStore, world: &mut ScenePathWorld) -> Result<(), TargetLockError> {
    let owner = world
        .primary_player
        .ok_or(WorldInputError::MissingSelectedPlayer(
            PlayerTarget::Primary,
        ))?;
    if !world
        .targeting_upgrade
        .ok_or(TargetLockError::MissingUpgrade)?
        .active_pilot_has_upgrade()
    {
        world.published_homing_target = Some(PublishedHomingTarget::default());
        return cancel(objects, world, owner);
    }
    let candidate = selection(objects, world, owner)?.candidate;
    if candidate.is_none() {
        return cancel(objects, world, owner);
    }
    if !world
        .contacts_enabled()
        .ok_or(WorldInputError::MissingContactEnable)?
    {
        return cancel(objects, world, owner);
    }
    if world
        .scripted_view_active()
        .ok_or(TargetLockError::MissingScriptedViewMode)?
    {
        return cancel(objects, world, owner);
    }
    if world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .action_flags
        & PLAYER_ACTIVE
        == 0
    {
        return cancel(objects, world, owner);
    }
    let control = selection(objects, world, owner)?.control_flags;
    if control & PATH_REQUESTED != 0 {
        world.published_homing_target = Some(PublishedHomingTarget::default());
        return cancel(objects, world, owner);
    }
    if lock_mut(objects, world, owner)?.previous_candidate != candidate {
        return cancel(objects, world, owner);
    }
    lock_mut(objects, world, owner)?.previous_candidate = None;
    world.published_homing_target = Some(PublishedHomingTarget::default());
    if control & RETENTION_SUPPRESSED != 0 {
        return cancel(objects, world, owner);
    }
    let screen = selection(objects, world, owner)?.screen;
    let horizontal = world
        .target_reticle
        .horizontal
        .ok_or(TargetLockError::MissingHorizontalReticle)?;
    let inside = inside_window(horizontal, screen[0])
        && inside_window(
            world
                .target_reticle
                .vertical
                .ok_or(TargetLockError::MissingVerticalReticle)?,
            screen[1],
        );
    if inside {
        let lock = lock_mut(objects, world, owner)?;
        lock.previous_candidate = candidate;
        if lock.acquisition_clock != 0 {
            lock.acquisition_clock = lock.acquisition_clock.wrapping_add(1);
        } else {
            if selection(objects, world, owner)?.forced_owner.is_none() {
                lock_mut(objects, world, owner)?.marker_style &= MARKER_STYLE_MASK;
                // The source queues a literal primary cue, without listener
                // routing, before publishing ownership and the grace period.
                world.audio.queue(SoundEvent::Authored(AuthoredCue::new(
                    ACQUISITION_CUE,
                    0,
                    PlayerTarget::Primary,
                )));
            }
            world
                .player_mut(objects, owner)?
                .target_selection
                .as_mut()
                .unwrap()
                .forced_owner = candidate;
            lock_mut(objects, world, owner)?.previous_candidate = candidate;
            world.published_homing_target = Some(PublishedHomingTarget { object: candidate });
            lock_mut(objects, world, owner)?.grace_remaining = RETENTION_GRACE;
        }
    } else {
        let forced = selection(objects, world, owner)?.forced_owner;
        if forced.is_none() || lock_mut(objects, world, owner)?.grace_remaining == 0 {
            return cancel(objects, world, owner);
        }
        lock_mut(objects, world, owner)?.grace_remaining -= 1;
        world.published_homing_target = Some(PublishedHomingTarget { object: forced });
        lock_mut(objects, world, owner)?.previous_candidate = forced;
    }
    finish(objects, world, owner);
    Ok(())
}

#[cfg(test)]
#[path = "player_target_lock_tests.rs"]
mod tests;
