//! Shared death dispatch and pooled effects ($03:A055..A326, $03:A62A).
//!
//! Death is not retirement: contacts, owned programs and the actor slot remain
//! live until deferred cleanup. The world owns native override/map callbacks;
//! neither may be replaced with assumed success by a scene adapter.

use super::intro_destruction::{IntroExplosionProfile, IntroExplosionSize, IntroExplosionVolume};
use super::path_appearance::AnimationControl;
use super::path_control::PlayerTarget;
use super::path_relationships::RelationshipError;
use super::path_sound::AuthoredCue;
use super::scene_proxy::{SceneProxyError, SceneProxyStore};
use super::{
    Behavior, Object, ObjectId, ObjectKind, ObjectSpawnDefaults, ObjectStore, ShapeId, Vector3,
    OBJECT_CAPACITY,
};

const EXPLOSION_CUE: u8 = 112;
const COMPANION_INITIAL_DURATION: u8 = 2;
const COMPANION_TRAIL_DURATION: u8 = 64;
const COMPANION_STYLE: u8 = 30;
const COMPANION_SPREAD: u8 = 7;
const COLOR_PERIOD: u8 = 8;
const SCROLL_COMPENSATION_MODE: u8 = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectPhase {
    Animate,
    Trail,
}

/// Map-program actor accounting ($1ACA/$1ACC), not campaign objective counts.
/// The common death routine changes each LOW byte with wrapping arithmetic.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MapDeathCounts {
    pub active: u16,
    pub destroyed: u16,
}

impl MapDeathCounts {
    fn record_death(&mut self) {
        const HIGH_BYTE: u16 = 0xFF00;
        self.active = (self.active & HIGH_BYTE) | u16::from((self.active as u8).wrapping_sub(1));
        self.destroyed =
            (self.destroyed & HIGH_BYTE) | u16::from((self.destroyed as u8).wrapping_add(1));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectInputs {
    pub spawn: ObjectSpawnDefaults,
    /// Fixed view markers, not the selected player's pose. Scene flag 1AA6
    /// bit 02 suppresses the secondary cue; its distance does not reroute it.
    pub primary_marker: Vector3,
    pub secondary_marker: Option<Vector3>,
}

pub trait DestructionHost {
    type Error;
    fn objects(&self) -> &ObjectStore;
    fn objects_mut(&mut self) -> &mut ObjectStore;
    fn objects_and_proxies_mut(&mut self) -> (&mut ObjectStore, &mut SceneProxyStore);
    /// Resolve auxiliary type 12 to its native handler and execute it. Some
    /// returns that handler's actual actor and bypasses ALL default work.
    /// None means there is genuinely no registered handler, not unsupported.
    fn run_death_override(&mut self, owner: ObjectId) -> Result<Option<ObjectId>, Self::Error>;
    fn map_death_counts(&mut self) -> Result<&mut MapDeathCounts, Self::Error>;
    /// $03:915F: match actor or proxy, run its map continuation, remove that
    /// registration and restore the outer map context. Called AFTER counts.
    fn resume_map_on_death(&mut self, owner: ObjectId) -> Result<(), Self::Error>;
    fn effect_inputs(&mut self) -> Result<EffectInputs, Self::Error>;
    fn queue_death_sound(&mut self, cue: AuthoredCue) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DestructionError<E> {
    MissingActor(ObjectId),
    MissingShape(ShapeId),
    ObjectCapacity,
    Relationships(RelationshipError),
    SceneProxy(SceneProxyError),
    Host(E),
}

fn sound(position: Vector3, listener: Vector3) -> AuthoredCue {
    let parameter = match IntroExplosionVolume::between(position, listener) {
        IntroExplosionVolume::Near => 0,
        IntroExplosionVolume::Middle => 0x30,
        IntroExplosionVolume::Far => 0x60,
    };
    // Both $06:8000 and $06:8007 enqueue the same route. Their fixed marker
    // selects distance only; unlike path cues they do not OR secondary routing.
    AuthoredCue::new(EXPLOSION_CUE, parameter, PlayerTarget::Primary)
}

fn fresh_effect(defaults: ObjectSpawnDefaults) -> Object {
    let mut effect = Object::new_authored(
        ObjectKind::Effect,
        ShapeId::EMPTY,
        Behavior::Destruction(EffectPhase::Animate),
        defaults,
    );
    effect.base.flags.collision_disabled = true;
    effect.base.flags.exclude_from_shape_footprint_search = true;
    effect.base.flags.maximum_draw_distance = true;
    effect.base.contacts.run_when_paused = true;
    effect
}

fn allocate_effect<E>(
    objects: &mut ObjectStore,
    effect: Object,
) -> Result<ObjectId, DestructionError<E>> {
    // $7F:2A17 allocates after the GLOBAL head, not after the dying actor.
    // The head is not temporarily scoped as it is for path/weapon children.
    let head = objects.active_ids().first().copied();
    objects
        .allocate_after(head, effect)
        .ok_or(DestructionError::ObjectCapacity)
}

/// One complete common-death invocation. Errors are terminal diagnostics,
/// never invitations to restart an invocation after its callbacks/allocations.
/// The source out-of-object diagnostic occurs at the failing allocation;
/// earlier sounds and an earlier huge-effect allocation remain observable.
pub fn destroy<H: DestructionHost>(
    host: &mut H,
    owner: ObjectId,
) -> Result<ObjectId, DestructionError<H::Error>> {
    if host.objects().get(owner).is_none() {
        return Err(DestructionError::MissingActor(owner));
    }
    if let Some(returned) = host
        .run_death_override(owner)
        .map_err(DestructionError::Host)?
    {
        return Ok(returned);
    }
    let tracked = host
        .objects()
        .get(owner)
        .ok_or(DestructionError::MissingActor(owner))?
        .base
        .flags
        .tracked_map_actor;
    if tracked {
        host.map_death_counts()
            .map_err(DestructionError::Host)?
            .record_death();
        host.resume_map_on_death(owner)
            .map_err(DestructionError::Host)?;
    }
    // The map continuation can change this actor, including its effect gate.
    let current = host
        .objects()
        .get(owner)
        .ok_or(DestructionError::MissingActor(owner))?;
    if !current.base.flags.suppress_death_effects {
        let inputs = host.effect_inputs().map_err(DestructionError::Host)?;
        let position = host
            .objects()
            .get(owner)
            .ok_or(DestructionError::MissingActor(owner))?
            .base
            .position;
        if let Some(marker) = inputs.secondary_marker {
            host.queue_death_sound(sound(position, marker))
                .map_err(DestructionError::Host)?;
        }
        host.queue_death_sound(sound(position, inputs.primary_marker))
            .map_err(DestructionError::Host)?;
        let current = host
            .objects_mut()
            .get_mut(owner)
            .ok_or(DestructionError::MissingActor(owner))?;
        current.base.flags.exclude_from_shape_footprint_search = true;
        current.base.contacts.run_when_paused = true;

        let mut sprite = fresh_effect(inputs.spawn);
        sprite.base.flags.scaled_sprite = true;
        let sprite_id = allocate_effect(host.objects_mut(), sprite)?;
        let shape = host
            .objects()
            .get(owner)
            .ok_or(DestructionError::MissingActor(owner))?
            .base
            .shape;
        let profile =
            IntroExplosionProfile::for_shape(shape).ok_or(DestructionError::MissingShape(shape))?;
        if profile.size == IntroExplosionSize::Huge {
            let mut companion = fresh_effect(inputs.spawn);
            companion.base.flags.reclaim_on_pool_pressure = true;
            companion.base.wait_timer = COMPANION_SPREAD;
            companion.base.child_number = COMPANION_STYLE;
            companion.extension.path_state.repeat_counter = COMPANION_STYLE;
            companion.base.acceleration = COMPANION_INITIAL_DURATION;
            companion.base.position = position;
            // Health and automatic animation controls retain fresh zeroes.
            // In particular this is NOT a healthy generic particle: after
            // its first-visit exemption it re-enters common death itself.
            allocate_effect(host.objects_mut(), companion)?;
        }
        let sprite = host
            .objects_mut()
            .get_mut(sprite_id)
            .expect("new death sprite");
        sprite.base.shape = profile.size.shape();
        sprite.base.acceleration = profile.size.updates();
        sprite.extension.texture_scroll_x = profile.sprite_size_bias;
        sprite.extension.path_state.animation.color = AnimationControl::from_packed(0x80);
        sprite.extension.color_frame = 0;
        sprite.base.position = position;
        sprite.base.hit_points = 1;
        let current = host
            .objects_mut()
            .get_mut(owner)
            .ok_or(DestructionError::MissingActor(owner))?;
        current.base.shape = ShapeId::EMPTY;
        current.base.behavior = Behavior::Destruction(EffectPhase::Animate);
    }
    detach_dying_children(host.objects_mut(), owner).map_err(DestructionError::Relationships)?;
    let (objects, proxies) = host.objects_and_proxies_mut();
    proxies
        .release_actor_proxy(objects, owner)
        .map_err(DestructionError::SceneProxy)?;
    objects
        .get_mut(owner)
        .ok_or(DestructionError::MissingActor(owner))?
        .base
        .flags
        .remove_after_tick = true;
    Ok(owner)
}

/// Death's child service ($7F:2AA4) differs from final retirement: gate off
/// the owner, detach direct children and kill only owned lifetimes. Retain
/// sibling links, extension parents, health of independent children, incoming
/// links, contacts and program resources until their respective owners act.
pub fn detach_dying_children(
    objects: &mut ObjectStore,
    owner: ObjectId,
) -> Result<(), RelationshipError> {
    let actor = objects
        .get(owner)
        .ok_or(RelationshipError::MissingActor(owner))?;
    if !actor.extension.path_state.motion.refresh_child_chain {
        return Ok(());
    }
    let mut next = actor.base.first_child;
    let mut children = Vec::new();
    let mut visited = [false; OBJECT_CAPACITY];
    visited[owner.index()] = true;
    while let Some(child) = next {
        if visited[child.index()] {
            return Err(RelationshipError::ChildCycle(child));
        }
        visited[child.index()] = true;
        let actor = objects
            .get(child)
            .ok_or(RelationshipError::MissingActor(child))?;
        next = actor.base.next_sibling;
        children.push(child);
    }
    objects
        .get_mut(owner)
        .expect("validated death owner")
        .extension
        .path_state
        .motion
        .refresh_child_chain = false;
    for child in children {
        let actor = objects.get_mut(child).expect("validated death child");
        actor.extension.path_state.motion.attached_coordinates = false;
        actor.base.attachment = None;
        if actor.base.flags.remove_with_parent {
            actor.base.flags.collision_disabled = true;
            actor.base.hit_points = 0;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectMotion {
    /// High nibble of the PRIMARY player's auxiliary mode, not selection.
    pub primary_mode: u8,
    pub displacement: Vector3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectError {
    WrongBehavior,
    MissingMotion,
}

/// One assigned strategy invocation, without death/pause/first-visit routing.
/// Animation needs fresh primary inputs; the trailing delay does not read them.
pub fn step_effect(actor: &mut Object, motion: Option<EffectMotion>) -> Result<(), EffectError> {
    match actor.base.behavior {
        Behavior::Destruction(EffectPhase::Animate) => {
            let input = motion.ok_or(EffectError::MissingMotion)?;
            if input.primary_mode & 0xF0 == SCROLL_COMPENSATION_MODE {
                actor.base.position.x = actor
                    .base
                    .position
                    .x
                    .wrapping_sub(input.displacement.x.wrapping_mul(2));
                actor.base.position.z = actor
                    .base
                    .position
                    .z
                    .wrapping_sub(input.displacement.z.wrapping_mul(2));
            }
            actor.base.target_speed = actor.base.target_speed.wrapping_add(1);
            if (actor
                .base
                .target_speed
                .wrapping_sub(actor.base.acceleration) as i8)
                < 0
            {
                actor
                    .extension
                    .path_state
                    .animation
                    .color
                    .advance(1, COLOR_PERIOD);
                actor.extension.color_frame = actor.extension.path_state.animation.color.resolve(0);
                return Ok(());
            }
            if actor.base.flags.reclaim_on_pool_pressure {
                actor.base.wait_timer = 0;
                actor.base.child_number = 0;
                actor.extension.path_state.repeat_counter = 0;
                actor.base.flags.casts_shadow = false;
                actor.base.behavior = Behavior::Destruction(EffectPhase::Trail);
                actor.base.target_speed = COMPANION_TRAIL_DURATION;
            } else {
                return finish_effect(actor);
            }
        }
        Behavior::Destruction(EffectPhase::Trail) => {}
        _ => return Err(EffectError::WrongBehavior),
    }
    if actor.base.target_speed == 0 {
        finish_effect(actor)
    } else {
        actor.base.target_speed = actor.base.target_speed.wrapping_sub(1);
        Ok(())
    }
}

fn finish_effect(actor: &mut Object) -> Result<(), EffectError> {
    actor.base.flags.visible = false;
    actor.base.flags.remove_after_tick = true;
    Ok(())
}

#[cfg(test)]
#[path = "common_destruction_tests.rs"]
mod tests;
