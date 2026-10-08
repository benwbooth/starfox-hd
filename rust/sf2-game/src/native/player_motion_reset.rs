//! Partial player-record reset (`$06:DA38..DA53`) and the complete
//! scene-entry movement preparation (`$06:DE7F..DEED`).
//! The source clears the motion/camera prefix, not the complete player
//! allocation. Equipment, score, protection, scripted action, configured
//! limits and retained return position survive. No allocation is replaced.

use super::path_program::SelectedAuxiliaryState;
use super::player_storage::PlayerStorage;
use super::scene_path_world::PlayerPathRecords;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MotionResetError {
    World(super::scene_path_world::WorldInputError),
    Storage(super::player_storage::PlayerStorageError),
    MissingRetainedRecord(MissingRetainedRecord),
    MissingAmbientControl,
    MissingPublishedWeaponLevel,
    MissingEquipment(super::ObjectId),
    MissingPublishedConsumables,
    MissingSurfaceMode,
}

/// Reset ambient motion, steering and the player prefix, restore published
/// equipment, then reset actor-relative motion and select ordinary surface
/// mode. Earlier writes survive a missing later dependency.
pub fn prepare_scene_entry(
    objects: &mut super::ObjectStore,
    world: &mut super::scene_path_world::ScenePathWorld,
    resources: &mut super::program_resources::ProgramResources<super::program_state::ProgramData>,
    owner: super::ObjectId,
) -> Result<(), MotionResetError> {
    const AMBIENT_RETAINED_FLAGS: u16 = 0xFF00;
    const SURFACE_KIND_MASK: u8 = 0x07;
    const ORDINARY_SURFACE_KIND: u8 = 1;
    let ambient = world
        .render_environment
        .ambient_control
        .as_mut()
        .ok_or(MotionResetError::MissingAmbientControl)?;
    // Unlike the surface publisher, this byte-width writer leaves the
    // ambient control's upper byte intact.
    *ambient = super::player_surface_render::AmbientParticleControl::from_bits(
        ambient.bits() & AMBIENT_RETAINED_FLAGS,
    );
    world.player_pitch_target = Some(0);
    world.player_yaw_increment = Some(0);
    world.player_roll_increment = Some(0);
    reset(objects, world, resources, owner)?;

    let level = world
        .scene
        .active_weapon_level
        .ok_or(MotionResetError::MissingPublishedWeaponLevel)?;
    world
        .player_mut(objects, owner)
        .map_err(MotionResetError::World)?
        .equipment
        .as_mut()
        .ok_or(MotionResetError::MissingEquipment(owner))?
        .weapon_level = level;
    let consumables = world
        .active_consumables
        .ok_or(MotionResetError::MissingPublishedConsumables)?;
    let equipment = world
        .player_mut(objects, owner)
        .map_err(MotionResetError::World)?
        .equipment
        .as_mut()
        .ok_or(MotionResetError::MissingEquipment(owner))?;
    equipment.consumable_type = consumables.kind;
    equipment.packed_consumables = consumables.packed_count;

    let actor = objects.get_mut(owner).ok_or(MotionResetError::World(
        super::scene_path_world::WorldInputError::MissingActor(owner),
    ))?;
    actor.extension.path_state.motion_delta = Default::default();
    actor.extension.path_state.platform_carry.saved_position = Default::default();
    actor.extension.path_state.motion.carry_selected_player = false;
    let mode = world
        .surface_mode
        .as_mut()
        .ok_or(MotionResetError::MissingSurfaceMode)?;
    mode.flags = (mode.flags & !SURFACE_KIND_MASK) | ORDINARY_SURFACE_KIND;
    Ok(())
}

/// Reset the currently bound allocation without releasing programs,
/// replacing player bindings or touching shared scene publications.
pub fn reset(
    objects: &super::ObjectStore,
    world: &mut super::scene_path_world::ScenePathWorld,
    resources: &mut super::program_resources::ProgramResources<super::program_state::ProgramData>,
    owner: super::ObjectId,
) -> Result<(), MotionResetError> {
    let records = world
        .player_mut(objects, owner)
        .map_err(MotionResetError::World)?;
    let storage = super::player_storage::get_mut(objects, resources, owner)
        .map_err(MotionResetError::Storage)?;
    clear(storage, records).map_err(MotionResetError::MissingRetainedRecord)
}

/// These records straddle the source clear boundary. A missing retained
/// portion cannot be manufactured as zero by a partial reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingRetainedRecord {
    Contact,
    Charge,
    Consumable,
    TargetControl,
    Vertical,
    Boundary,
    ModeSelection,
    CameraAngles,
    CameraGround,
    Carried,
}

/// Validate the retained portions before applying the typed record clear.
/// Fully cleared service records are initialized by this source operation;
/// wholly retained records (including missing publications) are untouched.
pub fn clear(
    storage: &mut PlayerStorage,
    records: &mut PlayerPathRecords,
) -> Result<(), MissingRetainedRecord> {
    let contact = records.contact.ok_or(MissingRetainedRecord::Contact)?;
    let charge = records.charge.ok_or(MissingRetainedRecord::Charge)?;
    let consumable = records
        .consumable
        .ok_or(MissingRetainedRecord::Consumable)?;
    let target = records
        .target_control
        .ok_or(MissingRetainedRecord::TargetControl)?;
    let vertical = records.vertical.ok_or(MissingRetainedRecord::Vertical)?;
    let boundary = records.boundary.ok_or(MissingRetainedRecord::Boundary)?;
    let mode = records
        .mode_selection
        .ok_or(MissingRetainedRecord::ModeSelection)?;
    let angles = records
        .camera_angles
        .ok_or(MissingRetainedRecord::CameraAngles)?;
    let ground = records
        .camera_ground
        .ok_or(MissingRetainedRecord::CameraGround)?;
    let carried = records.carried.ok_or(MissingRetainedRecord::Carried)?;

    // Exhaustive construction makes a future service addition require an
    // explicit decision about this partial-reset boundary.
    *records = PlayerPathRecords {
        contact: Some(super::scene_contact::PlayerContactControl {
            hit: super::player_hit_control::PlayerHitControl {
                feedback_duration: contact.hit.feedback_duration,
                feedback_flags: contact.hit.feedback_flags,
                tint: contact.hit.tint,
                tint_step: contact.hit.tint_step,
                reserve_shield: contact.hit.reserve_shield,
                ..Default::default()
            },
            ignores_contacts: false,
        }),
        protection: records.protection,
        auxiliary: Some(SelectedAuxiliaryState {
            mode: 0,
            action_flags: 0,
            stored_world_position: Default::default(),
            stored_rotation: Default::default(),
        }),
        charge: Some(super::player_charge::PlayerCharge {
            progress: charge.progress,
            control: charge.control,
            ..Default::default()
        }),
        rapid_aim: Some(Default::default()),
        rapid_rejection_consumes_queue: records.rapid_rejection_consumes_queue,
        action: records.action,
        // 6AE6 lies inside the cleared 391-byte prefix.
        saved_scene_selection: Some(0),
        mission: Some(Default::default()),
        palette_effects: records.palette_effects,
        consumable: Some(super::player_consumable::PlayerConsumableControl {
            projectile_blockers: consumable.projectile_blockers,
            ..Default::default()
        }),
        target_control: Some(super::path_player_control::PlayerTargetControl {
            transition_delay: target.transition_delay,
            mode: target.mode,
            positive_range: target.positive_range,
            limit: target.limit,
            axis_mode: target.axis_mode,
            control: target.control,
            axis_limits: target.axis_limits,
            progress: target.progress,
            ..Default::default()
        }),
        target_selection: Some(Default::default()),
        target_lock: Some(Default::default()),
        reticle_display: Some(Default::default()),
        visit: records.visit,
        status: records.status,
        appearance: Some(Default::default()),
        injected_input: Some(Default::default()),
        roll: Some(Default::default()),
        pose: Some(Default::default()),
        steering: Some(Default::default()),
        vertical: Some(super::player_vertical::PlayerVerticalControl {
            profile: vertical.profile,
            ..Default::default()
        }),
        throttle: Some(Default::default()),
        ambient: Some(Default::default()),
        flight_displacement: Some(Default::default()),
        surface: Some(Default::default()),
        speed: Some(Default::default()),
        motion: Some(Default::default()),
        boundary: Some(super::player_boundary::PlayerBoundary {
            return_position: boundary.return_position,
            ..Default::default()
        }),
        occupancy: Some(Default::default()),
        mode_selection: Some(super::player_mode_selection::PlayerModeSelection {
            transition_control: mode.transition_control,
            ..Default::default()
        }),
        view_distance: Some(Default::default()),
        camera_angles: Some(super::player_camera_angles::PlayerCameraAngles {
            profile: angles.profile,
            ..Default::default()
        }),
        camera_tracking: Some(Default::default()),
        camera_position: Some(Default::default()),
        camera_ground: Some(super::player_camera_ground::PlayerCameraGround {
            carried_target_height: ground.carried_target_height,
            ..Default::default()
        }),
        camera_surface: Some(Default::default()),
        camera_auxiliary: Some(Default::default()),
        camera_dispatch: Some(Default::default()),
        yaw_motion: Some(0),
        occupancy_exempt: records.occupancy_exempt,
        equipment: records.equipment,
        score: records.score,
        particles: Some(Default::default()),
        controlled_flags: Some(Default::default()),
        carried: Some(super::platform_carry::CarriedPlayer {
            fine_yaw: 0,
            ..carried
        }),
    };
    *storage = PlayerStorage {
        fine_pitch: 0,
        fine_yaw: 0,
        bank: super::Angle::ZERO,
        retained_shield: storage.retained_shield,
    };
    Ok(())
}

#[cfg(test)]
#[path = "player_motion_reset_tests.rs"]
mod tests;
