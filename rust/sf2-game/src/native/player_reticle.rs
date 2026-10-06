//! Reticle mode preparation ($07:B038) and the complete position producer
//! ($07:A418..A504). Player logic owns the former; display logic owns the
//! latter. Neither boundary invents a cadence or refreshes the view matrix.

use super::intro_projection::{project_individual_point, ProjectionViewport};
use super::player_target_lock::ReticlePositionError;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::view_transition::FixedViewAngles;
use super::{Angle, ObjectId, ObjectStore};

const TRACK_AIM: u8 = 0x40;
const LINKED_ROLL: u8 = 0x80;
const SINGLE_VIEW: u8 = 0x04;
const INACTIVE_POINT: u8 = 100;
const MARKER_FORWARD_OFFSET: i8 = -30;

/// Player-owned marker state (6BA2..6BA4), cleared with player storage.
/// A hidden/ordinary visit preserves the roll retained by a linked visit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ReticleDisplay {
    pub roll: Angle,
    pub mode_flags: u8,
    pub display_flags: u8,
}

impl ReticleDisplay {
    pub const fn tracks_aim(self) -> bool {
        self.mode_flags & TRACK_AIM != 0
    }
}

/// Canonical display projection inputs. The matrix is retained from earlier
/// scene rendering, not derived from the camera angles published by this
/// service. The frame owner must supply both matrix and viewport explicitly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MarkerProjection {
    pub view_matrix: Option<[[i16; 3]; 3]>,
    pub viewport: Option<ProjectionViewport>,
    /// Full fine angles published by the selected view ($1595..159A).
    pub published_view_angles: Option<FixedViewAngles>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReticleError {
    World(WorldInputError),
    MissingDisplay(ObjectId),
    MissingInhibition,
    MissingScriptedViewMode,
    MissingServiceFlags,
    MissingEnable,
    MissingReflectionMode,
    MissingAimProxy,
    MissingRetainedAim(ObjectId),
    MissingView,
    MissingViewMatrix,
    MissingViewport,
    Position(ReticlePositionError),
}

impl From<WorldInputError> for ReticleError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}

fn display_mut<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut ReticleDisplay, ReticleError> {
    world
        .player_mut(objects, owner)?
        .reticle_display
        .as_mut()
        .ok_or(ReticleError::MissingDisplay(owner))
}

fn admitted(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<bool, ReticleError> {
    if world
        .reticle_inhibited
        .ok_or(ReticleError::MissingInhibition)?
    {
        return Ok(false);
    }
    if world
        .scripted_view_active()
        .ok_or(ReticleError::MissingScriptedViewMode)?
    {
        return Ok(false);
    }
    if !world
        .contacts_enabled()
        .ok_or(WorldInputError::MissingContactEnable)?
    {
        return Ok(false);
    }
    if world
        .player_service_flags
        .ok_or(ReticleError::MissingServiceFlags)?
        .minimum_protection()
    {
        return Ok(false);
    }
    if !world.reticle_enabled.ok_or(ReticleError::MissingEnable)? {
        return Ok(false);
    }
    Ok(!world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .hold_secondary_protection)
}

/// Full mode preparer, called on the actual visiting actor. Earlier writes
/// survive a missing input, and later gates are read only when reached.
pub fn prepare(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ReticleError> {
    display_mut(objects, world, owner)?.display_flags = 0;
    if !admitted(objects, world, owner)? {
        display_mut(objects, world, owner)?.mode_flags = 0;
    } else {
        let charge = world
            .player(objects, owner)?
            .charge
            .ok_or(WorldInputError::MissingPlayerCharge(owner))?;
        if charge.linked_muzzle_disabled || !charge.linked_mode {
            display_mut(objects, world, owner)?.mode_flags = TRACK_AIM;
        } else {
            let display = display_mut(objects, world, owner)?;
            display.mode_flags = TRACK_AIM | LINKED_ROLL;
            display.display_flags = 0;
            display.roll = objects
                .get(owner)
                .ok_or(WorldInputError::MissingActor(owner))?
                .base
                .roll;
        }
    }
    if !world
        .reflect_all_contacts
        .ok_or(ReticleError::MissingReflectionMode)?
    {
        display_mut(objects, world, owner)?.display_flags |= SINGLE_VIEW;
    }
    Ok(())
}

/// Complete position producer. The proxy copies the current player's coarse
/// rotation but the separately retained aim position. The fixed view's saved
/// render origin, not its ordinary position, defines the relative point.
/// After publishing fresh view angles, projection uses the retained matrix.
pub fn position(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), ReticleError> {
    if !world.reticle_enabled.ok_or(ReticleError::MissingEnable)?
        || !world
            .player(objects, owner)?
            .reticle_display
            .ok_or(ReticleError::MissingDisplay(owner))?
            .tracks_aim()
    {
        world.target_reticle.horizontal = Some(INACTIVE_POINT);
        world.target_reticle.vertical = Some(INACTIVE_POINT);
        return Ok(());
    }
    let proxy = world
        .weapons
        .as_ref()
        .and_then(|weapons| weapons.fallback)
        .ok_or(ReticleError::MissingAimProxy)?;
    let actor = objects
        .get(owner)
        .ok_or(WorldInputError::MissingActor(owner))?;
    let angles = [actor.base.pitch, actor.base.yaw, actor.base.roll];
    let proxy_actor = objects
        .get_mut(proxy)
        .ok_or(WorldInputError::MissingActor(proxy))?;
    proxy_actor.base.pitch = angles[0];
    proxy_actor.base.yaw = angles[1];
    proxy_actor.base.roll = angles[2];
    // Resolve the retained input after the preceding proxy rotation stores.
    let aim = world
        .player(objects, owner)?
        .rapid_aim
        .ok_or(ReticleError::MissingRetainedAim(owner))?
        .retained_aim;
    objects
        .get_mut(proxy)
        .expect("validated aim proxy")
        .base
        .position = aim;

    let view = world.fixed_players[0]
        .and_then(|view| objects.get(view))
        .ok_or(ReticleError::MissingView)?;
    let origin = view.extension.path_state.platform_carry.saved_position;
    world.marker_projection.published_view_angles = Some(FixedViewAngles::capture(view));
    let (vertical, forward) =
        sf_core::snes_trig::rotate_8yz(angles[0].units(), 0, MARKER_FORWARD_OFFSET);
    let (horizontal, forward) = sf_core::snes_trig::rotate_8xz(angles[1].units(), 0, forward as i8);
    let relative = [
        aim.x
            .wrapping_add(horizontal as i8 as i16)
            .wrapping_sub(origin.x),
        aim.y
            .wrapping_add(vertical as i8 as i16)
            .wrapping_sub(origin.y),
        aim.z
            .wrapping_add(forward as i8 as i16)
            .wrapping_sub(origin.z),
    ];
    let matrix = world
        .marker_projection
        .view_matrix
        .ok_or(ReticleError::MissingViewMatrix)?;
    let rotated =
        sf_core::snes_trig::matrix_rotate_q15(matrix, relative[0], relative[1], relative[2]);
    let viewport = world
        .marker_projection
        .viewport
        .ok_or(ReticleError::MissingViewport)?;
    let projected = project_individual_point([rotated.0, rotated.1, rotated.2], viewport);
    world
        .target_reticle
        .track_projected([projected.x, projected.y])
        .map_err(ReticleError::Position)
}

#[cfg(test)]
#[path = "player_reticle_tests.rs"]
mod tests;
