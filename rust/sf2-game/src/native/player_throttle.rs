//! Boost/brake selection ($06:F010), state transitions ($06:A47B..A56C)
//! and numbered effect installers ($07:CE31..CFA4). Effects execute through
//! the ordinary authored-path scheduler; allocation never runs their paths.

use super::path_control::PlayerTarget;
use super::path_program::SelectedAuxiliaryState;
use super::path_relationships::{self, RelationshipError};
use super::path_sound::AuthoredCue;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{
    authored_paths, Behavior, Button, Object, ObjectId, ObjectKind, ObjectStore, ShapeId,
    SoundEvent, Vector3, OBJECT_CAPACITY,
};

#[cfg(test)]
#[path = "player_throttle_tests.rs"]
mod tests;

const BOOST: u8 = 0x40;
const BRAKE: u8 = 0x20;
const EFFECT_ACTIVE: u8 = 0xC0;
const EFFECT_LEVEL: u8 = 0x0F;
const EFFECT_PENDING: u8 = 0x3F;
const BOOST_NUMBER: u8 = 12;
const LEFT_BRAKE_NUMBER: u8 = 13;
const RIGHT_BRAKE_NUMBER: u8 = 14;
const EFFECT_SHAPE: ShapeId = ShapeId::from_catalog_index(19);
const BOOST_DISTANCE: i16 = -20;
const BRAKE_DISTANCE: i16 = 15;
const BRAKE_X: [i16; 3] = [15, 25, 15];
const BRAKE_Y: [i16; 3] = [0, 10, 10];
const PILOT_COUNT: u8 = 6;
const BOOST_CUE: u8 = 26;
const BRAKE_CUE: u8 = 27;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerThrottle {
    /// 6B78: zero prefers boost; any nonzero value prefers brake.
    pub brake_preference: u8,
    /// 6B79/7A: preserved effect-control fields, not invented timers or
    /// palette state. Cancellation decays the low level on even visits.
    pub effect_flags: u8,
    pub effect_level: u8,
    /// 6B7F: cleared on every boost visit and only on brake entry.
    pub entry_marker: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThrottleError {
    World(WorldInputError),
    Relationships(RelationshipError),
    MissingControl(ObjectId),
    MissingVisit(ObjectId),
    MissingProcessedInput,
    MissingActivity,
    MissingSpawnDefaults,
    ObjectPoolExhausted,
}

impl From<WorldInputError> for ThrottleError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<RelationshipError> for ThrottleError {
    fn from(error: RelationshipError) -> Self {
        Self::Relationships(error)
    }
}

fn state<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut PlayerThrottle, ThrottleError> {
    world
        .player_mut(objects, owner)?
        .throttle
        .as_mut()
        .ok_or(ThrottleError::MissingControl(owner))
}

fn auxiliary<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut SelectedAuxiliaryState, ThrottleError> {
    world
        .player_mut(objects, owner)?
        .auxiliary
        .as_mut()
        .ok_or(WorldInputError::MissingAuxiliary(owner).into())
}

fn linked_effect_suppressed(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<bool, ThrottleError> {
    let charge = world
        .player(objects, owner)?
        .charge
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
    Ok(charge.linked_mode && !charge.linked_muzzle_disabled)
}

fn cue(world: &mut ScenePathWorld, owner: ObjectId, id: u8) {
    let side = if world.primary_player == Some(owner) {
        PlayerTarget::Primary
    } else {
        PlayerTarget::Secondary
    };
    world
        .audio
        .queue(SoundEvent::Authored(AuthoredCue::new(id, 0, side)));
}

fn allocate_effect(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
    number: u8,
    position: Vector3,
) -> Result<Option<ObjectId>, ThrottleError> {
    if path_relationships::find_direct_child(objects, owner, number)?.is_some() {
        return Ok(None);
    }
    // $7F:2969 enters the non-returning $00:8032 fatal display when no
    // free record exists. The apparent carry-clear return is unreachable.
    // Preserve the partial visit and let the scene latch a native fault.
    if objects.len() == OBJECT_CAPACITY {
        return Err(ThrottleError::ObjectPoolExhausted);
    }
    let defaults = world
        .spawn_defaults()
        .ok_or(ThrottleError::MissingSpawnDefaults)?;
    let fresh = Object::new_authored(
        ObjectKind::Effect,
        EFFECT_SHAPE,
        Behavior::FollowPath,
        defaults,
    );
    let head = objects.active_ids().first().copied();
    let effect = objects
        .allocate_after(head, fresh)
        .ok_or(ThrottleError::ObjectPoolExhausted)?;
    path_relationships::attach_fresh_child(objects, owner, effect, number)?;
    let record = objects.get_mut(effect).expect("allocated throttle effect");
    record.extension.relative_position = position;
    record.base.path = Some(if number == BOOST_NUMBER {
        authored_paths::CALLBACK_GATED_SPRITE
    } else {
        authored_paths::AUXILIARY_GATED_SPRITE
    });
    super::player_effect::format(objects, owner, effect)?;
    Ok(Some(effect))
}

/// Returns only a newly allocated effect. Existing numbered children are
/// not refreshed, even when already marked for deferred retirement.
pub fn ensure_boost_effect(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<Option<ObjectId>, ThrottleError> {
    // Unlike brake, boost checks linked mode before searching the chain.
    if linked_effect_suppressed(objects, world, owner)? {
        return Ok(None);
    }
    allocate_effect(
        objects,
        world,
        owner,
        BOOST_NUMBER,
        Vector3 {
            x: 0,
            y: 0,
            z: BOOST_DISTANCE,
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrakeSide {
    Left,
    Right,
}

pub fn ensure_brake_effect(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
    side: BrakeSide,
) -> Result<Option<ObjectId>, ThrottleError> {
    let number = match side {
        BrakeSide::Left => LEFT_BRAKE_NUMBER,
        BrakeSide::Right => RIGHT_BRAKE_NUMBER,
    };
    if path_relationships::find_direct_child(objects, owner, number)?.is_some() {
        return Ok(None);
    }
    let pilot = world
        .player(objects, owner)?
        .visit
        .ok_or(ThrottleError::MissingVisit(owner))?
        .pilot_code;
    // $06:90F8 returns the even byte offset, not twice the pilot index.
    let pair = usize::from(if pilot < PILOT_COUNT { pilot / 2 } else { 0 });
    let x = if side == BrakeSide::Left {
        -BRAKE_X[pair]
    } else {
        BRAKE_X[pair]
    };
    if linked_effect_suppressed(objects, world, owner)? {
        return Ok(None);
    }
    allocate_effect(
        objects,
        world,
        owner,
        number,
        Vector3 {
            x,
            y: BRAKE_Y[pair],
            z: BRAKE_DISTANCE,
        },
    )
}

pub fn boost(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ThrottleError> {
    state(objects, world, owner)?.entry_marker = 0;
    ensure_boost_effect(objects, world, owner)?;
    if auxiliary(objects, world, owner)?.action_flags & BOOST == 0 {
        cue(world, owner, BOOST_CUE);
    }
    let flags = &mut auxiliary(objects, world, owner)?.action_flags;
    *flags = (*flags & !BRAKE) | BOOST;
    activate(objects, world, owner)
}

pub fn brake(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ThrottleError> {
    // Both installers execute when the first returns without allocation
    // (existing child or linked mode). A fatal pool failure does not return.
    ensure_brake_effect(objects, world, owner, BrakeSide::Left)?;
    ensure_brake_effect(objects, world, owner, BrakeSide::Right)?;
    if auxiliary(objects, world, owner)?.action_flags & BRAKE == 0 {
        state(objects, world, owner)?.entry_marker = 0;
        cue(world, owner, BRAKE_CUE);
        let flags = &mut auxiliary(objects, world, owner)?.action_flags;
        *flags = (*flags & !BOOST) | BRAKE;
    }
    // Already-braking skips the source store altogether: preserve even an
    // incoming state with both action bits set, rather than normalizing it.
    activate(objects, world, owner)
}

fn activate(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ThrottleError> {
    let control = state(objects, world, owner)?;
    control.effect_flags |= EFFECT_ACTIVE;
    control.effect_level |= EFFECT_LEVEL;
    Ok(())
}

fn clear_action(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ThrottleError> {
    auxiliary(objects, world, owner)?.action_flags &= !(BOOST | BRAKE);
    let even = world.strategy_clock & 1 == 0;
    let control = state(objects, world, owner)?;
    control.effect_level &= !EFFECT_ACTIVE;
    if control.effect_flags & EFFECT_PENDING == 0
        && control.effect_level & EFFECT_LEVEL != 0
        && even
    {
        control.effect_level -= 1;
    }
    Ok(())
}

pub fn cancel(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ThrottleError> {
    state(objects, world, owner)?.effect_flags &= !BOOST;
    clear_action(objects, world, owner)
}

/// $06:A55B is a distinct entry: clear effect bit 80, retaining bit 40,
/// before the same action/level decay. It does not reset preference.
pub fn deactivate_effect(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ThrottleError> {
    state(objects, world, owner)?.effect_flags &= !0x80;
    clear_action(objects, world, owner)
}

/// Complete throttle visit, consuming the canonical processed input only
/// when the live activity byte admits input. The enclosing strategy owns
/// order relative to surface response, speed, pose and movement.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ThrottleError> {
    if !world
        .contacts_enabled()
        .ok_or(ThrottleError::MissingActivity)?
    {
        return cancel(objects, world, owner);
    }
    let input = world
        .processed_player_input
        .ok_or(ThrottleError::MissingProcessedInput)?;
    if input.pressed.contains(Button::A) {
        state(objects, world, owner)?.brake_preference = 1;
    } else if input.pressed.contains(Button::Y) {
        state(objects, world, owner)?.brake_preference = 0;
    }
    if state(objects, world, owner)?.brake_preference == 0 && input.held.contains(Button::Y) {
        boost(objects, world, owner)
    } else if input.held.contains(Button::A) {
        brake(objects, world, owner)
    } else {
        state(objects, world, owner)?.brake_preference = 0;
        cancel(objects, world, owner)
    }
}
