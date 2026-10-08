//! Shared player-entry reset ($06:958C..9691), for the native services that
//! already have scene-owned state. This is separate from storage replacement:
//! it must not clear either player's records or refresh their copied shield.
//!
//! The enclosing initializer still owns its strategy and death-callback
//! installation. The source also resets movement-mode, script-selector,
//! terrain-render and continuous-audio state whose producers/consumers have
//! not yet been bound to this scene host. This service does not substitute
//! zero-filled opaque storage for those remaining ports.

use super::hit_response::HitSide;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Angle, InputState, ObjectId, ObjectStore};

const INACTIVE_RETICLE_COORDINATE: u8 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSceneResetError {
    World(WorldInputError),
    MissingController(HitSide),
    MissingShieldCapacity,
    MissingPublishedShield,
    MissingWeaponState,
    MissingHandoff,
}

/// Reset the existing shared service publications in source order. Fields
/// outside this reset keep their prior values, including the other reticle
/// axis, homing target, configuration companion, camera-handoff coordinates,
/// input options, view mode and pending sound events. Missing retained owners
/// diagnose before mutating them, with completed earlier writes preserved.
pub fn reset_services(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), PlayerSceneResetError> {
    let side = objects
        .get(owner)
        .ok_or(PlayerSceneResetError::World(WorldInputError::MissingActor(
            owner,
        )))?
        .base
        .contacts
        .hit_side;
    let controller = match side {
        HitSide::Primary => 0,
        HitSide::Secondary => 1,
    };
    // The source selects the visiting actor's sampled controller before
    // clearing both processed words; it does not remap or resample input.
    world.processed_player_input = Some(
        world.controller_inputs[controller]
            .ok_or(PlayerSceneResetError::MissingController(side))?,
    );
    world.processed_player_input = Some(InputState::default());
    world.scene.player_configuration = Some(0);
    world.interception_music_ready = Some(false);
    world.linked_effect_activity = Some(Default::default());

    let capacity = world
        .active_shield_capacity
        .ok_or(PlayerSceneResetError::MissingShieldCapacity)?;
    let published = world
        .scene
        .active_shield
        .as_mut()
        .ok_or(PlayerSceneResetError::MissingPublishedShield)?;
    // CMP/BPL uses the sign of a wrapping byte subtraction, not unsigned
    // minimum. Storage was copied before this clamp by the enclosing entry.
    if (capacity.wrapping_sub(*published) as i8) < 0 {
        *published = capacity;
    }
    world
        .weapons
        .as_mut()
        .ok_or(PlayerSceneResetError::MissingWeaponState)?
        .published_pitch = Some(Angle::ZERO);
    world.node_exit.presentation_flags = Some(0);
    world.surface_mode = Some(Default::default());
    world.reticle_enabled = Some(false);
    world.action_gate = Some(Default::default());
    // The reset changes only this byte, not the retained x/z/heading.
    world
        .handoff
        .as_mut()
        .ok_or(PlayerSceneResetError::MissingHandoff)?
        .player_flags = 0;
    world.node_exit.completion_code = Some(0);
    world.shield_recovery = Some(Default::default());
    world.player_service_flags = Some(Default::default());
    // Original code stores horizontal twice and does not touch vertical.
    world.target_reticle.horizontal = Some(INACTIVE_RETICLE_COORDINATE);
    world.camera_tracking = Some(Default::default());
    world.camera_focus = Some(Default::default());
    world.published_motion = Some(Default::default());
    // `$06:966F..9675`: the background base (1E4E) belongs to the path
    // runtime and is reset by `reset_background_base`, in the same order.
    world.camera_projection_offset = Some(0);
    world.published_camera_projection = Some(0);
    world.environment_plane_height = Some(0);
    world.player_surface_support = Some(Default::default());
    Ok(())
}

/// The shared reset's background-base store (`$06:966F`, 1E4E), owned by
/// the path runtime. Callers apply it immediately after `reset_services`;
/// nothing in between reads it.
pub fn reset_background_base(runtime: &mut super::path_runtime::PathRuntime) {
    runtime.background_horizontal = Some(0);
}

#[cfg(test)]
#[path = "player_scene_reset_tests.rs"]
mod tests;
