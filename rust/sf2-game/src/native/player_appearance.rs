//! Pilot materials and low-shield feedback ($06:AA1A), the surface depth
//! arbitration ($07:C32F..C354), and depth publication ($06:9F21..9F35).
//! All three services share the real player-owned appearance byte.

use super::path_relationships::{self, RelationshipError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{
    authored_paths, Behavior, MaterialSetId, Object, ObjectId, ObjectKind, ObjectStore, ShapeId,
    OBJECT_CAPACITY,
};

#[cfg(test)]
#[path = "player_appearance_tests.rs"]
mod tests;

const PILOT_MATERIAL_SIDE: u8 = 0x01;
const EVEN_PILOT_MATERIAL: MaterialSetId = MaterialSetId::from_catalog_token(33268);
const ODD_PILOT_MATERIAL: MaterialSetId = MaterialSetId::from_catalog_token(33534);
const LOW_SHIELD_LIMIT: u8 = 13;
const LOW_SHIELD_OVERRIDE: u8 = 0x80;
const FLASH_CLOCK_MASK: u16 = 0x05;
const PARTICLE_CLOCK_MASK: u16 = 0x01;
const FLASH_DEPTH: u8 = 3;
const CARRIED_DEPTH: u8 = 2;
const ACTIVE_CARRY_MODE: u8 = 1;
const DEPTH_MASK: u8 = 0x03;
const DAMAGE_PARTICLE_SHAPE: ShapeId = ShapeId::from_catalog_index(36);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerAppearance {
    /// Shared appearance/depth control (6AA2). A low-shield override is
    /// cleared as a whole byte on recovery, not merely by removing bit 80.
    pub depth_control: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppearanceError {
    World(WorldInputError),
    Relationships(RelationshipError),
    MissingAppearance(ObjectId),
    MissingVisit(ObjectId),
    MissingCarryMode,
    MissingSpawnDefaults,
    MissingParticleNumber,
    ObjectPoolExhausted,
}
impl From<WorldInputError> for AppearanceError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<RelationshipError> for AppearanceError {
    fn from(error: RelationshipError) -> Self {
        Self::Relationships(error)
    }
}

/// Complete damage-particle installer ($07:CFB1..D047). Its number is an
/// inherited caller input (direct-page $00, the frame loop's render-paced
/// countdown); it is neither a random sample nor a fixed slot. An absent
/// number faults only when an admitted allocation needs it. Allocation does
/// not execute the authored path or consume randomness.
pub fn emit_damage_particle(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
    child_number: Option<u8>,
) -> Result<Option<ObjectId>, AppearanceError> {
    let actor = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    if actor.base.shape == ShapeId::EMPTY || !actor.base.flags.visible || actor.base.hit_points == 0
    {
        return Ok(None);
    }
    let record = world.player(objects, owner)?;
    if record
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .ignores_contacts
    {
        return Ok(None);
    }
    let charge = record
        .charge
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
    if (charge.linked_mode && !charge.linked_muzzle_disabled)
        || world.strategy_clock & PARTICLE_CLOCK_MASK != 0
    {
        return Ok(None);
    }
    if objects.len() == OBJECT_CAPACITY {
        return Err(AppearanceError::ObjectPoolExhausted);
    }
    let child_number = child_number.ok_or(AppearanceError::MissingParticleNumber)?;
    let defaults = world
        .spawn_defaults()
        .ok_or(AppearanceError::MissingSpawnDefaults)?;
    let fresh = Object::new_authored(
        ObjectKind::Effect,
        DAMAGE_PARTICLE_SHAPE,
        Behavior::FollowPath,
        defaults,
    );
    let head = objects.active_ids().first().copied();
    let child = objects
        .allocate_after(head, fresh)
        .ok_or(AppearanceError::ObjectPoolExhausted)?;
    path_relationships::attach_fresh_child(objects, owner, child, child_number)?;
    super::player_effect::format(objects, owner, child)?;
    objects
        .get_mut(child)
        .expect("fresh damage particle")
        .base
        .path = Some(authored_paths::LOCAL_JITTER_SPRITE);
    Ok(Some(child))
}

/// Update the pilot material before reading shield state. On low shield the
/// particle installer precedes the appearance write, including fatal errors.
/// Healthy visits preserve non-override values, including unrelated bits.
pub fn update(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    child_number: Option<u8>,
) -> Result<Option<ObjectId>, AppearanceError> {
    let pilot = world
        .player(objects, owner)?
        .visit
        .ok_or(AppearanceError::MissingVisit(owner))?
        .pilot_code;
    objects
        .get_mut(owner)
        .expect("validated player")
        .extension
        .material_set = Some(if pilot & PILOT_MATERIAL_SIDE == 0 {
        EVEN_PILOT_MATERIAL
    } else {
        ODD_PILOT_MATERIAL
    });
    let shield = world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .reserve_shield;
    let child = if shield < LOW_SHIELD_LIMIT {
        emit_damage_particle(objects, world, owner, child_number)?
    } else {
        None
    };
    let clock = world.strategy_clock;
    let appearance = world
        .player_mut(objects, owner)?
        .appearance
        .as_mut()
        .ok_or(AppearanceError::MissingAppearance(owner))?;
    if shield < LOW_SHIELD_LIMIT {
        appearance.depth_control = LOW_SHIELD_OVERRIDE
            | if clock & FLASH_CLOCK_MASK == 0 {
                FLASH_DEPTH
            } else {
                0
            };
    } else if appearance.depth_control & LOW_SHIELD_OVERRIDE != 0 {
        appearance.depth_control = 0;
    }
    Ok(child)
}

/// Surface carry depth cannot replace the active low-shield override. An
/// override also bypasses the carry-mode read, even if that input is absent.
pub fn update_surface_depth(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), AppearanceError> {
    let current = world
        .player(objects, owner)?
        .appearance
        .ok_or(AppearanceError::MissingAppearance(owner))?;
    if current.depth_control & LOW_SHIELD_OVERRIDE != 0 {
        return Ok(());
    }
    let carried = world
        .player_carry_mode
        .ok_or(AppearanceError::MissingCarryMode)?
        == ACTIVE_CARRY_MODE
        && objects
            .get(owner)
            .expect("validated player")
            .extension
            .path_state
            .motion
            .carry_selected_player;
    world
        .player_mut(objects, owner)?
        .appearance
        .as_mut()
        .expect("validated appearance")
        .depth_control = if carried { CARRIED_DEPTH } else { 0 };
    Ok(())
}

/// The original four-word table maps selectors 0..3 to depth offsets 0..3.
/// Replace the complete draw depth word, not just its low byte.
pub fn publish_depth(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<(), AppearanceError> {
    let control = world
        .player(objects, owner)?
        .appearance
        .ok_or(AppearanceError::MissingAppearance(owner))?
        .depth_control;
    objects
        .get_mut(owner)
        .expect("validated player")
        .extension
        .depth_offset = u16::from(control & DEPTH_MASK);
    Ok(())
}
