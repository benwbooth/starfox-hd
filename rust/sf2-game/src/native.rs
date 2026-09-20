//! Typed Star Fox 2 runtime.
//!
//! The native runtime deliberately has no byte-addressed state container.
//! Source-machine encodings are decoded at data boundaries and game systems
//! operate on the domain structs exported from this module.

pub mod attachments;
pub mod authored_paths;
pub mod collision_boxes;
pub mod collision_contacts;
pub mod collision_math;
pub mod collision_pass;
pub mod collision_surface;
mod game;
pub mod hit_response;
pub mod hostile_laser_control;
mod input;
pub mod intro_attached_craft;
pub mod intro_bsp_work;
pub mod intro_camera;
pub mod intro_chain;
pub mod intro_controller;
pub mod intro_destruction;
pub mod intro_draw;
pub mod intro_flyby;
pub mod intro_formation;
pub mod intro_free_craft;
pub mod intro_late_target;
pub mod intro_logo;
pub mod intro_material;
pub mod intro_motion;
pub mod intro_projection;
pub mod intro_render_work;
pub mod intro_root;
pub mod intro_scene;
pub mod intro_second_camera_target;
pub mod intro_second_flyby;
pub mod intro_second_flyby_craft;
pub mod intro_second_flyby_scene;
pub mod intro_second_flyby_wings;
pub mod intro_target;
pub mod intro_transform;
pub mod intro_visibility;
mod object;
pub mod path_actor_context;
pub mod path_appearance;
pub mod path_calls;
pub mod path_commands;
pub mod path_conditions;
pub mod path_contact;
pub mod path_impact;
pub mod path_control;
pub mod path_death;
pub mod path_equipment;
pub mod path_fields;
pub mod path_math;
pub mod path_motion;
pub mod path_player_control;
pub mod path_protection;
pub mod path_countdown;
pub mod path_scene_state;
pub mod path_radio;
pub mod path_charge;
pub mod path_program;
pub mod path_random;
pub mod path_relationships;
pub mod path_runtime;
pub mod path_sound;
pub mod path_score;
pub mod path_spawn;
pub mod path_steering;
pub mod path_target;
pub mod path_trigger_conditions;
pub mod path_triggers;
pub mod platform_carry;
pub mod player_contact;
pub mod player_hit_control;
pub mod program_resources;
pub mod program_state;
pub mod proximity_warning;
pub mod radar;
mod render;
mod results;
pub mod retirement;
pub mod scene_proxy;
mod state;
pub mod strategy_schedule;
pub mod weapon_launch;
pub mod weapon_creation;
pub mod weapon_dispatch;
pub mod world_occupancy;

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
    ObjectSpawnDefaults, ObjectStore, PathCursor, PathId, ShapeId, SpatialDistance, SpatialLoop, SpatialSound,
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
