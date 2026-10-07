//! Post-flight surface contact and damage ($07:E18E..E2F1). The forecast
//! uses the real reserved actor and pre-movement position. It does not redo
//! flight movement or overwrite the first probe's shared height publication.

use super::collision_surface::{self, ObjectSurfaceContact, SurfaceQueryError};
use super::hit_response::health_after_damage;
use super::path_control::PlayerTarget;
use super::path_motion;
use super::path_sound::AuthoredCue;
use super::player_hit_control::Impact;
use super::player_impact::{self, ImpactError};
use super::player_storage::{self, PlayerStorageError};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::{ObjectId, ObjectStore, SoundEvent};

const NON_DAMAGING_MATERIAL: u8 = 6;
const CONSTRAINED_CONFIGURATION: u8 = 9;
const OBSTRUCTION: u8 = 0x10;
const PITCH_IMPACT: u16 = 59_392;
const HEAVY_DAMAGE: u8 = 4;
const HEAVY_CUE: u8 = 18;
const LIGHT_CUE: u8 = 19;
const DEFLECTION_CUE: u8 = 24;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceDamageObservation {
    /// Either downward probe traversed a broadly admitted collider. This
    /// includes rejected polygons/heights, not only a supporting object.
    pub broad_candidate_seen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceDamageError {
    World(WorldInputError),
    Query(SurfaceQueryError),
    Impact(ImpactError),
    Storage(PlayerStorageError),
    MissingSurfaceMode,
    MissingMotion(ObjectId),
    MissingFlightDisplacement(ObjectId),
    MissingProxy,
}

impl From<WorldInputError> for SurfaceDamageError {
    fn from(error: WorldInputError) -> Self {
        Self::World(error)
    }
}
impl From<ImpactError> for SurfaceDamageError {
    fn from(error: ImpactError) -> Self {
        Self::Impact(error)
    }
}
impl From<PlayerStorageError> for SurfaceDamageError {
    fn from(error: PlayerStorageError) -> Self {
        Self::Storage(error)
    }
}

fn probe(
    objects: &mut ObjectStore,
    world: &ScenePathWorld,
    actor: ObjectId,
    preserve_group: bool,
    observation: &mut SurfaceDamageObservation,
) -> Result<ObjectSurfaceContact, SurfaceDamageError> {
    let search = world
        .surface_mode
        .ok_or(SurfaceDamageError::MissingSurfaceMode)?
        .search();
    let query = collision_surface::query_object_surface_geometry(
        objects,
        actor,
        world.strategy_clock as u8,
        search,
    )
    .map_err(SurfaceDamageError::Query)?;
    observation.broad_candidate_seen |= query.broad_candidate_seen;
    let result = query.surface;
    let contact = &mut objects
        .get_mut(actor)
        .expect("queried actor")
        .extension
        .surface_contact;
    contact.supporting_object = result.contact.supporting_object;
    contact.flags = result.contact.flags;
    if !preserve_group {
        contact.group = result.contact.group;
    }
    Ok(result)
}

fn cue(world: &mut ScenePathWorld, owner: ObjectId, cue: u8) {
    let side = if world.primary_player == Some(owner) {
        PlayerTarget::Primary
    } else {
        PlayerTarget::Secondary
    };
    world
        .audio
        .queue(SoundEvent::Authored(AuthoredCue::new(cue, 0, side)));
}

pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<(), SurfaceDamageError> {
    advance_observed(objects, world, resources, owner).map(|_| ())
}

/// Same damage visit with the traversal observation required by the
/// enclosing flight frame. No second query is run to reconstruct it.
pub fn advance_observed(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
) -> Result<SurfaceDamageObservation, SurfaceDamageError> {
    let mut observation = SurfaceDamageObservation::default();
    advance_inner(objects, world, resources, owner, &mut observation)?;
    Ok(observation)
}

fn advance_inner(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    observation: &mut SurfaceDamageObservation,
) -> Result<(), SurfaceDamageError> {
    let surface = probe(objects, world, owner, true, observation)?;
    world.player_surface_height = Some(surface.height);
    let height = objects.get(owner).expect("queried player").base.position.y;
    if surface.contact.flags == NON_DAMAGING_MATERIAL || height.wrapping_sub(surface.height) <= 0 {
        if world
            .scene
            .player_configuration
            .ok_or(WorldInputError::MissingPlayerConfiguration)?
            != CONSTRAINED_CONFIGURATION
        {
            return Ok(());
        }
        let proxy = world
            .weapons
            .as_ref()
            .and_then(|weapons| weapons.fallback)
            .ok_or(SurfaceDamageError::MissingProxy)?;
        let record = world.player(objects, owner)?;
        let displacement = record
            .flight_displacement
            .ok_or(SurfaceDamageError::MissingFlightDisplacement(owner))?;
        let mut forecast = record
            .motion
            .ok_or(SurfaceDamageError::MissingMotion(owner))?
            .previous_position;
        path_motion::integrate(&mut forecast, displacement);
        objects
            .get_mut(proxy)
            .ok_or(WorldInputError::MissingActor(proxy))?
            .base
            .position = forecast;
        let projected = probe(objects, world, proxy, false, observation)?;
        if height.wrapping_sub(projected.height) < 0
            || projected.contact.flags == NON_DAMAGING_MATERIAL
        {
            return Ok(());
        }
        world
            .player_mut(objects, owner)?
            .motion
            .as_mut()
            .expect("validated motion")
            .contact_flags &= !OBSTRUCTION;
        let Some(support) = projected.contact.supporting_object else {
            return Ok(());
        };
        let contact = &mut objects
            .get_mut(owner)
            .expect("queried player")
            .extension
            .surface_contact;
        contact.supporting_object = Some(support);
        contact.flags = projected.contact.flags;
    }

    let protection = world
        .player(objects, owner)?
        .protection
        .ok_or(WorldInputError::MissingPlayerProtection(owner))?;
    if protection.remaining() != 0 {
        if world
            .player(objects, owner)?
            .contact
            .ok_or(WorldInputError::MissingPlayerContact(owner))?
            .hit
            .deflection_sound_due()
        {
            cue(world, owner, DEFLECTION_CUE);
            let random = world.random.next_byte();
            world
                .player_mut(objects, owner)?
                .contact
                .as_mut()
                .expect("validated hit control")
                .hit
                .set_deflection_sound_cooldown(random);
        }
        return Ok(());
    }
    let actor = objects.get(owner).expect("queried player");
    let Some(support) = actor.extension.surface_contact.supporting_object else {
        return Ok(());
    };
    if actor.base.contacts.skip_contacts {
        return Ok(());
    }
    player_impact::turn_from_actor(objects, world, owner, support)?;
    if world
        .scene
        .player_configuration
        .ok_or(WorldInputError::MissingPlayerConfiguration)?
        != CONSTRAINED_CONFIGURATION
    {
        let storage = player_storage::get_mut(objects, resources, owner)?;
        storage.fine_pitch = storage.fine_pitch.wrapping_add(PITCH_IMPACT);
    }
    player_impact::impact(objects, world, owner, Impact::Heavy)?;
    let damage = objects
        .get(support)
        .ok_or(WorldInputError::MissingActor(support))?
        .base
        .attack_power;
    let hit = &mut world
        .player_mut(objects, owner)?
        .contact
        .as_mut()
        .expect("validated hit control")
        .hit;
    if hit.reserve_shield != 0 {
        hit.reserve_shield = health_after_damage(hit.reserve_shield, damage);
    } else {
        objects
            .get_mut(owner)
            .expect("queried player")
            .base
            .hit_points = 0;
    }
    cue(
        world,
        owner,
        if damage >= HEAVY_DAMAGE {
            HEAVY_CUE
        } else {
            LIGHT_CUE
        },
    );
    Ok(())
}
