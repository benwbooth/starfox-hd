//! Continuous post-mode publication ($06:9DBA..9FAC): sound cooldown,
//! numbered-child pose, progress palette, position history, music admission,
//! terrain-contact feedback, then effects, recovery and attachments.
//! Mode execution and the later mission/exit branches remain separate.

use super::path_control::PlayerTarget;
use super::path_relationships::{self, RelationshipError};
use super::path_sound::{AuthoredCue, MusicControlRequest};
use super::player_frame_effects::{self, FrameEffectsError};
use super::player_motion::{self, MotionError};
use super::player_storage::{self, PlayerStorageError};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{Angle, ObjectId, ObjectStore, SoundEvent};

const SOUND_COUNT_MASK: u8 = 0x07;
const HEADING_CHILD_NUMBER: u8 = 22;
const FINE_ANGLE_SHIFT: u32 = 8;
const PROGRESS_CONFIGURATION: u8 = 9;
const ASTROPOLIS_LOCATION: u16 = 11;
const TRANSITION_PROGRESS: u8 = 254;
const COMPLETE_PROGRESS: u8 = 255;
const MUSIC_REQUESTED: u8 = 0x20;
const SURFACE_OBSTRUCTION: u8 = 0x10;
const SURFACE_CONTACT_LATCHED: u8 = 0x08;
const FAMILY_MASK: u8 = 0xF0;
const WALKER_FAMILY: u8 = 0x20;
const SURFACE_CONTACT_CUE: u8 = 167;
const CONTACT_FEEDBACK_DURATION: u8 = 2;
const CONTACT_FEEDBACK_FLAGS: u8 = 0x68;
const CONTACT_PITCH_RECOIL: i16 = 128;
const REDUCED_IMPULSE: u8 = 0x20;
const ORDINARY_SPEED_IMPULSE: i16 = 70;
const REDUCED_SPEED_IMPULSE: i16 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostMotionError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    Relationships(RelationshipError),
    Motion(MotionError),
    Effects(FrameEffectsError),
    MissingLocation,
    MissingCoordination,
    MissingPaletteControl(ObjectId),
    MissingInterceptionGate,
    MissingMusicReadiness,
    MissingMissionControl(ObjectId),
    MissingMotion(ObjectId),
    MissingPrimaryPlayer,
}

impl From<WorldInputError> for PostMotionError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<PlayerStorageError> for PostMotionError {
    fn from(error: PlayerStorageError) -> Self {
        Self::Storage(error)
    }
}
impl From<RelationshipError> for PostMotionError {
    fn from(error: RelationshipError) -> Self {
        Self::Relationships(error)
    }
}

/// Run the prefix through the surface-contact edge latch ($06:9EE7).
/// This captures the final mode position, for use by the NEXT movement
/// visit. It must not replace the pre-movement history before mode execution.
pub fn prepare(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<(), PostMotionError> {
    let cooldown = &mut world
        .player_mut(objects, owner)?
        .contact
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .deflection_sound_cooldown;
    if *cooldown != 0 {
        *cooldown = (*cooldown - 1) & SOUND_COUNT_MASK;
    }

    // Fine heading is read even when there is no matching child. Pending
    // removal is not a filter; only the first direct numbered child is used.
    let yaw = Angle::from_units(
        (player_storage::get(objects, resources, owner)?.fine_yaw >> FINE_ANGLE_SHIFT) as u8,
    );
    if let Some(child) =
        path_relationships::find_direct_child(objects, owner, HEADING_CHILD_NUMBER)?
    {
        let position = objects
            .get(owner)
            .expect("validated post-mode owner")
            .base
            .position;
        let child = objects.get_mut(child).expect("validated numbered child");
        child.base.position = position;
        child.base.pitch = Angle::ZERO;
        child.base.roll = Angle::ZERO;
        child.base.yaw = yaw;
    }

    if world
        .scene
        .player_configuration
        .ok_or(WorldInputError::MissingPlayerConfiguration)?
        == PROGRESS_CONFIGURATION
    {
        let location = world
            .scene
            .encounter_location
            .ok_or(PostMotionError::MissingLocation)?;
        let progress = world
            .coordination
            .ok_or(PostMotionError::MissingCoordination)?
            .progress;
        let requested = if location == ASTROPOLIS_LOCATION {
            progress == COMPLETE_PROGRESS
        } else {
            progress == TRANSITION_PROGRESS
        };
        if requested {
            world
                .player_mut(objects, owner)?
                .palette_effects
                .as_mut()
                .ok_or(PostMotionError::MissingPaletteControl(owner))?
                .request_progress_pulse();
        }
    }
    player_motion::capture_position(objects, world, owner).map_err(PostMotionError::Motion)?;

    if world
        .contacts_enabled()
        .ok_or(WorldInputError::MissingContactEnable)?
        && world
            .interception_active
            .ok_or(PostMotionError::MissingInterceptionGate)?
        && world
            .interception_music_ready
            .ok_or(PostMotionError::MissingMusicReadiness)?
    {
        let flags = &mut world
            .player_mut(objects, owner)?
            .mission
            .as_mut()
            .ok_or(PostMotionError::MissingMissionControl(owner))?
            .flags;
        if *flags & MUSIC_REQUESTED == 0 {
            *flags |= MUSIC_REQUESTED;
            world
                .audio
                .request_music_control(MusicControlRequest::InterceptionReady);
        }
    }

    let flags = &mut world
        .player_mut(objects, owner)?
        .motion
        .as_mut()
        .ok_or(PostMotionError::MissingMotion(owner))?
        .contact_flags;
    if *flags & SURFACE_OBSTRUCTION == 0 {
        *flags &= !(SURFACE_OBSTRUCTION | SURFACE_CONTACT_LATCHED);
        return Ok(());
    }
    *flags &= !SURFACE_OBSTRUCTION;
    if *flags & SURFACE_CONTACT_LATCHED != 0 {
        return Ok(());
    }
    *flags |= SURFACE_CONTACT_LATCHED;
    if world
        .player(objects, owner)?
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .mode
        & FAMILY_MASK
        == WALKER_FAMILY
    {
        return Ok(());
    }

    let primary = world
        .primary_player
        .ok_or(PostMotionError::MissingPrimaryPlayer)?;
    world.audio.queue(SoundEvent::Authored(AuthoredCue::new(
        SURFACE_CONTACT_CUE,
        0,
        if owner == primary {
            PlayerTarget::Primary
        } else {
            PlayerTarget::Secondary
        },
    )));
    let records = world.player_mut(objects, owner)?;
    let hit = &mut records
        .contact
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit;
    hit.feedback_duration = CONTACT_FEEDBACK_DURATION;
    hit.feedback_flags |= CONTACT_FEEDBACK_FLAGS;
    hit.initialize_pitch_recoil(CONTACT_PITCH_RECOIL);
    let reduced = records
        .auxiliary
        .ok_or(WorldInputError::MissingAuxiliary(owner))?
        .action_flags
        & REDUCED_IMPULSE
        != 0;
    records
        .charge
        .as_mut()
        .ok_or(WorldInputError::MissingPlayerCharge(owner))?
        .speed_impulse = if reduced {
        REDUCED_SPEED_IMPULSE
    } else {
        ORDINARY_SPEED_IMPULSE
    };
    Ok(())
}

/// Continuous tail through attachment publication. The inherited damage
/// particle number is unchanged by this prefix's source helpers; it remains
/// an explicit caller input rather than a made-up default.
pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &ProgramResources<ProgramData>,
    owner: ObjectId,
    damage_particle_number: u8,
) -> Result<(), PostMotionError> {
    prepare(objects, world, resources, owner)?;
    player_frame_effects::advance_with_attachments(objects, world, owner, damage_particle_number)
        .map_err(PostMotionError::Effects)
}

#[cfg(test)]
#[path = "player_post_motion_tests.rs"]
mod tests;
