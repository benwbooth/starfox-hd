//! Player charged-fire control (`$07:DAB2..DC7E`) and its numbered effect
//! installer (`$07:CDC5..CE30`). Charge is an unsigned fine value, not an
//! elapsed held-button counter. The enclosing player strategy owns cadence,
//! input remapping, aiming publication and the rapid-fire service.

use super::path_charge::SelectedChargeInput;
use super::path_control::PlayerTarget;
use super::path_relationships::{self, RelationshipError};
use super::path_sound::AuthoredCue;
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::weapon_dispatch::{self, LaunchError, LaunchRequest, LaunchWorld, PathWeapon};
use super::{
    authored_paths, Behavior, Button, InputState, Object, ObjectId, ObjectKind, ObjectStore,
    ShapeId, SoundEvent, OBJECT_CAPACITY,
};

#[cfg(test)]
#[path = "player_charge_tests.rs"]
mod tests;

const RELEASED_SHOT: u8 = 0x10;
const CHARGE_SOUND_ACTIVE: u8 = 0x20;
const DECAY: u8 = 0x40;
const CHARGING: u8 = 0x80;
const EFFECT_NUMBER: u8 = 39;
const EFFECT_THRESHOLD: u8 = 8;
// All six valid pilot rows of $07:DC7F contain this same fine increment.
const CHARGE_INCREMENT: u16 = 384;
const CHARGE_START_CUE: u8 = 49;
const CHARGE_READY_CUE: u8 = 53;
const CHARGE_STOP_CUE: u8 = 244;
const SHOT_DELAY: u8 = 5;
const SHOT_SPEED_IMPULSE: i16 = 30;
const FLIGHT_EFFECT_DISTANCE: u16 = 70;
const OTHER_EFFECT_DISTANCE: u16 = 20;
const EFFECT_HEALTH: u8 = 1;
const EFFECT_ATTACK: u8 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCharge {
    /// Fine charge 6C07/08; the high byte is the path callback's full level.
    pub progress: u16,
    /// Charge service flags 6C09, including all unassigned bits.
    pub control: u8,
    /// Auxiliary 6B63 bit 80, shared with the charge-orb observation.
    pub linked_mode: bool,
    /// Rapid-shot queue and delay (6B60), also written on charged release.
    pub rapid_control: u8,
    /// Speed impulse and lifetime (6B56/58), consumed by flight dynamics.
    pub speed_impulse: i16,
    pub speed_impulse_ticks: u8,
}

impl PlayerCharge {
    pub const fn level(self) -> u8 {
        (self.progress >> 8) as u8
    }
    pub const fn path_input(self) -> SelectedChargeInput {
        SelectedChargeInput {
            linked_mode: self.linked_mode,
            level: self.level(),
        }
    }
    fn request_decay(&mut self) {
        self.control = (self.control | DECAY) & !CHARGING;
    }
    fn decay(&mut self) {
        // The source first clears the fraction, then halves the signed high
        // byte toward zero, rather than shifting the complete fine value.
        let level = ((self.level() as i8) / 2) as u8;
        self.progress = u16::from(level) << 8;
        if level == 0 {
            self.control &= !DECAY;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeError {
    World(WorldInputError),
    MissingThreshold,
    MissingWeapons,
    MissingSpawnDefaults,
    MissingActor(ObjectId),
    Relationships(RelationshipError),
    Launch(LaunchError),
}

fn state<'a>(
    world: &'a mut ScenePathWorld,
    objects: &ObjectStore,
    owner: ObjectId,
) -> Result<&'a mut PlayerCharge, ChargeError> {
    world
        .player_mut(objects, owner)
        .map_err(ChargeError::World)?
        .charge
        .as_mut()
        .ok_or(ChargeError::World(WorldInputError::MissingPlayerCharge(
            owner,
        )))
}

fn mode(world: &ScenePathWorld, objects: &ObjectStore, owner: ObjectId) -> Result<u8, ChargeError> {
    Ok(world
        .player(objects, owner)
        .map_err(ChargeError::World)?
        .auxiliary
        .ok_or(ChargeError::World(WorldInputError::MissingAuxiliary(owner)))?
        .mode)
}

fn threshold(world: &ScenePathWorld) -> Result<u8, ChargeError> {
    world
        .active_charge_threshold
        .ok_or(ChargeError::MissingThreshold)
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

fn ensure_effect(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ChargeError> {
    if path_relationships::find_direct_child(objects, owner, EFFECT_NUMBER)
        .map_err(ChargeError::Relationships)?
        .is_some()
    {
        return Ok(());
    }
    if objects.len() == OBJECT_CAPACITY {
        return Ok(());
    }
    let defaults = world
        .spawn_defaults
        .ok_or(ChargeError::MissingSpawnDefaults)?;
    let fresh = Object::new_authored(
        ObjectKind::Effect,
        ShapeId::EMPTY,
        Behavior::FollowPath,
        defaults,
    );
    let head = objects.active_ids().first().copied();
    let Some(effect) = objects.allocate_after(head, fresh) else {
        return Ok(());
    };
    path_relationships::attach_fresh_child(objects, owner, effect, EFFECT_NUMBER)
        .map_err(ChargeError::Relationships)?;
    let distance = if mode(world, objects, owner)? & 0xF0 == 0x10 {
        FLIGHT_EFFECT_DISTANCE
    } else {
        OTHER_EFFECT_DISTANCE
    };
    let player = objects.get(owner).ok_or(ChargeError::MissingActor(owner))?;
    let (position, pitch, yaw, roll) = (
        player.base.position,
        player.base.pitch,
        player.base.yaw,
        player.base.roll,
    );
    let effect = objects.get_mut(effect).expect("allocated charge effect");
    effect.base.path = Some(authored_paths::PLAYER_CHARGE_ORB);
    effect.extension.path_state.needs_path_initialization = true;
    effect.extension.path_state.script_value = distance;
    effect.extension.spawn_group = u8::MAX;
    effect.base.hit_points = EFFECT_HEALTH;
    effect.base.attack_power = EFFECT_ATTACK;
    effect.base.flags.general_search_eligible = true;
    effect.base.position = position;
    effect.base.pitch = pitch;
    effect.base.yaw = yaw;
    effect.base.roll = roll;
    effect.base.contacts.run_when_paused = true;
    // Fresh class flags already exclude the path-spawn membership cleared by
    // the source's common player-effect formatter ($07:BF30).
    effect.base.flags.collision_disabled = true;
    Ok(())
}

fn launch(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<(), ChargeError> {
    let weapons = world.weapons.as_mut().ok_or(ChargeError::MissingWeapons)?;
    weapons.parameters = Default::default();
    if objects.len() == OBJECT_CAPACITY {
        return Ok(());
    }
    let defaults = world
        .spawn_defaults
        .ok_or(ChargeError::MissingSpawnDefaults)?;
    weapon_dispatch::launch(
        objects,
        resources,
        owner,
        LaunchRequest {
            weapon: PathWeapon::PlayerChargedMesh,
            parameters: weapons.parameters,
            defaults,
        },
        &mut LaunchWorld {
            caller_inputs: None,
            fallback: weapons.fallback,
            published_pitch: weapons.published_pitch,
            primary: world.primary_player,
            secondary: world.secondary_player,
            primary_auxiliary_mode: None,
            hostile_counts: Some(&mut weapons.hostile_counts),
            random: &mut world.random,
        },
    )
    .map_err(ChargeError::Launch)?;
    Ok(())
}

/// One admitted player-service visit. Input is the processed player input,
/// not necessarily the raw controller. Errors preserve already-executed
/// source side effects; the scene owner must not retry the failed visit.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    input: InputState,
) -> Result<(), ChargeError> {
    let ignored = world
        .player(objects, owner)
        .map_err(ChargeError::World)?
        .contact
        .as_ref()
        .ok_or(ChargeError::World(WorldInputError::MissingPlayerContact(
            owner,
        )))?
        .ignores_contacts;
    if ignored {
        state(world, objects, owner)?.request_decay();
    }
    let blocked = world
        .action_gate
        .ok_or(ChargeError::World(WorldInputError::MissingActionGate))?
        .code
        != 0;
    if blocked {
        state(world, objects, owner)?.request_decay();
    } else {
        let held = input.held.contains(Button::B);
        let pressed = input.pressed.contains(Button::B);
        if held && !pressed {
            state(world, objects, owner)?.control |= CHARGING;
        } else {
            state(world, objects, owner)?.control &= !RELEASED_SHOT;
        }
        if state(world, objects, owner)?.control & RELEASED_SHOT == 0 && (!held || pressed) {
            if state(world, objects, owner)?.level() == threshold(world)? {
                launch(objects, world, resources, owner)?;
                let charge = state(world, objects, owner)?;
                charge.control |= RELEASED_SHOT;
                charge.rapid_control = SHOT_DELAY;
                if mode(world, objects, owner)? & 0xF0 != 0x20 {
                    let charge = state(world, objects, owner)?;
                    charge.speed_impulse_ticks = SHOT_DELAY;
                    charge.speed_impulse = SHOT_SPEED_IMPULSE;
                }
            }
            state(world, objects, owner)?.request_decay();
        }
    }
    if state(world, objects, owner)?.control & DECAY != 0 {
        if state(world, objects, owner)?.control & CHARGE_SOUND_ACTIVE != 0 {
            state(world, objects, owner)?.control &= !CHARGE_SOUND_ACTIVE;
            cue(world, owner, CHARGE_STOP_CUE);
        }
        if let Some(effect) = path_relationships::find_direct_child(objects, owner, EFFECT_NUMBER)
            .map_err(ChargeError::Relationships)?
        {
            objects
                .get_mut(effect)
                .expect("live numbered effect")
                .base
                .flags
                .remove_after_tick = true;
        }
        state(world, objects, owner)?.decay();
        if state(world, objects, owner)?.level() == 0 {
            return Ok(());
        }
    }
    if state(world, objects, owner)?.control & (DECAY | CHARGING) != CHARGING {
        return Ok(());
    }
    if state(world, objects, owner)?.level() >= EFFECT_THRESHOLD {
        ensure_effect(objects, world, owner)?;
        if state(world, objects, owner)?.control & CHARGE_SOUND_ACTIVE == 0 {
            state(world, objects, owner)?.control |= CHARGE_SOUND_ACTIVE;
            cue(world, owner, CHARGE_START_CUE);
        }
    }
    let limit = threshold(world)?;
    if state(world, objects, owner)?.level() != limit {
        let progress = state(world, objects, owner)?
            .progress
            .wrapping_add(CHARGE_INCREMENT);
        let maximum = u16::from(limit) << 8;
        if progress >= maximum {
            cue(world, owner, CHARGE_READY_CUE);
        }
        state(world, objects, owner)?.progress = progress.min(maximum);
    }
    Ok(())
}
