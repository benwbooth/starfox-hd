//! Player attachment publication ($06:9F54..9FAC). Walker children use a
//! temporary parent height and yaw; the player's own pose is then restored.

use super::attachments::{self, AttachmentError};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore};

#[cfg(test)]
#[path = "player_attachments_tests.rs"]
mod tests;

const FAMILY_MASK: u8 = 0xF0;
const WALKER_FAMILY: u8 = 0x20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerAttachmentError {
    World(WorldInputError),
    Attachment(AttachmentError),
    MissingGround(ObjectId),
    MissingMotion(ObjectId),
}
impl From<WorldInputError> for PlayerAttachmentError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<AttachmentError> for PlayerAttachmentError {
    fn from(error: AttachmentError) -> Self {
        Self::Attachment(error)
    }
}

/// Ignoring contacts bypasses only the Walker pose override, never chain
/// publication itself. The ordinary branch does not need motion or ground
/// records. Failed publication retains the exact completed prefix.
pub fn publish(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<(), PlayerAttachmentError> {
    let record = world.player(objects, owner)?;
    let ignore_contacts = record
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .ignores_contacts;
    let walker = !ignore_contacts
        && record
            .auxiliary
            .ok_or(WorldInputError::MissingAuxiliary(owner))?
            .mode
            & FAMILY_MASK
            == WALKER_FAMILY;
    if !walker {
        attachments::refresh_child_chain(objects, owner)?;
        return Ok(());
    }
    let actor = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    let (height, yaw) = (actor.base.position.y, actor.base.yaw);
    let target_height = record
        .camera_ground
        .ok_or(PlayerAttachmentError::MissingGround(owner))?
        .carried_target_height;
    objects
        .get_mut(owner)
        .expect("validated player attachment owner")
        .base
        .position
        .y = target_height;
    let yaw_offset = world
        .player(objects, owner)?
        .motion
        .ok_or(PlayerAttachmentError::MissingMotion(owner))?
        .walker_attachment_yaw;
    objects
        .get_mut(owner)
        .expect("validated player attachment owner")
        .base
        .yaw = yaw.wrapping_add(yaw_offset as i8);
    attachments::refresh_child_chain(objects, owner)?;
    let actor = objects
        .get_mut(owner)
        .expect("validated player attachment owner");
    actor.base.position.y = height;
    actor.base.yaw = yaw;
    Ok(())
}
