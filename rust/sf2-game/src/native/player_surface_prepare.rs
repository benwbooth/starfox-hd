//! Plane/material preparation for the free-flight mode ($07:E5C8..E684).
//! This observes the preceding contact; it does not run a second collision
//! query. A rejected plane clears only the player's plane, preserving the
//! scene's last published clipping height and the caller's effect offset.

use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore};

#[cfg(test)]
#[path = "player_surface_prepare_tests.rs"]
mod tests;

const ORDINARY_MATERIAL: u8 = 0;
const RAISED_PLANE_MATERIALS: [u8; 2] = [1, 4];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfacePreparationError {
    World(WorldInputError),
    MissingMotion(ObjectId),
    MissingSurface(ObjectId),
    MissingEnvironmentPlane,
    MissingEnvironmentMaterial,
}

impl From<WorldInputError> for SurfacePreparationError {
    fn from(value: WorldInputError) -> Self {
        Self::World(value)
    }
}

/// Some(height) identifies the actual plane publication and the corresponding
/// inherited vertical effect offset. None is the source's carry-clear return,
/// not permission to fabricate a neutral caller offset.
pub fn prepare(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<Option<i16>, SurfacePreparationError> {
    let contact = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .extension
        .surface_contact;
    let material;
    let plane;
    if let Some(support) = contact
        .supporting_object
        .filter(|_| contact.flags != ORDINARY_MATERIAL)
    {
        material = contact.flags;
        world
            .player_mut(objects, owner)?
            .surface
            .as_mut()
            .ok_or(SurfacePreparationError::MissingSurface(owner))?
            .material = material;
        plane = if RAISED_PLANE_MATERIALS.contains(&material) {
            let support = objects
                .get(support)
                .ok_or(WorldInputError::MissingActor(support))?;
            Some(
                support
                    .base
                    .position
                    .y
                    .wrapping_add(support.extension.path_state.script_value as i16),
            )
        } else {
            None
        };
    } else {
        let use_contact = if contact.supporting_object.is_some() {
            let height = world
                .player(objects, owner)?
                .motion
                .ok_or(SurfacePreparationError::MissingMotion(owner))?
                .surface_height;
            height.wrapping_sub(
                world
                    .environment_plane_height
                    .ok_or(SurfacePreparationError::MissingEnvironmentPlane)?,
            ) < 0
        } else {
            false
        };
        material = if use_contact {
            ORDINARY_MATERIAL
        } else {
            world
                .player_carry_mode
                .ok_or(SurfacePreparationError::MissingEnvironmentMaterial)?
        };
        world
            .player_mut(objects, owner)?
            .surface
            .as_mut()
            .ok_or(SurfacePreparationError::MissingSurface(owner))?
            .material = material;
        plane = if use_contact {
            None
        } else {
            let height = world
                .environment_plane_height
                .ok_or(SurfacePreparationError::MissingEnvironmentPlane)?;
            (height != 0).then_some(height)
        };
    }
    if let Some(height) = plane {
        world.surface_clipping_plane_height = Some(height);
    }
    world
        .player_mut(objects, owner)?
        .surface
        .as_mut()
        .ok_or(SurfacePreparationError::MissingSurface(owner))?
        .plane_height = plane.unwrap_or(0);
    Ok(plane)
}
