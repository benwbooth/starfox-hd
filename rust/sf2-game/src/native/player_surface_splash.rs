//! Surface-crossing splash installers ($07:CBD3/CBFD), their shared
//! placement ($07:CCC3..CD6F), and two-stage lifetime ($07:CC6E..CCC2).
//! These are distinct from the moving wing-contact surface particles.

use super::path_relationships::{self, RelationshipError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{
    Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId, Vector3, OBJECT_CAPACITY,
};
use sf_core::snes_trig::{rotate_8xz, rotate_8yx};

#[cfg(test)]
#[path = "player_surface_splash_tests.rs"]
mod tests;

const ATTACHMENT_NUMBER: u8 = 16;
const EFFECT_HEALTH: u8 = 1;
const EFFECT_ATTACK: u8 = 1;
const MAXIMUM_PLANE_OFFSET: i16 = 127;
const MANUAL_FRAME: u8 = 0x80;
const FRAME_VALUE: u8 = 0x7F;
const FRAME_COUNT_SHIFT: u32 = u8::BITS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceSplash {
    Short,
    Long,
}

impl SurfaceSplash {
    pub const fn frames(self) -> u8 {
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
pub enum SplashPhase {
    InheritParentMotion,
    Animate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplashInputs {
    /// Placement origin, separate from the attachment used on the first visit.
    pub origin: ObjectId,
    pub lateral: i8,
    pub forward: i8,
    /// Only the long installer inherits these; the short entry clears both.
    pub frame: u8,
    pub size: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplashError {
    World(WorldInputError),
    Relationships(RelationshipError),
    MissingSurface(ObjectId),
    MissingParent(ObjectId),
    MissingSpawnDefaults,
    ObjectPoolExhausted,
    WrongBehavior,
}

impl From<WorldInputError> for SplashError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<RelationshipError> for SplashError {
    fn from(error: RelationshipError) -> Self {
        Self::Relationships(error)
    }
}

/// Always allocates a fresh child, with no five-child limit. The attachment
/// number is subsequently replaced by one random byte, without relinking.
pub fn spawn(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    kind: SurfaceSplash,
    input: SplashInputs,
) -> Result<ObjectId, SplashError> {
    if objects.len() == OBJECT_CAPACITY {
        return Err(SplashError::ObjectPoolExhausted);
    }
    let defaults = world
        .spawn_defaults()
        .ok_or(SplashError::MissingSpawnDefaults)?;
    let head = objects.active_ids().first().copied();
    let child = objects
        .allocate_after(
            head,
            Object::new_authored(
                ObjectKind::Effect,
                kind.shape(),
                Behavior::SurfaceSplash(SplashPhase::InheritParentMotion),
                defaults,
            ),
        )
        .ok_or(SplashError::ObjectPoolExhausted)?;
    path_relationships::attach_fresh_child(objects, owner, child, ATTACHMENT_NUMBER)?;
    let child_number = world.random.next_byte();
    let origin = objects
        .lifetime_id(input.origin)
        .ok_or(WorldInputError::MissingActor(input.origin))?;
    let parent = &objects.get(owner).expect("attachment validated owner").base;
    let (position, pitch, yaw, roll) = (parent.position, parent.pitch, parent.yaw, parent.roll);
    let (frame, size) = match kind {
        SurfaceSplash::Short => (0, 0),
        SurfaceSplash::Long => (input.frame, input.size),
    };
    let actor = objects.get_mut(child).expect("allocated splash");
    actor.extension.parent = Some(child);
    actor.base.child_number = child_number;
    actor.base.effect_origin = Some(origin);
    // The lifetime's frame limit is the high byte of the ordinary motion
    // phase field (1CE3); preserve its low byte and read the live limit later.
    actor.extension.path_state.motion_phase = (actor.extension.path_state.motion_phase & 0x00FF)
        | (u16::from(kind.frames()) << FRAME_COUNT_SHIFT);
    actor.base.contacts.run_when_paused = true;
    actor.base.flags.exclude_from_shape_footprint_search = true;
    actor.base.flags.collision_disabled = true;
    actor.base.flags.scaled_sprite = true;
    actor.base.flags.general_search_eligible = false;
    actor.extension.path_state.needs_path_initialization = false;
    actor.base.hit_points = EFFECT_HEALTH;
    actor.base.attack_power = EFFECT_ATTACK;
    actor.extension.depth_offset &= 0xFF00;
    actor.extension.texture_scroll_x = size;
    actor.base.position = position;
    actor.base.pitch = pitch;
    actor.base.yaw = yaw;
    actor.base.roll = roll;
    actor.extension.path_state.animation.shape.initialize(frame);

    // Placement uses the selected player plane and the independent origin's
    // roll/yaw. Pitch is deliberately excluded. Clamp after word subtraction.
    let plane = world
        .player(objects, owner)?
        .surface
        .ok_or(SplashError::MissingSurface(owner))?
        .plane_height;
    let origin = &objects.get(input.origin).expect("validated origin").base;
    let vertical = plane
        .wrapping_sub(origin.position.y)
        .clamp(-MAXIMUM_PLANE_OFFSET, MAXIMUM_PLANE_OFFSET) as i8;
    let (rolled_x, rolled_y) = rotate_8yx(origin.roll.units(), input.lateral, vertical);
    let (x, z) = rotate_8xz(origin.yaw.units(), rolled_x as i8, input.forward);
    let position = Vector3 {
        x: origin.position.x.wrapping_add(x),
        y: origin.position.y.wrapping_add(rolled_y),
        z: origin.position.z.wrapping_add(z),
    };
    let actor = objects.get_mut(child).expect("allocated splash");
    actor.base.position = position;
    actor.base.flags.general_search_eligible = true;
    Ok(child)
}

/// The first visit applies the attachment's current horizontal velocity
/// once, then falls through to animation. Later visits do not move at all.
pub fn step(objects: &mut ObjectStore, child: ObjectId) -> Result<(), SplashError> {
    let actor = objects
        .get(child)
        .ok_or(WorldInputError::MissingActor(child))?;
    let Behavior::SurfaceSplash(phase) = actor.base.behavior else {
        return Err(SplashError::WrongBehavior);
    };
    if phase == SplashPhase::InheritParentMotion {
        let parent = actor
            .base
            .attachment
            .ok_or(SplashError::MissingParent(child))?;
        objects
            .get_mut(child)
            .expect("validated splash")
            .base
            .behavior = Behavior::SurfaceSplash(SplashPhase::Animate);
        let velocity = objects
            .get(parent)
            .ok_or(WorldInputError::MissingActor(parent))?
            .base
            .velocity;
        let actor = objects.get_mut(child).expect("validated splash");
        actor.base.position.x = actor.base.position.x.wrapping_add(velocity.x);
        actor.base.position.z = actor.base.position.z.wrapping_add(velocity.z);
    }
    let actor = objects.get_mut(child).expect("validated splash");
    let frames = (actor.extension.path_state.motion_phase >> FRAME_COUNT_SHIFT) as u8;
    let animation = &mut actor.extension.path_state.animation.shape;
    let mut frame = animation.packed().wrapping_add(1);
    if frame & MANUAL_FRAME == 0 {
        frame = frame.wrapping_add(frames);
    }
    frame &= FRAME_VALUE;
    if frame >= frames {
        animation.initialize(frames.wrapping_sub(1));
        actor.base.flags.remove_after_tick = true;
    } else {
        animation.initialize(frame);
    }
    Ok(())
}
