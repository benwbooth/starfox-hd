//! Surface-particle installers and strategies ($07:C6E8..C867).
//! These are attached effects with independent world motion, not paths.

use super::path_relationships::{self, RelationshipError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{
    Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId, Vector3, OBJECT_CAPACITY,
};
use sf_core::snes_trig::{rotate_8xz, rotate_8yx};

const PARTICLE_NUMBER: u8 = 16;
const PARENT_SPRITE_SIZE: u8 = 4;
const EFFECT_HEALTH: u8 = 1;
const EFFECT_ATTACK: u8 = 1;
const MANUAL_FRAME: u8 = 0x80;
const FRAME_VALUE: u8 = 0x7F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceParticle {
    /// Material four selects the four-frame shape ($C054).
    Short,
    /// Other particle-producing materials select the eight-frame shape ($BF04).
    Long,
}

impl SurfaceParticle {
    const fn frames(self) -> u8 {
        match self {
            Self::Short => 4,
            Self::Long => 8,
        }
    }
    const fn shape(self) -> ShapeId {
        ShapeId::from_catalog_index(match self {
            Self::Short => 34,
            Self::Long => 22,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleInputs {
    /// Signed byte offsets; roll precedes yaw and pitch is not applied.
    pub lateral: i8,
    pub forward: i8,
    pub size: u8,
    pub frame: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParticleError {
    World(WorldInputError),
    Relationships(RelationshipError),
    MissingSpawnDefaults,
    MissingFlightDisplacement(ObjectId),
    ObjectPoolExhausted,
}

impl From<WorldInputError> for ParticleError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<RelationshipError> for ParticleError {
    fn from(error: RelationshipError) -> Self {
        Self::Relationships(error)
    }
}

/// Always allocates a fresh sibling, even when another particle numbered 16
/// is still alive. A full pool is the original non-returning fatal branch.
pub fn spawn(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
    kind: SurfaceParticle,
    input: ParticleInputs,
) -> Result<ObjectId, ParticleError> {
    if objects.len() == OBJECT_CAPACITY {
        return Err(ParticleError::ObjectPoolExhausted);
    }
    let defaults = world
        .spawn_defaults()
        .ok_or(ParticleError::MissingSpawnDefaults)?;
    let fresh = Object::new_authored(
        ObjectKind::Effect,
        kind.shape(),
        Behavior::SurfaceParticle(kind),
        defaults,
    );
    let head = objects.active_ids().first().copied();
    let child = objects
        .allocate_after(head, fresh)
        .ok_or(ParticleError::ObjectPoolExhausted)?;
    path_relationships::attach_fresh_child(objects, owner, child, PARTICLE_NUMBER)?;
    let parent = &objects.get(owner).expect("attachment validated owner").base;
    let (position, pitch, yaw, roll) = (parent.position, parent.pitch, parent.yaw, parent.roll);
    let actor = objects.get_mut(child).expect("fresh surface particle");
    // Extension parent deliberately names self; the base attachment still
    // names the player for ordered lifetime cleanup.
    actor.extension.parent = Some(child);
    actor.base.contacts.run_when_paused = true;
    actor.base.flags.exclude_from_shape_footprint_search = true;
    actor.base.flags.collision_disabled = true;
    actor.base.flags.scaled_sprite = true;
    actor.base.flags.general_search_eligible = false;
    actor.extension.path_state.needs_path_initialization = false;
    actor
        .extension
        .path_state
        .animation
        .shape
        .initialize(input.frame);
    actor.extension.depth_offset &= 0xFF00;
    actor.extension.texture_scroll_x = input.size;
    actor.base.hit_points = EFFECT_HEALTH;
    actor.base.attack_power = EFFECT_ATTACK;
    actor.base.pitch = pitch;
    actor.base.yaw = yaw;
    actor.base.roll = roll;
    let (rolled_x, rolled_y) = rotate_8yx(roll.units(), input.lateral, 0);
    let (x, z) = rotate_8xz(yaw.units(), rolled_x as i8, input.forward);
    actor.base.position = Vector3 {
        x: position.x.wrapping_add(x),
        y: position.y.wrapping_add(rolled_y),
        z: position.z.wrapping_add(z),
    };
    // The preceding flight visit's combined thrust/base displacement is
    // distinct from both the actor velocity and the global motion snapshot.
    // Read it only after allocation, formatting and position publication.
    let movement = world
        .player(objects, owner)?
        .flight_displacement
        .ok_or(ParticleError::MissingFlightDisplacement(owner))?;
    objects
        .get_mut(child)
        .expect("fresh surface particle")
        .base
        .velocity = Vector3 {
        x: movement.x,
        y: 0,
        z: movement.z,
    };
    objects
        .get_mut(owner)
        .expect("validated owner")
        .extension
        .texture_scroll_x = PARENT_SPRITE_SIZE;
    Ok(child)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrongParticleBehavior;

/// Advance animation first. Terminal frames only request deferred removal;
/// earlier frames integrate all axes, then decay horizontal velocity with
/// two independently rounded arithmetic shifts ($03:A9C2), not multiply 5/8.
pub fn step(actor: &mut Object) -> Result<(), WrongParticleBehavior> {
    let Behavior::SurfaceParticle(kind) = actor.base.behavior else {
        return Err(WrongParticleBehavior);
    };
    let frames = kind.frames();
    let control = &mut actor.extension.path_state.animation.shape;
    let mut next = control.packed().wrapping_add(1);
    if next & MANUAL_FRAME == 0 {
        next = next.wrapping_add(frames);
    }
    next &= FRAME_VALUE;
    if next >= frames {
        control.initialize(frames - 1);
        actor.base.flags.remove_after_tick = true;
        return Ok(());
    }
    control.initialize(next);
    actor.base.position.x = actor.base.position.x.wrapping_add(actor.base.velocity.x);
    actor.base.position.y = actor.base.position.y.wrapping_add(actor.base.velocity.y);
    actor.base.position.z = actor.base.position.z.wrapping_add(actor.base.velocity.z);
    actor.base.velocity.x = (actor.base.velocity.x >> 1).wrapping_add(actor.base.velocity.x >> 3);
    actor.base.velocity.z = (actor.base.velocity.z >> 1).wrapping_add(actor.base.velocity.z >> 3);
    Ok(())
}

#[cfg(test)]
#[path = "player_surface_particle_tests.rs"]
mod tests;
