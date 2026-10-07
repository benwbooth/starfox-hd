//! Scene binding for the shared native actor services.
//!
//! One owner composes strategy selection, path invocation, hit/death dispatch,
//! positional sound and complete retirement over the same live stores. Scene-
//! specific native callbacks and map continuations are required services, not
//! ignored defaults. The frame owner still decides when the two strategy
//! portions, collision pass, cleanup and audio publication occur.

use super::collision_contacts::{Contact, ContactHost, ContactId, ContactStore};
use super::collision_pass::{self, CollisionEpochHost, EpochError};
use super::common_destruction::{
    self, DestructionError, DestructionHost, EffectError, EffectInputs, EffectMotion, EffectPhase,
    MapDeathCounts,
};
use super::hit_response::{
    self, HitActor, HitActorMut, HitCallback, HitContext, HitError, HitResponseHost,
};
use super::path_invocation::{InvocationEntry, InvocationError, PathInvocation};
use super::path_program::PathCatalog;
use super::path_runtime::PathRuntimeError;
use super::path_sound::AuthoredCue;
use super::positional_audio::{LoopListener, MissingLoopListener, PositionalAudio};
use super::retirement::{self, RetirementError, RetirementHost};
use super::scene_path_world::{ScenePathWorld, WorldInputError};
use super::scene_proxy::SceneProxyStore;
use super::strategy_schedule::{
    select_strategy, ScheduleError, StrategyAction, StrategyCompletion, StrategyHost,
    StrategyInputs, StrategySchedule,
};
use super::{Behavior, ObjectId, ObjectStore, SoundEvent, Vector3};

/// Scene strategies/death/map services are required. Contact methods default
/// to native auxiliary registrations and the real player handler; alternate
/// providers must preserve the same registration and lifetime contracts.
/// Callback code may mutate the live scene, but must not recycle a currently
/// executing actor or either endpoint of the contact being separated.
pub trait SceneCallbacks: Sized {
    type Error;
    fn assigned(
        host: &mut SceneActors<'_, Self>,
        owner: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error>;
    fn death_override(
        host: &mut SceneActors<'_, Self>,
        owner: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error>;
    fn has_hit_callback(
        host: &SceneActors<'_, Self>,
        owner: ObjectId,
        kind: HitCallback,
    ) -> Result<bool, SceneError<Self::Error>> {
        super::scene_contact::has_callback(host, owner, kind)
    }
    fn hit_callback(
        host: &mut SceneActors<'_, Self>,
        owner: ObjectId,
        _other: ObjectId,
        kind: HitCallback,
        context: &mut HitContext,
    ) -> Result<(), SceneError<Self::Error>> {
        super::scene_contact::hit(host, owner, kind, context)
    }
    fn separation(
        host: &mut SceneActors<'_, Self>,
        contact: ContactId,
        entry: Contact,
    ) -> Result<(), SceneError<Self::Error>> {
        super::scene_contact::separate(host, contact, entry)
    }
    fn resume_map_on_death(
        host: &mut SceneActors<'_, Self>,
        owner: ObjectId,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SceneStrategyControls {
    /// Entry observation used until the full shared mode word is bound.
    /// Authored transitions then override this snapshot for each actor visit.
    pub paused: bool,
    pub excluded_actor: Option<ObjectId>,
    pub positional_suppressed: bool,
    pub loop_listener: Option<LoopListener>,
    pub death_effects: Option<EffectInputs>,
}

/// Shared invocation state belongs to the scene, not to an individual actor.
/// A failed world service latches the scene at the diagnostic boundary: an
/// outer retry must not apply contact damage, callbacks or allocations twice.
#[derive(Debug, Default)]
pub struct SceneExecution {
    pub paths: PathInvocation,
    pub hit_context: HitContext,
    pub positional: PositionalAudio,
    pub controls: SceneStrategyControls,
    pub map_counts: Option<MapDeathCounts>,
    faulted: bool,
    retire_immediately: bool,
}

impl SceneExecution {
    pub fn is_faulted(&self) -> bool {
        self.faulted
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneError<E> {
    Faulted,
    MissingActor(ObjectId),
    MissingPrimaryPlayer,
    MissingDeathInputs,
    MissingMapCounts,
    MissingContactCallback(ObjectId),
    Auxiliary(super::actor_auxiliary::AuxiliaryError),
    Reflection(super::weapon_reflection::ReflectionError),
    Charge(super::player_charge::ChargeError),
    Rapid(super::player_rapid::RapidError),
    WeaponAim(super::player_weapon_aim::WeaponAimError),
    PlayerAction(super::player_action::PlayerActionError),
    Consumable(super::player_consumable::ConsumableError),
    PlayerVisit(super::player_visit::PlayerVisitError),
    PlayerStorage(super::player_storage::PlayerStorageError),
    PlayerSceneReset(super::player_scene_reset::PlayerSceneResetError),
    TargetLock(super::player_target_lock::TargetLockError),
    ReticlePosition(super::player_target_lock::ReticlePositionError),
    Reticle(super::player_reticle::ReticleError),
    PlayerInput(super::player_input::PlayerInputError),
    PlayerRoll(super::player_roll::RollError),
    PlayerPose(super::player_pose::PoseError),
    PlayerSteering(super::player_steering::SteeringError),
    PlayerVertical(super::player_vertical::VerticalError),
    PlayerThrottle(super::player_throttle::ThrottleError),
    PlayerAmbient(super::player_ambient::AmbientError),
    PlayerSurfaceParticle(super::player_surface_particle::ParticleError),
    PlayerSurfaceSplash(super::player_surface_splash::SplashError),
    PlayerSurfaceRender(super::player_surface_render::SurfaceRenderError),
    PlayerAppearance(super::player_appearance::AppearanceError),
    PlayerSurfaceEffect(super::player_surface_effect::SurfaceEffectError),
    PlayerSurface(super::player_surface::SurfaceError),
    PlayerSpeed(super::player_speed::SpeedError),
    PlayerMotion(super::player_motion::MotionError),
    PlayerImpact(super::player_impact::ImpactError),
    PlayerSurfaceDamage(super::player_surface_damage::SurfaceDamageError),
    PlayerBoundary(super::player_boundary::BoundaryError),
    PlayerOccupancy(super::player_occupancy::OccupancyError),
    PlayerFlight(super::player_flight::FlightError),
    PlayerFlightMode(super::player_flight_mode::FlightModeError),
    PlayerFreeFlight(super::player_free_flight::FreeFlightError),
    ViewBlend(super::view_blend::ViewBlendError),
    PlayerViewDistance(super::player_view_distance::ViewDistanceError),
    PlayerCameraAngles(super::player_camera_angles::CameraAnglesError),
    PlayerCameraTracking(super::player_camera_tracking::CameraTrackingError),
    PlayerCameraPosition(super::player_camera_position::CameraPositionError),
    PlayerCameraGround(super::player_camera_ground::GroundCameraError),
    PlayerCameraCommon(super::player_camera_common::CommonCameraError),
    PlayerCameraSurface(super::player_camera_surface::SurfaceCameraError),
    PlayerCameraAuxiliary(super::player_camera_auxiliary::AuxiliaryCameraError),
    PlayerCameraDispatch(super::player_camera_dispatch::CameraDispatchError),
    PlayerStatus(super::player_status::StatusError),
    PlayerEngineSound(super::player_engine_sound::EngineSoundError),
    PlayerSurfacePreparation(super::player_surface_prepare::SurfacePreparationError),
    PlayerModeSelection(super::player_mode_selection::ModeSelectionError),
    SurfaceMotion(super::surface_motion::SurfaceMotionError),
    Recovery(super::player_recovery::RecoveryError),
    PlayerContact(Box<super::player_contact::PlayerContactError<SceneError<E>>>),
    NestedPathInvocation,
    World(WorldInputError),
    Path(InvocationError<WorldInputError>),
    Runtime(PathRuntimeError),
    Effect(EffectError),
    Positional(MissingLoopListener),
    Callbacks(E),
    Hit(Box<HitError<SceneError<E>>>),
    Destruction(Box<DestructionError<SceneError<E>>>),
    Retirement(Box<RetirementError<SceneError<E>>>),
    Epoch(Box<EpochError<SceneError<E>>>),
}

/// Short-lived borrow of a real scene. Objects, resources, contacts and player
/// records are never copied into a second strategy-owned object collection.
pub struct SceneActors<'a, C: SceneCallbacks> {
    pub objects: &'a mut ObjectStore,
    pub world: &'a mut ScenePathWorld,
    pub execution: &'a mut SceneExecution,
    pub catalog: &'a PathCatalog,
    pub callbacks: &'a mut C,
    /// Diagnostic statement bound, not a movement count or frame budget.
    pub statement_budget: usize,
}

impl<C: SceneCallbacks> SceneActors<'_, C> {
    pub fn advance_player_engine_sound(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_engine_sound::advance(self.objects, self.world, owner)
            .map_err(SceneError::PlayerEngineSound);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_shield_status(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_status::advance_shield(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
        )
        .map_err(SceneError::PlayerStatus);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_status_filters(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_status::advance_filters(self.objects, self.world, owner)
            .map_err(SceneError::PlayerStatus);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_transform_cues(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_status::advance_transform_cues(self.objects, self.world, owner)
            .map_err(SceneError::PlayerStatus);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn select_player_free_flight_camera(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_dispatch::select_free_flight_camera(
            self.objects,
            self.world,
            owner,
        )
        .map_err(SceneError::PlayerCameraDispatch);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_dispatch::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime,
            owner,
        )
        .map_err(SceneError::PlayerCameraDispatch);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_view(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_dispatch::advance_with_continuity(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime,
            owner,
        )
        .map_err(SceneError::PlayerCameraDispatch);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn clamp_player_view_to_plane(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result =
            super::player_camera_dispatch::clamp_normal_to_plane(self.objects, self.world, owner)
                .map_err(SceneError::PlayerCameraDispatch);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn install_player_camera_auxiliary(
        &mut self,
        owner: ObjectId,
        task: super::player_camera_auxiliary::AuxiliaryCameraTask,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_auxiliary::install(self.objects, self.world, owner, task)
            .map_err(SceneError::PlayerCameraAuxiliary);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera_auxiliary(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_auxiliary::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime,
            owner,
        )
        .map_err(SceneError::PlayerCameraAuxiliary);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera_surface_height(
        &mut self,
        owner: ObjectId,
        style: super::player_camera_tracking::TrackingStyle,
        auxiliary_camera: bool,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_surface::advance_height(
            self.objects,
            self.world,
            owner,
            style,
            auxiliary_camera,
        )
        .map_err(SceneError::PlayerCameraSurface);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn prepare_player_camera_surface(
        &mut self,
        owner: ObjectId,
        style: super::player_camera_tracking::TrackingStyle,
        auxiliary_camera: bool,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_surface::prepare(
            self.objects,
            self.world,
            &self.execution.paths.runtime,
            owner,
            style,
            auxiliary_camera,
        )
        .map_err(SceneError::PlayerCameraSurface);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera_ground(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_ground::advance_pitch(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime,
            owner,
        )
        .map_err(SceneError::PlayerCameraGround);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera_orientation(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_common::advance_yaw_roll(
            self.objects,
            self.world,
            &self.execution.paths.runtime,
            owner,
        )
        .map_err(SceneError::PlayerCameraCommon);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera_common(
        &mut self,
        owner: ObjectId,
        style: super::player_camera_tracking::TrackingStyle,
        auxiliary_camera: bool,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_common::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime,
            owner,
            style,
            auxiliary_camera,
        )
        .map_err(SceneError::PlayerCameraCommon);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn prepare_player_camera_position(
        &mut self,
        owner: ObjectId,
        style: super::player_camera_tracking::TrackingStyle,
        auxiliary_camera: bool,
    ) -> Result<super::Vector3, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_position::prepare(
            self.objects,
            self.world,
            owner,
            style,
            auxiliary_camera,
        )
        .map_err(SceneError::PlayerCameraPosition);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera_lateral(
        &mut self,
        owner: ObjectId,
        prepared: super::Vector3,
    ) -> Result<super::Vector3, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_position::advance_lateral(
            self.objects,
            self.world,
            owner,
            prepared,
        )
        .map_err(SceneError::PlayerCameraPosition);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera_distance(
        &mut self,
        owner: ObjectId,
        prepared: super::Vector3,
        style: super::player_camera_tracking::TrackingStyle,
    ) -> Result<super::Vector3, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_position::advance_distance(
            self.objects,
            self.world,
            owner,
            prepared,
            style,
        )
        .map_err(SceneError::PlayerCameraPosition);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera_boost(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_position::advance_boost(self.objects, self.world, owner)
            .map_err(SceneError::PlayerCameraPosition);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera_height(
        &mut self,
        owner: ObjectId,
        prepared_height: i16,
        style: super::player_camera_tracking::TrackingStyle,
        auxiliary_camera: bool,
    ) -> Result<i16, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_tracking::advance_height(
            self.objects,
            self.world,
            owner,
            prepared_height,
            style,
            auxiliary_camera,
        )
        .map_err(SceneError::PlayerCameraTracking);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_camera_pitch(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_angles::advance_pitch(self.objects, self.world, owner)
            .map_err(SceneError::PlayerCameraAngles);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn publish_player_camera_pose(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_camera_angles::publish(self.objects, self.world, owner)
            .map_err(SceneError::PlayerCameraAngles);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn initialize_player_view_distance(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_view_distance::initialize(self.objects, self.world, owner)
            .map_err(SceneError::PlayerViewDistance);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_view_distance(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_view_distance::advance(self.objects, self.world, owner)
            .map_err(SceneError::PlayerViewDistance);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_view_blend(&mut self) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result =
            super::view_blend::advance(self.objects, self.world).map_err(SceneError::ViewBlend);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_free_flight(
        &mut self,
        owner: ObjectId,
        context: super::player_free_flight::FreeFlightContext,
    ) -> Result<super::player_flight::FlightResult, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_free_flight::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
            context,
        )
        .map_err(SceneError::PlayerFreeFlight);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn spawn_player_surface_effect(
        &mut self,
        owner: ObjectId,
        input: super::player_surface_effect::SurfaceEffectInputs,
    ) -> Result<Option<super::player_surface_effect::SurfaceEffectResult>, SceneError<C::Error>>
    {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_surface_effect::spawn(self.objects, self.world, owner, input)
            .map_err(SceneError::PlayerSurfaceEffect);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn select_player_mode(
        &mut self,
        owner: ObjectId,
        request: super::player_mode_selection::ModeRequest,
    ) -> Result<bool, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result =
            super::player_mode_selection::advance(self.objects, self.world, owner, request)
                .map_err(SceneError::PlayerModeSelection);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn prepare_player_surface(
        &mut self,
        owner: ObjectId,
    ) -> Result<Option<i16>, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_surface_prepare::prepare(self.objects, self.world, owner)
            .map_err(SceneError::PlayerSurfacePreparation);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_retained_pitch_flight(
        &mut self,
        owner: ObjectId,
        context: super::player_flight_mode::FlightModeContext,
    ) -> Result<super::player_flight::FlightResult, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_flight_mode::advance_retained_pitch(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
            context,
        )
        .map_err(SceneError::PlayerFlightMode);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_flight(
        &mut self,
        owner: ObjectId,
        context: super::player_flight::FlightContext,
    ) -> Result<super::player_flight::FlightResult, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_flight::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
            context,
        )
        .map_err(SceneError::PlayerFlight);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_occupancy(
        &mut self,
        owner: ObjectId,
        context: super::player_occupancy::OccupancyContext,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_occupancy::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
            context,
        )
        .map_err(SceneError::PlayerOccupancy);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_corridor(
        &mut self,
        owner: ObjectId,
    ) -> Result<bool, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_boundary::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
        )
        .map_err(SceneError::PlayerBoundary);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn install_player_corridor(
        &mut self,
        anchor: ObjectId,
        owner: ObjectId,
        inputs: super::player_boundary::RegionInputs,
    ) -> Result<bool, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result =
            super::player_boundary::install_region(self.objects, self.world, anchor, owner, inputs)
                .map_err(SceneError::PlayerBoundary);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_surface_damage(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_surface_damage::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
        )
        .map_err(SceneError::PlayerSurfaceDamage);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn advance_player_recoil(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_impact::advance_recoil(self.objects, self.world, owner)
            .map_err(SceneError::PlayerImpact);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Full flight translation after pose; the caller retains any surface tilt.
    pub fn advance_player_motion(
        &mut self,
        owner: ObjectId,
        context: super::player_motion::MotionContext,
    ) -> Result<Option<super::surface_motion::SurfaceMotionResult>, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_motion::advance(
            self.objects,
            self.world,
            &self.execution.paths.runtime.resources,
            owner,
            context,
        )
        .map_err(SceneError::PlayerMotion);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Shared constrained movement, with the caller's real gravity mode and tilt.
    pub fn advance_surface_motion(
        &mut self,
        owner: ObjectId,
        inputs: super::surface_motion::SurfaceMotionInputs,
    ) -> Result<super::surface_motion::SurfaceMotionResult, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::surface_motion::advance(self.objects, owner, inputs)
            .map_err(SceneError::SurfaceMotion);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Speed follows surface response and precedes pose and displacement.
    pub fn advance_player_speed(
        &mut self,
        owner: ObjectId,
        context: super::player_speed::SpeedContext,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_speed::advance(self.objects, self.world, owner, context)
            .map_err(SceneError::PlayerSpeed);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Retained ambient terms, after roll input and before boost/brake.
    pub fn advance_player_ambient(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_ambient::advance(self.objects, self.world, owner)
            .map_err(SceneError::PlayerAmbient);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Boost/brake service after roll/ambient and before surface/speed work.
    pub fn advance_player_throttle(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_throttle::advance(self.objects, self.world, owner)
            .map_err(SceneError::PlayerThrottle);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Map configuration targets the actual primary player ($06:9A2F).
    pub fn configure_player_vertical(
        &mut self,
        profile: super::player_vertical::VerticalProfile,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = self
            .world
            .primary_player
            .ok_or(SceneError::MissingPrimaryPlayer)
            .and_then(|owner| {
                super::player_vertical::configure(self.objects, self.world, owner, profile)
                    .map_err(SceneError::PlayerVertical)
            });
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Retain input history before horizontal control, not before each pitch
    /// helper. Otherwise an intervening history clear would be lost.
    pub fn retain_player_input(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_vertical::retain_input(self.objects, self.world, owner)
            .map_err(SceneError::PlayerVertical);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// The source-adjacent pitch and terrain pair, after horizontal control.
    pub fn advance_player_vertical(
        &mut self,
        owner: ObjectId,
        mode: super::player_vertical::VerticalMode,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_vertical::advance(
            self.objects,
            self.world,
            &self.execution.paths.runtime.resources,
            owner,
            mode,
        )
        .map_err(SceneError::PlayerVertical);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Horizontal control runs between shoulder selection and roll/pose.
    pub fn advance_player_steering(
        &mut self,
        owner: ObjectId,
        context: super::player_steering::SteeringContext,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_steering::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
            context,
        )
        .map_err(SceneError::PlayerSteering);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Compose the live player orientation after steering, roll and speed.
    /// This is not the enclosing mode: motion and camera follow separately.
    pub fn compose_player_pose(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_pose::compose(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
        )
        .map_err(SceneError::PlayerPose);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// The mode runs this after input preparation and before steering.
    pub fn prepare_player_shoulders(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_roll::prepare_shoulders(self.objects, self.world, owner)
            .map_err(SceneError::PlayerRoll);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Later movement service: publish projectile protection, then advance
    /// double-tap/roll state. Steering is a distinct intervening service.
    pub fn advance_player_roll(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_roll::advance(self.objects, self.world, owner)
            .map_err(SceneError::PlayerRoll);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Reset the scene publications owned by the currently ported player
    /// services. Remaining enclosing initialization is explicitly separate.
    pub fn reset_player_services(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_scene_reset::reset_services(self.objects, self.world, owner)
            .map_err(SceneError::PlayerSceneReset);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Full shared storage/formatter entry. This does not run the enclosing
    /// player strategy's global reset or first movement-mode visit.
    pub fn initialize_player_storage(
        &mut self,
        owner: ObjectId,
        inputs: super::player_storage::PlayerStorageInputs,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        if self.execution.paths.is_active() {
            self.execution.faulted = true;
            return Err(SceneError::NestedPathInvocation);
        }
        let result = super::player_storage::initialize(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime,
            owner,
            inputs,
        )
        .map_err(SceneError::PlayerStorage);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Publish processed controller/script input at the enclosing mode's
    /// source-ordered callsite. This is not implicit in the earlier player
    /// prefix: that prefix's action service observes the prior publication.
    pub fn prepare_player_input(
        &mut self,
        owner: ObjectId,
    ) -> Result<super::InputState, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_input::prepare(self.objects, self.world, owner)
            .map_err(SceneError::PlayerInput);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Target reset after the enclosing initializer's shared-state reset.
    /// Its source order is deliberately separate from storage replacement.
    pub fn initialize_player_target(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::path_target::initialize_player(self.objects, self.world, owner)
            .map_err(SceneError::World);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Display-service boundary, after positioning the shared reticle and
    /// before drawing its marker. Never invoked implicitly by actor strategy
    /// traversal: the enclosing display owner decides when it runs.
    pub fn retain_primary_target(&mut self) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_target_lock::update(self.objects, self.world)
            .map_err(SceneError::TargetLock);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Reticle positioning tail after the actual point projector completes.
    /// The display caller supplies that result and owns update ordering.
    pub fn track_target_reticle(
        &mut self,
        projected: [i16; 2],
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = self
            .world
            .target_reticle
            .track_projected(projected)
            .map_err(SceneError::ReticlePosition);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Player-mode service ($07:B038), separate from display positioning.
    pub fn prepare_player_reticle(&mut self, owner: ObjectId) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_reticle::prepare(self.objects, self.world, owner)
            .map_err(SceneError::Reticle);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Continuous display slice ($07:A418..A66B), including real projection,
    /// reticle easing and target retention. The caller owns display cadence
    /// and must publish the earlier retained matrix/viewport. Marker drawing
    /// and the display service's preceding instrument work remain separate.
    pub fn position_and_retain_primary_target(&mut self) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = (|| {
            let owner = self
                .world
                .primary_player
                .ok_or(SceneError::MissingPrimaryPlayer)?;
            super::player_reticle::position(self.objects, self.world, owner)
                .map_err(SceneError::Reticle)?;
            self.retain_primary_target()
        })();
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Real player-record allocation and publication prefix. The enclosing
    /// scene initializer still owns view selection, formatting and globals.
    pub fn replace_player_storage(
        &mut self,
        owner: ObjectId,
        inputs: super::player_storage::PlayerStorageInputs,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        if self.execution.paths.is_active() {
            self.execution.faulted = true;
            return Err(SceneError::NestedPathInvocation);
        }
        let result = super::player_storage::replace(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime,
            owner,
            inputs,
        )
        .map_err(SceneError::PlayerStorage);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
    /// Consume the actual shared recovery request and install visual feedback.
    /// The surrounding player mode owns this service's position in the visit.
    pub fn consume_player_recovery(
        &mut self,
        owner: ObjectId,
    ) -> Result<bool, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_recovery::consume(self.objects, self.world, owner)
            .map_err(SceneError::Recovery);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
    /// Complete ordered prefix before the source player-mode dispatcher.
    /// This does not substitute for that mode's movement/weapon strategy.
    pub fn begin_player_visit(
        &mut self,
        owner: ObjectId,
        input: super::InputState,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_visit::begin(self.objects, self.world, owner, input)
            .map_err(SceneError::PlayerVisit);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
    /// Dispatch one admitted consumable request, after the outer caller has
    /// handled input delay, child gates and placement.
    pub fn use_player_consumable(&mut self, owner: ObjectId) -> Result<bool, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_consumable::use_item(self.objects, self.world, owner)
            .map_err(SceneError::Consumable);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
    /// Parallel action stream, independently scheduled by player control.
    pub fn advance_player_action(
        &mut self,
        owner: ObjectId,
        input: super::InputState,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_action::advance(self.objects, self.world, owner, input)
            .map_err(SceneError::PlayerAction);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
    /// Independently scheduled forward-point update for future rapid fire.
    pub fn retain_player_weapon_aim(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result =
            super::player_weapon_aim::retain_forward_point(self.objects, self.world, owner)
                .map_err(SceneError::WeaponAim);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
    /// Beginning of the weapon service, before consumables and rapid fire.
    pub fn publish_player_weapon_aim(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_weapon_aim::publish(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.steering,
            owner,
        )
        .map_err(SceneError::WeaponAim);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
    /// Rapid tail of the player weapon service. Aiming and consumable work
    /// precede this call; the charged-fire service follows it.
    pub fn advance_player_rapid(
        &mut self,
        owner: ObjectId,
        input: super::InputState,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_rapid::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
            input,
        )
        .map_err(SceneError::Rapid);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
    /// Called at the charged-fire point of the real player strategy. The
    /// caller supplies its processed inputs and retains frame ownership.
    pub fn advance_player_charge(
        &mut self,
        owner: ObjectId,
        input: super::InputState,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_charge::advance(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
            input,
        )
        .map_err(SceneError::Charge);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
    pub fn respond_player_surface(
        &mut self,
        owner: ObjectId,
        context: super::player_surface::SurfaceContext,
    ) -> Result<super::player_surface::SurfaceResponse, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_surface::respond(
            self.objects,
            self.world,
            &mut self.execution.paths.runtime.resources,
            owner,
            context,
        )
        .map_err(SceneError::PlayerSurface);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Surface effects allocate/format here, but their independent strategies
    /// first run when the live actor scheduler reaches them.
    pub fn spawn_player_surface_particle(
        &mut self,
        owner: ObjectId,
        kind: super::player_surface_particle::SurfaceParticle,
        input: super::player_surface_particle::ParticleInputs,
    ) -> Result<ObjectId, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result =
            super::player_surface_particle::spawn(self.objects, self.world, owner, kind, input)
                .map_err(SceneError::PlayerSurfaceParticle);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn spawn_player_surface_splash(
        &mut self,
        owner: ObjectId,
        kind: super::player_surface_splash::SurfaceSplash,
        input: super::player_surface_splash::SplashInputs,
    ) -> Result<ObjectId, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result =
            super::player_surface_splash::spawn(self.objects, self.world, owner, kind, input)
                .map_err(SceneError::PlayerSurfaceSplash);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn update_player_appearance(
        &mut self,
        owner: ObjectId,
        child_number: u8,
    ) -> Result<Option<ObjectId>, SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result =
            super::player_appearance::update(self.objects, self.world, owner, child_number)
                .map_err(SceneError::PlayerAppearance);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn update_player_surface_depth(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result =
            super::player_appearance::update_surface_depth(self.objects, self.world, owner)
                .map_err(SceneError::PlayerAppearance);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn publish_player_depth(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_appearance::publish_depth(self.objects, self.world, owner)
            .map_err(SceneError::PlayerAppearance);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn publish_player_surface_environment(
        &mut self,
        owner: ObjectId,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_surface_render::publish_environment(
            self.objects,
            self.world,
            owner,
        )
        .map_err(SceneError::PlayerSurfaceRender);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    pub fn replace_player_surface_palette(
        &mut self,
        side: super::player_surface_render::SurfaceViewSide,
    ) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = super::player_surface_render::replace_polygon_palette(self.world, side)
            .map_err(SceneError::PlayerSurfaceRender);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    /// Publish the shared clock even for an empty or entirely suspended pass.
    /// Render readiness and positional-accumulator reset are separate owners.
    pub fn begin_strategy_epoch(
        &mut self,
        schedule: &mut StrategySchedule,
    ) -> Result<(), ScheduleError<SceneError<C::Error>>> {
        if self.execution.faulted {
            return Err(ScheduleError::Host(SceneError::Faulted));
        }
        schedule.begin(self.objects)?;
        self.world.strategy_clock = schedule.clock();
        Ok(())
    }

    /// Source contact/deferred-retirement pass. Its placement relative to
    /// strategy/render/collision work is supplied by the frame owner.
    pub fn clean_epoch(&mut self) -> Result<(), SceneError<C::Error>> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result =
            collision_pass::clean_epoch(self).map_err(|error| SceneError::Epoch(Box::new(error)));
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }

    fn assigned(&mut self, owner: ObjectId) -> Result<ObjectId, SceneError<C::Error>> {
        let behavior = self
            .objects
            .get(owner)
            .ok_or(SceneError::MissingActor(owner))?
            .base
            .behavior;
        match behavior {
            Behavior::Unassigned => Ok(owner),
            Behavior::FollowPath | Behavior::PathMovement => {
                let entry = if behavior == Behavior::PathMovement {
                    InvocationEntry::Movement
                } else {
                    InvocationEntry::Program
                };
                self.execution
                    .paths
                    .begin(owner, entry)
                    .map_err(|_| SceneError::NestedPathInvocation)?;
                self.execution
                    .paths
                    .resume(
                        self.catalog,
                        self.objects,
                        self.world,
                        self.statement_budget,
                    )
                    .map_err(SceneError::Path)
            }
            Behavior::ImpactBurst(_) => {
                super::path_effect::step(self.objects.get_mut(owner).expect("live impact actor"))
                    .expect("validated impact behavior");
                Ok(owner)
            }
            Behavior::SurfaceParticle(_) => {
                super::player_surface_particle::step(
                    self.objects.get_mut(owner).expect("live surface particle"),
                )
                .expect("validated surface particle behavior");
                Ok(owner)
            }
            Behavior::SurfaceSplash(_) => {
                super::player_surface_splash::step(self.objects, owner)
                    .map_err(SceneError::PlayerSurfaceSplash)?;
                Ok(owner)
            }
            Behavior::SurfaceEffect(_) => {
                super::player_surface_effect::step(self.objects, self.world, owner)
                    .map_err(SceneError::PlayerSurfaceEffect)?;
                Ok(owner)
            }
            Behavior::Destruction(phase) => {
                let motion = if phase == EffectPhase::Animate {
                    let primary = self
                        .world
                        .primary_player
                        .ok_or(SceneError::MissingPrimaryPlayer)?;
                    let record = self
                        .world
                        .player_mut(self.objects, primary)
                        .map_err(SceneError::World)?;
                    let mode = record
                        .auxiliary
                        .ok_or(SceneError::World(WorldInputError::MissingAuxiliary(
                            primary,
                        )))?
                        .mode;
                    let displacement = if mode & 0xF0 == 0x10 {
                        self.world
                            .published_motion
                            .ok_or(SceneError::World(WorldInputError::MissingPublishedMotion))?
                            .delta
                    } else {
                        Vector3::default()
                    };
                    Some(EffectMotion {
                        primary_mode: mode,
                        displacement,
                    })
                } else {
                    None
                };
                common_destruction::step_effect(
                    self.objects.get_mut(owner).expect("live death effect"),
                    motion,
                )
                .map_err(SceneError::Effect)?;
                Ok(owner)
            }
            _ => {
                let result = C::assigned(self, owner).map_err(SceneError::Callbacks)?;
                self.execution.retire_immediately |= result.retire_now;
                Ok(result.actor)
            }
        }
    }

    fn strategy(
        &mut self,
        owner: ObjectId,
        clock: u16,
    ) -> Result<StrategyCompletion, SceneError<C::Error>> {
        self.world.strategy_clock = clock;
        self.execution.retire_immediately = false;
        let actor = self
            .objects
            .get(owner)
            .ok_or(SceneError::MissingActor(owner))?;
        let decision = select_strategy(
            StrategyInputs {
                health: actor.base.hit_points,
                suspended: actor.base.flags.strategy_suspended,
                excluded_actor: self.execution.controls.excluded_actor == Some(owner),
                first_visit: actor.base.contacts.first_strategy_visit,
                hit_pending: actor.base.contacts.pending_hit,
                run_when_paused: actor.base.contacts.run_when_paused,
                has_assigned_strategy: actor.base.behavior != Behavior::Unassigned,
            },
            self.world
                .view_transition_mode
                .map_or(self.execution.controls.paused, |mode| mode.active()),
        );
        if decision.clear_first_visit {
            self.objects
                .get_mut(owner)
                .expect("live strategy actor")
                .base
                .contacts
                .first_strategy_visit = false;
        }
        let returned = match decision.action {
            StrategyAction::Skip => owner,
            StrategyAction::Assigned => self.assigned(owner)?,
            StrategyAction::CommonDestruction => common_destruction::destroy(self, owner)
                .map_err(|error| SceneError::Destruction(Box::new(error)))?,
            StrategyAction::HitResponse => {
                // Retain the shared callback context even on a terminal fault;
                // no copyback of actor health or flags spans a callback.
                let mut context = std::mem::take(&mut self.execution.hit_context);
                let result = hit_response::respond(self, owner, &mut context);
                self.execution.hit_context = context;
                result.map_err(|error| SceneError::Hit(Box::new(error)))?
            }
        };
        if decision.service_positional_sound {
            let actor = self
                .objects
                .get(returned)
                .ok_or(SceneError::MissingActor(returned))?;
            self.execution
                .positional
                .observe(
                    returned,
                    actor,
                    self.execution.controls.positional_suppressed,
                    self.execution.controls.loop_listener,
                )
                .map_err(SceneError::Positional)?;
        }
        Ok(StrategyCompletion {
            actor: returned,
            retire_now: self.execution.retire_immediately,
        })
    }
}

impl<C: SceneCallbacks> StrategyHost for SceneActors<'_, C> {
    type Error = SceneError<C::Error>;
    fn objects(&self) -> &ObjectStore {
        self.objects
    }
    fn strategy_suspended(&self, _: ObjectId) -> bool {
        false
    }
    fn run_strategy(
        &mut self,
        owner: ObjectId,
        clock: u16,
    ) -> Result<StrategyCompletion, Self::Error> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = self.strategy(owner, clock);
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
    fn retire_object(&mut self, owner: ObjectId) -> Result<(), Self::Error> {
        if self.execution.faulted {
            return Err(SceneError::Faulted);
        }
        let result = retirement::retire(self, owner)
            .map(|_| ())
            .map_err(|error| SceneError::Retirement(Box::new(error)));
        if result.is_err() {
            self.execution.faulted = true;
        }
        result
    }
}

impl<C: SceneCallbacks> ContactHost for SceneActors<'_, C> {
    type Error = SceneError<C::Error>;
    fn contacts(&self) -> &ContactStore {
        &self.world.contacts
    }
    fn contacts_mut(&mut self) -> &mut ContactStore {
        &mut self.world.contacts
    }
    fn on_separation(&mut self, id: ContactId, contact: Contact) -> Result<(), Self::Error> {
        C::separation(self, id, contact)
    }
}

impl<C: SceneCallbacks> HitResponseHost for SceneActors<'_, C> {
    fn hit_actor(&self, owner: ObjectId) -> Option<HitActor> {
        self.objects.get(owner).map(HitActor::from_object)
    }
    fn hit_actor_mut(&mut self, owner: ObjectId) -> Option<HitActorMut<'_>> {
        self.objects.get_mut(owner).map(HitActorMut::from_object)
    }
    fn has_hit_callback(&self, owner: ObjectId, kind: HitCallback) -> Result<bool, Self::Error> {
        C::has_hit_callback(self, owner, kind)
    }
    fn run_hit_callback(
        &mut self,
        owner: ObjectId,
        other: ObjectId,
        kind: HitCallback,
        context: &mut HitContext,
    ) -> Result<(), Self::Error> {
        C::hit_callback(self, owner, other, kind, context)
    }
    fn strategies_paused(&self) -> bool {
        self.execution.controls.paused
    }
    fn run_assigned_strategy(&mut self, owner: ObjectId) -> Result<ObjectId, Self::Error> {
        self.assigned(owner)
    }
}

impl<C: SceneCallbacks> DestructionHost for SceneActors<'_, C> {
    type Error = SceneError<C::Error>;
    fn objects(&self) -> &ObjectStore {
        self.objects
    }
    fn objects_mut(&mut self) -> &mut ObjectStore {
        self.objects
    }
    fn objects_and_proxies_mut(&mut self) -> (&mut ObjectStore, &mut SceneProxyStore) {
        (self.objects, &mut self.world.proxies)
    }
    fn run_death_override(&mut self, owner: ObjectId) -> Result<Option<ObjectId>, Self::Error> {
        C::death_override(self, owner)
            .map(|result| {
                result.map(|result| {
                    self.execution.retire_immediately |= result.retire_now;
                    result.actor
                })
            })
            .map_err(SceneError::Callbacks)
    }
    fn map_death_counts(&mut self) -> Result<&mut MapDeathCounts, Self::Error> {
        self.execution
            .map_counts
            .as_mut()
            .ok_or(SceneError::MissingMapCounts)
    }
    fn resume_map_on_death(&mut self, owner: ObjectId) -> Result<(), Self::Error> {
        C::resume_map_on_death(self, owner).map_err(SceneError::Callbacks)
    }
    fn effect_inputs(&mut self) -> Result<EffectInputs, Self::Error> {
        self.execution
            .controls
            .death_effects
            .ok_or(SceneError::MissingDeathInputs)
    }
    fn queue_death_sound(&mut self, cue: AuthoredCue) -> Result<(), Self::Error> {
        self.world.audio.queue(SoundEvent::Authored(cue));
        Ok(())
    }
}

impl<C: SceneCallbacks> RetirementHost for SceneActors<'_, C> {
    fn objects(&self) -> &ObjectStore {
        self.objects
    }
    fn objects_and_proxies_mut(&mut self) -> (&mut ObjectStore, &mut SceneProxyStore) {
        (self.objects, &mut self.world.proxies)
    }
    fn release_actor_programs(&mut self, owner: ObjectId) -> Result<(), Self::Error> {
        self.execution
            .paths
            .runtime
            .release_actor_programs(self.objects, owner)
            .map_err(SceneError::Runtime)?;
        self.world.release_player_bindings(owner);
        Ok(())
    }
}

impl<C: SceneCallbacks> CollisionEpochHost for SceneActors<'_, C> {
    fn objects(&self) -> &ObjectStore {
        self.objects
    }
    fn objects_mut(&mut self) -> &mut ObjectStore {
        self.objects
    }
    fn retire_object(&mut self, actor: ObjectId) -> Result<(), Self::Error> {
        StrategyHost::retire_object(self, actor)
    }
}

#[cfg(test)]
#[path = "scene_strategy_tests.rs"]
mod tests;
