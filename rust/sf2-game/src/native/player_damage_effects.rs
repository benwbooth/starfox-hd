//! Sustained damage and particle service ($07:D1A8..D3B1), including the
//! countdown puff installer ($07:D048). Authored scenery and this service
//! share the player's particle flags; impact, shield and roll are live owners.

use super::path_control::PlayerTarget;
use super::path_program::SelectedParticleEffects;
use super::path_relationships::{self, RelationshipError};
use super::path_sound::AuthoredCue;
use super::player_hit_control::Impact;
use super::player_impact::{self, ImpactError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{
    authored_paths, Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId, SoundEvent,
    Vector3, OBJECT_CAPACITY,
};

const SUSTAINED_FLAGS: u8 = 0xE0;
const LEFT_FLAME: u8 = 0x80;
const RIGHT_FLAME: u8 = 0x40;
const CENTER_FLAME: u8 = 0x20;
const COUNTDOWN_MASK: u8 = 0x0F;
const EXPIRATION_AGE: u8 = 100;
const ROLL_PHASE_MASK: u8 = 0x1F;
const DAMAGE_CLOCK_MASK: u16 = 0x1F;
const RECOVERY_COUNT_MASK: u8 = 0x3F;
const ALTERNATING_CLOCK: u16 = 1;
const MODE_FAMILY_MASK: u8 = 0xF0;
const WALKER_FAMILY: u8 = 0x20;
const SURFACE_MODE_MASK: u8 = 7;
const EXTINGUISH_SURFACE_MODE: u8 = 3;
const SUPPRESS_PUFF_SURFACE_MODE: u8 = 5;
const PUFF_CHILD_LIMIT: usize = 5;
const PUFF_SHAPE: ShapeId = ShapeId::from_catalog_index(8);
const PUFF_NUMBER: u8 = 19;
const PUFF_CUE: u8 = 109;
const FLAME_SHAPE: ShapeId = ShapeId::from_catalog_index(38);
const FLAME_NUMBER: u8 = 37;
const FLAME_JITTER_MASK: u8 = 0x0F;
const FLAME_SIDE_OFFSET: i16 = 25;
const FLAME_LATERAL_CENTER: i16 = 7;
const FLAME_FORWARD_CENTER: i16 = 10;
const WALKER_VERTICAL_OFFSET: i16 = -10;
const LINKED_VERTICAL_OFFSET: i16 = -10;
const LINKED_FORWARD_OFFSET: i16 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageEffectsError {
    World(WorldInputError),
    Relationships(RelationshipError),
    Impact(ImpactError),
    MissingViewMode,
    MissingRoll(ObjectId),
    MissingMotion(ObjectId),
    MissingSpawnDefaults,
    ObjectPoolExhausted,
}
impl From<WorldInputError> for DamageEffectsError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<RelationshipError> for DamageEffectsError {
    fn from(error: RelationshipError) -> Self {
        Self::Relationships(error)
    }
}
impl From<ImpactError> for DamageEffectsError {
    fn from(error: ImpactError) -> Self {
        Self::Impact(error)
    }
}

fn particles<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut SelectedParticleEffects, DamageEffectsError> {
    Ok(world
        .player_mut(objects, owner)?
        .particles
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerParticles(owner))?)
}

fn walker(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<bool, DamageEffectsError> {
    Ok(world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode
        & MODE_FAMILY_MASK
        == WALKER_FAMILY)
}

fn allocate(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    shape: ShapeId,
) -> Result<ObjectId, DamageEffectsError> {
    if objects.len() == OBJECT_CAPACITY {
        return Err(DamageEffectsError::ObjectPoolExhausted);
    }
    let defaults = world
        .spawn_defaults()
        .ok_or(DamageEffectsError::MissingSpawnDefaults)?;
    let fresh = Object::new_authored(ObjectKind::Effect, shape, Behavior::FollowPath, defaults);
    let head = objects.active_ids().first().copied();
    objects
        .allocate_after(head, fresh)
        .ok_or(DamageEffectsError::ObjectPoolExhausted)
}

/// Five children of ANY number suppress puffs before the player's motion
/// record is read. Suppressed or newly allocated puffs consume no randomness.
pub fn emit_puff(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<Option<ObjectId>, DamageEffectsError> {
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
    if count >= PUFF_CHILD_LIMIT {
        return Ok(None);
    }
    if world
        .player(objects, owner)?
        .motion
        .ok_or(DamageEffectsError::MissingMotion(owner))?
        .walker_contact_control
        & SURFACE_MODE_MASK
        >= SUPPRESS_PUFF_SURFACE_MODE
    {
        return Ok(None);
    }
    let child = allocate(objects, world, PUFF_SHAPE)?;
    path_relationships::attach_fresh_child(objects, owner, child, PUFF_NUMBER)?;
    let actor = objects.get_mut(child).expect("fresh puff");
    actor.extension.parent = Some(child);
    actor.base.path = Some(authored_paths::RANDOMIZED_COLOR_PARTICLE);
    super::player_effect::format(objects, owner, child)?;
    let speed = objects.get(owner).expect("validated owner").base.speed as i8;
    // Independently rounded arithmetic half and quarter, not unsigned 3/4.
    objects.get_mut(child).unwrap().base.speed = (speed >> 1).wrapping_add(speed >> 2) as u8;
    world.audio.queue(SoundEvent::Authored(AuthoredCue::new(
        PUFF_CUE,
        0,
        PlayerTarget::Primary,
    )));
    Ok(Some(child))
}

/// A flame consumes one random byte before observing mode, charge or pool
/// availability. Its offsets are parent-relative; formatting copies only the
/// world pose. No five-child limit or existing-number reuse applies here.
pub fn emit_flame(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    lateral: i16,
) -> Result<ObjectId, DamageEffectsError> {
    let jitter = i16::from(world.random.next_byte() & FLAME_JITTER_MASK);
    let mut position = Vector3 {
        x: (jitter - FLAME_LATERAL_CENTER).wrapping_add(lateral),
        y: (jitter >> 1)
            + if walker(objects, world, owner)? {
                WALKER_VERTICAL_OFFSET
            } else {
                0
            },
        z: jitter - FLAME_FORWARD_CENTER,
    };
    if world
        .player(objects, owner)?
        .charge
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?
        .linked_mode
    {
        position.y += LINKED_VERTICAL_OFFSET;
        position.z = LINKED_FORWARD_OFFSET;
    }
    let child = allocate(objects, world, FLAME_SHAPE)?;
    path_relationships::attach_fresh_child(objects, owner, child, FLAME_NUMBER)?;
    objects
        .get_mut(child)
        .expect("fresh flame")
        .extension
        .relative_position = position;
    super::player_effect::format(objects, owner, child)?;
    objects.get_mut(child).unwrap().base.path = Some(authored_paths::CHILD_DETACHING_SPRITE);
    Ok(child)
}

fn extinguish(particles: &mut SelectedParticleEffects) {
    particles.flags = (particles.flags & !SUSTAINED_FLAGS) | COUNTDOWN_MASK;
    particles.age = 0;
}

/// Complete admitted visit. An exhausted shield kills only when it was
/// already zero on entry to the damage tick, and does not skip the remaining
/// particle emissions. Recovery TAGS alone reject feedback, but not damage.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), DamageEffectsError> {
    if world
        .scripted_view_active()
        .ok_or(DamageEffectsError::MissingViewMode)?
    {
        return Ok(());
    }
    if !world
        .contacts_enabled()
        .ok_or(WorldInputError::MissingContactEnable)?
    {
        return Ok(());
    }
    if objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .base
        .hit_points
        == 0
    {
        return Ok(());
    }
    let effects = particles(objects, world, owner)?;
    if effects.flags & SUSTAINED_FLAGS != 0 {
        effects.age = effects.age.wrapping_add(1);
        if effects.age == EXPIRATION_AGE {
            extinguish(effects);
        } else {
            let roll = world
                .player(objects, owner)?
                .roll
                .ok_or(DamageEffectsError::MissingRoll(owner))?
                .impulse;
            if roll != 0 && roll as u8 & ROLL_PHASE_MASK == 0 {
                extinguish(particles(objects, world, owner)?);
            }
        }
    }
    if particles(objects, world, owner)?.flags & SUSTAINED_FLAGS != 0 {
        if world.strategy_clock & DAMAGE_CLOCK_MASK == 0
            && world
                .player(objects, owner)?
                .contact
                .ok_or(WorldInputError::MissingPlayerContact(owner))?
                .hit
                .recovery
                & RECOVERY_COUNT_MASK
                == 0
        {
            player_impact::impact(objects, world, owner, Impact::Light)?;
            let hit = &mut world
                .player_mut(objects, owner)?
                .contact
                .as_mut()
                .expect("validated contact")
                .hit;
            if hit.reserve_shield == 0 {
                objects.get_mut(owner).unwrap().base.hit_points = 0;
            } else {
                hit.reserve_shield -= 1;
            }
        }
        if walker(objects, world, owner)?
            && world
                .player(objects, owner)?
                .motion
                .ok_or(DamageEffectsError::MissingMotion(owner))?
                .walker_contact_control
                & SURFACE_MODE_MASK
                >= EXTINGUISH_SURFACE_MODE
        {
            extinguish(particles(objects, world, owner)?);
        }
    }
    let effects = particles(objects, world, owner)?;
    if effects.flags & COUNTDOWN_MASK != 0 {
        effects.flags = effects.flags.wrapping_sub(1);
        if world.strategy_clock & ALTERNATING_CLOCK != 0 {
            emit_puff(objects, world, owner)?;
        }
    }
    let flags = particles(objects, world, owner)?.flags;
    if flags & SUSTAINED_FLAGS == 0 {
        return Ok(());
    }
    if walker(objects, world, owner)? {
        emit_flame(objects, world, owner, 0)?;
    } else if world.strategy_clock & ALTERNATING_CLOCK != 0 {
        if flags & RIGHT_FLAME != 0 {
            emit_flame(objects, world, owner, FLAME_SIDE_OFFSET)?;
        }
    } else {
        if flags & LEFT_FLAME != 0 {
            emit_flame(objects, world, owner, -FLAME_SIDE_OFFSET)?;
        }
        if flags & CENTER_FLAME != 0 {
            emit_flame(objects, world, owner, 0)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "player_damage_effects_tests.rs"]
mod tests;
