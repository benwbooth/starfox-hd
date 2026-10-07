//! Protected-flight / Walker surface effect ($07:C475..C6E7).
//! Child ownership, transform origin and relative parent are three distinct
//! relationships. The effect is a real scheduled actor with a deferred first
//! visit, live movement and ordinary ordered retirement.
//! 14A3/14A4 are routine-local scratch: written before every read here, and
//! read by no other 65816 code.

use super::path_relationships::{self, RelationshipError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{
    Angle, Behavior, Object, ObjectId, ObjectKind, ObjectLifetimeId, ObjectStore, ShapeId, Vector3,
    OBJECT_CAPACITY,
};
use sf_core::snes_trig::{rotate_8xz, rotate_8yz};

#[cfg(test)]
#[path = "player_surface_effect_tests.rs"]
mod tests;

const CHILD_LIMIT: usize = 5;
const MODE_FAMILY: u8 = 0xF0;
const FLIGHT_FAMILY: u8 = 0x10;
const WALKER_SURFACE_MODE: u8 = 0x07;
const FIRST_EMITTING_MODE: u8 = 5;
const EFFECT_SHAPE: ShapeId = ShapeId::from_catalog_index(37);
const EFFECT_NUMBER: u8 = 15;
const EFFECT_HEALTH: u8 = 1;
const EFFECT_ATTACK: u8 = 1;
const EFFECT_LIFETIME: u8 = 10;
const EFFECT_SPEED: u8 = 3;
const EFFECT_SIZE: i8 = -5;
const SIZE_PHASE_MASK: u8 = 0x03;
const SIZE_PHASE_CENTER: u8 = 1;
const RANDOM_MASK: u8 = 0x07;
const RANDOM_CENTER: i16 = 3;
const SURFACE_CARRY: u8 = 0x02;
const QUARTER_TURN: i8 = 64;
const VERTICAL_STEP: i16 = -4;
const ORDINARY_VELOCITY_SCALE: i16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceEffectPhase {
    Initialize,
    Active,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceEffectInputs {
    pub origin: Option<ObjectId>,
    pub lateral: Option<i8>,
    pub vertical: Option<i8>,
    pub forward: Option<i8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceEffectResult {
    pub object: ObjectId,
    /// The pitch-rotated vertical offset is the following steering call's
    /// inherited response target. Neither yaw nor random jitter replaces it.
    pub steering_response_target: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceEffectError {
    World(WorldInputError),
    Relationships(RelationshipError),
    MissingMotion(ObjectId),
    MissingSurface(ObjectId),
    MissingModeSelection(ObjectId),
    MissingOrigin,
    RetiredOrigin(ObjectLifetimeId),
    MissingLateralOffset,
    MissingVerticalOffset,
    MissingForwardOffset,
    MissingOwner(ObjectId),
    MissingSpawnDefaults,
    ObjectPoolExhausted,
    WrongBehavior,
}

impl From<WorldInputError> for SurfaceEffectError {
    fn from(value: WorldInputError) -> Self {
        Self::World(value)
    }
}
impl From<RelationshipError> for SurfaceEffectError {
    fn from(value: RelationshipError) -> Self {
        Self::Relationships(value)
    }
}

fn child_count(objects: &ObjectStore, owner: ObjectId) -> Result<usize, SurfaceEffectError> {
    let mut next = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .attachment_next;
    let mut visited = [false; OBJECT_CAPACITY];
    let mut count = 0;
    while let Some(child) = next {
        if visited[child.index()] {
            return Err(RelationshipError::ChildCycle(child).into());
        }
        visited[child.index()] = true;
        count += 1;
        next = objects
            .get(child)
            .ok_or(WorldInputError::MissingActor(child))?
            .base
            .attachment_next;
    }
    Ok(count)
}

pub fn spawn(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    input: SurfaceEffectInputs,
) -> Result<Option<SurfaceEffectResult>, SurfaceEffectError> {
    if child_count(objects, owner)? >= CHILD_LIMIT {
        return Ok(None);
    }
    if world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode
        & MODE_FAMILY
        != FLIGHT_FAMILY
        && world
            .player(objects, owner)?
            .motion
            .ok_or(SurfaceEffectError::MissingMotion(owner))?
            .walker_contact_control
            & WALKER_SURFACE_MODE
            < FIRST_EMITTING_MODE
    {
        return Ok(None);
    }
    // Allocation failure invokes the original non-returning fatal handler;
    // the caller's nominal carry-clear branch is not a successful no-op.
    if objects.len() == OBJECT_CAPACITY {
        return Err(SurfaceEffectError::ObjectPoolExhausted);
    }
    let defaults = world
        .spawn_defaults()
        .ok_or(SurfaceEffectError::MissingSpawnDefaults)?;
    let fresh = Object::new_authored(
        ObjectKind::Effect,
        EFFECT_SHAPE,
        Behavior::SurfaceEffect(SurfaceEffectPhase::Initialize),
        defaults,
    );
    let head = objects.active_ids().first().copied();
    let child = objects
        .allocate_after(head, fresh)
        .ok_or(SurfaceEffectError::ObjectPoolExhausted)?;
    path_relationships::attach_fresh_child(objects, owner, child, EFFECT_NUMBER)?;
    let player = &objects.get(owner).expect("validated owner").base;
    let (position, pitch, yaw, roll) = (player.position, player.pitch, player.yaw, player.roll);
    let actor = objects.get_mut(child).expect("fresh effect");
    actor.extension.parent = Some(child);
    actor.base.contacts.run_when_paused = true;
    actor.extension.spawn_group = u8::MAX;
    actor.base.flags.exclude_from_shape_footprint_search = true;
    actor.base.flags.collision_disabled = true;
    actor.extension.path_state.animation.shape.initialize(0);
    actor.base.hit_points = EFFECT_HEALTH;
    actor.base.attack_power = EFFECT_ATTACK;
    actor.base.flags.scaled_sprite = true;
    actor.extension.depth_offset &= 0xFF00;
    actor.extension.texture_scroll_x = 0;
    actor.base.pitch = pitch;
    actor.base.yaw = yaw;
    actor.base.roll = roll;
    actor.base.position = position;
    actor.extension.path_state.needs_path_initialization = false;
    actor.base.flags.general_search_eligible = false;

    let origin = input.origin.ok_or(SurfaceEffectError::MissingOrigin)?;
    let origin_lifetime = objects
        .lifetime_id(origin)
        .ok_or(WorldInputError::MissingActor(origin))?;
    objects.get_mut(child).unwrap().base.effect_origin = Some(origin_lifetime);
    let origin = &objects
        .get(origin)
        .ok_or(WorldInputError::MissingActor(origin))?
        .base;
    let lateral = input
        .lateral
        .ok_or(SurfaceEffectError::MissingLateralOffset)?;
    let vertical = input
        .vertical
        .ok_or(SurfaceEffectError::MissingVerticalOffset)?;
    let forward = input
        .forward
        .ok_or(SurfaceEffectError::MissingForwardOffset)?;
    let (y, z) = rotate_8yz(origin.pitch.units(), vertical, forward);
    let (x, z) = rotate_8xz(origin.yaw.units(), lateral, z as i8);
    let position = Vector3 {
        x: origin.position.x.wrapping_add(x),
        y: origin.position.y.wrapping_add(y),
        z: origin.position.z.wrapping_add(z),
    };
    objects.get_mut(child).unwrap().base.position = position;
    let plane = world
        .player(objects, owner)?
        .surface
        .ok_or(SurfaceEffectError::MissingSurface(owner))?
        .plane_height;
    let actor = objects.get_mut(child).unwrap();
    actor.extension.path_state.script_value = plane as u16;
    actor.base.target_speed = EFFECT_LIFETIME;
    actor.base.speed = EFFECT_SPEED;
    actor.base.pitch = Angle::ZERO;
    actor.base.position.x = actor
        .base
        .position
        .x
        .wrapping_add(i16::from(world.random.next_byte() & RANDOM_MASK) - RANDOM_CENTER);
    actor.base.position.y = actor
        .base
        .position
        .y
        .wrapping_add(i16::from(world.random.next_byte() & RANDOM_MASK) - RANDOM_CENTER);
    actor.base.position.z = actor
        .base
        .position
        .z
        .wrapping_add(i16::from(world.random.next_byte() & RANDOM_MASK) - RANDOM_CENTER);
    actor.extension.path_state.motion_phase =
        (actor.extension.path_state.motion_phase & 0xFF00) | u16::from(world.strategy_clock as u8);
    Ok(Some(SurfaceEffectResult {
        object: child,
        steering_response_target: y as u16,
    }))
}

pub fn step(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    effect: ObjectId,
) -> Result<(), SurfaceEffectError> {
    let actor = objects
        .get_mut(effect)
        .ok_or(WorldInputError::MissingActor(effect))?;
    match actor.base.behavior {
        Behavior::SurfaceEffect(SurfaceEffectPhase::Initialize) => {
            actor.base.behavior = Behavior::SurfaceEffect(SurfaceEffectPhase::Active);
            actor.extension.texture_scroll_x = EFFECT_SIZE as u8;
            return Ok(());
        }
        Behavior::SurfaceEffect(SurfaceEffectPhase::Active) => {}
        _ => return Err(SurfaceEffectError::WrongBehavior),
    }
    let phase = ((world.strategy_clock as u8)
        .wrapping_add(actor.extension.path_state.motion_phase as u8)
        & SIZE_PHASE_MASK)
        .wrapping_sub(SIZE_PHASE_CENTER) as i8;
    actor.extension.texture_scroll_x = (EFFECT_SIZE + if phase < 0 { phase } else { -phase }) as u8;
    let owner = actor
        .base
        .attachment
        .ok_or(SurfaceEffectError::MissingOwner(effect))?;
    let carried = world
        .player(objects, owner)?
        .mode_selection
        .ok_or(SurfaceEffectError::MissingModeSelection(owner))?
        .surface_control
        & SURFACE_CARRY
        != 0;
    if !carried {
        let plane = world
            .player(objects, owner)?
            .surface
            .ok_or(SurfaceEffectError::MissingSurface(owner))?
            .plane_height;
        objects.get_mut(effect).unwrap().base.position.y = plane;
    } else {
        let chase = world
            .player(objects, owner)?
            .motion
            .ok_or(SurfaceEffectError::MissingMotion(owner))?
            .walker_motion_control
            != 0;
        if chase {
            let origin = objects
                .get(effect)
                .unwrap()
                .base
                .effect_origin
                .ok_or(SurfaceEffectError::MissingOrigin)?;
            if objects.lifetime_id(origin.slot()) != Some(origin) {
                return Err(SurfaceEffectError::RetiredOrigin(origin));
            }
            let position = objects
                .get(origin.slot())
                .ok_or(WorldInputError::MissingActor(origin.slot()))?
                .base
                .position;
            let actor = objects.get_mut(effect).unwrap();
            actor.base.position.x =
                super::player_pose::quarter_word(actor.base.position.x as u16, position.x as u16)
                    as i16;
            actor.base.position.z =
                super::player_pose::quarter_word(actor.base.position.z as u16, position.z as u16)
                    as i16;
        }
        let actor = objects.get_mut(effect).unwrap();
        actor.base.yaw = actor.base.yaw.wrapping_add(QUARTER_TURN);
        actor.base.velocity = super::path_motion::direction_velocity(
            actor.base.pitch,
            actor.base.yaw,
            actor.base.speed,
            ORDINARY_VELOCITY_SCALE,
        );
        super::path_motion::integrate(&mut actor.base.position, actor.base.velocity);
        actor.base.position.y = actor.base.position.y.wrapping_add(VERTICAL_STEP);
        if actor
            .base
            .position
            .y
            .wrapping_sub(actor.extension.path_state.script_value as i16)
            < 0
        {
            actor.base.flags.remove_after_tick = true;
            return Ok(());
        }
    }
    let actor = objects.get_mut(effect).unwrap();
    actor.base.target_speed = actor.base.target_speed.wrapping_sub(1);
    if actor.base.target_speed == 0 {
        actor.base.flags.remove_after_tick = true;
    }
    Ok(())
}
