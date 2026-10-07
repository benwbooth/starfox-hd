//! Consumable dispatch (`$07:DC8B..DD6E`) and the concrete recovery/triggered
//! effect installers (`$07:D11D..D1A7`). The outer button/delay and temporary
//! placement helper is a separate caller, not implicitly performed here.

use super::path_equipment::SelectedEquipment;
use super::path_protection::DeflectionProtection;
use super::path_relationships::{self, RelationshipError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{
    authored_paths, Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId, OBJECT_CAPACITY,
};

#[cfg(test)]
#[path = "player_consumable_tests.rs"]
mod tests;

const COUNT_MASK: u8 = 0x0F;
const KIND_MASK: u8 = 0x7F;
const TRIGGERED_USE_BLOCKED: u8 = 0x18;
const PROTECTION_ACTIVE_MASK: u8 = 0x5F;
const PROTECTION_COUNT_MASK: u8 = 0x1F;
const RECOVERY_EFFECT_NUMBER: u8 = 22;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerConsumableControl {
    /// Packed input delay (6B61); its outer service is independently scheduled.
    pub input_control: u8,
    /// Only the two triggered-use blockers in 6BE9. Other bits of that byte
    /// are not owned by the consumable service; target control uses 6A8C.
    pub projectile_blockers: TriggeredUseBlockers,
    /// 6B7D bit 40. Its protection-hold bit 80 has a separate typed owner.
    pub recovery_blocked: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TriggeredUseBlockers(u8);

impl TriggeredUseBlockers {
    pub const fn from_control(control: u8) -> Self {
        Self(control & TRIGGERED_USE_BLOCKED)
    }
    pub const fn bits(self) -> u8 {
        self.0
    }
    pub const fn blocked(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConsumableEffect {
    Recovery,
    TriggeredProjectile,
    Deflection,
    /// Source handlers for other types go directly to consumption. This is
    /// the original complete branch, not a fallback for an unported service.
    ConsumeOnly,
}

impl ConsumableEffect {
    fn decode(kind: u8) -> Self {
        // Source byte-width table indexing discards the type's top bit.
        match kind & KIND_MASK {
            0 => Self::Recovery,
            1 => Self::TriggeredProjectile,
            3 => Self::Deflection,
            _ => Self::ConsumeOnly,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumableError {
    World(WorldInputError),
    Relationships(RelationshipError),
    MissingSpawnDefaults,
    MissingControl(ObjectId),
    MissingPlayerAction(ObjectId),
    MissingShieldCapacity,
    MissingProjectileTrigger,
    ObjectPoolExhausted,
}

impl From<WorldInputError> for ConsumableError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<RelationshipError> for ConsumableError {
    fn from(error: RelationshipError) -> Self {
        Self::Relationships(error)
    }
}

fn equipment<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut SelectedEquipment, ConsumableError> {
    world
        .player_mut(objects, owner)?
        .equipment
        .as_mut()
        .ok_or(ConsumableError::World(WorldInputError::MissingEquipment(
            owner,
        )))
}

fn control(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<PlayerConsumableControl, ConsumableError> {
    world
        .player(objects, owner)?
        .consumable
        .ok_or(ConsumableError::MissingControl(owner))
}

fn allocate_effect(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    kind: ObjectKind,
) -> Result<ObjectId, ConsumableError> {
    if objects.len() == OBJECT_CAPACITY {
        return Err(ConsumableError::ObjectPoolExhausted);
    }
    let defaults = world
        .spawn_defaults()
        .ok_or(ConsumableError::MissingSpawnDefaults)?;
    let effect = Object::new_authored(kind, ShapeId::EMPTY, Behavior::FollowPath, defaults);
    let head = objects.active_ids().first().copied();
    objects
        .allocate_after(head, effect)
        .ok_or(ConsumableError::ObjectPoolExhausted)
}

fn install_triggered(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ConsumableError> {
    let effect = allocate_effect(objects, world, ObjectKind::Projectile)?;
    objects
        .get_mut(effect)
        .expect("allocated projectile")
        .base
        .path = Some(authored_paths::TRIGGERED_LINKED_PROJECTILE);
    super::player_effect::format(objects, owner, effect)?;
    objects
        .get_mut(effect)
        .expect("allocated projectile")
        .base
        .contacts
        .run_when_paused = false;
    Ok(())
}

pub fn install_recovery(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<bool, ConsumableError> {
    if path_relationships::find_direct_child(objects, owner, RECOVERY_EFFECT_NUMBER)?.is_some() {
        return Ok(false);
    }
    let effect = allocate_effect(objects, world, ObjectKind::Effect)?;
    path_relationships::attach_fresh_child(objects, owner, effect, RECOVERY_EFFECT_NUMBER)?;
    objects
        .get_mut(effect)
        .expect("allocated recovery effect")
        .base
        .path = Some(authored_paths::ATTACHED_RECOVERY_EFFECT);
    super::player_effect::format(objects, owner, effect)?;
    // The installer selects its own relative frame without clearing any
    // relative transform; attachment to the player is a distinct relationship.
    objects
        .get_mut(effect)
        .expect("allocated recovery effect")
        .extension
        .parent = Some(effect);
    Ok(true)
}

/// Return whether the item was consumed. Ordinary admission rejection is not
/// a fault, but pool exhaustion enters the source's non-returning diagnostic.
/// Earlier writes remain committed; the scene wrapper prevents replay.
pub fn use_item(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<bool, ConsumableError> {
    if world
        .scripted_view_active()
        .ok_or(ConsumableError::MissingSpawnDefaults)?
    {
        return Ok(false);
    }
    let selected = *equipment(objects, world, owner)?;
    if selected.packed_consumables & COUNT_MASK == 0 {
        return Ok(false);
    }
    match ConsumableEffect::decode(selected.consumable_type) {
        ConsumableEffect::ConsumeOnly => {}
        ConsumableEffect::TriggeredProjectile => {
            let action = world
                .player_mut(objects, owner)?
                .action
                .as_mut()
                .ok_or(ConsumableError::MissingPlayerAction(owner))?;
            if action.action.is_some() {
                return Ok(false);
            }
            if control(objects, world, owner)?
                .projectile_blockers
                .blocked()
            {
                return Ok(false);
            }
            world
                .player_mut(objects, owner)?
                .action
                .as_mut()
                .expect("validated action")
                .install_triggered_projectile();
            install_triggered(objects, world, owner)?;
            world
                .projectile_trigger
                .as_mut()
                .ok_or(ConsumableError::MissingProjectileTrigger)?
                .activation = 0;
        }
        ConsumableEffect::Deflection => {
            let protection = world
                .player_mut(objects, owner)?
                .protection
                .as_mut()
                .ok_or(WorldInputError::MissingPlayerProtection(owner))?;
            if protection.control() & PROTECTION_ACTIVE_MASK != 0 {
                return Ok(false);
            }
            *protection = DeflectionProtection::from_control(
                (protection.control() & !PROTECTION_COUNT_MASK) | PROTECTION_COUNT_MASK,
            );
        }
        ConsumableEffect::Recovery => {
            if control(objects, world, owner)?.recovery_blocked {
                return Ok(false);
            }
            let shield = world
                .player(objects, owner)?
                .contact
                .as_ref()
                .ok_or(WorldInputError::MissingPlayerContact(owner))?
                .hit
                .reserve_shield;
            if shield
                == world
                    .active_shield_capacity
                    .ok_or(ConsumableError::MissingShieldCapacity)?
            {
                return Ok(false);
            }
            if !install_recovery(objects, world, owner)? {
                return Ok(false);
            }
        }
    }
    let selected = equipment(objects, world, owner)?;
    selected.packed_consumables = selected.packed_consumables.wrapping_sub(1);
    Ok(true)
}
