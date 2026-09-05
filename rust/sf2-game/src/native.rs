//! Typed Star Fox 2 runtime.
//!
//! The native runtime deliberately has no byte-addressed state container.
//! Source-machine encodings are decoded at data boundaries and game systems
//! operate on the domain structs exported from this module.

mod game;
mod input;
pub mod intro_camera;
pub mod intro_controller;
pub mod intro_destruction;
pub mod intro_flyby;
pub mod intro_free_craft;
pub mod intro_late_target;
pub mod intro_logo;
pub mod intro_motion;
pub mod intro_root;
pub mod intro_target;
mod object;
mod render;
mod results;
mod state;

mod astropolis_assault;
mod campaign_major_objectives;
mod campaign_world_assignments;

pub use campaign_world_assignments::{
    CampaignWorld, CAMPAIGN_WORLD_COUNT, MAX_OCCUPIED_WORLD_COUNT,
};
pub use game::{Error, Game};
pub use input::{Button, Buttons, InputState};
pub use object::{
    Angle, Behavior, CollisionClass, Object, ObjectFlags, ObjectId, ObjectKind, ObjectLifetimeId,
    ObjectStore, PathCursor, PathId, ShapeId, SpatialDistance, SpatialLoop, SpatialSound,
    StereoPosition, Vector3, WeaponKind, OBJECT_CAPACITY,
};
pub use render::{AnimationState, Camera, MaterialSetId, RenderFlags, RenderObject, Rotation};
pub use state::{
    AstropolisBranch, AstropolisCoreSpike, AstropolisEye, AstropolisEyes, AstropolisMissionState,
    AstropolisPhase, AudioOutput, AudioState, CampaignObjectives, CampaignPlanetObjectives,
    CampaignProgress, CampaignState, CarrierAssaultPhase, CarrierAssaultState, CarrierReactorPanel,
    ChargeSound, CorneriaDefensePhase, CorneriaDefenseState, Difficulty, EladardBarrierStatus,
    EladardDefenderStatus, EladardGeneratorStatus, EladardMissionState, EladardPhase, EndingPhase,
    EndingState, FlightControlStyle, FortunaCoreStatus, FortunaDefenderStatus, FortunaMissionState,
    FortunaPhase, FortunaSwitchStatus, GameMode, GameOverChoice, GameOverDestination,
    GameOverPhase, GameOverState, GameState, IntroPhase, MacbethCoreStatus, MacbethDefenderStatus,
    MacbethInstallationStatus, MacbethMissionState, MacbethPhase, MacbethSwitchStatus,
    MeteorCoreStatus, MeteorMissionState, MeteorPhase, MeteorSwitchStatus, MissionMessage,
    MissionMessageIrisFrame, MissionMessagePhase, MissionMessageState, MissionPhase, MissionState,
    MissionVisit, Pilot, PilotCraftClass, PilotCraftProfile, PilotSelectionCursor,
    PilotSelectionPhase, PilotSelectionState, PlanetObjectiveStatus, PlayerBlasterState,
    PlayerCraftForm, PlayerCraftTransformation, PlayerCraftTransformationDirection,
    PlayerDamageState, PlayerWalkerState, RandomState, RecurringAttacker, RecurringAttackerStatus,
    RecurringAttackersState, ResultsChoice, ResultsPhase, ResultsState, Roster, SoundEvent,
    StrategicMapActor, StrategicMapActorKind, StrategicMapAppearance, StrategicMapPhase,
    StrategicMapState, StrategicMapTutorialPage, StrategicOpeningPage, StrategicOpeningState,
    TitaniaFinalSwitchStatus, TitaniaMissionState, TitaniaPhase, TitaniaSurfaceSwitchStatus,
    TitleMenuItem, TitlePage, TitleState, VenomDefenderStatus, VenomDoorStatus, VenomMissionState,
    VenomPhase, VenomReactorStatus, VenomSwitchStatus, WalkerJumpMotion, WalkerJumpState,
    WalkerMotionProfile, FORTUNA_MAXIMUM_CORE_DEFENDER_COUNT, FORTUNA_SURFACE_SWITCH_COUNT,
    RECURRING_ATTACKER_COUNT, SOUND_EVENT_CAPACITY, STRATEGIC_MAP_ACTOR_CAPACITY,
    VENOM_SURFACE_SWITCH_COUNT,
};
pub use state::{
    BattleCarrierDeployment, CampaignForceCount, CampaignWorldAssignment, DifficultyProfile,
    OpeningAttackerWavePattern, OPENING_ATTACKER_WAVE_CAPACITY,
};
