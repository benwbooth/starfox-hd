//! Vertical input and both terrain-limit controllers ($06:E39A..E4B0,
//! $06:E9F6..ECAF), with their real history and configuration producers.
//! These publish pitch/lean and motion permissions; they do not move actors.

use super::player_pose::quarter_byte;
use super::player_storage::{self, PlayerStorageError};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Button, ObjectId, ObjectStore};

const ACTIVE: u8 = 0x01;
const PITCH_CONTROL: u8 = 0x80;
const UPPER_LIMIT: u8 = 0x04;
const LOWER_LIMIT: u8 = 0x08;
const ALL_MOTION_AXES: u8 = 0xE0;
const VERTICAL_MOTION: u8 = 0x40;
const VERTICAL_BUTTONS: u16 = Button::Up as u16 | Button::Down as u16;
const FINE_SHIFT: u32 = u8::BITS;
const PLANE_CLEARANCE: i16 = 50;
const PLANE_PITCH: u16 = 49_152;
const NEUTRAL_MARGIN: i16 = 32;
const UPPER_RESPONSE_MARGIN: i16 = 64;
const LOWER_RESPONSE_MARGIN: i16 = 110;
const NEUTRAL_PITCH: u16 = 1_280;
const SOFT_UPPER_LEAN: i8 = -30;
const HARD_LEAN: i8 = 10;
const LEAN_RECOVERY: i8 = 2;
const TERRAIN_ADJUSTMENT: u8 = 3;
const MAX_RESPONSE_DISTANCE: u16 = 127;

/// Map-selected parameters copied by $06:9A44. Offsets are added to actor
/// height with word wrapping; these are not absolute world-space bounds.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VerticalProfile {
    pub upper_height_offset: i16,
    pub lower_height_offset: i16,
    pub up_pitch: u8,
    pub down_pitch: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerVerticalControl {
    /// 6BF5..FA: initialized to zero by allocation, then configured by maps.
    pub profile: VerticalProfile,
    /// 6B81: retain unrelated flags while replacing limit bits 04/08.
    pub limit_flags: u8,
    /// 6B82: pitch-control bit 80; other bits have separate owners.
    pub control_flags: u8,
    /// 6B84: movement-axis permission byte, reset by each limit visit.
    pub motion_axes: u8,
    /// 6ABF: pitch-side adjustment publication (zero or three here).
    /// It must not be treated as player speed without its consumer contract.
    pub pitch_adjustment: u8,
    /// 6B87/89: retained held-edge history, not the shared pressed snapshot.
    pub latched_input: u16,
    pub previous_held: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalMode {
    /// $06:E371/E374: retained-history pitch, then soft limits.
    Flight,
    /// $06:E30E/E311: held/retained-fine pitch, then hard limits.
    RetainedPitch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerticalError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    MissingControl(ObjectId),
    MissingPose(ObjectId),
    MissingProcessedInput,
    MissingPitchTarget,
    MissingEnvironmentPlane,
}

impl From<WorldInputError> for VerticalError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<PlayerStorageError> for VerticalError {
    fn from(error: PlayerStorageError) -> Self {
        Self::Storage(error)
    }
}

fn control<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut PlayerVerticalControl, VerticalError> {
    world
        .player_mut(objects, owner)?
        .vertical
        .as_mut()
        .ok_or(VerticalError::MissingControl(owner))
}

fn pitch(world: &ScenePathWorld) -> Result<u16, VerticalError> {
    world
        .player_pitch_target
        .ok_or(VerticalError::MissingPitchTarget)
}

fn lean<'a>(
    objects: &ObjectStore,
    world: &'a mut ScenePathWorld,
    owner: ObjectId,
) -> Result<&'a mut i8, VerticalError> {
    Ok(&mut world
        .player_mut(objects, owner)?
        .pose
        .as_mut()
        .ok_or(VerticalError::MissingPose(owner))?
        .pitch_lean)
}

/// Complete parameter-copy helper. The scene wrapper selects the primary
/// player, as the map-facing $06:9A2F caller does.
pub fn configure(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    profile: VerticalProfile,
) -> Result<(), VerticalError> {
    control(objects, world, owner)?.profile = profile;
    Ok(())
}

/// $06:F2D9: retain a newly held bit until release or a separate explicit
/// history clear. Full input words participate, not just pitch buttons.
pub fn retain_input(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), VerticalError> {
    let record = *control(objects, world, owner)?;
    let held = world
        .processed_player_input
        .ok_or(VerticalError::MissingProcessedInput)?
        .held
        .bits();
    let control = control(objects, world, owner)?;
    control.latched_input = ((record.previous_held ^ held) | record.latched_input) & held;
    control.previous_held = held;
    Ok(())
}

/// $06:E3F7: up is held-input driven; down deliberately reads retained
/// history. The contact-plane override runs after normal pitch publication.
pub fn flight_pitch(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), VerticalError> {
    control(objects, world, owner)?.pitch_adjustment = 0;
    world.player_pitch_target = Some(0);
    let input = world
        .processed_player_input
        .ok_or(VerticalError::MissingProcessedInput)?;
    let record = control(objects, world, owner)?;
    if input.pressed.bits() & VERTICAL_BUTTONS != 0 {
        record.control_flags |= PITCH_CONTROL;
    }
    record.control_flags &= !PITCH_CONTROL;
    let target = if input.held.contains(Button::Up) {
        record.profile.up_pitch
    } else if record.latched_input & Button::Down as u16 != 0 {
        record.profile.down_pitch
    } else {
        0
    };
    world.player_pitch_target = Some(u16::from(target) << FINE_SHIFT);
    let held_protection = world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .hold_secondary_protection;
    if held_protection {
        let plane = world
            .environment_plane_height
            .ok_or(VerticalError::MissingEnvironmentPlane)?;
        let threshold = plane.wrapping_sub(PLANE_CLEARANCE);
        if objects
            .get(owner)
            .expect("validated player")
            .base
            .position
            .y
            .wrapping_sub(threshold)
            >= 0
        {
            world.player_pitch_target = Some(PLANE_PITCH);
        }
    }
    Ok(())
}

/// $06:E39A: active held-input mode retains *fine* pitch when neutral.
/// An inactive actor only clears the shared target, preserving its record.
pub fn held_pitch(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    resources: &ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<(), VerticalError> {
    world.player_pitch_target = Some(0);
    if world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .action_flags
        & ACTIVE
        == 0
    {
        return Ok(());
    }
    let record = control(objects, world, owner)?;
    record.control_flags |= PITCH_CONTROL;
    record.pitch_adjustment = 0;
    let profile = record.profile;
    let input = world
        .processed_player_input
        .ok_or(VerticalError::MissingProcessedInput)?;
    let target = if input.held.contains(Button::Up) {
        u16::from(profile.up_pitch) << FINE_SHIFT
    } else if input.held.contains(Button::Down) {
        u16::from(profile.down_pitch) << FINE_SHIFT
    } else {
        player_storage::get(objects, resources, owner)?.fine_pitch
    };
    world.player_pitch_target = Some(target);
    Ok(())
}

fn approach(current: i8, target: i8, step: i8) -> i8 {
    let delta = current.wrapping_sub(target);
    if delta == 0 {
        return current;
    }
    if delta < 0 {
        let next = current.wrapping_add(step);
        if next.wrapping_sub(target) >= 0 {
            target
        } else {
            next
        }
    } else {
        let next = current.wrapping_sub(step);
        if next.wrapping_sub(target) < 0 {
            target
        } else {
            next
        }
    }
}

/// $06:EBC7 and the original signed multiplication: saturate the unsigned
/// distance, then retain product bits 8..23 (arithmetic, not truncating divide).
fn response(value: u16, distance: u16) -> u16 {
    let sample = sf_core::snes_trig::COSTAB[usize::from(distance.min(MAX_RESPONSE_DISTANCE))];
    ((i32::from(value as i16) * i32::from(sample)) >> FINE_SHIFT) as u16
}

fn scale_pitch(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
    distance: u16,
) -> Result<(), VerticalError> {
    world.player_pitch_target = Some(response(pitch(world)?, distance));
    control(objects, world, owner)?.pitch_adjustment = TERRAIN_ADJUSTMENT;
    Ok(())
}

/// $06:E9F6: complete soft upper/lower limits, in that order. The lower
/// branch can observe a pitch already reversed/scaled by the upper branch.
pub fn soft_limits(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), VerticalError> {
    let record = control(objects, world, owner)?;
    record.motion_axes = ALL_MOTION_AXES;
    record.limit_flags &= !UPPER_LIMIT;
    let upper = record.profile.upper_height_offset;
    let height = objects
        .get(owner)
        .expect("validated player")
        .base
        .position
        .y;
    if upper.wrapping_sub(NEUTRAL_MARGIN).wrapping_add(height) < 0 && pitch(world)? == 0 {
        world.player_pitch_target = Some(NEUTRAL_PITCH);
    }
    let distance = upper
        .wrapping_sub(UPPER_RESPONSE_MARGIN)
        .wrapping_add(height);
    if (pitch(world)? as i16) < 0 && distance < 0 {
        control(objects, world, owner)?.limit_flags |= UPPER_LIMIT;
        scale_pitch(objects, world, owner, distance.wrapping_neg() as u16)?;
        if world.strategy_clock & 1 != 0 {
            let lean = lean(objects, world, owner)?;
            *lean = approach(*lean, SOFT_UPPER_LEAN, 1);
        }
    } else {
        let lean = lean(objects, world, owner)?;
        if *lean < 0 {
            *lean = approach(*lean, 0, LEAN_RECOVERY);
        }
    }
    let record = control(objects, world, owner)?;
    record.limit_flags &= !LOWER_LIMIT;
    let lower = record.profile.lower_height_offset;
    if lower.wrapping_add(NEUTRAL_MARGIN).wrapping_add(height) >= 0 && pitch(world)? == 0 {
        world.player_pitch_target = Some(NEUTRAL_PITCH.wrapping_neg());
    }
    let distance = lower
        .wrapping_add(LOWER_RESPONSE_MARGIN)
        .wrapping_add(height);
    if (pitch(world)? as i16) > 0 && distance >= 0 {
        let distance = (distance as u16) >> 1;
        control(objects, world, owner)?.limit_flags |= LOWER_LIMIT;
        scale_pitch(objects, world, owner, distance)?;
        let pose = world
            .player_mut(objects, owner)?
            .pose
            .as_mut()
            .ok_or(VerticalError::MissingPose(owner))?;
        pose.steering_bank =
            (response(i16::from(pose.steering_bank) as u16, distance) as u8).wrapping_mul(2) as i8;
    } else {
        let lean = lean(objects, world, owner)?;
        if *lean >= 0 {
            *lean = approach(*lean, 0, LEAN_RECOVERY);
        }
    }
    Ok(())
}

/// $06:EA10: hard limits disable vertical motion, halve pitch arithmetically,
/// and approach limit lean on odd visits before neutral-input recovery.
pub fn hard_limits(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    owner: ObjectId,
) -> Result<(), VerticalError> {
    control(objects, world, owner)?.motion_axes = ALL_MOTION_AXES;
    for (flag, upper, target_lean) in [
        (UPPER_LIMIT, true, -HARD_LEAN),
        (LOWER_LIMIT, false, HARD_LEAN),
    ] {
        let record = control(objects, world, owner)?;
        record.limit_flags &= !flag;
        let offset = if upper {
            record.profile.upper_height_offset
        } else {
            record.profile.lower_height_offset
        };
        let distance = offset.wrapping_add(
            objects
                .get(owner)
                .expect("validated player")
                .base
                .position
                .y,
        );
        let outside = if upper { distance < 0 } else { distance >= 0 };
        if outside {
            let target = pitch(world)? as i16;
            if (target < 0) == upper {
                control(objects, world, owner)?.motion_axes &= !VERTICAL_MOTION;
                world.player_pitch_target = Some((target >> 1) as u16);
                control(objects, world, owner)?.limit_flags |= flag;
                if world.strategy_clock & 1 != 0 {
                    let lean = lean(objects, world, owner)?;
                    *lean = approach(*lean, target_lean, 1);
                }
            }
        }
    }
    if world
        .processed_player_input
        .ok_or(VerticalError::MissingProcessedInput)?
        .held
        .bits()
        & VERTICAL_BUTTONS
        == 0
    {
        let lean = lean(objects, world, owner)?;
        *lean = quarter_byte(*lean as u8, 0) as i8;
    }
    Ok(())
}

/// The adjacent pitch/limits pair used by each flight caller. History runs
/// earlier, before horizontal steering; movement and camera still follow.
pub fn advance(
    objects: &ObjectStore,
    world: &mut ScenePathWorld,
    resources: &ProgramResources<ProgramData>,
    owner: ObjectId,
    mode: VerticalMode,
) -> Result<(), VerticalError> {
    match mode {
        VerticalMode::Flight => {
            flight_pitch(objects, world, owner)?;
            soft_limits(objects, world, owner)
        }
        VerticalMode::RetainedPitch => {
            held_pitch(objects, world, resources, owner)?;
            hard_limits(objects, world, owner)
        }
    }
}

#[cfg(test)]
#[path = "player_vertical_tests.rs"]
mod tests;
