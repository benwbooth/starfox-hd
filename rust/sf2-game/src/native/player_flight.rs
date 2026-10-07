//! Ordered shared flight update ($06:E258..E2ED). Every stage operates on
//! the same live scene and uses the preceding stage's real publications.
//! Input preparation, mode selection and the camera remain caller-owned.

use super::player_ambient::{self, AmbientError};
use super::player_boundary::{self, BoundaryError};
use super::player_impact::{self, ImpactError};
use super::player_motion::{self, MotionContext, MotionError};
use super::player_occupancy::{self, OccupancyContext, OccupancyError};
use super::player_pose::{self, PoseError};
use super::player_roll::{self, RollError};
use super::player_speed::{self, SpeedError};
use super::player_surface::{self, SurfaceContext, SurfaceError, SurfaceResponse};
use super::player_surface_damage::{self, SurfaceDamageError};
use super::player_throttle::{self, ThrottleError};
use super::program_resources::ProgramResources;
use super::program_state::ProgramData;
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::surface_motion::SurfaceMotionResult;
use super::{ObjectId, ObjectStore};

/// Actual values entering the shared flight subroutine. None means the
/// caller has not established an input, not that the original value is zero.
/// Each stage resolves only the fields required by its admitted branch.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FlightContext {
    pub surface: SurfaceContext,
    pub motion: MotionContext,
    pub occupancy: OccupancyContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlightResult {
    pub surface_response: SurfaceResponse,
    pub surface_motion: Option<SurfaceMotionResult>,
    pub diagonal_tie_bias: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlightError {
    Roll(RollError),
    Ambient(AmbientError),
    Throttle(ThrottleError),
    Surface(SurfaceError),
    Speed(SpeedError),
    Pose(PoseError),
    Motion(MotionError),
    Impact(ImpactError),
    SurfaceDamage(SurfaceDamageError),
    Boundary(BoundaryError),
    Occupancy(OccupancyError),
    World(WorldInputError),
}

pub fn advance(
    objects: &mut ObjectStore,
    world: &mut ScenePathWorld,
    resources: &mut ProgramResources<ProgramData>,
    owner: ObjectId,
    mut context: FlightContext,
) -> Result<FlightResult, FlightError> {
    player_roll::advance(objects, world, owner).map_err(FlightError::Roll)?;
    player_ambient::advance(objects, world, owner).map_err(FlightError::Ambient)?;
    player_throttle::advance(objects, world, owner).map_err(FlightError::Throttle)?;
    let surface_response =
        player_surface::respond(objects, world, resources, owner, context.surface)
            .map_err(FlightError::Surface)?;
    player_speed::advance(objects, world, owner, surface_response.into())
        .map_err(FlightError::Speed)?;
    player_pose::compose(objects, world, resources, owner).map_err(FlightError::Pose)?;
    let surface_motion = player_motion::advance(objects, world, resources, owner, context.motion)
        .map_err(FlightError::Motion)?;
    player_impact::advance_recoil(objects, world, owner).map_err(FlightError::Impact)?;
    let damage = player_surface_damage::advance_observed(objects, world, resources, owner)
        .map_err(FlightError::SurfaceDamage)?;
    // The source clears the diagonal arbitration bias on a broadly admitted
    // candidate, before testing its polygons/height. A final support-only
    // check would miss rejected candidates and earlier movement retries.
    if damage.broad_candidate_seen
        || surface_motion.is_some_and(|response| response.broad_candidate_seen)
    {
        context.occupancy.diagonal_tie_bias = Some(0);
    }
    if world
        .reflect_all_contacts
        .ok_or(FlightError::World(WorldInputError::MissingReflectionMode))?
    {
        player_boundary::advance(objects, world, resources, owner)
            .map_err(FlightError::Boundary)?;
        player_occupancy::advance(objects, world, resources, owner, context.occupancy)
            .map_err(FlightError::Occupancy)?;
        // The final $07:E2F2 entry consists only of RTL; it has no game effect.
    }
    Ok(FlightResult {
        surface_response,
        surface_motion,
        diagonal_tie_bias: context.occupancy.diagonal_tie_bias,
    })
}

#[cfg(test)]
#[path = "player_flight_tests.rs"]
mod tests;
