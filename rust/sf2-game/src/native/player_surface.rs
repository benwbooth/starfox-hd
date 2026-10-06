//! Two-wing surface response ($07:DD6F..E0C8). The shared probe actor is
//! updated in place; effects and feedback precede the later speed/pose visit.

use super::path_control::PlayerTarget;
use super::path_relationships::{self, RelationshipError};
use super::path_sound::AuthoredCue;
use super::player_storage::{self, PlayerStorageError};
use super::player_surface_particle::{self, ParticleError, ParticleInputs, SurfaceParticle};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::render::ClippingPlaneSelection;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{
    authored_paths, Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId, SoundEvent,
    Vector3, OBJECT_CAPACITY,
};

const SURFACE_MODE_MASK: u8 = 0x07;
const SPECIAL_CONFIGURATION: u8 = 9;
const ACTIVE_CARRY_MODE: u8 = 1;
const PROBE_LATERAL: i8 = 35;
const PROBE_VERTICAL: i8 = 5;
const PARTICLE_LATERAL: i8 = 20;
const PARTICLE_SIZE: u8 = 2;
const SHORT_MATERIAL: u8 = 4;
const WING_MATERIALS: [u8; 2] = [0, 5];
const UPPER_FEEDBACK: u8 = 0x0C;
const CONTACT_PITCH: i8 = 24;
const CONTACT_RECOIL: i16 = 64;
const WING_SHAPE: ShapeId = ShapeId::from_catalog_index(36);
const WING_X: [i16; 3] = [35, 35, 15];
const WING_Y: [i16; 3] = [15, 20, 20];
const WING_Z: [i16; 3] = [-40, -40, -50];
const PILOT_COUNT: u8 = 6;
const SOUND_EVENTS: u8 = 0x07;
const WING_SOUND: u8 = 19;
const PARTICLE_SOUND: u8 = 60;
const FIRST_SOUND_POSITION: u8 = 32;
const SECOND_SOUND_POSITION: u8 = 16;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerSurface {
    /// Selected contact plane (6A7D), not the scene's environment plane.
    pub plane_height: i16,
    /// Material selector (6A82), independent of particle/count flags.
    pub material: u8,
}

/// Values inherited from the enclosing caller. No neutral substitution is
/// permitted: the wing installer does not initialize its child number, and
/// early skips preserve the alternate speed target's incoming value.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceContext {
    pub wing_child_number: Option<u8>,
    pub alternate_thrust_target: Option<i8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceResponse {
    pub contact: bool,
    /// The following speed service reuses these event bits as a target in
    /// one locked-heading branch. Keep the real value even without sound.
    pub effect_events: u8,
    /// Replaced by the upper-limit high byte when probes are enabled.
    pub alternate_thrust_target: Option<i8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceError {
    World(WorldInputError),
    Storage(PlayerStorageError),
    Relationships(RelationshipError),
    Particle(ParticleError),
    MissingSurfaceMode,
    MissingSurface(ObjectId),
    MissingVertical(ObjectId),
    MissingPose(ObjectId),
    MissingVisit(ObjectId),
    MissingCarryMode,
    MissingProbe,
    MissingViewMode,
    MissingWingChildNumber,
    MissingSpawnDefaults,
    ObjectPoolExhausted,
}

impl From<WorldInputError> for SurfaceError {
    fn from(value: WorldInputError) -> Self {
        Self::World(value)
    }
}
impl From<PlayerStorageError> for SurfaceError {
    fn from(value: PlayerStorageError) -> Self {
        Self::Storage(value)
    }
}
impl From<RelationshipError> for SurfaceError {
    fn from(value: RelationshipError) -> Self {
        Self::Relationships(value)
    }
}
impl From<ParticleError> for SurfaceError {
    fn from(value: ParticleError) -> Self {
        Self::Particle(value)
    }
}

fn carry_suppressed(
    objects: &ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
) -> Result<bool, SurfaceError> {
    Ok(world
        .player_carry_mode
        .ok_or(SurfaceError::MissingCarryMode)?
        == ACTIVE_CARRY_MODE
        && objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .extension
            .path_state
            .motion
            .carry_selected_player)
}

fn sound(world: &mut ScenePathWorld, owner: ObjectId, events: u8) -> Result<(), SurfaceError> {
    if world
        .scripted_view_active()
        .ok_or(SurfaceError::MissingViewMode)?
    {
        return Ok(());
    }
    let (id, position) = match events & SOUND_EVENTS {
        1 => (WING_SOUND, FIRST_SOUND_POSITION),
        2 => (WING_SOUND, SECOND_SOUND_POSITION),
        3 => (WING_SOUND, 0),
        5 => (PARTICLE_SOUND, FIRST_SOUND_POSITION),
        6 => (PARTICLE_SOUND, SECOND_SOUND_POSITION),
        7 => (PARTICLE_SOUND, 0),
        _ => return Ok(()),
    };
    let side = if world.primary_player == Some(owner) {
        PlayerTarget::Primary
    } else {
        PlayerTarget::Secondary
    };
    world
        .audio
        .queue(SoundEvent::Authored(AuthoredCue::new(id, position, side)));
    Ok(())
}

/// Both wing installers always create a new child; unlike boost/brake they
/// do not search for an existing number. Pilot selection precedes allocation.
pub fn spawn_wing(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    owner: ObjectId,
    negative_side: bool,
    child_number: Option<u8>,
) -> Result<ObjectId, SurfaceError> {
    let pilot = world
        .player(objects, owner)?
        .visit
        .ok_or(SurfaceError::MissingVisit(owner))?
        .pilot_code;
    let pair = usize::from(if pilot < PILOT_COUNT { pilot / 2 } else { 0 });
    if objects.len() == OBJECT_CAPACITY {
        return Err(SurfaceError::ObjectPoolExhausted);
    }
    let defaults = world
        .spawn_defaults()
        .ok_or(SurfaceError::MissingSpawnDefaults)?;
    let head = objects.active_ids().first().copied();
    let child = objects
        .allocate_after(
            head,
            Object::new_authored(
                ObjectKind::Effect,
                WING_SHAPE,
                Behavior::Unassigned,
                defaults,
            ),
        )
        .ok_or(SurfaceError::ObjectPoolExhausted)?;
    let number = child_number.ok_or(SurfaceError::MissingWingChildNumber)?;
    path_relationships::attach_fresh_child(objects, owner, child, number)?;
    let actor = objects.get_mut(child).expect("allocated wing effect");
    actor.extension.relative_position = Vector3 {
        x: if negative_side {
            -WING_X[pair]
        } else {
            WING_X[pair]
        },
        y: WING_Y[pair],
        z: WING_Z[pair],
    };
    actor.base.path = Some(authored_paths::ALTERNATE_EXHAUST);
    super::player_effect::format(objects, owner, child)?;
    Ok(child)
}

pub fn respond(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    context: SurfaceContext,
) -> Result<SurfaceResponse, SurfaceError> {
    let mut response = SurfaceResponse {
        contact: false,
        effect_events: 0,
        alternate_thrust_target: context.alternate_thrust_target,
    };
    objects
        .get_mut(owner)
        .ok_or(WorldInputError::MissingActor(owner))?
        .extension
        .clipping_plane = ClippingPlaneSelection::DISABLED;
    let mode = world
        .surface_mode
        .ok_or(SurfaceError::MissingSurfaceMode)?
        .flags;
    if mode & SURFACE_MODE_MASK == 0 {
        sound(world, owner, response.effect_events)?;
        return Ok(response);
    }
    if world
        .player(objects, owner)?
        .contact
        .ok_or(WorldInputError::MissingPlayerContact(owner))?
        .hit
        .hold_secondary_protection
    {
        sound(world, owner, response.effect_events)?;
        return Ok(response);
    }
    let surface = world
        .player(objects, owner)?
        .surface
        .ok_or(SurfaceError::MissingSurface(owner))?;
    let upper = world
        .player(objects, owner)?
        .vertical
        .ok_or(SurfaceError::MissingVertical(owner))?
        .profile
        .upper_height_offset;
    response.alternate_thrust_target = Some((upper >> u8::BITS) as i8);
    let plane = surface.plane_height.wrapping_neg();
    let proxy = world
        .weapons
        .as_ref()
        .and_then(|state| state.fallback)
        .ok_or(SurfaceError::MissingProbe)?;
    let mut feedback = 0_u8;
    for (negative, lower_mask, upper_mask, wing_event, particle_event) in [
        (false, 0x02, 0x08, 0x01, 0x05),
        (true, 0x01, 0x04, 0x02, 0x06),
    ] {
        let actor = &objects
            .get(owner)
            .ok_or(WorldInputError::MissingActor(owner))?
            .base;
        let (x, y, z) = sf_core::snes_trig::strat_roffs_full(
            actor.roll.units(),
            actor.pitch.units(),
            actor.yaw.units(),
            if negative {
                -PROBE_LATERAL
            } else {
                PROBE_LATERAL
            },
            PROBE_VERTICAL,
            0,
        );
        let position = Vector3 {
            x: actor.position.x.wrapping_add(x),
            y: actor.position.y.wrapping_add(y),
            z: actor.position.z.wrapping_add(z),
        };
        objects
            .get_mut(proxy)
            .ok_or(WorldInputError::MissingActor(proxy))?
            .base
            .position = position;
        let special = world
            .scene
            .player_configuration
            .ok_or(WorldInputError::MissingPlayerConfiguration)?
            == SPECIAL_CONFIGURATION;
        let upper_contact = special && position.y.wrapping_add(upper) < 0;
        if upper_contact {
            feedback |= upper_mask;
        } else {
            if plane.wrapping_add(position.y) < 0 {
                continue;
            }
            objects
                .get_mut(owner)
                .expect("validated owner")
                .extension
                .clipping_plane = ClippingPlaneSelection::SECOND;
            feedback |= lower_mask;
            if carry_suppressed(objects, world, owner)? {
                continue;
            }
        }
        if upper_contact || WING_MATERIALS.contains(&surface.material) {
            spawn_wing(objects, world, owner, negative, context.wing_child_number)?;
            response.effect_events |= wing_event;
        } else {
            let (kind, frame) = if surface.material == SHORT_MATERIAL {
                (SurfaceParticle::Short, 0)
            } else {
                (SurfaceParticle::Long, 3)
            };
            player_surface_particle::spawn(
                objects,
                world,
                owner,
                kind,
                ParticleInputs {
                    lateral: if negative {
                        -PARTICLE_LATERAL
                    } else {
                        PARTICLE_LATERAL
                    },
                    forward: 0,
                    size: PARTICLE_SIZE,
                    frame,
                },
            )?;
            response.effect_events |= particle_event;
        }
    }
    if feedback != 0 {
        let pitch = if feedback & UPPER_FEEDBACK != 0 {
            CONTACT_PITCH
        } else {
            -CONTACT_PITCH
        };
        let storage = player_storage::get_mut(objects, resources, owner)?;
        storage.fine_pitch = (storage.fine_pitch & 0x00FF) | (u16::from(pitch as u8) << u8::BITS);
        let pose = world
            .player_mut(objects, owner)?
            .pose
            .as_mut()
            .ok_or(SurfaceError::MissingPose(owner))?;
        let bank_low = (pose.shoulder_bank as i8) >> 2;
        pose.shoulder_bank =
            ((pose.shoulder_bank as u16 & 0xFF00) | u16::from(bank_low as u8)) as i16;
        if !carry_suppressed(objects, world, owner)? {
            world
                .player_mut(objects, owner)?
                .contact
                .as_mut()
                .ok_or(WorldInputError::MissingPlayerContact(owner))?
                .hit
                .initialize_pitch_recoil(CONTACT_RECOIL);
            response.contact = true;
        }
    }
    sound(world, owner, response.effect_events)?;
    Ok(response)
}

#[cfg(test)]
#[path = "player_surface_tests.rs"]
mod tests;
