//! Native Star Fox 2 game implementation.
//!
//! Shipping code is expressed as typed Rust state and systems. The optional
//! `oracle-bridge` feature exposes the old byte-exact compatibility host only
//! for differential verification; it is not part of [`Game`].

mod native;

pub use native::{
    intro_attached_craft, intro_bsp_work, intro_camera, intro_chain, intro_controller,
    intro_destruction, intro_flyby, intro_formation, intro_free_craft, intro_late_target,
    intro_logo, intro_motion, intro_render_work, intro_root, intro_scene,
    intro_second_camera_target, intro_second_flyby, intro_second_flyby_craft,
    intro_second_flyby_scene, intro_second_flyby_wings, intro_target,
};
pub use native::{
    Angle, AnimationState, AstropolisBranch, AstropolisCoreSpike, AstropolisEye, AstropolisEyes,
    AstropolisMissionState, AstropolisPhase, AudioOutput, AudioState, Behavior, Button, Buttons,
    Camera, CampaignObjectives, CampaignPlanetObjectives, CampaignProgress, CampaignState,
    CampaignWorld, CampaignWorldAssignment, CarrierAssaultPhase, CarrierAssaultState,
    CarrierReactorPanel, ChargeSound, CollisionClass, CorneriaDefensePhase, CorneriaDefenseState,
    Difficulty, EladardBarrierStatus, EladardDefenderStatus, EladardGeneratorStatus,
    EladardMissionState, EladardPhase, EndingPhase, EndingState, Error, FlightControlStyle,
    FortunaCoreStatus, FortunaDefenderStatus, FortunaMissionState, FortunaPhase,
    FortunaSwitchStatus, Game, GameMode, GameOverChoice, GameOverDestination, GameOverPhase,
    GameOverState, GameState, InputState, IntroPhase, MacbethCoreStatus, MacbethDefenderStatus,
    MacbethInstallationStatus, MacbethMissionState, MacbethPhase, MacbethSwitchStatus,
    MaterialSetId, MeteorCoreStatus, MeteorMissionState, MeteorPhase, MeteorSwitchStatus,
    MissionMessage, MissionMessageIrisFrame, MissionMessagePhase, MissionMessageState,
    MissionPhase, MissionState, MissionVisit, Object, ObjectFlags, ObjectId, ObjectKind,
    ObjectLifetimeId, ObjectStore, PathCursor, PathId, Pilot, PilotCraftClass, PilotCraftProfile,
    PilotSelectionCursor, PilotSelectionPhase, PilotSelectionState, PlanetObjectiveStatus,
    PlayerBlasterState, PlayerCraftForm, PlayerCraftTransformation,
    PlayerCraftTransformationDirection, PlayerDamageState, PlayerWalkerState, RandomState,
    RecurringAttacker, RecurringAttackerStatus, RecurringAttackersState, RenderFlags, RenderObject,
    ResultsChoice, ResultsPhase, ResultsState, Roster, Rotation, ShapeId, SoundEvent,
    SpatialDistance, SpatialLoop, SpatialSound, StereoPosition, StrategicMapActor,
    StrategicMapActorKind, StrategicMapAppearance, StrategicMapPhase, StrategicMapState,
    StrategicMapTutorialPage, StrategicOpeningPage, StrategicOpeningState,
    TitaniaFinalSwitchStatus, TitaniaMissionState, TitaniaPhase, TitaniaSurfaceSwitchStatus,
    TitleMenuItem, TitlePage, TitleState, Vector3, VenomDefenderStatus, VenomDoorStatus,
    VenomMissionState, VenomPhase, VenomReactorStatus, VenomSwitchStatus, WalkerJumpMotion,
    WalkerJumpState, WalkerMotionProfile, WeaponKind, CAMPAIGN_WORLD_COUNT,
    FORTUNA_MAXIMUM_CORE_DEFENDER_COUNT, FORTUNA_SURFACE_SWITCH_COUNT, MAX_OCCUPIED_WORLD_COUNT,
    OBJECT_CAPACITY, RECURRING_ATTACKER_COUNT, SOUND_EVENT_CAPACITY, STRATEGIC_MAP_ACTOR_CAPACITY,
    VENOM_SURFACE_SWITCH_COUNT,
};
pub use native::{
    BattleCarrierDeployment, CampaignForceCount, DifficultyProfile, OpeningAttackerWavePattern,
    OPENING_ATTACKER_WAVE_CAPACITY,
};

#[cfg(feature = "oracle-bridge")]
#[path = "cpu_bridge.rs"]
mod cpu_bridge;
#[cfg(feature = "oracle-bridge")]
#[path = "map_host.rs"]
mod map_host;
#[cfg(feature = "oracle-bridge")]
#[path = "memory.rs"]
pub mod memory;
#[cfg(feature = "oracle-bridge")]
#[path = "object.rs"]
pub mod object;
#[cfg(feature = "oracle-bridge")]
#[path = "path_host.rs"]
mod path_host;
#[cfg(feature = "oracle-bridge")]
#[path = "strategy.rs"]
mod strategy;

#[cfg(feature = "oracle-bridge")]
pub mod oracle_compat;
