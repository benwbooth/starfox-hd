//! Ordered post-movement effects ($06:9EE8..9F53) and the complete linked
//! protection installer ($07:CD70). The composed publication service also
//! includes attachments through $06:9FAC; mission transitions remain separate.

use super::path_relationships::{self, RelationshipError};
use super::player_appearance::{self, AppearanceError};
use super::player_attachments::{self, PlayerAttachmentError};
use super::player_damage_effects::{self, DamageEffectsError};
use super::player_recovery::{self, RecoveryError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{
    authored_paths, Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId, OBJECT_CAPACITY,
};

#[cfg(test)]
#[path = "player_frame_effects_tests.rs"]
mod tests;

const PROTECTION_NUMBER: u8 = 18;
const PROTECTION_SHAPE: ShapeId = ShapeId::from_catalog_index(84);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameEffectsError {
    World(WorldInputError),
    Relationships(RelationshipError),
    Appearance(AppearanceError),
    Damage(DamageEffectsError),
    Recovery(RecoveryError),
    Attachments(PlayerAttachmentError),
    MissingSpawnDefaults,
    ObjectPoolExhausted,
}
impl From<WorldInputError> for FrameEffectsError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<RelationshipError> for FrameEffectsError {
    fn from(error: RelationshipError) -> Self {
        Self::Relationships(error)
    }
}

/// Reuse an existing direct numbered child without changing it, even when
/// removal is pending. Only a newly allocated child produces Some. There is
/// no health gate, random draw, or shared linked-activity publication here.
pub fn install_protection(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<Option<ObjectId>, FrameEffectsError> {
    if !world
        .contacts_enabled()
        .ok_or(WorldInputError::MissingContactEnable)?
    {
        return Ok(None);
    }
    if world
        .player(objects, owner)?
        .protection
        .ok_or(WorldInputError::MissingPlayerProtection(owner))?
        .remaining()
        == 0
    {
        return Ok(None);
    }
    if path_relationships::find_direct_child(objects, owner, PROTECTION_NUMBER)?.is_some() {
        return Ok(None);
    }
    if objects.len() == OBJECT_CAPACITY {
        return Err(FrameEffectsError::ObjectPoolExhausted);
    }
    let defaults = world
        .spawn_defaults()
        .ok_or(FrameEffectsError::MissingSpawnDefaults)?;
    let fresh = Object::new_authored(
        ObjectKind::Effect,
        PROTECTION_SHAPE,
        Behavior::FollowPath,
        defaults,
    );
    let head = objects.active_ids().first().copied();
    let child = objects
        .allocate_after(head, fresh)
        .ok_or(FrameEffectsError::ObjectPoolExhausted)?;
    path_relationships::attach_fresh_child(objects, owner, child, PROTECTION_NUMBER)?;
    super::player_effect::format(objects, owner, child)?;
    objects
        .get_mut(child)
        .expect("fresh protection effect")
        .base
        .path = Some(authored_paths::LINKED_PROTECTION_EFFECT);
    Ok(Some(child))
}

/// Appearance samples shield before damage and recovery. Death from damage
/// does not skip protection, depth or recovery. An action gate skips both
/// protection installation AND countdown; a contact gate skips only birth.
/// Partial publications remain visible if a later service cannot complete.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    damage_particle_number: Option<u8>,
) -> Result<(), FrameEffectsError> {
    player_appearance::update(objects, world, owner, damage_particle_number)
        .map_err(FrameEffectsError::Appearance)?;
    player_damage_effects::advance(objects, world, owner).map_err(FrameEffectsError::Damage)?;
    if world
        .action_gate
        .ok_or(WorldInputError::MissingActionGate)?
        .code
        == 0
    {
        install_protection(objects, world, owner)?;
        let clock = world.strategy_clock as u8;
        world
            .player_mut(objects, owner)?
            .protection
            .as_mut()
            .ok_or(WorldInputError::MissingPlayerProtection(owner))?
            .advance_countdown(clock);
    }
    player_appearance::publish_depth(objects, world, owner)
        .map_err(FrameEffectsError::Appearance)?;
    player_recovery::consume(objects, world, owner).map_err(FrameEffectsError::Recovery)?;
    Ok(())
}

/// Continuous player tail ($06:9EE8..9FAC): effects and recovery finish
/// before linked objects sample the final ordinary or temporary Walker pose.
pub fn advance_with_attachments(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    damage_particle_number: Option<u8>,
) -> Result<(), FrameEffectsError> {
    advance(objects, world, owner, damage_particle_number)?;
    player_attachments::publish(objects, world, owner).map_err(FrameEffectsError::Attachments)
}
