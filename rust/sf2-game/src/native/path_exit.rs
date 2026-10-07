//! Ordinary encounter-exit craft and camera placement ($07:F808, $07:F3E1).
//! All direction tables are decoded immutable catalog data, not runtime ROM reads.

use super::path_actor_context::ActorContextError;
use super::path_program::{PathWorld, ProgramError};
use super::path_runtime::PathRuntimeError;
use super::{Angle, ObjectId, ObjectStore, Rotation, Vector3};

const EXIT_SPEED: u8 = 50;
const DIRECTION_MASK: usize = 7;
const PHASE_COMPANION_MASK: u16 = 0xFF00;
const WORLD_OFFSET_SHIFT: u32 = 6;
const CAMERA_HEIGHT: i16 = 10;
const CAMERA_DEPTH: i8 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitCraftPose {
    pub offset: Vector3,
    pub rotation: Rotation,
    pub relative_pitch: Angle,
    pub turn_rate: u8,
    pub relative_roll: Angle,
    pub target_speed: u8,
}

pub(super) fn initialize_craft(
    objects: &mut ObjectStore,
    owner: ObjectId,
    world: &PathWorld<'_>,
    poses: &[ExitCraftPose; 8],
) -> Result<(), ProgramError> {
    let actor = objects
        .get_mut(owner)
        .ok_or(PathRuntimeError::MissingActor(owner))?;
    // This publication precedes the first shared-state read in the source.
    actor.base.speed = EXIT_SPEED;
    let direction = world
        .handoff
        .as_ref()
        .ok_or(ProgramError::MissingEncounterHandoff)?
        .heading_word
        .to_le_bytes()[1];
    actor.base.attack_power = direction;
    let pose = poses[usize::from(direction) & DIRECTION_MASK];
    actor.extension.relative_rotation.pitch = pose.relative_pitch;
    actor.extension.path_state.motion_phase = (actor.extension.path_state.motion_phase
        & PHASE_COMPANION_MASK)
        | u16::from(pose.turn_rate);
    actor.extension.relative_rotation.roll = pose.relative_roll;
    actor.base.pitch = pose.rotation.pitch;
    actor.base.yaw = pose.rotation.yaw;
    actor.base.roll = pose.rotation.roll;
    actor.base.target_speed = pose.target_speed;
    actor.extension.relative_position = pose.offset;
    actor.base.position.x = actor.base.position.x.wrapping_add(pose.offset.x);
    actor.base.position.y = actor.base.position.y.wrapping_add(pose.offset.y);
    actor.base.position.z = actor.base.position.z.wrapping_add(pose.offset.z);
    Ok(())
}

pub(super) fn position_view(
    objects: &mut ObjectStore,
    owner: ObjectId,
    last_spawn: Option<ObjectId>,
    world: &PathWorld<'_>,
    depths: &[i8; 8],
) -> Result<(), ProgramError> {
    let target = last_spawn.ok_or(ProgramError::ActorContext(
        ActorContextError::MissingLastSpawn,
    ))?;
    let actor = objects
        .get(owner)
        .ok_or(PathRuntimeError::MissingActor(owner))?;
    let origin = actor.base.position;
    let (x, z) = sf_core::snes_trig::rotate_8xz(actor.base.yaw.units(), 0, CAMERA_DEPTH);
    let view = objects
        .get_mut(target)
        .ok_or(PathRuntimeError::MissingActor(target))?;
    view.base.position = Vector3 {
        x: origin.x.wrapping_add(x.wrapping_shl(WORLD_OFFSET_SHIFT)),
        y: origin
            .y
            .wrapping_add(CAMERA_HEIGHT.wrapping_shl(WORLD_OFFSET_SHIFT)),
        z: origin.z.wrapping_add(z.wrapping_shl(WORLD_OFFSET_SHIFT)),
    };
    // The initial placement survives a missing later scene publication.
    let direction = world
        .handoff
        .as_ref()
        .ok_or(ProgramError::MissingEncounterHandoff)?
        .heading_word
        .to_le_bytes()[1];
    let heading = world
        .scene
        .entry_heading
        .ok_or(ProgramError::MissingSceneByte(
            super::path_program::SceneByte::EntryHeading,
        ))?;
    view.base.yaw = Angle::from_units(heading);
    view.base.pitch = Angle::ZERO;
    view.base.roll = Angle::ZERO;
    let depth = depths[usize::from(direction) & DIRECTION_MASK];
    let (x, z) = sf_core::snes_trig::rotate_8xz(heading, 0, depth);
    view.base.position.x = view
        .base
        .position
        .x
        .wrapping_add(x.wrapping_shl(WORLD_OFFSET_SHIFT));
    view.base.position.z = view
        .base
        .position
        .z
        .wrapping_add(z.wrapping_shl(WORLD_OFFSET_SHIFT));
    Ok(())
}
