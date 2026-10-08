//! Native Star Fox 2 game implementation.
//!
//! Shipping code is expressed as typed Rust state and systems. The optional
//! `oracle-bridge` feature exposes the old byte-exact compatibility host only
//! for differential verification; it is not part of [`Game`].

mod native;
pub use native::surface_motion;

pub use native::actor_auxiliary;
pub use native::common_destruction;
pub use native::cinematic_exit;
pub use native::exit_shield;
pub use native::path_exit;
pub use native::path_invocation;
pub use native::{path_countdown, path_death, path_protection, weapon_reflection};
pub use native::path_shots;
pub use native::positional_audio;
pub use native::scene_path_world;
pub use native::scene_map;
pub use native::scene_display;
pub use native::scene_artwork;
pub use native::scene_video;
pub use native::scene_frame;
pub use native::scene_contact;
pub use native::player_charge;
pub use native::player_rapid;
pub use native::player_weapon_aim;
pub use native::player_action;
pub use native::player_mission;
pub use native::player_palette;
pub use native::player_node_exit;
pub use native::scene_install;
pub use native::frame_background;
pub use native::map_streaming;
pub use native::player_motion_reset;
pub use native::player_entry_reset;
pub use native::scene_world_reset;
pub use native::{path_equipment, path_player_control};
pub use native::scene_clear;
pub use native::player_consumable;
pub use native::player_visit;
pub use native::player_recovery;
pub use native::player_storage;
pub use native::player_status;
pub use native::player_engine_sound;
pub use native::player_scene_reset;
pub use native::player_target_lock;
pub use native::player_reticle;
pub use native::player_input;
pub use native::player_roll;
pub use native::player_pose;
pub use native::player_steering;
pub use native::player_vertical;
pub use native::player_throttle;
pub use native::player_ambient;
pub use native::player_surface_particle;
pub use native::player_surface_splash;
pub use native::player_surface_render;
pub use native::player_appearance;
pub use native::player_attachments;
pub use native::player_damage_effects;
pub use native::player_frame_effects;
pub use native::player_post_motion;
pub use native::player_action_wait;
pub use native::player_scene_entry;
pub use native::player_scene_init;
pub use native::player_surface_effect;
pub use native::player_surface;
pub use native::player_speed;
pub use native::player_motion;
pub use native::player_boundary;
pub use native::player_occupancy;
pub use native::player_flight;
pub use native::player_flight_mode;
pub use native::player_flight_prepare;
pub use native::player_free_flight;
pub use native::player_surface_prepare;
pub use native::player_mode_selection;
pub use native::player_impact;
pub use native::player_surface_damage;
pub use native::path_target;
pub use native::target_search;
pub use native::scene_runner;
pub use native::scene_strategy;
pub use native::view_transition;
pub use native::view_blend;
pub use native::player_view_distance;
pub use native::player_camera_angles;
pub use native::player_camera_tracking;
pub use native::player_camera_position;
pub use native::player_camera_ground;
pub use native::player_camera_common;
pub use native::player_camera_surface;
pub use native::player_camera_auxiliary;
pub use native::player_camera_dispatch;
pub use native::weapon_rapid;
pub use native::{
    attachments, collision_boxes, collision_contacts, collision_math, collision_pass,
    collision_surface, hit_response, hostile_laser_control, path_calls, path_commands,
    path_conditions, path_control, path_fields, path_math, path_motion, path_runtime, path_sound,
    path_steering, path_trigger_conditions, path_triggers, platform_carry, player_contact,
    player_hit_control, program_resources, program_state, proximity_warning, radar, retirement,
    scene_proxy, strategy_schedule, weapon_creation, weapon_dispatch, weapon_launch,
    world_occupancy,
};
pub use native::{
    authored_paths, path_appearance, path_program, path_random, path_relationships, path_spawn,
};
pub use native::{path_effect, path_impact, path_radio, path_scene_state};

pub use native::{
    intro_attached_craft, intro_bsp_work, intro_camera, intro_chain, intro_controller,
    intro_destruction, intro_draw, intro_flyby, intro_formation, intro_free_craft,
    intro_late_target, intro_logo, intro_material, intro_motion, intro_projection,
    intro_render_work, intro_root, intro_scene, intro_second_camera_target, intro_second_flyby,
    intro_second_flyby_craft, intro_second_flyby_scene, intro_second_flyby_wings, intro_target,
    intro_transform, intro_visibility,
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
    ObjectLifetimeId, ObjectSpawnDefaults, ObjectStore, PathCursor, PathId, Pilot, PilotCraftClass,
    PilotCraftProfile, PilotSelectionCursor, PilotSelectionPhase, PilotSelectionState,
    PlanetObjectiveStatus, PlayerBlasterState, PlayerCraftForm, PlayerCraftTransformation,
    PlayerCraftTransformationDirection, PlayerDamageState, PlayerWalkerState, RandomSource, RandomState,
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
