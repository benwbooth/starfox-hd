use super::astropolis_assault;
use super::campaign_major_objectives::{
    BATTLE_CARRIER_REQUIRED_VISITS, TITANIA_BASE_ENTRY_RETAIL_FRAME, TITANIA_INTERIOR_RETAIL_FRAME,
    TITANIA_MAP_READY_RETAIL_FRAME, TITANIA_REACTOR_COUNT, TITANIA_REACTOR_RETAIL_FRAME,
    TITANIA_RETURN_RETAIL_FRAME, TITANIA_SURFACE_SWITCH_COUNT,
};
use super::input::{Button, Buttons};
use super::object::{
    Angle, Behavior, CapitalFlightAngles, CapitalFlightState, CapitalMovementPhase,
    CapitalWeaponPhase, CollisionClass, FighterAltitudePhase, FighterAngles,
    FighterCenteringTargetOrder, FighterFlightState, FighterInterceptFlightState,
    FighterInterceptMovementPhase, FighterInterceptWeaponPhase, FighterLogicCadence,
    FighterWaveDirection, FighterWaveOrder, FighterWavePolarity, FighterWeaponPhase,
    FinalRivalFlightPhase, FinalRivalFlightState, HostileProjectileFlightPhase,
    HostileProjectileFlightState, HostileProjectileMovementPhase, InterceptionMissileFlightState,
    InterceptionMissileSteering, LeonRivalFlightPhase, LeonRivalFlightState,
    LeonRivalMovementPhase, Object, ObjectActivity, ObjectId, ObjectKind, PigmaRivalFlightPhase,
    PigmaRivalFlightState, PlayerChargeOrbPhase, PlayerChargeOrbState, PlayerProjectileKind,
    PlayerProjectileState, ReengagementFighterFlightState,
    ReengagementFighterMovementPhase, ShapeId, Vector3, WeaponKind,
};
use super::render::{AnimationState, Camera, MaterialSetId, RenderFlags, RenderObject, Rotation};
use super::state::{
    AstropolisMissionState, AstropolisPhase, AstropolisStatus, CampaignRouteStep,
    CarrierAssaultPhase, CarrierAssaultState, CarrierObjectiveStatus, CarrierReactorPanel,
    CorneriaDefensePhase, CorneriaDefenseState, EladardMissionState, EladardPhase, EndingPhase,
    EndingState, GameMode, GameState, IntroPhase, MapPoint, MissionId, MissionPhase, MissionVisit,
    Pilot, PilotCraftClass, PilotSelectionPhase, PlanetObjectiveStatus, PlayerBlasterState,
    PlayerCraftForm, PlayerCraftTransformation, PlayerCraftTransformationDirection,
    StrategicEncounter, StrategicMapActor, StrategicMapActorKind, StrategicMapAppearance,
    StrategicMapPhase, StrategicMapTutorialPage, StrategicThreatCount, TitaniaMissionState,
    TitaniaPhase, TitaniaReactorStatus, TitaniaSurfaceSwitchStatus, TitleMenuItem, TitlePage,
    WalkerJumpMotion, WalkerJumpState, WolfBlockadeStatus, STRATEGIC_MAP_ACTOR_CAPACITY,
};

#[path = "astropolis_entry.rs"]
mod astropolis_entry;
#[path = "capital_continuation.rs"]
mod capital_continuation;
#[path = "fighter_continuation.rs"]
mod fighter_continuation;
#[path = "fighter_intercept.rs"]
mod fighter_intercept;
#[path = "fighter_intercept_fighters.rs"]
mod fighter_intercept_fighters;
#[path = "fighter_intercept_projectiles.rs"]
mod fighter_intercept_projectiles;
#[path = "final_pursuer.rs"]
mod final_pursuer;
#[path = "final_rivals_flight.rs"]
mod final_rivals_flight;
#[path = "leon_duel.rs"]
mod leon_duel;
#[path = "leon_duel_rival.rs"]
mod leon_duel_rival;
#[path = "leon_pressure.rs"]
mod leon_pressure;
#[path = "mirage_dragon.rs"]
mod mirage_dragon;
#[path = "mirage_dragon_segments.rs"]
mod mirage_dragon_segments;
#[path = "missile_interception.rs"]
mod missile_interception;
#[path = "missile_interception_targets.rs"]
mod missile_interception_targets;
#[path = "opening_continuation.rs"]
mod opening_continuation;
#[path = "pigma_duel.rs"]
mod pigma_duel;
#[path = "pigma_duel_projectiles.rs"]
mod pigma_duel_projectiles;
#[path = "pigma_duel_rival.rs"]
mod pigma_duel_rival;
#[path = "pressure_fighters.rs"]
mod pressure_fighters;
#[path = "second_sortie.rs"]
mod second_sortie;
#[path = "second_sortie_capital.rs"]
mod second_sortie_capital;
#[path = "second_sortie_fighters.rs"]
mod second_sortie_fighters;
#[path = "second_sortie_projectiles.rs"]
mod second_sortie_projectiles;
#[path = "wolf_blockade.rs"]
mod wolf_blockade;

const BOOT_INTRO_TICKS: u32 = 5;
const ARGONAUT_LOGO_TICKS: u32 = 34;
const NINTENDO_LOGO_TICKS: u32 = 58;
const FORMATION_INTRO_TICKS: u32 = 240;
const BRIEFING_PRESENTATION_TICKS: u32 = 276;
const RETAIL_PRESENTATION_FRAMES_PER_TICK: u32 = 4;
const STRATEGIC_OVERVIEW_RETAIL_FRAMES: u32 = 2_576;
const PILOT_SELECTION_REVEAL_RETAIL_FRAMES: u32 = 92;
const PILOT_LAUNCH_RETAIL_FRAMES: u32 = 228;
const STRATEGIC_OVERVIEW_TICKS: u32 =
    STRATEGIC_OVERVIEW_RETAIL_FRAMES / RETAIL_PRESENTATION_FRAMES_PER_TICK;
const PILOT_SELECTION_REVEAL_TICKS: u32 =
    PILOT_SELECTION_REVEAL_RETAIL_FRAMES / RETAIL_PRESENTATION_FRAMES_PER_TICK;
const PILOT_LAUNCH_TICKS: u32 = PILOT_LAUNCH_RETAIL_FRAMES / RETAIL_PRESENTATION_FRAMES_PER_TICK;
const MISSION_STAGE_LOAD_RETAIL_FRAMES: u32 = 50;
const MISSION_ACTIVE_RETAIL_FRAMES: u32 = 320;
const MISSION_STAGE_LOAD_TICKS: u32 =
    MISSION_STAGE_LOAD_RETAIL_FRAMES.div_ceil(RETAIL_PRESENTATION_FRAMES_PER_TICK);
const MISSION_ACTIVE_TICKS: u32 =
    MISSION_ACTIVE_RETAIL_FRAMES / RETAIL_PRESENTATION_FRAMES_PER_TICK;
const MISSION_ENTRY_FORMATION_RETAIL_FRAME: u16 = 80;
const MISSION_PLAYER_CRAFT_HIDE_RETAIL_FRAME: u16 = 350;
const MISSION_CONTROL_HANDOFF_RETAIL_FRAME: u16 = 400;
const MISSION_PLAYER_INPUT_START_RETAIL_FRAME: u16 = 400;
const MISSION_PLAYER_CONTROL_START_RETAIL_FRAME: u16 = 470;
const OPENING_SORTIE_RETURN_TRIGGER_RETAIL_FRAME: u16 = 7_844;
const OPENING_SORTIE_STRATEGIC_MAP_RETURN_RETAIL_FRAME: u16 = 8_056;
const INITIAL_STRATEGIC_TRAVEL_TICKS: u16 = 42;
const REENGAGEMENT_STRATEGIC_TRAVEL_TICKS: u16 = 30;
const MISSILE_INTERCEPTION_STRATEGIC_TRAVEL_TICKS: u16 = 469;
const ELADARD_STRATEGIC_TRAVEL_TICKS: u16 = 385;
const CARRIER_STRATEGIC_TRAVEL_TICKS: u16 = 348;
const LEON_STRATEGIC_TRAVEL_TICKS: u16 = 30;
const MIRAGE_DRAGON_STRATEGIC_TRAVEL_TICKS: u16 = 21;
const STRATEGIC_CURSOR_STEP: i16 = 1;
const CAMPAIGN_TICKS_PER_DISPLAY_SECOND: u64 = 15;
const FIRST_RETURN_DISPLAY_SECONDS: u64 = 12;
const SECOND_RETURN_DISPLAY_SECONDS: u64 = 19;
const SECOND_RETURN_CORNERIA_DAMAGE_PERCENT: u8 = 10;
const MISSILE_INTERCEPTION_RETURN_DISPLAY_SECONDS: u64 = 55;
const FIGHTER_INTERCEPT_RETURN_DISPLAY_SECONDS: u64 = 51;
const PIGMA_RETURN_DISPLAY_SECONDS: u64 = 61;
const ELADARD_RETURN_DISPLAY_SECONDS: u64 = 68;
const CARRIER_RETURN_DISPLAY_SECONDS: u64 = 75;
const LEON_RETURN_DISPLAY_SECONDS: u64 = 76;
const MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT: u8 = 74;
const ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT: u8 = 89;
const INTERCEPTION_TIMER_START_RETAIL_FRAME: u16 = 320;
const INTERCEPTION_RETAIL_FRAMES_PER_TENTH: u16 = 84;
const INTERCEPTION_PLAYER_REVEAL_RETAIL_FRAME: u16 = 280;
const FIGHTER_INTERCEPT_PLAYER_REVEAL_RETAIL_FRAME: u16 = 204;
const FIGHTER_INTERCEPT_PLAYER_HIDDEN_RETAIL_FRAMES: [u16; 2] = [1_368, 1_820];
const PIGMA_PLAYER_REVEAL_RETAIL_FRAME: u16 = 376;
const LEON_PLAYER_REVEAL_RETAIL_FRAME: u16 = 400;
const MIRAGE_DRAGON_PLAYER_REVEAL_RETAIL_FRAME: u16 = 400;
const INTERCEPTION_MISSILE_COUNT: usize = 3;
const FIGHTER_INTERCEPT_TARGET_COUNT: usize = 3;
const PIGMA_HEALTH: u8 = 100;
const PIGMA_ATTACK_POWER: u8 = 4;
const PIGMA_SCORE_AWARD: u32 = 1_000;
const RIVAL_APPROACH_SPEED: u8 = 100;
const RIVAL_APPROACH_ACCELERATION: u8 = 1;
const RIVAL_MANEUVER_SPEED: u8 = 70;
const RIVAL_MANEUVER_ACCELERATION: u8 = 5;
const PIGMA_DECELERATION_SPEED: u8 = 40;
const PIGMA_DECELERATION_RATE: u8 = 5;
const PIGMA_ESCAPE_SPEED: u8 = 10;
const PIGMA_ESCAPE_DECELERATION: u8 = 1;
const RIVAL_COMBAT_ALTITUDE: i16 = -4_000;
const PIGMA_SECOND_APPROACH_ALTITUDE_OFFSET: i16 = 600;
const PIGMA_SECOND_APPROACH_INITIAL_BANK: i8 = -10;
const PIGMA_SECOND_APPROACH_VERTICAL_STEP: i16 = -60;
const PIGMA_ESCAPE_YAW_STEP: i8 = -2;
const RIVAL_APPROACH_ANGLE_CHASE_SHIFT: u32 = 3;
const RIVAL_PLAYER_FACING_CHASE_SHIFT: u32 = 2;
const FINAL_RIVAL_PITCH_LEVEL_CHASE_SHIFT: u32 = 3;
const PIGMA_PLAYER_PITCH_LEVEL_CHASE_SHIFT: u32 = 3;
const PIGMA_SECOND_APPROACH_WAVE: [i8; 10] = [20, -18, 16, -14, 12, -10, 8, -6, 4, -2];
const PIGMA_ESCAPE_WOBBLE: [i8; 10] = [-10, 20, -18, 16, -14, 12, -10, 8, -6, 4];
const LEON_HEALTH: u8 = 100;
const LEON_ATTACK_POWER: u8 = 4;
const LEON_SCORE_AWARD: u32 = 400;
const LEON_RETURN_SCORE: u32 = 3_403;
const LEON_RETURN_ITEM_COUNT: u8 = 3;
const LEON_RETURN_SHIELD: u8 = 40;
const MIRAGE_DRAGON_HEALTH: u8 = 16;
const MIRAGE_DRAGON_ATTACK_POWER: u8 = 4;
const MIRAGE_DRAGON_BODY_SEGMENT_COUNT: usize = 8;
const MIRAGE_DRAGON_SEGMENT_HEALTH: u8 = 15;
const MIRAGE_DRAGON_SEGMENT_ATTACK_POWER: u8 = 1;
const MIRAGE_DRAGON_SCORE_AWARD: u32 = 500;
const MIRAGE_DRAGON_RETURN_DISPLAY_SECONDS: u64 = 80;
const MIRAGE_DRAGON_RETURN_SCORE: u32 = 3_903;
const MIRAGE_DRAGON_RETURN_ITEM_COUNT: u8 = 3;
const MIRAGE_DRAGON_RETURN_SHIELD: u8 = 100;
const PRESSURE_FIGHTER_COUNT: usize = 4;
const PRESSURE_FIGHTER_HEALTH: u8 = 100;
const PRESSURE_FIGHTER_ATTACK_POWER: u8 = 4;
const LEON_PRESSURE_ELAPSED_DISPLAY_SECONDS: u64 = 2;
const RECURRING_ATTACKERS_ELAPSED_DISPLAY_SECONDS: u64 = 5;
const ASTROPOLIS_BASE_ENTRY_RETAIL_FRAME: u16 = 790;
const ASTROPOLIS_INTERIOR_CORRIDOR_RETAIL_FRAME: u16 = 1_332;
const CORNERIA_DESTROYED_DAMAGE_PERCENT: u8 = 100;
const POST_LEON_DAMAGE_RETAIL_FRAMES: [u16; 11] =
    [47, 49, 677, 681, 684, 689, 693, 697, 701, 704, 2_992];
const POST_LEON_PLANET_CANNON_WARNING_RETAIL_FRAME: u16 = 1_576;
const POST_LEON_PLANET_CANNON_CINEMATIC_RETAIL_FRAME: u16 = 1_663;
const POST_LEON_CARRIER_APPROACH_RETAIL_FRAME: u16 = 2_808;
const POST_LEON_CARRIER_WARNING_RETAIL_FRAME: u16 = 3_314;
const POST_LEON_CARRIER_CINEMATIC_RETAIL_FRAME: u16 = 3_399;
const POST_LEON_RESULTS_RETAIL_FRAME: u16 = 4_840;
const ELADARD_SURFACE_BARRIER_COUNT: u8 = 2;
const ELADARD_WALL_SPIDER_HEALTH: u8 = 100;
const ELADARD_GENERATOR_HEALTH: u8 = 125;
const ELADARD_RETURN_SCORE: u32 = 2_251;
const ELADARD_RETURN_ITEM_COUNT: u8 = 3;
const ELADARD_RETURN_SHIELD: u8 = 40;
const CARRIER_RETURN_SCORE: u32 = 3_003;
const CARRIER_RETURN_ITEM_COUNT: u8 = 3;
const CARRIER_RETURN_PRIMARY_SHIELD: u8 = 34;
const CARRIER_RETURN_WINGMATE_SHIELD: u8 = 13;
const CARRIER_PANEL_INITIAL_INTEGRITY: u8 = 100;
const CARRIER_PANEL_DESTROYED_INTEGRITY: u8 = 90;
const CARRIER_PANEL_AFTER_ONE_HIT: u8 = 98;
const CARRIER_PANEL_AFTER_TWO_HITS: u8 = 96;
const CARRIER_PANEL_AFTER_THREE_HITS: u8 = 94;
const CARRIER_PANEL_AFTER_FOUR_HITS: u8 = 92;
const CARRIER_REACTOR_PANEL_COUNT: u32 = 2;
const CARRIER_PORT_PANEL_INDEX: usize = 0;
const CARRIER_STARBOARD_PANEL_INDEX: usize = 1;
const CARRIER_EXTERIOR_END_RETAIL_FRAME: u16 = 2_084;
const CARRIER_REACTOR_APPROACH_RETAIL_FRAME: u16 = 5_160;
const CARRIER_REACTOR_OPEN_RETAIL_FRAME: u16 = 5_390;
const CARRIER_STARBOARD_FIRST_HIT_RETAIL_FRAME: u16 = 5_110;
const CARRIER_STARBOARD_THIRD_HIT_RETAIL_FRAME: u16 = 5_220;
const CARRIER_STARBOARD_FOURTH_HIT_RETAIL_FRAME: u16 = 5_240;
const CARRIER_STARBOARD_EIGHTH_HIT_RETAIL_FRAME: u16 = 5_405;
const CARRIER_STARBOARD_FINAL_HIT_RETAIL_FRAME: u16 = 7_620;
const CARRIER_STARBOARD_DESTROYED_RETAIL_FRAME: u16 = 7_635;
const CARRIER_PORT_FIRST_HIT_RETAIL_FRAME: u16 = 5_110;
const CARRIER_PORT_SECOND_HIT_RETAIL_FRAME: u16 = 8_530;
const CARRIER_PORT_THIRD_HIT_RETAIL_FRAME: u16 = 8_570;
const CARRIER_PORT_FOURTH_HIT_RETAIL_FRAME: u16 = 8_575;
const CARRIER_PORT_FINAL_HIT_RETAIL_FRAME: u16 = 8_635;
const CARRIER_PORT_DESTROYED_RETAIL_FRAME: u16 = 8_650;
const CARRIER_CORE_DESTROYED_RETAIL_FRAME: u16 = 8_658;
const CARRIER_RETURN_FLIGHT_RETAIL_FRAME: u16 = 9_200;
const CARRIER_MAP_READY_RETAIL_FRAME: u16 = 9_822;
const CARRIER_CORRIDOR_LENGTH: u16 = 12_441;
const CARRIER_EXTERIOR_CAMERA_HEIGHT: i16 = -20;
const CARRIER_CORRIDOR_CAMERA_HEIGHT: i16 = -160;
const CARRIER_CORRIDOR_CAMERA_FORWARD_OFFSET: i16 = 700;
const CARRIER_EXTERIOR_START_POSITION: Vector3 = Vector3 {
    x: 711,
    y: 0,
    z: -16_860,
};
const CARRIER_EXTERIOR_ENTRY_POSITION: Vector3 = Vector3 {
    x: -1_619,
    y: 0,
    z: -5_508,
};
const CARRIER_CORRIDOR_START_POSITION: Vector3 = Vector3 {
    x: 1_280,
    y: -120,
    z: 64,
};
const CARRIER_REACTOR_PLAYER_POSITION: Vector3 = Vector3 {
    x: 1_280,
    y: -30,
    z: 12_505,
};
const CARRIER_REACTOR_CAMERA_POSITION: Vector3 = Vector3 {
    x: 1_215,
    y: -160,
    z: 13_882,
};
const CARRIER_HULL_YAW: Angle = Angle::from_units(73);
const CARRIER_EXTERIOR_SCENE: [(ShapeId, Vector3, Angle); 5] = [
    (
        ShapeId::CARRIER_HULL_AFT_PORT,
        Vector3 {
            x: -3_747,
            y: 0,
            z: -842,
        },
        CARRIER_HULL_YAW,
    ),
    (
        ShapeId::CARRIER_HULL_FORWARD_PORT,
        Vector3 {
            x: -1_031,
            y: 0,
            z: -232,
        },
        CARRIER_HULL_YAW,
    ),
    (
        ShapeId::CARRIER_HULL_CENTER,
        Vector3 {
            x: 780,
            y: 0,
            z: 175,
        },
        CARRIER_HULL_YAW,
    ),
    (
        ShapeId::CARRIER_HULL_FORWARD_STARBOARD,
        Vector3 {
            x: 2_809,
            y: 0,
            z: 630,
        },
        CARRIER_HULL_YAW,
    ),
    (
        ShapeId::CARRIER_HULL_AFT_STARBOARD,
        Vector3 {
            x: -1_408,
            y: 0,
            z: -6_080,
        },
        CARRIER_HULL_YAW,
    ),
];
const CARRIER_CORRIDOR_SCENE: [(ShapeId, Vector3, Angle); 7] = [
    (
        ShapeId::CARRIER_CORRIDOR_DOOR,
        Vector3 {
            x: 1_440,
            y: 0,
            z: 3_072,
        },
        Angle::HALF_TURN,
    ),
    (
        ShapeId::CARRIER_CORRIDOR_WALL,
        Vector3 {
            x: 512,
            y: 0,
            z: 1_024,
        },
        Angle::from_units(192),
    ),
    (
        ShapeId::CARRIER_CORRIDOR_WALL,
        Vector3 {
            x: 512,
            y: 0,
            z: 3_072,
        },
        Angle::from_units(192),
    ),
    (
        ShapeId::CARRIER_CORRIDOR_WALL,
        Vector3 {
            x: 512,
            y: 0,
            z: 5_120,
        },
        Angle::from_units(192),
    ),
    (
        ShapeId::CARRIER_CORRIDOR_WALL,
        Vector3 {
            x: 2_048,
            y: 0,
            z: 1_024,
        },
        Angle::from_units(64),
    ),
    (
        ShapeId::CARRIER_CORRIDOR_WALL,
        Vector3 {
            x: 2_048,
            y: 0,
            z: 3_072,
        },
        Angle::from_units(64),
    ),
    (
        ShapeId::CARRIER_CORRIDOR_WALL,
        Vector3 {
            x: 2_048,
            y: 0,
            z: 5_120,
        },
        Angle::from_units(64),
    ),
];
const CARRIER_REACTOR_SCENE: [(ShapeId, Vector3, Angle); 6] = [
    (
        ShapeId::CARRIER_REACTOR_CORE,
        Vector3 {
            x: 1_280,
            y: 0,
            z: 13_312,
        },
        Angle::ZERO,
    ),
    (
        ShapeId::CARRIER_REACTOR_REAR_WALL,
        Vector3 {
            x: 1_280,
            y: 0,
            z: 14_848,
        },
        Angle::HALF_TURN,
    ),
    (
        ShapeId::CARRIER_REACTOR_SIDE_WALL,
        Vector3 {
            x: -512,
            y: 0,
            z: 11_776,
        },
        Angle::from_units(192),
    ),
    (
        ShapeId::CARRIER_REACTOR_SIDE_WALL,
        Vector3 {
            x: 3_072,
            y: 0,
            z: 11_776,
        },
        Angle::ZERO,
    ),
    (
        ShapeId::CARRIER_REACTOR_SIDE_WALL,
        Vector3 {
            x: -512,
            y: 0,
            z: 14_848,
        },
        Angle::HALF_TURN,
    ),
    (
        ShapeId::CARRIER_REACTOR_SIDE_WALL,
        Vector3 {
            x: 3_072,
            y: 0,
            z: 14_848,
        },
        Angle::from_units(64),
    ),
];
const CARRIER_PANEL_SCENE: [(Vector3, Angle); 2] = [
    (
        Vector3 {
            x: 1_040,
            y: -160,
            z: 13_312,
        },
        Angle::from_units(64),
    ),
    (
        Vector3 {
            x: 1_519,
            y: -160,
            z: 13_312,
        },
        Angle::from_units(192),
    ),
];
const ELADARD_SURFACE_BARRIERS_RETAIL_FRAME: u16 = 1_600;
const ELADARD_BASE_ENTRANCE_RETAIL_FRAME: u16 = 4_700;
const ELADARD_WALKER_TRANSFORMATION_RETAIL_FRAME: u16 = 5_000;
#[cfg(test)]
const ELADARD_WALKER_READY_RETAIL_FRAME: u16 = ELADARD_WALKER_TRANSFORMATION_RETAIL_FRAME
    + PLAYER_TRANSFORMATION_TO_WALKER_END_RETAIL_FRAMES as u16;
const ELADARD_PLATFORM_SWITCH_RETAIL_FRAME: u16 = 7_000;
const ELADARD_WALL_SPIDER_RETAIL_FRAME: u16 = 8_600;
const ELADARD_GENERATOR_RETAIL_FRAME: u16 = 10_400;
const ELADARD_GENERATOR_DESTROYED_RETAIL_FRAME: u16 = 11_822;
const ELADARD_RETURN_RETAIL_FRAME: u16 = 12_434;
const ELADARD_MAP_READY_RETAIL_FRAME: u16 = 12_436;
const SECOND_SORTIE_DEFEATED_TARGET_RETAIL_FRAME: u16 = 6_104;
const FIRST_RETURN_PRIMARY_SHIELD: u8 = 32;
const FIRST_RETURN_WINGMATE_SHIELD: u8 = 16;
const SECOND_RETURN_PRIMARY_SHIELD: u8 = 8;
const SECOND_RETURN_WINGMATE_SHIELD: u8 = 0;
const INITIAL_PLAYER_MAP_POSITION: MapPoint = MapPoint { x: 72, y: 102 };
const FIRST_REENGAGEMENT_DESTINATION: MapPoint = MapPoint { x: 71, y: 101 };
const MISSILE_INTERCEPTION_DESTINATION: MapPoint = MapPoint { x: 140, y: 140 };
const FIGHTER_INTERCEPT_DESTINATION: MapPoint = MapPoint { x: 132, y: 119 };
const PIGMA_DUEL_DESTINATION: MapPoint = MapPoint { x: 135, y: 119 };
const ELADARD_BASE_DESTINATION: MapPoint = MapPoint { x: 16, y: 14 };
const POST_ELADARD_RECOMMENDED_DESTINATION: MapPoint = MapPoint { x: 50, y: 90 };
const TITANIA_BASE_DESTINATION: MapPoint = MapPoint { x: 208, y: 110 };
const SECOND_BATTLE_CARRIER_DESTINATION: MapPoint = MapPoint { x: 220, y: 7 };
const LEON_DUEL_DESTINATION: MapPoint = MapPoint { x: 220, y: 7 };
const MIRAGE_DRAGON_DESTINATION: MapPoint = MapPoint { x: 54, y: 123 };
const RECURRING_ATTACKERS_DESTINATION: MapPoint = MapPoint { x: 14, y: 120 };
const LEON_PRESSURE_DESTINATION: MapPoint = MapPoint { x: 104, y: 113 };
const FINAL_PURSUER_DESTINATION: MapPoint = MapPoint { x: 140, y: 64 };
const WOLF_BLOCKADE_DESTINATION: MapPoint = MapPoint { x: 134, y: 77 };
const ASTROPOLIS_DESTINATION: MapPoint = MapPoint { x: 236, y: 24 };
const OPENING_ASSAULT_MAP_ACTORS: [Option<StrategicMapActor>; STRATEGIC_MAP_ACTOR_CAPACITY] = [
    Some(StrategicMapActor::new(
        StrategicMapActorKind::NorthernInstallation,
        StrategicMapAppearance::OpeningAssault,
        16,
        14,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::SouthernInstallation,
        StrategicMapAppearance::OpeningAssault,
        208,
        110,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyCarrier,
        StrategicMapAppearance::OpeningAssault,
        220,
        7,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyFormation,
        StrategicMapAppearance::OpeningAssault,
        62,
        40,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EasternInterceptor,
        StrategicMapAppearance::OpeningAssault,
        203,
        88,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::PatrolShip,
        StrategicMapAppearance::OpeningAssault,
        12,
        150,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::MissileTrail,
        StrategicMapAppearance::OpeningAssault,
        100,
        132,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::Missile,
        StrategicMapAppearance::OpeningAssault,
        180,
        117,
    )),
    None,
    None,
];
const ESCALATED_ASSAULT_MAP_ACTORS: [Option<StrategicMapActor>; STRATEGIC_MAP_ACTOR_CAPACITY] = [
    Some(StrategicMapActor::new(
        StrategicMapActorKind::NorthernInstallation,
        StrategicMapAppearance::EscalatedAssault,
        16,
        14,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::SouthernInstallation,
        StrategicMapAppearance::EscalatedAssault,
        208,
        110,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyCarrier,
        StrategicMapAppearance::EscalatedAssault,
        220,
        7,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyFormation,
        StrategicMapAppearance::EscalatedAssault,
        45,
        45,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EasternInterceptor,
        StrategicMapAppearance::EscalatedAssault,
        198,
        89,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::PatrolShip,
        StrategicMapAppearance::EscalatedAssault,
        12,
        150,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::Missile,
        StrategicMapAppearance::EscalatedAssault,
        147,
        125,
    )),
    None,
    None,
    None,
];
const POST_INTERCEPTION_MAP_ACTORS: [Option<StrategicMapActor>; STRATEGIC_MAP_ACTOR_CAPACITY] = [
    Some(StrategicMapActor::new(
        StrategicMapActorKind::NorthernInstallation,
        StrategicMapAppearance::PostInterception,
        16,
        14,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::SouthernInstallation,
        StrategicMapAppearance::PostInterception,
        208,
        110,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyCarrier,
        StrategicMapAppearance::PostInterception,
        220,
        7,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyFormation,
        StrategicMapAppearance::PostInterception,
        47,
        66,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EasternInterceptor,
        StrategicMapAppearance::PostInterception,
        172,
        94,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::PatrolShip,
        StrategicMapAppearance::PostInterception,
        12,
        150,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::AttackingFighter,
        StrategicMapAppearance::PostInterception,
        132,
        119,
    )),
    None,
    None,
    None,
];
const POST_FIGHTER_INTERCEPT_MAP_ACTORS: [Option<StrategicMapActor>; STRATEGIC_MAP_ACTOR_CAPACITY] = [
    Some(StrategicMapActor::new(
        StrategicMapActorKind::NorthernInstallation,
        StrategicMapAppearance::PostFighterIntercept,
        16,
        14,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::SouthernInstallation,
        StrategicMapAppearance::PostFighterIntercept,
        208,
        110,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyCarrier,
        StrategicMapAppearance::PostFighterIntercept,
        220,
        7,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyFormation,
        StrategicMapAppearance::PostFighterIntercept,
        46,
        64,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EasternInterceptor,
        StrategicMapAppearance::PostFighterIntercept,
        170,
        95,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::PatrolShip,
        StrategicMapAppearance::PostFighterIntercept,
        12,
        150,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::AttackingFighter,
        StrategicMapAppearance::PostFighterIntercept,
        135,
        119,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::FighterProjectile,
        StrategicMapAppearance::PostFighterIntercept,
        86,
        136,
    )),
    None,
    None,
];
const POST_PIGMA_MAP_ACTORS: [Option<StrategicMapActor>; STRATEGIC_MAP_ACTOR_CAPACITY] = [
    Some(StrategicMapActor::new(
        StrategicMapActorKind::NorthernInstallation,
        StrategicMapAppearance::PostPigma,
        16,
        14,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::SouthernInstallation,
        StrategicMapAppearance::PostPigma,
        208,
        110,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyCarrier,
        StrategicMapAppearance::PostPigma,
        220,
        7,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyFormation,
        StrategicMapAppearance::PostPigma,
        44,
        71,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EasternInterceptor,
        StrategicMapAppearance::PostPigma,
        140,
        95,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::PatrolShip,
        StrategicMapAppearance::PostPigma,
        12,
        145,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::RivalFighter,
        StrategicMapAppearance::PostPigma,
        211,
        120,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::FighterProjectile,
        StrategicMapAppearance::PostPigma,
        115,
        132,
    )),
    None,
    None,
];
const POST_ELADARD_MAP_ACTORS: [Option<StrategicMapActor>; STRATEGIC_MAP_ACTOR_CAPACITY] = [
    Some(StrategicMapActor::new(
        StrategicMapActorKind::NorthernInstallation,
        StrategicMapAppearance::PostEladard,
        16,
        12,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::SouthernInstallation,
        StrategicMapAppearance::PostEladard,
        208,
        110,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyCarrier,
        StrategicMapAppearance::PostEladard,
        220,
        7,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyFormation,
        StrategicMapAppearance::PostEladard,
        41,
        75,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EasternInterceptor,
        StrategicMapAppearance::PostEladard,
        161,
        96,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::PatrolShip,
        StrategicMapAppearance::PostEladard,
        12,
        150,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::AttackingFighter,
        StrategicMapAppearance::PostEladard,
        192,
        122,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::UnknownSignal,
        StrategicMapAppearance::PostEladard,
        45,
        101,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::FighterProjectile,
        StrategicMapAppearance::PostEladard,
        86,
        139,
    )),
    None,
];
const POST_CARRIER_MAP_ACTORS: [Option<StrategicMapActor>; STRATEGIC_MAP_ACTOR_CAPACITY] = [
    Some(StrategicMapActor::new(
        StrategicMapActorKind::NorthernInstallation,
        StrategicMapAppearance::PostCarrier,
        16,
        14,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::SouthernInstallation,
        StrategicMapAppearance::PostCarrier,
        208,
        110,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyCarrier,
        StrategicMapAppearance::PostCarrier,
        220,
        7,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyFormation,
        StrategicMapAppearance::PostCarrier,
        25,
        78,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::AttackingFighter,
        StrategicMapAppearance::PostCarrier,
        125,
        112,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::PatrolShip,
        StrategicMapAppearance::PostCarrier,
        12,
        150,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::UnknownSignal,
        StrategicMapAppearance::PostCarrier,
        54,
        123,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::MissileTrail,
        StrategicMapAppearance::PostCarrier,
        8,
        80,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::Missile,
        StrategicMapAppearance::PostCarrier,
        24,
        111,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::FighterProjectile,
        StrategicMapAppearance::PostCarrier,
        49,
        148,
    )),
];
const POST_LEON_MAP_ACTORS: [Option<StrategicMapActor>; STRATEGIC_MAP_ACTOR_CAPACITY] = [
    Some(StrategicMapActor::new(
        StrategicMapActorKind::NorthernInstallation,
        StrategicMapAppearance::PostLeon,
        16,
        14,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::SouthernInstallation,
        StrategicMapAppearance::PostLeon,
        208,
        110,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyCarrier,
        StrategicMapAppearance::PostLeon,
        220,
        7,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyFormation,
        StrategicMapAppearance::PostLeon,
        25,
        78,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::AttackingFighter,
        StrategicMapAppearance::PostLeon,
        125,
        112,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::PatrolShip,
        StrategicMapAppearance::PostLeon,
        12,
        150,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::UnknownSignal,
        StrategicMapAppearance::PostLeon,
        54,
        123,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::MissileTrail,
        StrategicMapAppearance::PostLeon,
        8,
        80,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::Missile,
        StrategicMapAppearance::PostLeon,
        24,
        111,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::FighterProjectile,
        StrategicMapAppearance::PostLeon,
        49,
        148,
    )),
];
const POST_MIRAGE_MAP_ACTORS: [Option<StrategicMapActor>; STRATEGIC_MAP_ACTOR_CAPACITY] = [
    Some(StrategicMapActor::new(
        StrategicMapActorKind::NorthernInstallation,
        StrategicMapAppearance::PostMirage,
        16,
        14,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::SouthernInstallation,
        StrategicMapAppearance::PostMirage,
        208,
        110,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyCarrier,
        StrategicMapAppearance::PostMirage,
        220,
        7,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::EnemyFormation,
        StrategicMapAppearance::PostMirage,
        25,
        78,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::AttackingFighter,
        StrategicMapAppearance::PostMirage,
        14,
        120,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::PatrolShip,
        StrategicMapAppearance::PostMirage,
        12,
        150,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::DefensePlatform,
        StrategicMapAppearance::PostMirage,
        72,
        102,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::UnknownSignal,
        StrategicMapAppearance::PostMirage,
        39,
        143,
    )),
    Some(StrategicMapActor::new(
        StrategicMapActorKind::FighterProjectile,
        StrategicMapAppearance::PostMirage,
        104,
        113,
    )),
    None,
];
const MISSION_ENTRY_FORMATION_TICKS: u32 =
    MISSION_ENTRY_FORMATION_RETAIL_FRAME as u32 / RETAIL_PRESENTATION_FRAMES_PER_TICK;
const PLAYER_CRUISE_SPEED: u8 = 30;
const PLAYER_TURN_SPEED: u8 = 8;
const PLAYER_SPEED_CHANGE_PER_TICK: u8 = 1;
const PLAYER_YAW_ACCUMULATOR_STEP: i16 = 512;
const PLAYER_PITCH_TARGET: i16 = 10_240;
const PLAYER_BOUNDARY_PITCH_TARGET: i16 = 5_120;
const PLAYER_CONTROL_RESPONSE_SHIFT: u32 = 3;
const PLAYER_PITCH_LEAN_LIMIT: i8 = 30;
const PLAYER_BOUNDARY_PITCH_LEAN: i8 = 15;
const PLAYER_PITCH_LEAN_RATE: i8 = 2;
const PLAYER_VISIBLE_PITCH_LEAN_SHIFT: u32 = 2;
const FLIGHT_ACCUMULATOR_FRACTION_BITS: u32 = 8;
const PLAYER_BANK_RATE: u8 = 4;
const PLAYER_LEFT_BANK: Angle = Angle::from_units(32);
const PLAYER_RIGHT_BANK: Angle = Angle::from_units(224);
const PLAYER_VERTICAL_UPPER_BOUND: i16 = 3_500;
const PLAYER_VERTICAL_LOWER_BOUND: i16 = -3_515;
const OPENING_FLIGHT_YAW_ACCUMULATOR: i16 = -7_424;
const PLAYER_TRANSFORMATION_RETAIL_FRAMES_PER_TICK: u8 = RETAIL_PRESENTATION_FRAMES_PER_TICK as u8;
const PLAYER_TRANSFORMATION_START_RETAIL_FRAMES: u8 = 8;
const PLAYER_TRANSFORMATION_SECOND_STAGE_RETAIL_FRAMES: u8 = 32;
const PLAYER_TRANSFORMATION_TO_FLIGHT_END_RETAIL_FRAMES: u8 = 48;
const PLAYER_TRANSFORMATION_TO_WALKER_END_RETAIL_FRAMES: u8 = 56;
const TO_WALKER_FLIGHT_SIDE_FRAMES: [u8; 6] = [6, 5, 4, 3, 2, 1];
const TO_WALKER_WALKER_SIDE_FRAMES: [u8; 6] = [6, 5, 3, 2, 1, 1];
const TO_FLIGHT_WALKER_SIDE_FRAMES: [u8; 6] = [1, 1, 2, 3, 4, 5];
const TO_FLIGHT_FLIGHT_SIDE_FRAMES: [u8; 4] = [1, 2, 4, 5];
const WALKER_FORWARD_SPEED: u8 = 36;
const WALKER_TURN_SPRING_TARGET: i16 = 8_704;
const WALKER_TURN_VELOCITY_TARGET: i8 = 5;
const WALKER_ACTIVE_SPRING_RESPONSE_DIVISOR: i16 = 4;
const WALKER_ACTIVE_VELOCITY_RESPONSE_DIVISOR: i16 = 8;
const WALKER_NEUTRAL_RESPONSE_DIVISOR: i16 = 2;
const WALKER_JUMP_INITIAL_POSE_EXTENSION: u16 = 20;
const WALKER_JUMP_FULL_POSE_EXTENSION: u16 = 1_024;
const WALKER_JUMP_RELEASE_RECOVERY_STEP: i16 = 448;
const WALKER_ASCENT_HALF_SCALE_DIVISOR: i16 = 2;
const WALKER_LOCAL_MOTION_SCALE: i16 = 8;
const WALKER_FALL_DEAD_ZONE: i16 = 3;
const WALKER_MAXIMUM_FALL_ACCELERATION: i16 = RETAIL_PRESENTATION_FRAMES_PER_TICK as i16;
const WALKER_TAKEOFF_COLLISION_TICKS: u8 = 1;
const PLAYER_LASER_MUZZLE_OFFSET_MAGNITUDE: u8 = 72;
const PLAYER_RAPID_LASER_SPEED: u8 = 62;
const PLAYER_CHARGED_LASER_SPEED: u8 = 120;
const PLAYER_RAPID_LASER_LAUNCH_VELOCITY_SCALE: i16 = 1;
const PLAYER_RAPID_LASER_EXPANDED_VELOCITY_SCALE: i16 = 2;
const PLAYER_RAPID_LASER_FAST_VELOCITY_SCALE: i16 = 32;
const PLAYER_CHARGED_LASER_LAUNCH_VELOCITY_SCALE: i16 = 4;
const PLAYER_CHARGED_LASER_ACTIVE_VELOCITY_SCALE: i16 = 8;
const PLAYER_PROJECTILE_DURABILITY: u8 = 120;
const PLAYER_RAPID_LASER_ATTACK_POWER: u8 = 1;
const PLAYER_CHARGED_LASER_ATTACK_POWER: u8 = 10;
const PLAYER_RAPID_LASER_VISIBLE_TICK: u8 = 2;
const PLAYER_RAPID_LASER_EXPANDED_TICK: u8 = 3;
const PLAYER_RAPID_LASER_FAST_TICK: u8 = 4;
const PLAYER_RAPID_LASER_DISTANT_TICK: u8 = 7;
const PLAYER_RAPID_LASER_END_TICK: u8 = 11;
const PLAYER_CHARGE_ORB_SPAWN_TICK: u8 = 12;
const FOX_FALCO_CHARGE_READY_TICK: u8 = 22;
const PEPPY_SLIPPY_CHARGE_READY_TICK: u8 = 34;
const MIYU_FAY_CHARGE_READY_TICK: u8 = 21;
const PLAYER_CHARGE_RELEASE_GRACE_TICKS: u8 = 2;
const PLAYER_CHARGE_ORB_RELEASE_TICKS: u8 = 3;
const PLAYER_CHARGED_LASER_VISIBLE_TICK: u8 = 2;
const PLAYER_CHARGED_LASER_ACTIVE_TICK: u8 = 5;
const PLAYER_CHARGED_LASER_END_TICK: u8 = 39;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WalkerTurnInput {
    Left,
    Right,
    Neutral,
}
const PLAYER_LASER_COLLISION_BOUNDS: CollisionBounds = CollisionBounds::cube(16);
const MISSION_ENCOUNTER_HEALTH: u8 = 100;
const MISSION_ENCOUNTER_ATTACK_POWER: u8 = 4;
const ENEMY_DESTRUCTION_TICKS: u8 = 9;
const ENEMY_SCORE_AWARD_TIMER: u8 = 6;
const ENEMY_EXPLOSION_START_TIMER: u8 = 3;
const MISSION_ENCOUNTER_START_RETAIL_FRAME: u16 = 330;
const MISSION_FIGHTER_COMBAT_HANDOFF_RETAIL_FRAME: u16 = 480;
const MISSION_BASE_KEYFRAME_END_RETAIL_FRAME: u16 = 900;
const CAPITAL_FLIGHT_HANDOFF_RETAIL_FRAME: u16 = MISSION_BASE_KEYFRAME_END_RETAIL_FRAME;
const MISSION_ENCOUNTER_CERTIFIED_END_RETAIL_FRAME: u16 =
    opening_continuation::ENCOUNTER_CERTIFIED_END_RETAIL_FRAME;
const MISSION_ENCOUNTER_POSITION_SCALE: i16 = 4;
const INTERCEPTION_MISSILE_POSITION_SCALE: i16 = 1;
const INTERCEPTION_MISSILE_STEERING_STEP: i8 = 1;
const INTERCEPTION_MISSILE_SPIN_STEP: i8 = 2;
const CAPITAL_LEVEL_PITCH_UNITS: u8 = 0;
const CAPITAL_DIVE_PITCH_UNITS: u8 = 206;
const CAPITAL_CLIMB_PITCH_UNITS: u8 = 50;
const CAPITAL_MANEUVER_BANK_UNITS: i8 = 4;
const CAPITAL_ANGLE_CHASE_DIVISOR: i8 = 8;
const CAPITAL_PLAYER_FACING_DIVISOR: i8 = 4;
const CAPITAL_BANK_TURN_DIVISOR: i8 = 4;
const CAPITAL_ALTITUDE_CENTERING_DIVISOR: i16 = 8;
const CAPITAL_ENTRY_WAVE_DIVISIONS: u8 = 3;
const CAPITAL_ENTRY_WAVE_PHASE_STEP: i8 = 4;
const CAPITAL_COMBAT_WAVE_PHASE_STEP: i8 = 1;
const FIRST_CAPITAL_VERTICAL_WAVE_PHASE: u8 = 104;
const SECOND_CAPITAL_VERTICAL_WAVE_PHASE: u8 = 101;
const CAPITAL_FLIGHT_HANDOFF_VELOCITIES: [Vector3; 2] = [
    Vector3 {
        x: 176,
        y: 0,
        z: 148,
    },
    Vector3 {
        x: 228,
        y: 0,
        z: 28,
    },
];
const FIGHTER_VERTICAL_WAVE_STEP: u8 = 4;
const FIGHTER_VERTICAL_WAVE_POSITION_SCALE: i16 = 4;
const FIGHTER_QUARTER_WAVE_DIVISOR: i16 = 4;
const FIGHTER_LOGIC_CREDIT_PER_TICK: u8 = 23;
const UPPER_FIGHTER_INITIAL_LOGIC_CREDIT: u8 = 2;
const FIGHTER_ENTRY_LOGIC_CREDIT_THRESHOLD: u8 = 25;
const FIGHTER_COMBAT_LOGIC_CREDIT_THRESHOLD: u8 = 26;
const LOWER_FIGHTER_POST_MANEUVER_LOGIC_CREDIT: u8 = 20;
const FIGHTER_ALTITUDE_CENTERING_TICKS: u8 = 32;
const FIGHTER_INITIAL_ACTIVITY_TICKS_REMAINING: u8 = 23;
const FIGHTER_MANEUVER_PERIOD_TICKS_REMAINING: u8 = 127;
const FIGHTER_FIRE_PERIOD_TICKS_REMAINING: u8 = 31;
const FIGHTER_FIRE_RANGE: i16 = 10_000;
const FIGHTER_FIRE_RANDOM_THRESHOLD: u8 = 127;
const FIGHTER_AIM_RESTORE_TICKS: u8 = 1;
const FIGHTER_HANDOFF_RANDOM_STATE: [u8; 4] = [84, 34, 5, 26];
const REENGAGEMENT_FIGHTER_ENTRY_ALTITUDE_OFFSET: i16 = -3_197;
const REENGAGEMENT_FIGHTER_INITIAL_WAVE_PHASE: u8 = 1;
const REENGAGEMENT_FIGHTER_ENTRY_YAW_UNITS: u8 = 38;
const REENGAGEMENT_FIGHTER_ENTRY_SPEED: u8 = 10;
const REENGAGEMENT_FIGHTER_ACCELERATION: u8 = 30;
const REENGAGEMENT_FIGHTER_MAXIMUM_SPEED: u8 = 63;
const REENGAGEMENT_FIGHTER_BANK_TURN_DIVISOR: i8 = 4;
const REENGAGEMENT_FIGHTER_ENTRY_WAVE_DIVISOR: i16 = 8;
const REENGAGEMENT_FIGHTER_ENTRY_WAVE_STEP: i8 = 2;
const REENGAGEMENT_FIGHTER_COMBAT_WAVE_STEP: i8 = 4;
const REENGAGEMENT_FIGHTER_WAVE_QUARTERS: u8 = 4;
const REENGAGEMENT_FIGHTER_PITCH_TARGET_DIVISOR: i8 = 2;
const REENGAGEMENT_FIGHTER_ALTITUDE_CENTERING_TICKS: u8 = 32;
const REENGAGEMENT_FIGHTER_ENTRY_PHASE_STARBOARD: u8 = 64;
const REENGAGEMENT_FIGHTER_ENTRY_PHASE_PORT: u8 = 192;
const FIGHTER_INTERCEPT_BANK_TURN_DIVISOR: i8 = 4;
const FIGHTER_INTERCEPT_ENTRY_WAVE_DIVISOR: i16 = 8;
const FIGHTER_INTERCEPT_COMBAT_WAVE_DIVISOR: i16 = 2;
const FIGHTER_INTERCEPT_WAVE_PHASE_STEP: i8 = 4;
const FIGHTER_INTERCEPT_ALTITUDE_DIVISOR: i16 = 8;
const FIGHTER_INTERCEPT_PLAYER_FACING_DIVISOR: i8 = 4;
const FIGHTER_COOPERATIVE_SCHEDULE_START_RETAIL_FRAME: u16 = 644;
const FIGHTER_COOPERATIVE_SCHEDULE_STEP: u16 = 4;
const FIGHTER_COOPERATIVE_CONTINUATION_START_RETAIL_FRAME: u16 = 904;
const FIGHTER_RANDOM_CADENCE_START_RETAIL_FRAME: u16 = 588;
const FIGHTER_RANDOM_CADENCE_COUNT: usize = 79;
const FIGHTER_RANDOM_CONTINUATION_START_RETAIL_FRAME: u16 = 904;
const FIGHTER_MANEUVER_BANKS: [Angle; 8] = [
    Angle::from_units(4),
    Angle::from_units(252),
    Angle::from_units(8),
    Angle::from_units(248),
    Angle::from_units(12),
    Angle::from_units(244),
    Angle::from_units(16),
    Angle::from_units(240),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReengagementFighterDirection {
    Starboard,
    Port,
}

impl ReengagementFighterDirection {
    const fn wave_phase(self) -> Angle {
        Angle::from_units(match self {
            Self::Starboard => REENGAGEMENT_FIGHTER_ENTRY_PHASE_STARBOARD,
            Self::Port => REENGAGEMENT_FIGHTER_ENTRY_PHASE_PORT,
        })
    }

    const fn bank(self) -> ReengagementFighterBankTarget {
        match self {
            Self::Starboard => ReengagementFighterBankTarget::StarboardEntry,
            Self::Port => ReengagementFighterBankTarget::PortEntry,
        }
    }
}

#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReengagementFighterBankTarget {
    Level = 0,
    StarboardGentle = 4,
    StarboardEntry = 24,
    PortGentle = -4,
    PortInitial = -8,
    PortEntry = -24,
}

impl ReengagementFighterBankTarget {
    const fn angle(self) -> Angle {
        Angle::from_units(self as i8 as u8)
    }
}

#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReengagementFighterPitchTarget {
    WeaponAim = -6,
}

impl ReengagementFighterPitchTarget {
    const fn angle(self) -> Angle {
        Angle::from_units(self as i8 as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReengagementFighterAcceleration {
    Hold,
    Accelerate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReengagementFighterAction {
    EntrySetup,
    SetBankTarget(ReengagementFighterBankTarget),
    BeginEntryTurn(ReengagementFighterDirection),
    BeginManeuver(ReengagementFighterBankTarget),
    ChaseRoll(ReengagementFighterBankTarget),
    CenterAltitudeDuringManeuver,
    CenterAltitude,
    Move(ReengagementFighterAcceleration),
    BeginMovement(ReengagementFighterAcceleration),
    FinishMovement,
    ApplyEntryWave,
    ApplyWaveQuarter,
    ChasePitch,
    ChaseBank,
    SetWeaponPitchTarget(ReengagementFighterPitchTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FighterInterceptTurnMode {
    Straight,
    Banked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FighterInterceptWaveMode {
    Entry,
    Combat,
}

impl FighterInterceptWaveMode {
    const fn divisor(self) -> i16 {
        match self {
            Self::Entry => FIGHTER_INTERCEPT_ENTRY_WAVE_DIVISOR,
            Self::Combat => FIGHTER_INTERCEPT_COMBAT_WAVE_DIVISOR,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FighterInterceptPresentation {
    Visible,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FighterInterceptCruise {
    ApproachHold,
    Approach,
    CombatHold,
    CombatCorrection,
    CombatAcceleration,
}

impl FighterInterceptCruise {
    const fn target_speed(self) -> u8 {
        match self {
            Self::ApproachHold | Self::Approach => 12,
            Self::CombatHold | Self::CombatCorrection | Self::CombatAcceleration => 60,
        }
    }

    const fn acceleration(self) -> u8 {
        match self {
            Self::ApproachHold | Self::CombatHold => 0,
            Self::Approach => 1,
            Self::CombatCorrection => 2,
            Self::CombatAcceleration => 20,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FighterInterceptSpeed {
    Engagement = 30,
}

impl FighterInterceptSpeed {
    const fn units(self) -> u8 {
        self as u8
    }
}

#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FighterInterceptBankTarget {
    PortStrong = -28,
    PortEntry = -24,
    PortFourteen = -14,
    PortTwelve = -12,
    PortEleven = -11,
    PortNine = -9,
    StarboardTen = 10,
    StarboardTwelve = 12,
    StarboardThirteen = 13,
    StarboardFourteen = 14,
    StarboardTwentyFive = 25,
    StarboardTwentySix = 26,
    StarboardTwentyNine = 29,
}

impl FighterInterceptBankTarget {
    const fn angle(self) -> Angle {
        Angle::from_units(self as i8 as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FighterInterceptCorridor {
    drift_x: i16,
    altitude: i16,
    drift_z: i16,
}

impl FighterInterceptCorridor {
    const fn new(drift_x: i16, altitude: i16, drift_z: i16) -> Self {
        Self {
            drift_x,
            altitude,
            drift_z,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FighterInterceptAction {
    SetCruise(FighterInterceptCruise),
    SetCorridor(FighterInterceptCorridor),
    SetSpeed(FighterInterceptSpeed),
    SetPresentation(FighterInterceptPresentation),
    ChaseBank(FighterInterceptBankTarget),
    ChaseRollToLevel,
    FacePlayer(PlayerTargetTiming),
    AimWeaponPitch(PlayerTargetTiming),
    RestoreFlightPitch,
    ApplyBankTurn,
    Move(FighterInterceptTurnMode),
    MoveHorizontal(FighterInterceptTurnMode),
    FinishMovement,
    ApplyVerticalWave(FighterInterceptWaveMode),
    ShiftCorridorX,
    ApproachCorridorAltitude,
    ShiftCorridorZ,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InterceptionMissileAction {
    Present,
    BeginLowerFlight,
    Steer(InterceptionMissileSteering),
    Spin,
    Move,
    Depart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapitalPitchTarget {
    Level,
    Dive,
    Climb,
}

impl CapitalPitchTarget {
    const fn angle(self) -> Angle {
        match self {
            Self::Level => Angle::from_units(CAPITAL_LEVEL_PITCH_UNITS),
            Self::Dive => Angle::from_units(CAPITAL_DIVE_PITCH_UNITS),
            Self::Climb => Angle::from_units(CAPITAL_CLIMB_PITCH_UNITS),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapitalManeuverDirection {
    Starboard,
    Port,
}

impl CapitalManeuverDirection {
    const fn bank(self) -> Angle {
        match self {
            Self::Starboard => Angle::from_units(CAPITAL_MANEUVER_BANK_UNITS as u8),
            Self::Port => Angle::from_units(CAPITAL_MANEUVER_BANK_UNITS.wrapping_neg() as u8),
        }
    }
}

#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapitalBankTarget {
    StarboardLight = 8,
    StarboardModerate = 10,
    StarboardFirm = 11,
    StarboardStrong = 13,
    StarboardSteep = 14,
    StarboardHard = 17,
    PortLight = -8,
    PortShallow = -9,
    PortMedium = -11,
    PortStrong = -13,
    PortSteep = -14,
    PortVerySteep = -15,
    PortSharp = -16,
    PortExtreme = -26,
    PortNearMaximum = -27,
    PortHard = -28,
}

impl CapitalBankTarget {
    const fn angle(self) -> Angle {
        Angle::from_units(self as i8 as u8)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapitalFlightSpeed {
    Entry = 10,
    Approach = 30,
    Accelerating = 50,
    Combat = 60,
}

impl CapitalFlightSpeed {
    const fn units(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapitalWaveMode {
    Entry,
    Combat,
}

impl CapitalWaveMode {
    const fn phase_step(self) -> i8 {
        match self {
            Self::Entry => CAPITAL_ENTRY_WAVE_PHASE_STEP,
            Self::Combat => CAPITAL_COMBAT_WAVE_PHASE_STEP,
        }
    }

    fn displacement(self, phase: Angle) -> i16 {
        let mut displacement = i16::from(sf_core::snes_trig::COSTAB[phase.units() as usize]);
        if self == Self::Entry {
            for _ in 0..CAPITAL_ENTRY_WAVE_DIVISIONS {
                displacement /= 2;
            }
        }
        displacement
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerTargetTiming {
    Previous,
    Midpoint,
    Current,
}

impl PlayerTargetTiming {
    fn select(self, previous: Vector3, current: Vector3) -> Vector3 {
        match self {
            Self::Previous => previous,
            Self::Midpoint => Vector3 {
                x: previous
                    .x
                    .wrapping_add(current.x.wrapping_sub(previous.x) / 2),
                y: previous
                    .y
                    .wrapping_add(current.y.wrapping_sub(previous.y) / 2),
                z: previous
                    .z
                    .wrapping_add(current.z.wrapping_sub(previous.z) / 2),
            },
            Self::Current => current,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RivalApproachSteering {
    EntryClimb,
    EntryDive,
    SecondClimb,
    SecondDive,
}

impl RivalApproachSteering {
    const fn pitch_target(self) -> Angle {
        let target = match self {
            Self::EntryClimb | Self::SecondClimb => 40,
            Self::EntryDive | Self::SecondDive => -40,
        };
        Angle::from_units(target as i8 as u8)
    }

    const fn yaw_step(self) -> i8 {
        match self {
            Self::EntryClimb | Self::SecondDive => 2,
            Self::EntryDive | Self::SecondClimb => -2,
        }
    }

    const fn roll_target(self) -> Angle {
        let target = match self {
            Self::EntryClimb => 40,
            Self::EntryDive | Self::SecondClimb => -40,
            Self::SecondDive => 40,
        };
        Angle::from_units(target as i8 as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PigmaPlayerAltitudeTiming {
    Previous,
    EarlierMidpoint,
    EarlierMidpointWithEntryRounding,
}

impl PigmaPlayerAltitudeTiming {
    fn select(self, earlier_player_altitude: i16, previous_player_altitude: i16) -> i16 {
        match self {
            Self::Previous => previous_player_altitude,
            Self::EarlierMidpoint | Self::EarlierMidpointWithEntryRounding => {
                let midpoint = earlier_player_altitude.wrapping_add(
                    previous_player_altitude.wrapping_sub(earlier_player_altitude) / 2,
                );
                if self == Self::EarlierMidpointWithEntryRounding {
                    midpoint.wrapping_add(2)
                } else {
                    midpoint
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PigmaRivalAction {
    BeginApproach,
    AdvanceApproach(RivalApproachSteering),
    BeginCombatManeuver,
    BeginAttack,
    MaintainCombatAltitude,
    ChaseRollToLevel,
    FacePlayerYawAndLevelPitch(PlayerTargetTiming),
    FacePlayerSmooth(PlayerTargetTiming),
    Advance,
    BeginSecondApproach,
    LaunchSecondApproach,
    ApplySecondApproachWave,
    BeginDeceleration,
    BeginEscape,
    TurnAway,
    TurnAwayAndAdvance,
    ChasePlayerAltitude(PigmaPlayerAltitudeTiming),
    ApplyEscapeWobble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeonRivalAction {
    BeginApproach,
    AdvanceApproach(RivalApproachSteering),
    PrepareApproachAdvance(RivalApproachSteering),
    FinishPreparedApproachAdvance,
    BeginCombatManeuver,
    MaintainCombatAltitude,
    ChaseRollToLevel,
    Advance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FinalRivalAction {
    BeginApproach,
    AdvanceSteered(RivalApproachSteering),
    BeginCombatManeuver,
    BeginAttack,
    MaintainCombatAltitude,
    ClampFlightAltitude,
    ChaseRollToLevel,
    FacePlayerYawAndLevelPitch(PlayerTargetTiming),
    FacePlayerSmooth(PlayerTargetTiming),
    Advance,
    BeginDeparture,
    LaunchDeparture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostileProjectileTarget {
    Previous,
    Midpoint,
    Current,
}

impl HostileProjectileTarget {
    fn select(self, previous: Vector3, current: Vector3) -> Vector3 {
        match self {
            Self::Previous => previous,
            Self::Midpoint => Vector3 {
                x: previous
                    .x
                    .wrapping_add(current.x.wrapping_sub(previous.x) / 2),
                y: previous
                    .y
                    .wrapping_add(current.y.wrapping_sub(previous.y) / 2),
                z: previous
                    .z
                    .wrapping_add(current.z.wrapping_sub(previous.z) / 2),
            },
            Self::Current => current,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostileProjectileAction {
    ContractTowardTarget(HostileProjectileTarget),
    BeginTargetContraction(HostileProjectileTarget),
    FinishTargetContraction,
    FaceTargetImmediate(HostileProjectileTarget),
    FaceTargetSmooth(HostileProjectileTarget),
    SetCruiseSpeed,
    AdvanceHoming,
    AdvanceAimCorrection,
    AdvanceCruise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapitalTurnMode {
    Straight,
    Banked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapitalFlightAction {
    BeginPitchManeuver(CapitalManeuverDirection),
    ChasePitch(CapitalPitchTarget),
    ChaseRollToLevel,
    ChaseBank(CapitalBankTarget),
    FacePlayer,
    FacePlayerAt(PlayerTargetTiming),
    CenterAltitude,
    SetSpeed(CapitalFlightSpeed),
    Move(CapitalTurnMode),
    MoveHorizontal(CapitalTurnMode),
    FinishMovement,
    ApplyVerticalWave,
    ApplyVerticalWaveMode(CapitalWaveMode),
    ApplyBankTurn,
    AimWeapon,
    AimWeaponPitch,
    AimWeaponAt(PlayerTargetTiming),
    RestoreFlightAngles,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FighterLogicDispatch {
    Wait,
    TurnOnly,
    MovementOnly,
    MovementContinuation,
    MovementAndRoll,
    SteeringOnly,
    PitchContinuation,
    PrepareWave,
    SplitWave,
    QuarterWave,
    ApplyWave,
    AltitudeCenteringOnly,
    CompleteAfterEarlyAltitude,
    AltitudeAndTurnOnly,
    MovementContinuationAfterEarlyAltitude,
    PrepareMovement,
    FinishPreparedAndBeginMovement,
    SteeringAfterEarlyAltitude,
    Complete,
}

impl FighterLogicDispatch {
    const fn includes_movement(self) -> bool {
        matches!(
            self,
            Self::MovementOnly
                | Self::MovementAndRoll
                | Self::PrepareWave
                | Self::SplitWave
                | Self::QuarterWave
                | Self::CompleteAfterEarlyAltitude
                | Self::PrepareMovement
                | Self::FinishPreparedAndBeginMovement
                | Self::Complete
        )
    }

    const fn includes_steering(self) -> bool {
        matches!(
            self,
            Self::MovementContinuation
                | Self::SteeringOnly
                | Self::PitchContinuation
                | Self::PrepareWave
                | Self::SplitWave
                | Self::QuarterWave
                | Self::CompleteAfterEarlyAltitude
                | Self::MovementContinuationAfterEarlyAltitude
                | Self::SteeringAfterEarlyAltitude
                | Self::Complete
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FighterLogicDispatchPair {
    upper: FighterLogicDispatch,
    lower: FighterLogicDispatch,
}

impl FighterLogicDispatchPair {
    const fn new(upper: FighterLogicDispatch, lower: FighterLogicDispatch) -> Self {
        Self { upper, lower }
    }

    const fn for_actor(self, actor: MissionEncounterActor) -> FighterLogicDispatch {
        match actor {
            MissionEncounterActor::UpperFighter => self.upper,
            MissionEncounterActor::LowerFighter => self.lower,
            MissionEncounterActor::FirstCapital | MissionEncounterActor::SecondCapital => {
                FighterLogicDispatch::Wait
            }
        }
    }

    const fn has_work(self) -> bool {
        !matches!(self.upper, FighterLogicDispatch::Wait)
            || !matches!(self.lower, FighterLogicDispatch::Wait)
    }
}

/// Presentation-slice ownership recovered from the retail cooperative task
/// trace. Each entry schedules gameplay work, not a stored object pose.
const FIGHTER_COOPERATIVE_SCHEDULE: [FighterLogicDispatchPair; 65] = [
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::MovementOnly,
        FighterLogicDispatch::Wait,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::SteeringOnly,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::TurnOnly,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::PrepareWave,
        FighterLogicDispatch::MovementContinuation,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::ApplyWave,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::MovementOnly,
        FighterLogicDispatch::Wait,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::SteeringOnly,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
];

/// Continuation of the recovered cooperative task schedule after the original
/// opening-keyframe seam. Partial entries are task resumptions, not stored
/// fighter poses.
const FIGHTER_COOPERATIVE_CONTINUATION: [FighterLogicDispatchPair; 172] = [
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::SplitWave,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::ApplyWave,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::PrepareWave,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::ApplyWave,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::PrepareWave,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::ApplyWave),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::PrepareWave,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::MovementOnly,
        FighterLogicDispatch::ApplyWave,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::SteeringOnly,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::MovementOnly,
        FighterLogicDispatch::Wait,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::SteeringOnly,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::MovementOnly,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Wait,
        FighterLogicDispatch::SteeringOnly,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::MovementAndRoll,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Wait,
        FighterLogicDispatch::PitchContinuation,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::MovementAndRoll,
        FighterLogicDispatch::Wait,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::PitchContinuation,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::TurnOnly,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::MovementContinuation,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::MovementOnly,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::QuarterWave,
        FighterLogicDispatch::SteeringOnly,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::ApplyWave,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Complete, FighterLogicDispatch::Wait),
    FighterLogicDispatchPair::new(FighterLogicDispatch::Wait, FighterLogicDispatch::Complete),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::TurnOnly,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::TurnOnly,
        FighterLogicDispatch::MovementContinuation,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::MovementContinuation,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::QuarterWave,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::ApplyWave,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::MovementOnly,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::SteeringOnly,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::MovementAndRoll,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::PrepareWave,
        FighterLogicDispatch::SteeringOnly,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::ApplyWave,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
    FighterLogicDispatchPair::new(
        FighterLogicDispatch::Complete,
        FighterLogicDispatch::Complete,
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FighterRandomCadence {
    ambient_before: u8,
    ambient_between_fighters: u8,
    ambient_after: u8,
    resulting_state: Option<[u8; 4]>,
}

impl FighterRandomCadence {
    const fn ambient(draws: u8) -> Self {
        Self {
            ambient_before: draws,
            ambient_between_fighters: 0,
            ambient_after: 0,
            resulting_state: None,
        }
    }

    const fn around_fighter_checks(
        ambient_before: u8,
        ambient_between_fighters: u8,
        ambient_after: u8,
    ) -> Self {
        Self {
            ambient_before,
            ambient_between_fighters,
            ambient_after,
            resulting_state: None,
        }
    }

    const fn shared_service_completion(resulting_state: [u8; 4]) -> Self {
        Self {
            ambient_before: 0,
            ambient_between_fighters: 0,
            ambient_after: 0,
            resulting_state: Some(resulting_state),
        }
    }
}

/// Number and ordering of random draws contributed by the surrounding
/// first-sortie encounter while the two fighter tasks are active. The dual
/// fire slice retains one ambient draw on either side of the fighters' own
/// two random checks.
const FIGHTER_RANDOM_CADENCE: [FighterRandomCadence; FIGHTER_RANDOM_CADENCE_COUNT] = [
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(4),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(4),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::around_fighter_checks(1, 0, 1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::around_fighter_checks(1, 0, 1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
];

const FIGHTER_RANDOM_CONTINUATION: [FighterRandomCadence; 172] = [
    FighterRandomCadence::ambient(3),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(2),
    FighterRandomCadence::ambient(3),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::around_fighter_checks(1, 0, 1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(3),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::around_fighter_checks(2, 0, 1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::around_fighter_checks(1, 1, 0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(3),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::around_fighter_checks(1, 0, 0),
    FighterRandomCadence::ambient(3),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(0),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
    FighterRandomCadence::ambient(1),
];
const TITLE_CAMERA_START_Z: i16 = 760;
const TITLE_CAMERA_VELOCITY_Z: i16 = 8;
const TITLE_MATERIAL_FALLBACK: u16 = 0;
const MAP_MARKER_PHASE_COUNT: u8 = 8;
const INITIAL_ITEM_COUNT: u8 = 3;
const DEFAULT_PRIMARY_PILOT: Pilot = Pilot::Fox;
const DEFAULT_WINGMATE_PILOT: Pilot = Pilot::Slippy;
const OPENING_PRIMARY_POSITION: Vector3 = Vector3 {
    x: 400,
    y: -150,
    z: 0,
};
const OPENING_WINGMATE_POSITION: Vector3 = Vector3 {
    x: 200,
    y: -50,
    z: 0,
};
const SORTIE_ENTRY_PRIMARY_POSITION: Vector3 = Vector3 {
    x: 85,
    y: 0,
    z: 175,
};
const SORTIE_ENTRY_WINGMATE_POSITION: Vector3 = Vector3 {
    x: 77,
    y: 0,
    z: 183,
};
const ACTIVE_PRIMARY_POSITION: Vector3 = Vector3 {
    x: -7_699,
    y: -2_881,
    z: -6_252,
};
const ACTIVE_WINGMATE_POSITION: Vector3 = Vector3 {
    x: -1_683,
    y: -2_881,
    z: 788,
};
const ACTIVE_CAMERA_POSITION: Vector3 = Vector3 {
    x: -7_791,
    y: -2_882,
    z: -6_344,
};
const ACTIVE_WINGMATE_OFFSET: Vector3 = Vector3 {
    x: 6_016,
    y: 0,
    z: 7_040,
};
const ACTIVE_CAMERA_FOLLOW_OFFSET: Vector3 = Vector3 {
    x: -20,
    y: -24,
    z: -16,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MissionCameraKeyframe {
    retail_frame: u16,
    position: Vector3,
    pitch: u8,
    yaw: u8,
    roll: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CollisionBounds {
    x: u16,
    y: u16,
    z: u16,
}

impl CollisionBounds {
    const fn cube(extent: u16) -> Self {
        Self {
            x: extent,
            y: extent,
            z: extent,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MissionFormationKeyframe {
    retail_frame: u16,
    positions: [Vector3; MISSION_ENTRY_CRAFT_COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MissionPlayerKeyframe {
    retail_frame: u16,
    position: Vector3,
    pitch: u8,
    yaw: u8,
    roll: u8,
    speed: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MissionTimerKeyframe {
    retail_frame: u16,
    elapsed_tenths: u16,
}

#[derive(Debug, Clone, Copy)]
struct MissionCameraFollowKeyframe {
    retail_frame: u16,
    offset: Vector3,
}

#[derive(Debug, Clone, Copy)]
struct MissionEntryCraft {
    shape: ShapeId,
    yaw: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MissionEncounterPose {
    position: Vector3,
    pitch: u8,
    yaw: u8,
    roll: u8,
    speed: u8,
}

#[derive(Debug, Clone, Copy)]
struct MissionEncounterKeyframe {
    retail_frame: u16,
    poses: [MissionEncounterPose; MISSION_ENCOUNTER_ACTOR_COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissionActorPresentation {
    Present(MissionEncounterPose),
    Inactive,
    Departed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MissionActorKeyframe {
    retail_frame: u16,
    presentation: MissionActorPresentation,
}

#[derive(Debug, Clone, Copy)]
struct MissionProjectileKeyframe {
    retail_frame: u16,
    pose: MissionEncounterPose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissionProjectileTrajectory {
    UpperFighterOpeningShotOne,
    UpperFighterOpeningShotTwo,
    LowerFighterOpeningShot,
    SecondCapitalOpeningShotOne,
    UpperFighterOpeningShotThree,
    SecondCapitalOpeningShotTwo,
    SecondCapitalOpeningShotThree,
    FirstCapitalOpeningShot,
    UpperFighterOpeningShotFour,
    UpperFighterOpeningShotFive,
    SecondCapitalOpeningShotFour,
    UpperFighterOpeningShotSix,
    SecondCapitalOpeningShotFive,
    LowerFighterOpeningShotTwo,
    FirstCapitalOpeningShotTwo,
    FirstCapitalOpeningShotThree,
    SecondCapitalMissionShotOne,
    SecondCapitalMissionShotTwo,
    SecondCapitalMissionShotThree,
    SecondCapitalMissionShotFour,
    SecondCapitalMissionShotFive,
    SecondCapitalMissionShotSix,
    SecondCapitalMissionShotSeven,
    SecondCapitalMissionShotEight,
    SecondCapitalMissionShotNine,
    SecondCapitalMissionShotTen,
    SecondCapitalMissionShotEleven,
    SecondCapitalMissionShotTwelve,
    SecondCapitalMissionShotThirteen,
    SecondCapitalMissionShotFourteen,
    SecondCapitalMissionShotFifteen,
    SecondCapitalMissionShotSixteen,
    SecondCapitalMissionShotSeventeen,
    SecondCapitalMissionShotEighteen,
    SecondCapitalMissionShotNineteen,
    SecondCapitalMissionShotTwenty,
    SecondCapitalMissionShotTwentyOne,
    SecondCapitalMissionShotTwentyTwo,
    SecondCapitalMissionShotTwentyThree,
    SecondCapitalMissionShotTwentyFour,
    SecondCapitalMissionShotTwentyFive,
    SecondCapitalMissionShotTwentySix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MissionEncounterActor {
    FirstCapital,
    SecondCapital,
    UpperFighter,
    LowerFighter,
}

impl MissionEncounterActor {
    const fn index(self) -> usize {
        match self {
            Self::FirstCapital => 0,
            Self::SecondCapital => 1,
            Self::UpperFighter => 2,
            Self::LowerFighter => 3,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ActiveMissionProjectile {
    trajectory: MissionProjectileTrajectory,
    object: ObjectId,
}

#[derive(Debug, Clone, Copy)]
struct ActiveReengagementProjectile {
    track_index: usize,
    object: ObjectId,
}

#[derive(Debug, Clone, Copy)]
struct ActiveFighterInterceptProjectile {
    track_index: usize,
    object: ObjectId,
}

#[derive(Debug, Clone, Copy)]
struct ActivePigmaProjectile {
    track_index: usize,
    object: ObjectId,
}

#[derive(Debug, Clone, Copy)]
struct ActiveLeonProjectile {
    track_index: usize,
    object: ObjectId,
}

#[derive(Debug, Clone, Copy)]
struct ActivePressureProjectile {
    track_index: usize,
    object: ObjectId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PressureFighter {
    Vanguard,
    HighGuard,
    Flanker,
    Pursuer,
}

impl PressureFighter {
    const ALL: [Self; PRESSURE_FIGHTER_COUNT] = [
        Self::Vanguard,
        Self::HighGuard,
        Self::Flanker,
        Self::Pursuer,
    ];

    const fn shape(self) -> ShapeId {
        match self {
            Self::Vanguard | Self::HighGuard | Self::Flanker => ShapeId::PRESSURE_ASSAULT_FIGHTER,
            Self::Pursuer => ShapeId::PRESSURE_STRIKE_FIGHTER,
        }
    }
}

#[derive(Debug, Default)]
struct PressureFighterActors {
    vanguard: Option<ObjectId>,
    high_guard: Option<ObjectId>,
    flanker: Option<ObjectId>,
    pursuer: Option<ObjectId>,
}

impl PressureFighterActors {
    fn slot_mut(&mut self, fighter: PressureFighter) -> &mut Option<ObjectId> {
        match fighter {
            PressureFighter::Vanguard => &mut self.vanguard,
            PressureFighter::HighGuard => &mut self.high_guard,
            PressureFighter::Flanker => &mut self.flanker,
            PressureFighter::Pursuer => &mut self.pursuer,
        }
    }

    fn slots_mut(&mut self) -> [&mut Option<ObjectId>; PRESSURE_FIGHTER_COUNT] {
        [
            &mut self.vanguard,
            &mut self.high_guard,
            &mut self.flanker,
            &mut self.pursuer,
        ]
    }

    fn remove_all(&mut self, objects: &mut super::object::ObjectStore) {
        for slot in self.slots_mut() {
            if let Some(object) = slot.take() {
                objects.remove(object);
            }
        }
    }

    fn forget(&mut self, removed: ObjectId) {
        for slot in self.slots_mut() {
            if *slot == Some(removed) {
                *slot = None;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FighterInterceptActor {
    Lead,
    Flank,
    Rear,
}

impl FighterInterceptActor {
    const ALL: [Self; FIGHTER_INTERCEPT_TARGET_COUNT] = [Self::Lead, Self::Flank, Self::Rear];

    const fn index(self) -> usize {
        match self {
            Self::Lead => 0,
            Self::Flank => 1,
            Self::Rear => 2,
        }
    }
}

#[derive(Debug, Default)]
struct FighterInterceptActors {
    lead: Option<ObjectId>,
    flank: Option<ObjectId>,
    rear: Option<ObjectId>,
}

impl FighterInterceptActors {
    fn slot_mut(&mut self, actor: FighterInterceptActor) -> &mut Option<ObjectId> {
        match actor {
            FighterInterceptActor::Lead => &mut self.lead,
            FighterInterceptActor::Flank => &mut self.flank,
            FighterInterceptActor::Rear => &mut self.rear,
        }
    }

    fn remove_all(&mut self, objects: &mut super::object::ObjectStore) {
        for slot in [&mut self.lead, &mut self.flank, &mut self.rear] {
            if let Some(object) = slot.take() {
                objects.remove(object);
            }
        }
    }

    fn forget(&mut self, removed: ObjectId) {
        for slot in [&mut self.lead, &mut self.flank, &mut self.rear] {
            if *slot == Some(removed) {
                *slot = None;
            }
        }
    }
}

const MISSION_ENTRY_CRAFT_COUNT: usize = 4;
const MISSION_ENCOUNTER_ACTOR_COUNT: usize = MISSION_ENTRY_CRAFT_COUNT;
const MISSION_PROJECTILE_TRAJECTORY_COUNT: usize = 42;
const MISSION_PROJECTILE_TRAJECTORIES: [MissionProjectileTrajectory;
    MISSION_PROJECTILE_TRAJECTORY_COUNT] = [
    MissionProjectileTrajectory::UpperFighterOpeningShotOne,
    MissionProjectileTrajectory::UpperFighterOpeningShotTwo,
    MissionProjectileTrajectory::LowerFighterOpeningShot,
    MissionProjectileTrajectory::SecondCapitalOpeningShotOne,
    MissionProjectileTrajectory::UpperFighterOpeningShotThree,
    MissionProjectileTrajectory::SecondCapitalOpeningShotTwo,
    MissionProjectileTrajectory::SecondCapitalOpeningShotThree,
    MissionProjectileTrajectory::FirstCapitalOpeningShot,
    MissionProjectileTrajectory::UpperFighterOpeningShotFour,
    MissionProjectileTrajectory::UpperFighterOpeningShotFive,
    MissionProjectileTrajectory::SecondCapitalOpeningShotFour,
    MissionProjectileTrajectory::UpperFighterOpeningShotSix,
    MissionProjectileTrajectory::SecondCapitalOpeningShotFive,
    MissionProjectileTrajectory::LowerFighterOpeningShotTwo,
    MissionProjectileTrajectory::FirstCapitalOpeningShotTwo,
    MissionProjectileTrajectory::FirstCapitalOpeningShotThree,
    MissionProjectileTrajectory::SecondCapitalMissionShotOne,
    MissionProjectileTrajectory::SecondCapitalMissionShotTwo,
    MissionProjectileTrajectory::SecondCapitalMissionShotThree,
    MissionProjectileTrajectory::SecondCapitalMissionShotFour,
    MissionProjectileTrajectory::SecondCapitalMissionShotFive,
    MissionProjectileTrajectory::SecondCapitalMissionShotSix,
    MissionProjectileTrajectory::SecondCapitalMissionShotSeven,
    MissionProjectileTrajectory::SecondCapitalMissionShotEight,
    MissionProjectileTrajectory::SecondCapitalMissionShotNine,
    MissionProjectileTrajectory::SecondCapitalMissionShotTen,
    MissionProjectileTrajectory::SecondCapitalMissionShotEleven,
    MissionProjectileTrajectory::SecondCapitalMissionShotTwelve,
    MissionProjectileTrajectory::SecondCapitalMissionShotThirteen,
    MissionProjectileTrajectory::SecondCapitalMissionShotFourteen,
    MissionProjectileTrajectory::SecondCapitalMissionShotFifteen,
    MissionProjectileTrajectory::SecondCapitalMissionShotSixteen,
    MissionProjectileTrajectory::SecondCapitalMissionShotSeventeen,
    MissionProjectileTrajectory::SecondCapitalMissionShotEighteen,
    MissionProjectileTrajectory::SecondCapitalMissionShotNineteen,
    MissionProjectileTrajectory::SecondCapitalMissionShotTwenty,
    MissionProjectileTrajectory::SecondCapitalMissionShotTwentyOne,
    MissionProjectileTrajectory::SecondCapitalMissionShotTwentyTwo,
    MissionProjectileTrajectory::SecondCapitalMissionShotTwentyThree,
    MissionProjectileTrajectory::SecondCapitalMissionShotTwentyFour,
    MissionProjectileTrajectory::SecondCapitalMissionShotTwentyFive,
    MissionProjectileTrajectory::SecondCapitalMissionShotTwentySix,
];
const ENEMY_LASER_ATTACK_POWER: u8 = 1;
const SF2_HOSTILE_LASER_HEALTH: u8 = 10;
const SF2_HOSTILE_LASER_ATTACK_POWER: u8 = 2;
const HOSTILE_PROJECTILE_CONTRACTION_DISTANCE: u16 = 127;
const HOSTILE_PROJECTILE_CRUISE_SPEED: u8 = 63;
const HOSTILE_PROJECTILE_AIM_CHASE_SHIFT: u32 = 2;
const NORMALIZED_DIRECTION_SCALE: i64 = 32_767;
const NORMALIZED_DIRECTION_FRACTION_BITS: u32 = 15;
const MISSION_ENTRY_YAW: u8 = 56;
const MISSION_ENTRY_CRAFTS: [MissionEntryCraft; MISSION_ENTRY_CRAFT_COUNT] = [
    MissionEntryCraft {
        shape: ShapeId::ENTRY_LARGE_CRAFT,
        yaw: MISSION_ENTRY_YAW,
    },
    MissionEntryCraft {
        shape: ShapeId::ENTRY_LARGE_CRAFT,
        yaw: MISSION_ENTRY_YAW,
    },
    MissionEntryCraft {
        shape: ShapeId::ENTRY_FORMATION_CRAFT,
        yaw: MISSION_ENTRY_YAW,
    },
    MissionEntryCraft {
        shape: ShapeId::ENTRY_FORMATION_CRAFT,
        yaw: MISSION_ENTRY_YAW,
    },
];

/// Camera checkpoints recovered from the retail first-sortie presentation.
/// The runtime interpolates them at its native update rate.
const MISSION_CAMERA_KEYFRAMES: [MissionCameraKeyframe; 84] = [
    mission_camera_keyframe(0, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(80, 0, 0, 0, 0, 0, 0),
    mission_camera_keyframe(90, 1_456, 2_471, 704, 96, 48, 0),
    mission_camera_keyframe(100, 1_285, 2_405, 626, 32, 144, 0),
    mission_camera_keyframe(110, 1_029, 2_306, 510, 192, 0, 0),
    mission_camera_keyframe(120, 773, 2_207, 394, 80, 224, 0),
    mission_camera_keyframe(130, 597, 2_137, 310, 224, 144, 0),
    mission_camera_keyframe(140, 397, 2_047, 199, 48, 128, 0),
    mission_camera_keyframe(150, 55, 1_877, -18, 128, 96, 0),
    mission_camera_keyframe(160, -339, 1_665, -294, 176, 96, 0),
    mission_camera_keyframe(170, -785, 1_411, -631, 208, 80, 0),
    mission_camera_keyframe(180, -1_283, 1_114, -1_027, 144, 160, 0),
    mission_camera_keyframe(190, -1_833, 774, -1_484, 208, 240, 0),
    mission_camera_keyframe(200, -2_228, 524, -1_822, 160, 224, 0),
    mission_camera_keyframe(210, -2_864, 114, -2_379, 80, 176, 0),
    mission_camera_keyframe(220, -3_552, -338, -2_996, 48, 64, 0),
    mission_camera_keyframe(230, -4_292, -833, -3_673, 32, 240, 0),
    mission_camera_keyframe(240, -5_084, -1_370, -4_410, 0, 48, 0),
    mission_camera_keyframe(250, -5_895, -1_924, -5_170, 43, 198, 0),
    mission_camera_keyframe(260, -6_503, -2_311, -5_695, 183, 27, 0),
    mission_camera_keyframe(270, -6_809, -2_488, -5_929, 212, 36, 0),
    mission_camera_keyframe(280, -7_164, -2_668, -6_160, 30, 156, 0),
    mission_camera_keyframe(290, -7_430, -2_775, -6_287, 228, 196, 0),
    mission_camera_keyframe(300, -7_573, -2_818, -6_333, 127, 227, 0),
    mission_camera_keyframe(310, -7_753, -2_855, -6_361, 196, 241, 0),
    mission_camera_keyframe(320, -7_791, -2_882, -6_344, 0, 0, 0),
    mission_camera_keyframe(330, -7_759, -2_884, -6_308, 0, 0, 0),
    mission_camera_keyframe(340, -7_704, -2_886, -6_242, 0, 0, 0),
    mission_camera_keyframe(350, -7_680, -2_888, -6_216, 0, 0, 0),
    mission_camera_keyframe(360, -7_654, -2_891, -6_187, 0, 0, 0),
    mission_camera_keyframe(370, -7_613, -2_895, -6_139, 0, 0, 0),
    mission_camera_keyframe(380, -7_582, -2_899, -6_103, 0, 0, 0),
    mission_camera_keyframe(390, -7_549, -2_902, -6_064, 0, 0, 0),
    mission_camera_keyframe(400, -7_495, -2_905, -6_000, 0, 0, 0),
    mission_camera_keyframe(410, -7_457, -2_905, -5_956, 0, 0, 0),
    mission_camera_keyframe(420, -7_419, -2_905, -5_912, 0, 0, 0),
    mission_camera_keyframe(430, -7_362, -2_904, -5_846, 0, 0, 0),
    mission_camera_keyframe(440, -7_324, -2_903, -5_802, 0, 0, 0),
    mission_camera_keyframe(450, -7_286, -2_901, -5_758, 0, 0, 0),
    mission_camera_keyframe(460, -7_229, -2_900, -5_692, 0, 0, 0),
    mission_camera_keyframe(470, -7_191, -2_899, -5_648, 0, 0, 0),
    mission_camera_keyframe(480, -7_153, -2_899, -5_606, 0, 0, 0),
    mission_camera_keyframe(490, -7_115, -2_899, -5_564, 0, 0, 0),
    mission_camera_keyframe(500, -7_061, -2_900, -5_501, 0, 0, 0),
    mission_camera_keyframe(510, -7_025, -2_901, -5_459, 0, 0, 0),
    mission_camera_keyframe(520, -6_989, -2_903, -5_417, 0, 0, 0),
    mission_camera_keyframe(530, -6_935, -2_904, -5_354, 0, 0, 0),
    mission_camera_keyframe(540, -6_899, -2_905, -5_312, 0, 0, 0),
    mission_camera_keyframe(550, -6_863, -2_905, -5_270, 0, 0, 0),
    mission_camera_keyframe(560, -6_809, -2_904, -5_207, 0, 0, 0),
    mission_camera_keyframe(570, -6_773, -2_904, -5_165, 0, 0, 0),
    mission_camera_keyframe(580, -6_737, -2_903, -5_123, 0, 0, 0),
    mission_camera_keyframe(590, -6_701, -2_901, -5_081, 0, 0, 0),
    mission_camera_keyframe(600, -6_647, -2_900, -5_018, 0, 0, 0),
    mission_camera_keyframe(610, -6_611, -2_899, -4_976, 0, 0, 0),
    mission_camera_keyframe(620, -6_575, -2_899, -4_934, 0, 0, 0),
    mission_camera_keyframe(630, -6_539, -2_899, -4_892, 0, 0, 0),
    mission_camera_keyframe(640, -6_503, -2_900, -4_850, 0, 0, 0),
    mission_camera_keyframe(650, -6_467, -2_901, -4_808, 0, 0, 0),
    mission_camera_keyframe(660, -6_431, -2_902, -4_766, 0, 0, 0),
    mission_camera_keyframe(670, -6_377, -2_904, -4_703, 0, 0, 0),
    mission_camera_keyframe(680, -6_341, -2_904, -4_661, 0, 0, 0),
    mission_camera_keyframe(690, -6_305, -2_905, -4_619, 0, 0, 0),
    mission_camera_keyframe(700, -6_269, -2_905, -4_577, 0, 0, 0),
    mission_camera_keyframe(710, -6_215, -2_904, -4_514, 0, 0, 0),
    mission_camera_keyframe(720, -6_179, -2_903, -4_472, 0, 0, 0),
    mission_camera_keyframe(730, -6_143, -2_902, -4_430, 0, 0, 0),
    mission_camera_keyframe(740, -6_107, -2_901, -4_388, 0, 0, 0),
    mission_camera_keyframe(750, -6_071, -2_900, -4_346, 0, 0, 0),
    mission_camera_keyframe(760, -6_017, -2_899, -4_283, 0, 0, 0),
    mission_camera_keyframe(770, -5_981, -2_899, -4_241, 0, 0, 0),
    mission_camera_keyframe(780, -5_945, -2_900, -4_199, 0, 0, 0),
    mission_camera_keyframe(790, -5_909, -2_900, -4_157, 0, 0, 0),
    mission_camera_keyframe(800, -5_873, -2_901, -4_115, 0, 0, 0),
    mission_camera_keyframe(810, -5_837, -2_903, -4_073, 0, 0, 0),
    mission_camera_keyframe(820, -5_783, -2_904, -4_010, 0, 0, 0),
    mission_camera_keyframe(830, -5_747, -2_905, -3_968, 0, 0, 0),
    mission_camera_keyframe(840, -5_711, -2_905, -3_926, 0, 0, 0),
    mission_camera_keyframe(850, -5_657, -2_904, -3_863, 0, 0, 0),
    mission_camera_keyframe(860, -5_621, -2_904, -3_821, 0, 0, 0),
    mission_camera_keyframe(870, -5_585, -2_903, -3_779, 0, 0, 0),
    mission_camera_keyframe(880, -5_549, -2_901, -3_737, 0, 0, 0),
    mission_camera_keyframe(890, -5_513, -2_900, -3_695, 0, 0, 0),
    mission_camera_keyframe(900, -5_477, -2_900, -3_653, 0, 0, 0),
];

/// Four independent craft poses from the same retail presentation. The two
/// source meshes share paths initially, then peel apart near the handoff to
/// player control, so each craft retains its own typed trajectory.
const MISSION_FORMATION_KEYFRAMES: [MissionFormationKeyframe; 29] = [
    mission_formation_keyframe(
        80,
        [-500, 3_028, -2_500],
        [500, 228, 2_500],
        [-2_532, 1_043, -500],
        [2_468, 2_243, 500],
    ),
    mission_formation_keyframe(
        90,
        [-564, 3_058, -2_500],
        [436, 258, 2_500],
        [-2_596, 1_073, -500],
        [2_404, 2_273, 500],
    ),
    mission_formation_keyframe(
        100,
        [-628, 3_088, -2_500],
        [372, 288, 2_500],
        [-2_660, 1_103, -500],
        [2_340, 2_303, 500],
    ),
    mission_formation_keyframe(
        110,
        [-724, 3_129, -2_500],
        [276, 329, 2_500],
        [-2_756, 1_147, -500],
        [2_244, 2_347, 500],
    ),
    mission_formation_keyframe(
        120,
        [-788, 3_152, -2_500],
        [212, 352, 2_500],
        [-2_820, 1_175, -500],
        [2_180, 2_375, 500],
    ),
    mission_formation_keyframe(
        130,
        [-884, 3_177, -2_500],
        [116, 377, 2_500],
        [-2_916, 1_213, -500],
        [2_084, 2_413, 500],
    ),
    mission_formation_keyframe(
        140,
        [-948, 3_187, -2_500],
        [52, 387, 2_500],
        [-2_980, 1_237, -500],
        [2_020, 2_437, 500],
    ),
    mission_formation_keyframe(
        150,
        [-1_044, 3_191, -2_500],
        [-44, 391, 2_500],
        [-3_076, 1_268, -500],
        [1_924, 2_468, 500],
    ),
    mission_formation_keyframe(
        160,
        [-1_140, 3_183, -2_500],
        [-140, 383, 2_500],
        [-3_172, 1_294, -500],
        [1_828, 2_494, 500],
    ),
    mission_formation_keyframe(
        170,
        [-1_236, 3_162, -2_500],
        [-236, 362, 2_500],
        [-3_268, 1_314, -500],
        [1_732, 2_514, 500],
    ),
    mission_formation_keyframe(
        200,
        [-1_492, 3_058, -2_500],
        [-492, 258, 2_500],
        [-3_524, 1_334, -500],
        [1_476, 2_534, 500],
    ),
    mission_formation_keyframe(
        210,
        [-1_588, 3_013, -2_500],
        [-588, 213, 2_500],
        [-3_620, 1_332, -500],
        [1_380, 2_532, 500],
    ),
    mission_formation_keyframe(
        220,
        [-1_684, 2_969, -2_500],
        [-684, 169, 2_500],
        [-3_716, 1_323, -500],
        [1_284, 2_523, 500],
    ),
    mission_formation_keyframe(
        230,
        [-1_780, 2_930, -2_500],
        [-780, 130, 2_500],
        [-3_812, 1_308, -500],
        [1_188, 2_508, 500],
    ),
    mission_formation_keyframe(
        240,
        [-1_876, 2_901, -2_500],
        [-876, 101, 2_500],
        [-3_908, 1_286, -500],
        [1_092, 2_486, 500],
    ),
    mission_formation_keyframe(
        250,
        [-1_972, 2_884, -2_500],
        [-972, 84, 2_500],
        [-4_004, 1_258, -500],
        [996, 2_458, 500],
    ),
    mission_formation_keyframe(
        260,
        [-2_036, 2_880, -2_500],
        [-1_036, 80, 2_500],
        [-4_068, 1_237, -500],
        [932, 2_437, 500],
    ),
    mission_formation_keyframe(
        270,
        [-2_132, 2_884, -2_500],
        [-1_132, 84, 2_500],
        [-4_164, 1_201, -500],
        [836, 2_401, 500],
    ),
    mission_formation_keyframe(
        280,
        [-2_228, 2_901, -2_500],
        [-1_228, 101, 2_500],
        [-4_260, 1_161, -500],
        [740, 2_361, 500],
    ),
    mission_formation_keyframe(
        290,
        [-2_372, 2_989, -2_484],
        [-1_528, 244, 2_548],
        [-4_644, 1_191, -428],
        [356, 2_331, 572],
    ),
    mission_formation_keyframe(
        300,
        [-3_016, 3_243, -2_380],
        [-2_212, 477, 2_648],
        [-5_340, 1_236, -236],
        [-340, 2_286, 764],
    ),
    mission_formation_keyframe(
        310,
        [-3_708, 3_517, -2_312],
        [-2_908, 731, 2_692],
        [-5_988, 1_280, 80],
        [-988, 2_242, 1_080],
    ),
    mission_formation_keyframe(
        320,
        [-4_286, 3_766, -2_334],
        [-3_482, 965, 2_640],
        [-6_406, 1_314, 502],
        [-1_406, 2_208, 1_502],
    ),
    mission_formation_keyframe(
        330,
        [-4_864, 4_016, -2_356],
        [-4_056, 1_199, 2_588],
        [-6_824, 1_348, 924],
        [-1_824, 2_174, 1_924],
    ),
    mission_formation_keyframe(
        340,
        [-5_316, 4_228, -2_448],
        [-4_488, 1_401, 2_436],
        [-7_020, 1_372, 1_368],
        [-2_020, 2_150, 2_368],
    ),
    mission_formation_keyframe(
        350,
        [-5_752, 4_446, -2_580],
        [-4_888, 1_609, 2_220],
        [-7_096, 1_394, 1_848],
        [-2_096, 2_128, 2_848],
    ),
    mission_formation_keyframe(
        360,
        [-6_380, 4_785, -2_856],
        [-5_476, 1_934, 1_872],
        [-7_004, 1_423, 2_564],
        [-2_004, 2_099, 3_564],
    ),
    mission_formation_keyframe(
        370,
        [-6_796, 5_018, -3_048],
        [-5_868, 2_159, 1_640],
        [-6_772, 1_439, 2_984],
        [-1_772, 2_083, 3_984],
    ),
    mission_formation_keyframe(
        380,
        [-7_212, 5_255, -3_240],
        [-6_260, 2_389, 1_408],
        [-6_424, 1_452, 3_316],
        [-1_424, 2_070, 4_316],
    ),
];

const MISSION_PLAYER_KEYFRAMES: [MissionPlayerKeyframe; 59] = [
    mission_player_keyframe(320, -7_699, -2_881, -6_252, 0, 227, 6, 6),
    mission_player_keyframe(330, -7_689, -2_881, -6_240, 0, 227, 4, 11),
    mission_player_keyframe(340, -7_665, -2_881, -6_211, 0, 227, 4, 16),
    mission_player_keyframe(350, -7_646, -2_881, -6_188, 0, 227, 4, 18),
    mission_player_keyframe(360, -7_624, -2_881, -6_161, 0, 227, 3, 20),
    mission_player_keyframe(370, -7_586, -2_881, -6_116, 0, 227, 1, 23),
    mission_player_keyframe(380, -7_557, -2_881, -6_082, 0, 227, 255, 25),
    mission_player_keyframe(390, -7_526, -2_881, -6_045, 0, 227, 254, 27),
    mission_player_keyframe(400, -7_475, -2_881, -5_984, 0, 227, 252, 30),
    mission_player_keyframe(410, -7_439, -2_881, -5_942, 0, 227, 252, 30),
    mission_player_keyframe(420, -7_403, -2_881, -5_900, 0, 227, 253, 30),
    mission_player_keyframe(430, -7_349, -2_881, -5_837, 0, 227, 254, 30),
    mission_player_keyframe(440, -7_313, -2_881, -5_795, 0, 227, 0, 30),
    mission_player_keyframe(450, -7_277, -2_881, -5_753, 0, 227, 2, 30),
    mission_player_keyframe(460, -7_223, -2_881, -5_690, 0, 227, 3, 30),
    mission_player_keyframe(470, -7_187, -2_881, -5_648, 0, 227, 4, 30),
    mission_player_keyframe(480, -7_151, -2_881, -5_606, 0, 227, 4, 30),
    mission_player_keyframe(490, -7_115, -2_881, -5_564, 0, 227, 3, 30),
    mission_player_keyframe(500, -7_061, -2_881, -5_501, 0, 227, 1, 30),
    mission_player_keyframe(510, -7_025, -2_881, -5_459, 0, 227, 255, 30),
    mission_player_keyframe(520, -6_989, -2_881, -5_417, 0, 227, 254, 30),
    mission_player_keyframe(530, -6_935, -2_881, -5_354, 0, 227, 252, 30),
    mission_player_keyframe(540, -6_899, -2_881, -5_312, 0, 227, 252, 30),
    mission_player_keyframe(550, -6_863, -2_881, -5_270, 0, 227, 253, 30),
    mission_player_keyframe(560, -6_809, -2_881, -5_207, 0, 227, 254, 30),
    mission_player_keyframe(570, -6_773, -2_881, -5_165, 0, 227, 0, 30),
    mission_player_keyframe(580, -6_737, -2_881, -5_123, 0, 227, 2, 30),
    mission_player_keyframe(590, -6_701, -2_881, -5_081, 0, 227, 3, 30),
    mission_player_keyframe(600, -6_647, -2_881, -5_018, 0, 227, 4, 30),
    mission_player_keyframe(610, -6_611, -2_881, -4_976, 0, 227, 4, 30),
    mission_player_keyframe(620, -6_575, -2_881, -4_934, 0, 227, 3, 30),
    mission_player_keyframe(630, -6_539, -2_881, -4_892, 0, 227, 2, 30),
    mission_player_keyframe(640, -6_503, -2_881, -4_850, 0, 227, 0, 30),
    mission_player_keyframe(650, -6_467, -2_881, -4_808, 0, 227, 254, 30),
    mission_player_keyframe(660, -6_431, -2_881, -4_766, 0, 227, 253, 30),
    mission_player_keyframe(670, -6_377, -2_881, -4_703, 0, 227, 252, 30),
    mission_player_keyframe(680, -6_341, -2_881, -4_661, 0, 227, 252, 30),
    mission_player_keyframe(690, -6_305, -2_881, -4_619, 0, 227, 253, 30),
    mission_player_keyframe(700, -6_269, -2_881, -4_577, 0, 227, 254, 30),
    mission_player_keyframe(710, -6_215, -2_881, -4_514, 0, 227, 1, 30),
    mission_player_keyframe(720, -6_179, -2_881, -4_472, 0, 227, 2, 30),
    mission_player_keyframe(730, -6_143, -2_881, -4_430, 0, 227, 3, 30),
    mission_player_keyframe(740, -6_107, -2_881, -4_388, 0, 227, 4, 30),
    mission_player_keyframe(750, -6_071, -2_881, -4_346, 0, 227, 4, 30),
    mission_player_keyframe(760, -6_017, -2_881, -4_283, 0, 227, 2, 30),
    mission_player_keyframe(770, -5_981, -2_881, -4_241, 0, 227, 1, 30),
    mission_player_keyframe(780, -5_945, -2_881, -4_199, 0, 227, 255, 30),
    mission_player_keyframe(790, -5_909, -2_881, -4_157, 0, 227, 254, 30),
    mission_player_keyframe(800, -5_873, -2_881, -4_115, 0, 227, 253, 30),
    mission_player_keyframe(810, -5_837, -2_881, -4_073, 0, 227, 252, 30),
    mission_player_keyframe(820, -5_783, -2_881, -4_010, 0, 227, 253, 30),
    mission_player_keyframe(830, -5_747, -2_881, -3_968, 0, 227, 254, 30),
    mission_player_keyframe(840, -5_711, -2_881, -3_926, 0, 227, 255, 30),
    mission_player_keyframe(850, -5_657, -2_881, -3_863, 0, 227, 2, 30),
    mission_player_keyframe(860, -5_621, -2_881, -3_821, 0, 227, 3, 30),
    mission_player_keyframe(870, -5_585, -2_881, -3_779, 0, 227, 4, 30),
    mission_player_keyframe(880, -5_549, -2_881, -3_737, 0, 227, 4, 30),
    mission_player_keyframe(890, -5_513, -2_881, -3_695, 0, 227, 3, 30),
    mission_player_keyframe(900, -5_477, -2_881, -3_653, 0, 227, 2, 30),
];

/// Camera-to-player offsets recovered from the retail control handoff. The
/// vertical component deliberately settles later than the horizontal pair.
const MISSION_CAMERA_FOLLOW_KEYFRAMES: [MissionCameraFollowKeyframe; 21] = [
    mission_follow_keyframe(400, -20, -24, -16),
    mission_follow_keyframe(405, -19, -24, -15),
    mission_follow_keyframe(410, -18, -24, -14),
    mission_follow_keyframe(415, -17, -24, -13),
    mission_follow_keyframe(420, -16, -24, -12),
    mission_follow_keyframe(425, -14, -23, -10),
    mission_follow_keyframe(430, -13, -23, -9),
    mission_follow_keyframe(435, -12, -22, -8),
    mission_follow_keyframe(440, -11, -22, -7),
    mission_follow_keyframe(445, -10, -21, -6),
    mission_follow_keyframe(450, -9, -20, -5),
    mission_follow_keyframe(455, -7, -19, -3),
    mission_follow_keyframe(460, -6, -19, -2),
    mission_follow_keyframe(465, -5, -19, -1),
    mission_follow_keyframe(470, -4, -18, 0),
    mission_follow_keyframe(475, -3, -18, 0),
    mission_follow_keyframe(480, -1, -18, 0),
    mission_follow_keyframe(485, 0, -18, 0),
    mission_follow_keyframe(490, 0, -19, 0),
    mission_follow_keyframe(495, 0, -19, 0),
    mission_follow_keyframe(500, 0, -19, 0),
];

/// The four entry craft remain live after the camera handoff. These poses are
/// the flat-world checkpoints sampled from the retail laser-hold trace. Firing
/// does not steer the player, so this certifies the authored encounter path
/// through retail frame 900 without reproducing the source coordinate model.
const MISSION_ENCOUNTER_KEYFRAMES: [MissionEncounterKeyframe; 58] = [
    mission_encounter_keyframe(
        330,
        [-4_864, 4_016, -2_356, 0, 70, 8, 60],
        [-4_056, 1_199, 2_588, 0, 74, 13, 60],
        [-6_824, 1_348, 924, 0, 24, 238, 63],
        [-1_824, 2_174, 1_924, 0, 24, 238, 63],
    ),
    mission_encounter_keyframe(
        340,
        [-5_536, 4_228, -2_508, 0, 76, 8, 60],
        [-4_488, 1_401, 2_436, 0, 80, 13, 60],
        [-7_020, 1_372, 1_368, 0, 15, 236, 63],
        [-2_020, 2_150, 2_368, 0, 15, 236, 63],
    ),
    mission_encounter_keyframe(
        350,
        [-5_964, 4_558, -2_664, 0, 80, 8, 60],
        [-5_084, 1_716, 2_104, 0, 86, 12, 60],
        [-7_096, 1_394, 1_848, 0, 5, 234, 63],
        [-2_096, 2_128, 2_848, 0, 5, 234, 63],
    ),
    mission_encounter_keyframe(
        360,
        [-6_380, 4_785, -2_856, 0, 82, 7, 60],
        [-5_476, 1_934, 1_872, 0, 86, 10, 60],
        [-7_004, 1_423, 2_564, 0, 244, 232, 63],
        [-2_004, 2_099, 3_564, 0, 244, 232, 63],
    ),
    mission_encounter_keyframe(
        370,
        [-6_796, 5_018, -3_048, 0, 82, 5, 60],
        [-5_868, 2_159, 1_640, 0, 86, 8, 60],
        [-6_772, 1_439, 2_984, 0, 232, 232, 63],
        [-1_772, 2_083, 3_984, 0, 232, 232, 63],
    ),
    mission_encounter_keyframe(
        380,
        [-7_420, 5_375, -3_336, 0, 82, 2, 60],
        [-6_456, 2_506, 1_292, 0, 86, 5, 60],
        [-6_216, 1_458, 3_436, 0, 214, 232, 63],
        [-1_216, 2_064, 4_436, 0, 214, 232, 63],
    ),
    mission_encounter_keyframe(
        390,
        [-7_836, 5_618, -3_528, 0, 82, 0, 60],
        [-6_848, 2_743, 1_060, 0, 86, 3, 60],
        [-5_756, 1_467, 3_576, 0, 202, 232, 63],
        [-756, 2_055, 4_576, 0, 202, 232, 63],
    ),
    mission_encounter_keyframe(
        400,
        [-8_460, 5_988, -3_816, 0, 82, 0, 60],
        [-7_240, 2_984, 828, 0, 86, 0, 60],
        [-5_268, 1_659, 3_600, 0, 193, 237, 63],
        [-268, 1_863, 4_600, 0, 193, 237, 63],
    ),
    mission_encounter_keyframe(
        410,
        [-8_876, 6_238, -4_008, 0, 82, 0, 60],
        [-7_828, 3_352, 480, 0, 86, 0, 60],
        [-4_548, 1_587, 3_512, 0, 183, 242, 63],
        [452, 1_935, 4_512, 0, 183, 242, 63],
    ),
    mission_encounter_keyframe(
        420,
        [-9_292, 6_490, -4_200, 0, 82, 0, 60],
        [-8_220, 3_601, 248, 0, 86, 0, 60],
        [-4_088, 1_299, 3_364, 0, 177, 244, 63],
        [912, 2_223, 4_364, 0, 177, 244, 63],
    ),
    mission_encounter_keyframe(
        430,
        [-9_916, 6_868, -4_488, 0, 82, 0, 60],
        [-8_808, 3_978, -100, 0, 86, 0, 60],
        [-3_436, 673, 3_044, 0, 171, 247, 63],
        [1_352, 2_699, 4_160, 0, 173, 246, 63],
    ),
    mission_encounter_keyframe(
        440,
        [-10_332, 7_121, -4_680, 0, 82, 0, 60],
        [-9_200, 4_230, -332, 0, 86, 0, 60],
        [-3_032, -193, 2_788, 0, 168, 249, 63],
        [1_968, 3_715, 3_788, 0, 168, 249, 63],
    ),
    mission_encounter_keyframe(
        450,
        [-10_748, 7_373, -4_872, 0, 82, 0, 60],
        [-9_592, 4_483, -564, 0, 86, 0, 60],
        [-2_640, -1_033, 2_512, 0, 166, 251, 63],
        [2_360, 4_555, 3_512, 0, 166, 251, 63],
    ),
    mission_encounter_keyframe(
        460,
        [-11_372, 7_750, -5_160, 0, 82, 0, 60],
        [-10_180, 4_861, -912, 0, 86, 0, 60],
        [-2_064, -2_457, 2_068, 0, 165, 254, 63],
        [2_936, 5_487, 3_068, 0, 165, 254, 63],
    ),
    mission_encounter_keyframe(
        470,
        [-11_788, 7_999, -5_352, 0, 82, 0, 60],
        [-10_572, 5_113, -1_144, 0, 86, 0, 60],
        [-1_680, -3_461, 1_772, 0, 165, 0, 63],
        [3_320, 6_983, 2_772, 0, 165, 0, 63],
    ),
    mission_encounter_keyframe(
        480,
        [-12_204, 8_245, -5_544, 0, 82, 0, 60],
        [-10_964, 5_363, -1_376, 0, 86, 0, 60],
        [-1_300, -4_505, 1_476, 243, 165, 254, 63],
        [3_700, 8_027, 2_476, 13, 165, 254, 63],
    ),
    mission_encounter_keyframe(
        490,
        [-12_620, 8_488, -5_736, 0, 82, 0, 60],
        [-11_356, 5_610, -1_608, 0, 86, 0, 60],
        [-948, -5_661, 1_204, 232, 165, 252, 63],
        [4_052, 9_183, 2_204, 24, 165, 252, 63],
    ),
    mission_encounter_keyframe(
        500,
        [-13_244, 8_845, -6_024, 0, 82, 0, 60],
        [-11_944, 5_976, -1_956, 0, 86, 0, 60],
        [-524, -7_417, 844, 225, 162, 250, 63],
        [4_348, 10_363, 1_960, 31, 163, 250, 63],
    ),
    mission_encounter_keyframe(
        510,
        [-13_660, 9_078, -6_216, 0, 82, 0, 60],
        [-12_336, 6_215, -2_188, 0, 86, 0, 60],
        [-300, -8_493, 616, 221, 159, 247, 63],
        [4_700, 12_015, 1_616, 35, 159, 247, 63],
    ),
    mission_encounter_keyframe(
        520,
        [-14_076, 9_305, -6_408, 0, 82, 0, 60],
        [-12_728, 6_450, -2_420, 0, 86, 0, 60],
        [-108, -9_433, 376, 221, 155, 245, 63],
        [4_892, 12_955, 1_376, 35, 155, 245, 63],
    ),
    mission_encounter_keyframe(
        530,
        [-14_700, 9_635, -6_696, 0, 82, 0, 60],
        [-13_316, 6_793, -2_768, 0, 86, 0, 60],
        [136, -10_485, -40, 223, 147, 244, 63],
        [5_060, 13_711, 1_108, 33, 150, 244, 63],
    ),
    mission_encounter_keyframe(
        540,
        [-15_116, 9_847, -6_888, 0, 82, 0, 60],
        [-13_708, 7_015, -3_000, 0, 86, 0, 60],
        [256, -10_909, -376, 230, 141, 244, 63],
        [5_256, 14_431, 624, 26, 141, 244, 63],
    ),
    mission_encounter_keyframe(
        550,
        [-15_532, 10_052, -7_080, 0, 82, 0, 60],
        [-14_100, 7_230, -3_232, 0, 86, 0, 60],
        [332, -11_085, -764, 237, 135, 244, 63],
        [5_332, 14_607, 236, 19, 135, 244, 63],
    ),
    mission_encounter_keyframe(
        560,
        [-16_156, 10_346, -7_368, 0, 82, 0, 60],
        [-14_688, 7_540, -3_580, 0, 86, 0, 60],
        [344, -10_845, -1_444, 245, 126, 244, 63],
        [5_348, 14_519, -208, 11, 129, 244, 63],
    ),
    mission_encounter_keyframe(
        570,
        [-16_572, 10_531, -7_560, 0, 82, 0, 60],
        [-15_080, 7_738, -3_812, 0, 86, 0, 60],
        [280, -10_341, -1_924, 2, 120, 244, 63],
        [5_280, 13_863, -924, 254, 120, 244, 63],
    ),
    mission_encounter_keyframe(
        580,
        [-16_988, 10_707, -7_752, 0, 82, 0, 60],
        [-15_472, 7_928, -4_044, 0, 86, 0, 60],
        [144, -9_577, -2_384, 12, 114, 244, 63],
        [5_144, 13_099, -1_384, 244, 114, 244, 63],
    ),
    mission_encounter_keyframe(
        590,
        [-17_404, 10_874, -7_944, 0, 82, 0, 60],
        [-15_864, 8_108, -4_276, 0, 86, 0, 60],
        [-52, -7_872, -2_796, 13, 108, 245, 63],
        [4_948, 10_954, -1_796, 239, 108, 245, 63],
    ),
    mission_encounter_keyframe(
        600,
        [-18_028, 11_107, -8_232, 0, 82, 0, 60],
        [-16_256, 8_280, -4_508, 0, 86, 0, 60],
        [-296, -5_897, -3_184, 10, 104, 247, 63],
        [4_712, 8_208, -2_172, 241, 104, 247, 63],
    ),
    mission_encounter_keyframe(
        610,
        [-18_444, 11_250, -8_424, 0, 82, 0, 60],
        [-16_844, 8_520, -4_856, 0, 86, 0, 60],
        [-580, -4_414, -3_560, 8, 100, 248, 63],
        [4_436, 6_128, -2_532, 243, 100, 249, 63],
    ),
    mission_encounter_keyframe(
        620,
        [-18_860, 11_382, -8_616, 0, 82, 0, 60],
        [-17_236, 8_668, -5_088, 0, 86, 0, 60],
        [-1_076, -2_858, -4_060, 5, 94, 248, 63],
        [3_976, 3_925, -3_044, 245, 97, 251, 63],
    ),
    mission_encounter_keyframe(
        630,
        [-19_276, 11_503, -8_808, 0, 82, 0, 60],
        [-17_628, 8_805, -5_320, 0, 86, 2, 60],
        [-1_460, -2_144, -4_352, 3, 90, 248, 63],
        [3_644, 2_905, -3_368, 248, 95, 252, 63],
    ),
    mission_encounter_keyframe(
        640,
        [-19_692, 11_614, -9_000, 0, 82, 254, 60],
        [-18_020, 8_932, -5_556, 0, 87, 4, 60],
        [-1_868, -1_620, -4_604, 1, 86, 248, 63],
        [3_288, 2_146, -3_680, 250, 93, 252, 63],
    ),
    mission_encounter_keyframe(
        650,
        [-20_112, 11_713, -9_184, 0, 81, 252, 60],
        [-18_400, 9_048, -5_812, 0, 89, 6, 60],
        [-2_304, -1_238, -4_816, 0, 82, 248, 63],
        [2_912, 1_588, -3_980, 252, 91, 252, 63],
    ),
    mission_encounter_keyframe(
        660,
        [-20_540, 11_800, -9_348, 0, 79, 250, 60],
        [-18_760, 9_153, -6_092, 0, 92, 8, 60],
        [-2_756, -949, -4_980, 0, 78, 248, 63],
        [2_520, 1_183, -4_256, 254, 89, 252, 63],
    ),
    mission_encounter_keyframe(
        670,
        [-21_200, 11_908, -9_532, 0, 74, 247, 60],
        [-19_092, 9_246, -6_404, 0, 96, 11, 60],
        [-3_224, -728, -5_096, 0, 74, 248, 63],
        [2_116, 896, -4_512, 0, 87, 252, 63],
    ),
    mission_encounter_keyframe(
        680,
        [-21_656, 11_965, -9_600, 0, 70, 245, 60],
        [-19_524, 9_363, -6_936, 0, 102, 11, 60],
        [-3_948, -489, -5_180, 0, 68, 248, 63],
        [1_480, 601, -4_860, 0, 84, 252, 63],
    ),
    mission_encounter_keyframe(
        690,
        [-22_120, 12_010, -9_612, 0, 64, 243, 60],
        [-19_764, 9_426, -7_324, 0, 106, 11, 60],
        [-4_436, -375, -5_184, 0, 64, 248, 63],
        [1_044, 461, -5_068, 0, 82, 252, 63],
    ),
    mission_encounter_keyframe(
        700,
        [-22_580, 12_043, -9_572, 0, 58, 243, 60],
        [-19_964, 9_477, -7_736, 0, 110, 11, 60],
        [-4_924, -288, -5_164, 0, 60, 248, 63],
        [596, 354, -5_252, 0, 80, 252, 63],
    ),
    mission_encounter_keyframe(
        710,
        [-23_240, 12_070, -9_388, 0, 49, 243, 60],
        [-20_180, 9_531, -8_384, 0, 116, 11, 60],
        [-5_404, -221, -5_096, 0, 56, 248, 63],
        [140, 272, -5_412, 0, 78, 252, 63],
    ),
    mission_encounter_keyframe(
        720,
        [-23_648, 12_073, -9_180, 0, 43, 243, 60],
        [-20_272, 9_552, -8_836, 0, 120, 11, 60],
        [-6_100, -149, -4_904, 0, 50, 248, 63],
        [-560, 183, -5_608, 0, 75, 252, 63],
    ),
    mission_encounter_keyframe(
        730,
        [-24_020, 12_064, -8_916, 0, 37, 243, 60],
        [-20_316, 9_561, -9_296, 0, 124, 11, 60],
        [-6_544, 317, -4_716, 0, 46, 248, 63],
        [-1_032, -291, -5_708, 0, 73, 252, 63],
    ),
    mission_encounter_keyframe(
        740,
        [-24_348, 12_043, -8_600, 0, 31, 243, 60],
        [-20_320, 9_558, -9_760, 0, 128, 11, 60],
        [-6_964, 1_281, -4_484, 12, 42, 248, 63],
        [-1_508, -1_255, -5_784, 244, 71, 252, 63],
    ),
    mission_encounter_keyframe(
        750,
        [-24_624, 12_010, -8_236, 0, 25, 243, 60],
        [-20_300, 9_543, -10_224, 0, 132, 11, 60],
        [-7_336, 2_441, -4_228, 22, 38, 248, 63],
        [-1_960, -2_415, -5_836, 234, 69, 252, 63],
    ),
    mission_encounter_keyframe(
        760,
        [-24_924, 11_938, -7_624, 0, 16, 243, 60],
        [-20_180, 9_498, -10_904, 0, 138, 11, 60],
        [-7_636, 3_725, -3_972, 31, 34, 248, 63],
        [-2_360, -3_699, -5_864, 225, 67, 252, 63],
    ),
    mission_encounter_keyframe(
        770,
        [-25_044, 11_875, -7_180, 0, 10, 243, 60],
        [-20_048, 9_453, -11_340, 0, 142, 11, 60],
        [-7_948, 5_749, -3_624, 40, 28, 248, 63],
        [-2_836, -5_604, -5_868, 216, 64, 252, 63],
    ),
    mission_encounter_keyframe(
        780,
        [-25_092, 11_800, -6_720, 0, 4, 243, 60],
        [-19_868, 9_396, -11_760, 0, 146, 11, 60],
        [-8_092, 7_053, -3_416, 43, 24, 248, 63],
        [-3_092, -7_027, -5_864, 213, 62, 252, 63],
    ),
    mission_encounter_keyframe(
        790,
        [-25_088, 11_713, -6_256, 0, 254, 243, 60],
        [-19_648, 9_327, -12_160, 0, 150, 11, 60],
        [-8_204, 8_273, -3_216, 45, 20, 248, 63],
        [-3_324, -8_247, -5_852, 211, 60, 252, 63],
    ),
    mission_encounter_keyframe(
        800,
        [-25_024, 11_614, -5_796, 0, 248, 243, 60],
        [-19_388, 9_246, -12_536, 0, 154, 11, 60],
        [-8_284, 9_357, -3_024, 45, 16, 248, 63],
        [-3_532, -9_331, -5_828, 211, 58, 252, 63],
    ),
    mission_encounter_keyframe(
        810,
        [-24_896, 11_503, -5_356, 0, 242, 243, 60],
        [-19_092, 9_153, -12_884, 0, 158, 11, 60],
        [-8_344, 10_273, -2_816, 43, 12, 248, 63],
        [-3_752, -10_247, -5_792, 213, 56, 252, 63],
    ),
    mission_encounter_keyframe(
        820,
        [-24_584, 11_317, -4_744, 0, 233, 243, 60],
        [-18_584, 8_991, -13_340, 0, 164, 11, 60],
        [-8_404, 11_185, -2_440, 39, 6, 248, 63],
        [-3_992, -10_959, -5_740, 217, 54, 252, 63],
    ),
    mission_encounter_keyframe(
        830,
        [-24_300, 11_180, -4_384, 0, 227, 243, 60],
        [-18_208, 8_870, -13_600, 0, 168, 11, 60],
        [-8_420, 11_625, -2_120, 30, 2, 248, 63],
        [-4_432, -11_599, -5_616, 226, 51, 252, 63],
    ),
    mission_encounter_keyframe(
        840,
        [-23_964, 11_032, -4_076, 0, 221, 243, 60],
        [-17_808, 8_738, -13_820, 0, 172, 11, 60],
        [-8_416, 11_741, -1_744, 22, 254, 248, 63],
        [-4_784, -11_715, -5_492, 234, 49, 252, 63],
    ),
    mission_encounter_keyframe(
        850,
        [-23_436, 10_792, -3_632, 0, 221, 246, 60],
        [-17_172, 8_520, -14_072, 0, 178, 11, 60],
        [-8_372, 11_585, -1_320, 14, 250, 248, 63],
        [-5_176, -11_559, -5_328, 242, 47, 252, 63],
    ),
    mission_encounter_keyframe(
        860,
        [-23_084, 10_620, -3_336, 0, 221, 248, 60],
        [-16_728, 8_362, -14_184, 0, 182, 11, 60],
        [-8_216, 10_837, -636, 255, 244, 248, 63],
        [-5_808, -10_811, -5_016, 1, 44, 252, 63],
    ),
    mission_encounter_keyframe(
        870,
        [-22_732, 10_440, -3_040, 0, 221, 250, 60],
        [-16_272, 8_195, -14_252, 0, 186, 11, 60],
        [-8_052, 10_017, -188, 245, 240, 248, 63],
        [-6_224, -9_991, -4_780, 11, 42, 252, 63],
    ),
    mission_encounter_keyframe(
        880,
        [-22_380, 10_250, -2_744, 0, 221, 252, 60],
        [-15_808, 8_019, -14_272, 0, 190, 11, 60],
        [-7_852, 8_973, 224, 236, 236, 248, 63],
        [-6_604, -8_947, -4_536, 20, 40, 252, 63],
    ),
    mission_encounter_keyframe(
        890,
        [-22_028, 10_052, -2_448, 0, 221, 254, 60],
        [-15_344, 7_834, -14_268, 0, 194, 11, 60],
        [-7_636, 7_753, 572, 228, 232, 248, 63],
        [-6_940, -7_727, -4_300, 28, 38, 252, 63],
    ),
    mission_encounter_keyframe(
        900,
        [-21_676, 9_847, -2_152, 0, 221, 0, 60],
        [-14_884, 7_640, -14_224, 0, 198, 11, 60],
        [-7_420, 6_421, 856, 221, 228, 248, 63],
        [-7_220, -6_395, -4_076, 35, 36, 252, 63],
    ),
];

/// Enemy-laser poses sampled at every native update boundary from the same
/// retail laser-hold trace as the encounter craft. These are semantic
/// projectile trajectories; source object addresses are not retained.
const UPPER_FIGHTER_OPENING_SHOT_ONE_KEYFRAMES: [MissionProjectileKeyframe; 37] = [
    mission_projectile_keyframe(588, [-361, -8_658, -2_750, 37, 79, 0, 63]),
    mission_projectile_keyframe(592, [-775, -8_216, -2_904, 37, 79, 0, 63]),
    mission_projectile_keyframe(596, [-1_191, -7_774, -3_058, 37, 79, 0, 63]),
    mission_projectile_keyframe(600, [-1_607, -7_335, -3_208, 37, 78, 0, 63]),
    mission_projectile_keyframe(604, [-1_700, -7_254, -3_242, 37, 78, 0, 63]),
    mission_projectile_keyframe(608, [-2_030, -6_900, -3_358, 36, 78, 0, 63]),
    mission_projectile_keyframe(612, [-2_453, -6_468, -3_508, 36, 78, 0, 63]),
    mission_projectile_keyframe(616, [-2_879, -6_036, -3_656, 36, 78, 0, 63]),
    mission_projectile_keyframe(620, [-3_305, -5_607, -3_803, 36, 78, 0, 63]),
    mission_projectile_keyframe(624, [-3_738, -5_185, -3_950, 35, 78, 0, 63]),
    mission_projectile_keyframe(628, [-4_174, -4_766, -4_097, 35, 78, 0, 63]),
    mission_projectile_keyframe(632, [-4_174, -4_766, -4_097, 35, 78, 0, 63]),
    mission_projectile_keyframe(636, [-4_613, -4_350, -4_244, 35, 77, 0, 63]),
    mission_projectile_keyframe(640, [-5_063, -3_941, -4_390, 34, 77, 0, 63]),
    mission_projectile_keyframe(644, [-5_523, -3_547, -4_534, 33, 77, 0, 63]),
    mission_projectile_keyframe(648, [-5_993, -3_165, -4_673, 32, 76, 0, 63]),
    mission_projectile_keyframe(652, [-6_169, -3_005, -4_721, 28, 76, 0, 63]),
    mission_projectile_keyframe(656, [-6_349, -2_849, -4_773, 28, 76, 0, 63]),
    mission_projectile_keyframe(660, [-6_349, -2_849, -4_773, 28, 76, 0, 63]),
    mission_projectile_keyframe(664, [-6_529, -2_693, -4_825, 28, 76, 0, 63]),
    mission_projectile_keyframe(668, [-6_709, -2_537, -4_877, 28, 76, 0, 63]),
    mission_projectile_keyframe(672, [-6_889, -2_381, -4_929, 28, 76, 0, 63]),
    mission_projectile_keyframe(676, [-7_069, -2_225, -4_981, 28, 76, 0, 63]),
    mission_projectile_keyframe(680, [-7_249, -2_069, -5_033, 28, 76, 0, 63]),
    mission_projectile_keyframe(684, [-7_429, -1_913, -5_085, 28, 76, 0, 63]),
    mission_projectile_keyframe(688, [-7_609, -1_757, -5_137, 28, 76, 0, 63]),
    mission_projectile_keyframe(692, [-7_789, -1_601, -5_189, 28, 76, 0, 63]),
    mission_projectile_keyframe(696, [-7_969, -1_445, -5_241, 28, 76, 0, 63]),
    mission_projectile_keyframe(700, [-7_969, -1_445, -5_241, 28, 76, 0, 63]),
    mission_projectile_keyframe(704, [-8_149, -1_289, -5_293, 28, 76, 0, 63]),
    mission_projectile_keyframe(708, [-8_329, -1_133, -5_345, 28, 76, 0, 63]),
    mission_projectile_keyframe(712, [-8_509, -977, -5_397, 28, 76, 0, 63]),
    mission_projectile_keyframe(716, [-8_689, -821, -5_449, 28, 76, 0, 63]),
    mission_projectile_keyframe(720, [-8_869, -665, -5_501, 28, 76, 0, 63]),
    mission_projectile_keyframe(724, [-9_049, -509, -5_553, 28, 76, 0, 63]),
    mission_projectile_keyframe(728, [-9_229, -353, -5_605, 28, 76, 0, 63]),
    mission_projectile_keyframe(732, [-9_229, -353, -5_605, 28, 76, 0, 63]),
];

const UPPER_FIGHTER_OPENING_SHOT_TWO_KEYFRAMES: [MissionProjectileKeyframe; 7] = [
    mission_projectile_keyframe(876, [-7_866, 9_350, -124, 201, 151, 0, 63]),
    mission_projectile_keyframe(880, [-7_866, 9_350, -124, 201, 151, 0, 63]),
    mission_projectile_keyframe(884, [-7_772, 8_745, -273, 201, 152, 0, 63]),
    mission_projectile_keyframe(888, [-7_676, 8_142, -421, 201, 152, 0, 63]),
    mission_projectile_keyframe(892, [-7_579, 7_539, -570, 201, 152, 0, 63]),
    mission_projectile_keyframe(896, [-7_579, 7_539, -570, 201, 152, 0, 63]),
    mission_projectile_keyframe(900, [-7_482, 6_936, -719, 201, 153, 0, 63]),
];

const LOWER_FIGHTER_OPENING_SHOT_KEYFRAMES: [MissionProjectileKeyframe; 6] = [
    mission_projectile_keyframe(880, [-6_359, -9_304, -4_594, 59, 226, 0, 63]),
    mission_projectile_keyframe(884, [-6_298, -8_681, -4_526, 59, 226, 0, 63]),
    mission_projectile_keyframe(888, [-6_230, -8_064, -4_451, 58, 226, 0, 63]),
    mission_projectile_keyframe(892, [-6_230, -8_064, -4_451, 58, 226, 0, 63]),
    mission_projectile_keyframe(896, [-6_159, -7_448, -4_373, 58, 226, 0, 63]),
    mission_projectile_keyframe(900, [-6_139, -7_204, -4_349, 58, 225, 0, 63]),
];

const fn mission_camera_keyframe(
    retail_frame: u16,
    x: i16,
    y: i16,
    z: i16,
    pitch: u8,
    yaw: u8,
    roll: u8,
) -> MissionCameraKeyframe {
    MissionCameraKeyframe {
        retail_frame,
        position: Vector3 { x, y, z },
        pitch,
        yaw,
        roll,
    }
}

const fn mission_formation_keyframe(
    retail_frame: u16,
    first: [i16; 3],
    second: [i16; 3],
    third: [i16; 3],
    fourth: [i16; 3],
) -> MissionFormationKeyframe {
    MissionFormationKeyframe {
        retail_frame,
        positions: [
            Vector3 {
                x: first[0],
                y: first[1],
                z: first[2],
            },
            Vector3 {
                x: second[0],
                y: second[1],
                z: second[2],
            },
            Vector3 {
                x: third[0],
                y: third[1],
                z: third[2],
            },
            Vector3 {
                x: fourth[0],
                y: fourth[1],
                z: fourth[2],
            },
        ],
    }
}

const fn mission_encounter_pose(raw: [i16; 7]) -> MissionEncounterPose {
    MissionEncounterPose {
        position: Vector3 {
            x: raw[0],
            y: raw[1],
            z: raw[2],
        },
        pitch: raw[3] as u8,
        yaw: raw[4] as u8,
        roll: raw[5] as u8,
        speed: raw[6] as u8,
    }
}

const fn mission_encounter_keyframe(
    retail_frame: u16,
    first: [i16; 7],
    second: [i16; 7],
    third: [i16; 7],
    fourth: [i16; 7],
) -> MissionEncounterKeyframe {
    MissionEncounterKeyframe {
        retail_frame,
        poses: [
            mission_encounter_pose(first),
            mission_encounter_pose(second),
            mission_encounter_pose(third),
            mission_encounter_pose(fourth),
        ],
    }
}

const fn mission_actor_keyframe(retail_frame: u16, pose: [i16; 7]) -> MissionActorKeyframe {
    MissionActorKeyframe {
        retail_frame,
        presentation: MissionActorPresentation::Present(mission_encounter_pose(pose)),
    }
}

const fn mission_actor_inactive_keyframe(retail_frame: u16) -> MissionActorKeyframe {
    MissionActorKeyframe {
        retail_frame,
        presentation: MissionActorPresentation::Inactive,
    }
}

const fn mission_actor_departure_keyframe(retail_frame: u16) -> MissionActorKeyframe {
    MissionActorKeyframe {
        retail_frame,
        presentation: MissionActorPresentation::Departed,
    }
}

const fn mission_timer_keyframe(retail_frame: u16, elapsed_tenths: u16) -> MissionTimerKeyframe {
    MissionTimerKeyframe {
        retail_frame,
        elapsed_tenths,
    }
}

const fn mission_projectile_keyframe(
    retail_frame: u16,
    pose: [i16; 7],
) -> MissionProjectileKeyframe {
    MissionProjectileKeyframe {
        retail_frame,
        pose: mission_encounter_pose(pose),
    }
}

const fn mission_follow_keyframe(
    retail_frame: u16,
    x: i16,
    y: i16,
    z: i16,
) -> MissionCameraFollowKeyframe {
    MissionCameraFollowKeyframe {
        retail_frame,
        offset: Vector3 { x, y, z },
    }
}

const fn mission_player_keyframe(
    retail_frame: u16,
    x: i16,
    y: i16,
    z: i16,
    pitch: u8,
    yaw: u8,
    roll: u8,
    speed: u8,
) -> MissionPlayerKeyframe {
    MissionPlayerKeyframe {
        retail_frame,
        position: Vector3 { x, y, z },
        pitch,
        yaw,
        roll,
        speed,
    }
}

#[derive(Debug, Clone, Copy)]
struct TitleCraftPose {
    position: Vector3,
    velocity: Vector3,
    pitch: u8,
    yaw: u8,
    roll: u8,
}

#[derive(Debug, Clone, Copy)]
struct TitleEffectPose {
    position: Vector3,
    velocity: Vector3,
}

/// Formation state at the first retail update that contains all three title
/// craft. Positions and velocities are ordinary scene data; no source-machine
/// storage convention reaches the native runtime.
const TITLE_CRAFT_POSES: [TitleCraftPose; 3] = [
    TitleCraftPose {
        position: Vector3 {
            x: -302,
            y: 196,
            z: -682,
        },
        velocity: Vector3 {
            x: -2,
            y: -4,
            z: 18,
        },
        pitch: 246,
        yaw: 8,
        roll: 246,
    },
    TitleCraftPose {
        position: Vector3 {
            x: 299,
            y: 95,
            z: -788,
        },
        velocity: Vector3 {
            x: -1,
            y: -5,
            z: 12,
        },
        pitch: 241,
        yaw: 8,
        roll: 246,
    },
    TitleCraftPose {
        position: Vector3 {
            x: 148,
            y: 296,
            z: -184,
        },
        velocity: Vector3 {
            x: -2,
            y: -4,
            z: 16,
        },
        pitch: 246,
        yaw: 8,
        roll: 246,
    },
];

const TITLE_EFFECT_POSES: [TitleEffectPose; 2] = [
    TitleEffectPose {
        position: Vector3 {
            x: -1_700,
            y: 195,
            z: -2_282,
        },
        velocity: Vector3 { x: 0, y: -5, z: 18 },
    },
    TitleEffectPose {
        position: Vector3 {
            x: -250,
            y: -400,
            z: -724,
        },
        velocity: Vector3 { x: 0, y: 0, z: 15 },
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    ObjectCapacityReached,
    MissingCatalogShape(ShapeId),
}

pub struct Game {
    state: GameState,
    render_objects: Vec<RenderObject>,
    title_flyby: Vec<ObjectId>,
    mission_entry_flyby: [Option<ObjectId>; MISSION_ENCOUNTER_ACTOR_COUNT],
    previous_mission_player_position: Option<Vector3>,
    mission_projectiles: Vec<ActiveMissionProjectile>,
    reengagement_projectiles: Vec<ActiveReengagementProjectile>,
    interception_missiles: [Option<ObjectId>; INTERCEPTION_MISSILE_COUNT],
    fighter_intercept_actors: FighterInterceptActors,
    fighter_intercept_projectiles: Vec<ActiveFighterInterceptProjectile>,
    pigma_rival: Option<ObjectId>,
    pigma_projectiles: Vec<ActivePigmaProjectile>,
    leon_rival: Option<ObjectId>,
    leon_projectiles: Vec<ActiveLeonProjectile>,
    pressure_fighter_actors: PressureFighterActors,
    pressure_fighter_projectiles: Vec<ActivePressureProjectile>,
    leon_pressure_projectiles: Vec<ActivePressureProjectile>,
    final_rival: Option<ObjectId>,
    final_rival_projectiles: Vec<ActivePressureProjectile>,
    mirage_dragon: Option<ObjectId>,
    mirage_dragon_body: [Option<ObjectId>; MIRAGE_DRAGON_BODY_SEGMENT_COUNT],
    mirage_dragon_tail: Option<ObjectId>,
    carrier_scenery: Vec<ObjectId>,
    carrier_panels: [Option<ObjectId>; 2],
}

impl Game {
    pub fn new() -> Self {
        Self {
            state: GameState::default(),
            render_objects: Vec::new(),
            title_flyby: Vec::with_capacity(TITLE_CRAFT_POSES.len() + TITLE_EFFECT_POSES.len()),
            mission_entry_flyby: [None; MISSION_ENCOUNTER_ACTOR_COUNT],
            previous_mission_player_position: None,
            mission_projectiles: Vec::with_capacity(MISSION_PROJECTILE_TRAJECTORY_COUNT),
            reengagement_projectiles: Vec::with_capacity(
                second_sortie_projectiles::PROJECTILE_COUNT,
            ),
            interception_missiles: [None; INTERCEPTION_MISSILE_COUNT],
            fighter_intercept_actors: FighterInterceptActors::default(),
            fighter_intercept_projectiles: Vec::with_capacity(
                fighter_intercept_projectiles::PROJECTILE_COUNT,
            ),
            pigma_rival: None,
            pigma_projectiles: Vec::with_capacity(pigma_duel_projectiles::PROJECTILE_COUNT),
            leon_rival: None,
            leon_projectiles: Vec::with_capacity(leon_duel::ENEMY_LASER_KEYFRAME_TRACKS.len()),
            pressure_fighter_actors: PressureFighterActors::default(),
            pressure_fighter_projectiles: Vec::with_capacity(
                pressure_fighters::ENEMY_LASER_KEYFRAME_TRACKS.len(),
            ),
            leon_pressure_projectiles: Vec::with_capacity(
                leon_pressure::ENEMY_LASER_KEYFRAME_TRACKS.len(),
            ),
            final_rival: None,
            final_rival_projectiles: Vec::with_capacity(
                final_pursuer::ENEMY_LASER_KEYFRAME_TRACKS.len()
                    + wolf_blockade::ENEMY_LASER_KEYFRAME_TRACKS.len(),
            ),
            mirage_dragon: None,
            mirage_dragon_body: [None; MIRAGE_DRAGON_BODY_SEGMENT_COUNT],
            mirage_dragon_tail: None,
            carrier_scenery: Vec::with_capacity(16),
            carrier_panels: [None; 2],
        }
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn frame(&self) -> u64 {
        self.state.frame
    }

    pub fn mode(&self) -> GameMode {
        self.state.mode
    }

    pub fn camera(&self) -> Camera {
        self.state.camera
    }

    pub fn mission(&self) -> Option<super::state::MissionId> {
        self.state.mission.mission
    }

    pub fn render_objects(&self) -> &[RenderObject] {
        &self.render_objects
    }

    fn primary_pilot(&self) -> Pilot {
        self.state.roster.selected[0].unwrap_or(DEFAULT_PRIMARY_PILOT)
    }

    fn primary_flight_craft_shape(&self) -> ShapeId {
        pilot_flight_craft_shape(self.primary_pilot())
    }

    fn primary_walker_shape(&self) -> ShapeId {
        pilot_walker_shape(self.primary_pilot())
    }

    fn primary_flight_side_transition_shape(&self) -> ShapeId {
        pilot_flight_side_transition_shape(self.primary_pilot())
    }

    fn primary_walker_side_transition_shape(&self) -> ShapeId {
        pilot_walker_side_transition_shape(self.primary_pilot())
    }

    fn player_transformation_allowed(&self) -> bool {
        matches!(
            self.state.mission.visit,
            MissionVisit::EladardBase | MissionVisit::TitaniaBase | MissionVisit::AstropolisAssault
        ) && self.state.mission.phase == MissionPhase::Active
    }

    fn cancel_player_charge_for_transformation(&mut self) {
        if let PlayerBlasterState::Holding {
            charge_orb: Some(charge_orb),
            ..
        } = self.state.mission.player_blaster
        {
            self.state.objects.remove(charge_orb);
        }
        self.state.mission.player_blaster = PlayerBlasterState::Ready;
    }

    fn begin_player_transformation(&mut self, direction: PlayerCraftTransformationDirection) {
        self.cancel_player_charge_for_transformation();
        if direction == PlayerCraftTransformationDirection::ToWalker {
            self.state.mission.player_walker = Default::default();
        }
        self.state.mission.player_craft_form =
            PlayerCraftForm::Transforming(PlayerCraftTransformation {
                direction,
                elapsed_retail_frames: 0,
            });
    }

    fn apply_player_craft_presentation(
        &mut self,
        player: ObjectId,
        presentation: PlayerCraftPresentation,
    ) {
        let (shape, animation_frame) = match presentation {
            PlayerCraftPresentation::Flight => (self.primary_flight_craft_shape(), 0),
            PlayerCraftPresentation::FlightSideTransition { animation_frame } => {
                (self.primary_flight_side_transition_shape(), animation_frame)
            }
            PlayerCraftPresentation::WalkerSideTransition { animation_frame } => {
                (self.primary_walker_side_transition_shape(), animation_frame)
            }
            PlayerCraftPresentation::Walker => (self.primary_walker_shape(), 0),
        };
        if let Some(player) = self.state.objects.get_mut(player) {
            player.base.shape = shape;
            player.extension.animation_frame = animation_frame;
        }
    }

    /// Advance the retail folding sequence at the native four-frame cadence.
    /// Returns true for the complete tick that belongs to a transformation,
    /// including the tick on which the stable destination form is reached.
    fn update_player_transformation(&mut self, player: ObjectId) -> bool {
        if self.player_transformation_allowed() && self.state.input.pressed.contains(Button::Select)
        {
            let direction = match self.state.mission.player_craft_form {
                PlayerCraftForm::Flight => Some(PlayerCraftTransformationDirection::ToWalker),
                PlayerCraftForm::Walker => Some(PlayerCraftTransformationDirection::ToFlight),
                PlayerCraftForm::Transforming(_) => None,
            };
            if let Some(direction) = direction {
                self.begin_player_transformation(direction);
            }
        }

        let PlayerCraftForm::Transforming(mut transformation) =
            self.state.mission.player_craft_form
        else {
            return false;
        };
        transformation.elapsed_retail_frames = transformation
            .elapsed_retail_frames
            .saturating_add(PLAYER_TRANSFORMATION_RETAIL_FRAMES_PER_TICK);
        let (next_form, presentation) = transformation_presentation(transformation);
        self.state.mission.player_craft_form = next_form;
        self.apply_player_craft_presentation(player, presentation);
        true
    }

    pub fn tick(&mut self, held_input: u16) -> Result<(), Error> {
        self.state.input.sample(Buttons::from_bits(held_input));
        self.state.frame = self.state.frame.wrapping_add(1);
        self.state.mode_frame = self.state.mode_frame.wrapping_add(1);

        self.update_corneria_defense();
        self.update_mode()?;
        self.update_objects();
        self.resolve_mission_collisions();
        self.build_render_objects()?;
        Ok(())
    }

    fn update_corneria_defense(&mut self) {
        if self.state.mode != GameMode::StrategicMap
            || matches!(
                self.state.campaign.corneria_defense.phase,
                CorneriaDefensePhase::Inactive | CorneriaDefensePhase::Complete
            )
        {
            return;
        }

        let defense = &mut self.state.campaign.corneria_defense;
        defense.elapsed_retail_frames = defense
            .elapsed_retail_frames
            .saturating_add(RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);

        while usize::from(defense.damage_steps_applied) < POST_LEON_DAMAGE_RETAIL_FRAMES.len()
            && defense.elapsed_retail_frames
                >= POST_LEON_DAMAGE_RETAIL_FRAMES[usize::from(defense.damage_steps_applied)]
        {
            self.state.campaign.corneria_damage_percent = self
                .state
                .campaign
                .corneria_damage_percent
                .saturating_add(1)
                .min(CORNERIA_DESTROYED_DAMAGE_PERCENT);
            defense.damage_steps_applied = defense.damage_steps_applied.saturating_add(1);
        }

        defense.phase = match defense.elapsed_retail_frames {
            elapsed if elapsed >= POST_LEON_RESULTS_RETAIL_FRAME => CorneriaDefensePhase::Complete,
            elapsed if elapsed >= POST_LEON_CARRIER_CINEMATIC_RETAIL_FRAME => {
                CorneriaDefensePhase::CarrierCinematic
            }
            elapsed if elapsed >= POST_LEON_CARRIER_WARNING_RETAIL_FRAME => {
                CorneriaDefensePhase::CarrierWarning
            }
            elapsed if elapsed >= POST_LEON_CARRIER_APPROACH_RETAIL_FRAME => {
                CorneriaDefensePhase::CarrierApproach
            }
            elapsed if elapsed >= POST_LEON_PLANET_CANNON_CINEMATIC_RETAIL_FRAME => {
                CorneriaDefensePhase::PlanetCannonCinematic
            }
            elapsed if elapsed >= POST_LEON_PLANET_CANNON_WARNING_RETAIL_FRAME => {
                CorneriaDefensePhase::PlanetCannonWarning
            }
            _ => CorneriaDefensePhase::PostLeonPressure,
        };

        if defense.phase == CorneriaDefensePhase::Complete {
            self.enter_mode(GameMode::Results);
        }
    }

    fn update_mode(&mut self) -> Result<(), Error> {
        if self.state.input.pressed.contains(Button::Start)
            && matches!(self.state.mode, GameMode::Intro(_))
        {
            self.enter_mode(GameMode::Title);
            return Ok(());
        }

        match self.state.mode {
            GameMode::Intro(IntroPhase::Boot) if self.state.mode_frame >= BOOT_INTRO_TICKS => {
                self.enter_mode(GameMode::Intro(IntroPhase::ArgonautLogo));
            }
            GameMode::Intro(IntroPhase::ArgonautLogo)
                if self.state.mode_frame >= ARGONAUT_LOGO_TICKS =>
            {
                self.enter_mode(GameMode::Intro(IntroPhase::NintendoLogo));
            }
            GameMode::Intro(IntroPhase::NintendoLogo)
                if self.state.mode_frame >= NINTENDO_LOGO_TICKS =>
            {
                self.spawn_title_flyby()?;
                self.enter_mode(GameMode::Intro(IntroPhase::Formation));
            }
            GameMode::Intro(IntroPhase::Formation)
                if self.state.mode_frame >= FORMATION_INTRO_TICKS =>
            {
                self.remove_title_flyby();
                self.enter_mode(GameMode::Title);
            }
            GameMode::Title if self.state.input.pressed.contains(Button::Start) => {
                self.update_title();
            }
            GameMode::Title => self.update_title(),
            GameMode::Records => {
                if self.cancel_pressed() || self.confirm_pressed() {
                    self.state.title.page = TitlePage::MainMenu;
                    self.enter_mode(GameMode::Title);
                }
            }
            GameMode::Briefing => {
                if self.confirm_pressed() || self.state.mode_frame >= BRIEFING_PRESENTATION_TICKS {
                    self.state.strategic_map.phase = StrategicMapPhase::OpeningOverview;
                    self.state.campaign.elapsed_frames = 0;
                    self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
                    self.state.strategic_map.destination = INITIAL_PLAYER_MAP_POSITION;
                    self.state.strategic_map.recommended_destination =
                        FIRST_REENGAGEMENT_DESTINATION;
                    self.state.strategic_map.actors = OPENING_ASSAULT_MAP_ACTORS;
                    self.enter_mode(GameMode::StrategicMap);
                }
            }
            GameMode::StrategicMap => {
                self.state.strategic_map.marker_phase =
                    (self.state.strategic_map.marker_phase + 1) % MAP_MARKER_PHASE_COUNT;
                match self.state.strategic_map.phase {
                    StrategicMapPhase::OpeningOverview
                        if self.state.mode_frame >= STRATEGIC_OVERVIEW_TICKS =>
                    {
                        self.begin_pilot_selection();
                    }
                    StrategicMapPhase::Tutorial(StrategicMapTutorialPage::Movement)
                        if self.confirm_pressed() =>
                    {
                        self.state.strategic_map.phase =
                            StrategicMapPhase::Tutorial(StrategicMapTutorialPage::Engagement);
                    }
                    StrategicMapPhase::Tutorial(StrategicMapTutorialPage::Engagement)
                        if self.confirm_pressed() =>
                    {
                        self.state.strategic_map.destination =
                            self.state.strategic_map.player_map_position;
                        self.state.strategic_map.phase = StrategicMapPhase::Planning;
                    }
                    StrategicMapPhase::Planning => {
                        if matches!(
                            self.state.campaign.route_step,
                            CampaignRouteStep::StrategicPressure
                                | CampaignRouteStep::WolfBlockade
                                | CampaignRouteStep::AstropolisAssault
                        ) {
                            self.update_post_mirage_encounter_selection();
                        } else {
                            self.update_strategic_destination();
                        }
                        if self.confirm_pressed()
                            && self.state.strategic_map.destination
                                != self.state.strategic_map.player_map_position
                        {
                            if matches!(
                                self.state.campaign.route_step,
                                CampaignRouteStep::StrategicPressure
                                    | CampaignRouteStep::WolfBlockade
                                    | CampaignRouteStep::AstropolisAssault
                            ) && self.state.strategic_map.selected_encounter.is_some()
                            {
                                self.begin_campaign_sortie()?;
                                return Ok(());
                            }
                            if matches!(
                                self.state.campaign.route_step,
                                CampaignRouteStep::FighterIntercept | CampaignRouteStep::PigmaDuel
                            ) {
                                // These nearby attackers engage the team at
                                // its mothership. Selecting either starts the
                                // defense without moving the player's map
                                // craft or advancing strategic time.
                                self.begin_campaign_sortie()?;
                                return Ok(());
                            }
                            let travel_ticks = match self.state.campaign.route_step {
                                CampaignRouteStep::OpeningEngagement => {
                                    INITIAL_STRATEGIC_TRAVEL_TICKS
                                }
                                CampaignRouteStep::Reengagement => {
                                    REENGAGEMENT_STRATEGIC_TRAVEL_TICKS
                                }
                                CampaignRouteStep::EladardBase => ELADARD_STRATEGIC_TRAVEL_TICKS,
                                CampaignRouteStep::FirstBattleCarrier => {
                                    CARRIER_STRATEGIC_TRAVEL_TICKS
                                }
                                CampaignRouteStep::LeonDuel => LEON_STRATEGIC_TRAVEL_TICKS,
                                CampaignRouteStep::MirageDragon => {
                                    MIRAGE_DRAGON_STRATEGIC_TRAVEL_TICKS
                                }
                                CampaignRouteStep::MissileInterception
                                | CampaignRouteStep::FighterIntercept
                                | CampaignRouteStep::PigmaDuel
                                | CampaignRouteStep::StrategicPressure
                                | CampaignRouteStep::WolfBlockade
                                | CampaignRouteStep::AstropolisAssault => {
                                    MISSILE_INTERCEPTION_STRATEGIC_TRAVEL_TICKS
                                }
                            };
                            let map = &mut self.state.strategic_map;
                            map.travel_origin = map.player_map_position;
                            map.travel_ticks_remaining = travel_ticks;
                            map.travel_total_ticks = travel_ticks;
                            map.travel_origin_damage_percent =
                                self.state.campaign.corneria_damage_percent;
                            map.travel_destination_damage_percent =
                                match self.state.campaign.route_step {
                                    CampaignRouteStep::MissileInterception => {
                                        MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT
                                    }
                                    CampaignRouteStep::EladardBase => {
                                        ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT
                                    }
                                    _ => self.state.campaign.corneria_damage_percent,
                                };
                            self.state.strategic_map.phase = StrategicMapPhase::Traveling;
                        }
                    }
                    StrategicMapPhase::Traveling if self.cancel_pressed() => {
                        self.state.strategic_map.travel_ticks_remaining = 0;
                        self.state.strategic_map.travel_total_ticks = 0;
                        self.state.strategic_map.destination =
                            self.state.strategic_map.player_map_position;
                        self.state.strategic_map.phase = StrategicMapPhase::Planning;
                    }
                    StrategicMapPhase::Traveling => {
                        self.state.campaign.elapsed_frames =
                            self.state.campaign.elapsed_frames.wrapping_add(1);
                        self.state.strategic_map.travel_ticks_remaining = self
                            .state
                            .strategic_map
                            .travel_ticks_remaining
                            .saturating_sub(1);
                        let map = &mut self.state.strategic_map;
                        let elapsed = map
                            .travel_total_ticks
                            .saturating_sub(map.travel_ticks_remaining);
                        map.player_map_position.x = interpolate_map_coordinate(
                            map.travel_origin.x,
                            map.destination.x,
                            elapsed,
                            map.travel_total_ticks,
                        );
                        map.player_map_position.y = interpolate_map_coordinate(
                            map.travel_origin.y,
                            map.destination.y,
                            elapsed,
                            map.travel_total_ticks,
                        );
                        self.state.campaign.corneria_damage_percent = interpolate_percent(
                            map.travel_origin_damage_percent,
                            map.travel_destination_damage_percent,
                            elapsed,
                            map.travel_total_ticks,
                        );
                        if self.state.strategic_map.travel_ticks_remaining == 0 {
                            self.state.strategic_map.player_map_position =
                                self.state.strategic_map.destination;
                            self.begin_campaign_sortie()?;
                        }
                    }
                    StrategicMapPhase::OpeningOverview | StrategicMapPhase::Tutorial(_) => {}
                }
            }
            GameMode::PilotSelection => self.update_pilot_selection(),
            GameMode::Mission => self.update_mission()?,
            GameMode::Ending => self.update_ending(),
            GameMode::Results | GameMode::Intro(_) => {}
        }
        Ok(())
    }

    fn begin_pilot_selection(&mut self) {
        self.state.roster.selected = [None; super::state::SELECTED_PILOT_COUNT];
        self.state.pilot_selection.phase = PilotSelectionPhase::Revealing;
        self.state.pilot_selection.cursor = Pilot::Fox;
        self.enter_mode(GameMode::PilotSelection);
    }

    fn update_pilot_selection(&mut self) {
        match self.state.pilot_selection.phase {
            PilotSelectionPhase::Revealing
                if self.state.mode_frame >= PILOT_SELECTION_REVEAL_TICKS =>
            {
                self.state.pilot_selection.phase = PilotSelectionPhase::ChoosingPrimary;
                self.state.mode_frame = 0;
            }
            PilotSelectionPhase::ChoosingPrimary => {
                self.update_pilot_cursor(None);
                if self.confirm_pressed() {
                    let primary = self.state.pilot_selection.cursor;
                    self.state.roster.selected[0] = Some(primary);
                    self.state.pilot_selection.cursor = if primary == Pilot::Slippy {
                        Pilot::Fox
                    } else {
                        Pilot::Slippy
                    };
                    self.state.pilot_selection.phase = PilotSelectionPhase::ChoosingWingmate;
                    self.state.mode_frame = 0;
                }
            }
            PilotSelectionPhase::ChoosingWingmate => {
                let primary = self.state.roster.selected[0];
                self.update_pilot_cursor(primary);
                if self.cancel_pressed() {
                    self.state.roster.selected[0] = None;
                    self.state.pilot_selection.cursor = Pilot::Fox;
                    self.state.pilot_selection.phase = PilotSelectionPhase::ChoosingPrimary;
                    self.state.mode_frame = 0;
                } else if self.confirm_pressed() {
                    self.state.roster.selected[1] = Some(self.state.pilot_selection.cursor);
                    self.state.pilot_selection.phase = PilotSelectionPhase::Ready;
                    self.state.mode_frame = 0;
                }
            }
            PilotSelectionPhase::Ready => {
                if self.cancel_pressed() {
                    self.state.roster.selected[1] = None;
                    self.state.pilot_selection.phase = PilotSelectionPhase::ChoosingWingmate;
                    self.state.mode_frame = 0;
                } else if self.confirm_pressed() {
                    self.state.pilot_selection.phase = PilotSelectionPhase::Launching;
                    self.state.mode_frame = 0;
                }
            }
            PilotSelectionPhase::Launching if self.state.mode_frame >= PILOT_LAUNCH_TICKS => {
                self.state.strategic_map.phase =
                    StrategicMapPhase::Tutorial(StrategicMapTutorialPage::Movement);
                self.enter_mode(GameMode::StrategicMap);
            }
            PilotSelectionPhase::Revealing | PilotSelectionPhase::Launching => {}
        }
    }

    fn update_pilot_cursor(&mut self, excluded: Option<Pilot>) {
        let previous = self.state.input.pressed.contains(Button::Left)
            || self.state.input.pressed.contains(Button::Up);
        let next = self.state.input.pressed.contains(Button::Right)
            || self.state.input.pressed.contains(Button::Down);
        let mut cursor = self.state.pilot_selection.cursor;
        if previous {
            cursor = cursor.previous();
        }
        if next {
            cursor = cursor.next();
        }
        if Some(cursor) == excluded {
            cursor = if previous {
                cursor.previous()
            } else {
                cursor.next()
            };
        }
        self.state.pilot_selection.cursor = cursor;
    }

    fn update_strategic_destination(&mut self) {
        let destination = &mut self.state.strategic_map.destination;
        if self.state.input.held.contains(Button::Left) {
            destination.x = destination.x.saturating_sub(STRATEGIC_CURSOR_STEP);
        }
        if self.state.input.held.contains(Button::Right) {
            destination.x = destination.x.saturating_add(STRATEGIC_CURSOR_STEP);
        }
        if self.state.input.held.contains(Button::Up) {
            destination.y = destination.y.saturating_sub(STRATEGIC_CURSOR_STEP);
        }
        if self.state.input.held.contains(Button::Down) {
            destination.y = destination.y.saturating_add(STRATEGIC_CURSOR_STEP);
        }
    }

    fn update_post_mirage_encounter_selection(&mut self) {
        match self.state.campaign.route_step {
            CampaignRouteStep::WolfBlockade => {
                self.state.strategic_map.selected_encounter =
                    Some(StrategicEncounter::WolfBlockade);
                self.state.strategic_map.destination = WOLF_BLOCKADE_DESTINATION;
                self.state.strategic_map.recommended_destination = WOLF_BLOCKADE_DESTINATION;
                return;
            }
            CampaignRouteStep::AstropolisAssault => {
                self.state.strategic_map.selected_encounter =
                    Some(StrategicEncounter::AstropolisAssault);
                self.state.strategic_map.destination = ASTROPOLIS_DESTINATION;
                self.state.strategic_map.recommended_destination = ASTROPOLIS_DESTINATION;
                return;
            }
            CampaignRouteStep::StrategicPressure => {}
            CampaignRouteStep::OpeningEngagement
            | CampaignRouteStep::Reengagement
            | CampaignRouteStep::MissileInterception
            | CampaignRouteStep::FighterIntercept
            | CampaignRouteStep::PigmaDuel
            | CampaignRouteStep::EladardBase
            | CampaignRouteStep::FirstBattleCarrier
            | CampaignRouteStep::LeonDuel
            | CampaignRouteStep::MirageDragon => return,
        }

        if self.state.input.pressed.contains(Button::Left) {
            self.state.strategic_map.selected_encounter =
                Some(StrategicEncounter::RecurringAttackers);
            self.state.strategic_map.destination = RECURRING_ATTACKERS_DESTINATION;
            self.state.strategic_map.recommended_destination = RECURRING_ATTACKERS_DESTINATION;
        } else if self.state.input.pressed.contains(Button::Right) {
            self.state.strategic_map.selected_encounter = Some(StrategicEncounter::LeonPressure);
            self.state.strategic_map.destination = LEON_PRESSURE_DESTINATION;
            self.state.strategic_map.recommended_destination = LEON_PRESSURE_DESTINATION;
        } else if self.state.input.pressed.contains(Button::Down) {
            let pending_major_objective =
                if self.state.campaign.objectives.titania == PlanetObjectiveStatus::Occupied {
                    Some((StrategicEncounter::TitaniaBase, TITANIA_BASE_DESTINATION))
                } else if self.state.campaign.objectives.second_carrier
                    == CarrierObjectiveStatus::Operational
                {
                    Some((
                        StrategicEncounter::SecondBattleCarrier,
                        SECOND_BATTLE_CARRIER_DESTINATION,
                    ))
                } else {
                    None
                };
            if let Some((encounter, destination)) = pending_major_objective {
                self.state.strategic_map.selected_encounter = Some(encounter);
                self.state.strategic_map.destination = destination;
                self.state.strategic_map.recommended_destination = destination;
            }
        } else if self.state.input.pressed.contains(Button::Up)
            && self.state.campaign.objectives.major_objectives_complete()
            && self.state.campaign.objectives.live_attackers.remaining() == 1
        {
            self.state.strategic_map.selected_encounter = Some(StrategicEncounter::FinalPursuer);
            self.state.strategic_map.destination = FINAL_PURSUER_DESTINATION;
            self.state.strategic_map.recommended_destination = FINAL_PURSUER_DESTINATION;
        }
    }

    fn begin_campaign_sortie(&mut self) -> Result<(), Error> {
        match self.state.campaign.route_step {
            CampaignRouteStep::OpeningEngagement => self.begin_opening_sortie(),
            CampaignRouteStep::Reengagement => self.begin_reengagement_sortie(),
            CampaignRouteStep::MissileInterception => self.begin_missile_interception_sortie(),
            CampaignRouteStep::FighterIntercept => self.begin_fighter_intercept_sortie(),
            CampaignRouteStep::PigmaDuel => self.begin_pigma_duel_sortie(),
            CampaignRouteStep::EladardBase => self.begin_eladard_sortie(),
            CampaignRouteStep::FirstBattleCarrier => {
                self.begin_carrier_assault(MissionVisit::FirstBattleCarrier)
            }
            CampaignRouteStep::LeonDuel => self.begin_leon_duel_sortie(),
            CampaignRouteStep::MirageDragon => self.begin_mirage_dragon_sortie(),
            CampaignRouteStep::StrategicPressure => {
                match self.state.strategic_map.selected_encounter {
                    Some(StrategicEncounter::TitaniaBase) => self.begin_titania_sortie(),
                    Some(StrategicEncounter::SecondBattleCarrier) => {
                        self.begin_carrier_assault(MissionVisit::SecondBattleCarrier)
                    }
                    Some(StrategicEncounter::RecurringAttackers) => {
                        self.begin_pressure_fighter_encounter()
                    }
                    Some(StrategicEncounter::LeonPressure) => self.begin_leon_pressure_encounter(),
                    Some(StrategicEncounter::FinalPursuer) => self.begin_final_pursuer_encounter(),
                    Some(StrategicEncounter::WolfBlockade)
                    | Some(StrategicEncounter::AstropolisAssault) => Ok(()),
                    None => Ok(()),
                }
            }
            CampaignRouteStep::WolfBlockade => self.begin_wolf_blockade_encounter(),
            CampaignRouteStep::AstropolisAssault => self.begin_astropolis_assault(),
        }
    }

    fn begin_opening_sortie(&mut self) -> Result<(), Error> {
        let primary_pilot = self.primary_pilot();
        let wingmate_pilot = self.state.roster.selected[1].unwrap_or(DEFAULT_WINGMATE_PILOT);
        let mut primary = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
        primary.base.position = OPENING_PRIMARY_POSITION;
        primary.base.hit_points = primary_pilot.craft_profile().maximum_shield;
        primary.base.collision_class = CollisionClass::Player;
        primary.base.flags.casts_shadow = true;
        primary.base.flags.visible = false;
        primary.base.flags.collision_disabled = true;
        let primary_id = self
            .state
            .objects
            .allocate(primary)
            .ok_or(Error::ObjectCapacityReached)?;

        let mut wingmate =
            Object::new(ObjectKind::Wingmate, ShapeId::EMPTY, Behavior::PlayerFlight);
        wingmate.base.position = OPENING_WINGMATE_POSITION;
        wingmate.base.hit_points = wingmate_pilot.craft_profile().maximum_shield;
        wingmate.base.collision_class = CollisionClass::Player;
        wingmate.base.flags.casts_shadow = true;
        wingmate.base.flags.visible = false;
        wingmate.base.flags.collision_disabled = true;
        wingmate.base.linked_object = Some(primary_id);
        let Some(wingmate_id) = self.state.objects.allocate(wingmate) else {
            self.state.objects.remove(primary_id);
            return Err(Error::ObjectCapacityReached);
        };
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            primary.base.linked_object = Some(wingmate_id);
        }

        self.start_sortie(MissionVisit::OpeningEngagement, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_reengagement_sortie(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        let first_player = second_sortie::PLAYER_KEYFRAMES[0];
        let first_wingmate = second_sortie::WINGMATE_KEYFRAMES[0];
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            apply_player_keyframe(primary, first_player);
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        self.previous_mission_player_position = Some(first_player.position);
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            apply_player_keyframe(wingmate, first_wingmate);
            wingmate.base.shape = ShapeId::EMPTY;
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.spawn_reengagement_targets()?;
        self.start_sortie(MissionVisit::Reengagement, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_missile_interception_sortie(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            apply_player_keyframe(primary, missile_interception::PLAYER_KEYFRAMES[0]);
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            apply_player_keyframe(wingmate, missile_interception::WINGMATE_KEYFRAMES[0]);
            wingmate.base.shape = ShapeId::EMPTY;
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.spawn_interception_missiles()?;
        self.start_sortie(MissionVisit::MissileInterception, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_fighter_intercept_sortie(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            apply_player_keyframe(primary, fighter_intercept::PLAYER_KEYFRAMES[0]);
            primary.base.shape = ShapeId::EMPTY;
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        self.previous_mission_player_position =
            Some(fighter_intercept::PLAYER_KEYFRAMES[0].position);
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            apply_player_keyframe(wingmate, fighter_intercept::WINGMATE_KEYFRAMES[0]);
            wingmate.base.shape = ShapeId::EMPTY;
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.spawn_fighter_intercept_targets()?;
        self.state.mission.score = 0;
        self.state.mission.objects_destroyed = 0;
        self.start_sortie(MissionVisit::FighterIntercept, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_pigma_duel_sortie(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            apply_player_keyframe(primary, pigma_duel::PLAYER_KEYFRAMES[0]);
            primary.base.shape = ShapeId::EMPTY;
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        self.previous_mission_player_position = Some(pigma_duel::PLAYER_KEYFRAMES[0].position);
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            apply_player_keyframe(wingmate, pigma_duel::WINGMATE_KEYFRAMES[0]);
            wingmate.base.shape = ShapeId::EMPTY;
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.spawn_pigma_rival()?;
        self.state.mission.objects_destroyed = 0;
        self.start_sortie(MissionVisit::PigmaDuel, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_leon_duel_sortie(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            apply_player_keyframe(primary, leon_duel::PLAYER_KEYFRAMES[0]);
            primary.base.shape = ShapeId::EMPTY;
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            apply_player_keyframe(wingmate, leon_duel::WINGMATE_KEYFRAMES[0]);
            wingmate.base.shape = ShapeId::EMPTY;
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.spawn_leon_rival()?;
        self.state.mission.objects_destroyed = 0;
        self.start_sortie(MissionVisit::LeonDuel, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_mirage_dragon_sortie(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            apply_player_keyframe(primary, mirage_dragon::PLAYER_KEYFRAMES[0]);
            primary.base.shape = ShapeId::EMPTY;
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            apply_player_keyframe(wingmate, mirage_dragon::WINGMATE_KEYFRAMES[0]);
            wingmate.base.shape = ShapeId::EMPTY;
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.spawn_mirage_dragon()?;
        self.state.mission.objects_destroyed = 0;
        self.start_sortie(MissionVisit::MirageDragon, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_pressure_fighter_encounter(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            apply_player_keyframe(primary, pressure_fighters::PLAYER_KEYFRAMES[0]);
            primary.base.shape = ShapeId::EMPTY;
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            apply_player_keyframe(wingmate, pressure_fighters::WINGMATE_KEYFRAMES[0]);
            wingmate.base.shape = ShapeId::EMPTY;
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.spawn_pressure_fighters()?;
        self.start_sortie(MissionVisit::RecurringAttackers, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_leon_pressure_encounter(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            apply_player_keyframe(primary, leon_pressure::PLAYER_KEYFRAMES[0]);
            primary.base.shape = ShapeId::EMPTY;
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            apply_player_keyframe(wingmate, leon_pressure::WINGMATE_KEYFRAMES[0]);
            wingmate.base.shape = ShapeId::EMPTY;
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.spawn_leon_rival()?;
        self.start_sortie(MissionVisit::LeonPressure, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_final_pursuer_encounter(&mut self) -> Result<(), Error> {
        self.begin_final_rival_encounter(
            MissionVisit::FinalPursuer,
            ShapeId::FINAL_PURSUER_CRAFT,
            final_pursuer::PLAYER_KEYFRAMES[0],
            final_pursuer::WINGMATE_KEYFRAMES[0],
        )
    }

    fn begin_wolf_blockade_encounter(&mut self) -> Result<(), Error> {
        self.begin_final_rival_encounter(
            MissionVisit::WolfBlockade,
            ShapeId::WOLF_BLOCKADE_CRAFT,
            wolf_blockade::PLAYER_KEYFRAMES[0],
            wolf_blockade::WINGMATE_KEYFRAMES[0],
        )
    }

    fn begin_final_rival_encounter(
        &mut self,
        visit: MissionVisit,
        shape: ShapeId,
        player_keyframe: MissionPlayerKeyframe,
        wingmate_keyframe: MissionPlayerKeyframe,
    ) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            apply_player_keyframe(primary, player_keyframe);
            primary.base.shape = ShapeId::EMPTY;
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        self.previous_mission_player_position = Some(player_keyframe.position);
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            apply_player_keyframe(wingmate, wingmate_keyframe);
            wingmate.base.shape = ShapeId::EMPTY;
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.spawn_final_rival(shape)?;
        self.start_sortie(visit, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_astropolis_assault(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            apply_player_keyframe(primary, astropolis_entry::PLAYER_KEYFRAMES[0]);
            primary.base.shape = ShapeId::EMPTY;
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            apply_player_keyframe(wingmate, astropolis_entry::WINGMATE_KEYFRAMES[0]);
            wingmate.base.shape = ShapeId::EMPTY;
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.state.mission.astropolis = AstropolisMissionState::default();
        self.start_sortie(MissionVisit::AstropolisAssault, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_eladard_sortie(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        for craft in [primary_id, wingmate_id] {
            if let Some(object) = self.state.objects.get_mut(craft) {
                object.base.position = Vector3::default();
                object.base.velocity = Vector3::default();
                object.base.pitch = Angle::ZERO;
                object.base.yaw = Angle::ZERO;
                object.base.roll = Angle::ZERO;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
            }
        }
        self.state.mission.eladard = EladardMissionState {
            phase: EladardPhase::SurfaceApproach,
            phase_started_retail_frame: 0,
            surface_barriers_remaining: ELADARD_SURFACE_BARRIER_COUNT,
            platform_switch_pressed: false,
            wall_spider_hit_points: ELADARD_WALL_SPIDER_HEALTH,
            generator_active: false,
            generator_hit_points: ELADARD_GENERATOR_HEALTH,
        };
        self.state.mission.objects_destroyed = 0;
        self.start_sortie(MissionVisit::EladardBase, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_titania_sortie(&mut self) -> Result<(), Error> {
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        for craft in [primary_id, wingmate_id] {
            if let Some(object) = self.state.objects.get_mut(craft) {
                object.base.position = Vector3::default();
                object.base.velocity = Vector3::default();
                object.base.pitch = Angle::ZERO;
                object.base.yaw = Angle::ZERO;
                object.base.roll = Angle::ZERO;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
            }
        }
        debug_assert_eq!(TITANIA_REACTOR_COUNT, 1);
        self.state.mission.titania = TitaniaMissionState {
            phase: TitaniaPhase::SurfaceApproach,
            phase_started_retail_frame: 0,
            surface_switches: [TitaniaSurfaceSwitchStatus::Active; TITANIA_SURFACE_SWITCH_COUNT],
            reactor: TitaniaReactorStatus::Shielded,
        };
        self.state.mission.objects_destroyed = 0;
        self.start_sortie(MissionVisit::TitaniaBase, primary_id, wingmate_id);
        Ok(())
    }

    fn begin_carrier_assault(&mut self, visit: MissionVisit) -> Result<(), Error> {
        debug_assert_eq!(BATTLE_CARRIER_REQUIRED_VISITS, 2);
        debug_assert!(matches!(
            visit,
            MissionVisit::FirstBattleCarrier | MissionVisit::SecondBattleCarrier
        ));
        let primary_id = self
            .state
            .mission
            .primary_player
            .ok_or(Error::ObjectCapacityReached)?;
        let wingmate_id = self
            .state
            .mission
            .wingmate
            .ok_or(Error::ObjectCapacityReached)?;
        if let Some(primary) = self.state.objects.get_mut(primary_id) {
            primary.base.shape = ShapeId::CARRIER_ASSAULT_CRAFT;
            primary.base.position = Vector3::default();
            primary.base.velocity = Vector3::default();
            primary.base.pitch = Angle::ZERO;
            primary.base.yaw = Angle::ZERO;
            primary.base.roll = Angle::ZERO;
            primary.base.flags.visible = false;
            primary.base.flags.collision_disabled = true;
        }
        if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
            wingmate.base.position = Vector3::default();
            wingmate.base.velocity = Vector3::default();
            wingmate.base.flags.visible = false;
            wingmate.base.flags.collision_disabled = true;
        }
        self.state.mission.carrier_assault = CarrierAssaultState {
            phase: CarrierAssaultPhase::ExteriorApproach,
            phase_started_retail_frame: 0,
            corridor_progress: 0,
            reactor_room_open: false,
            port_panel: CarrierReactorPanel {
                integrity: CARRIER_PANEL_INITIAL_INTEGRITY,
                active: true,
            },
            starboard_panel: CarrierReactorPanel {
                integrity: CARRIER_PANEL_INITIAL_INTEGRITY,
                active: true,
            },
        };
        self.state.mission.objects_destroyed = 0;
        self.spawn_carrier_exterior_scene()?;
        self.start_sortie(visit, primary_id, wingmate_id);
        Ok(())
    }

    fn start_sortie(&mut self, visit: MissionVisit, primary_id: ObjectId, wingmate_id: ObjectId) {
        self.state.mission.active = true;
        self.state.mission.phase = MissionPhase::Loading;
        self.state.mission.mission = Some(match visit {
            MissionVisit::OpeningEngagement | MissionVisit::Reengagement => {
                MissionId::OPENING_SORTIE
            }
            MissionVisit::MissileInterception => MissionId::MISSILE_INTERCEPTION,
            MissionVisit::FighterIntercept => MissionId::FIGHTER_INTERCEPT,
            MissionVisit::PigmaDuel => MissionId::PIGMA_DUEL,
            MissionVisit::LeonDuel => MissionId::LEON_DUEL,
            MissionVisit::LeonPressure => MissionId::LEON_DUEL,
            MissionVisit::MirageDragon => MissionId::MIRAGE_DRAGON,
            MissionVisit::RecurringAttackers => MissionId::FIGHTER_INTERCEPT,
            MissionVisit::FinalPursuer => MissionId::FINAL_PURSUER,
            MissionVisit::WolfBlockade => MissionId::WOLF_BLOCKADE,
            MissionVisit::AstropolisAssault => MissionId::ASTROPOLIS,
            MissionVisit::TitaniaBase => MissionId::TITANIA_BASE,
            MissionVisit::EladardBase => MissionId::ELADARD_BASE,
            MissionVisit::FirstBattleCarrier | MissionVisit::SecondBattleCarrier => {
                MissionId::BATTLE_CARRIER
            }
        });
        self.state.mission.visit = visit;
        self.state.mission.primary_player = Some(primary_id);
        self.state.mission.wingmate = Some(wingmate_id);
        self.state.mission.player_blaster = PlayerBlasterState::Ready;
        self.state.mission.player_craft_form = PlayerCraftForm::Flight;
        self.state.mission.player_walker = Default::default();
        self.state.mission.player_flight.pitch_accumulator = 0;
        self.state.mission.player_flight.yaw_accumulator = OPENING_FLIGHT_YAW_ACCUMULATOR;
        self.state.mission.player_flight.pitch_lean = 0;
        self.state.mission.departed_certified_neutral_path = false;
        if visit == MissionVisit::OpeningEngagement {
            self.state.mission.item_count = INITIAL_ITEM_COUNT;
        }
        self.state.mission.elapsed_time_tenths = 0;
        self.state.mission.camera_follow_offset = ACTIVE_CAMERA_FOLLOW_OFFSET;
        self.state.strategic_map.primary_player = Some(primary_id);
        self.state.camera = Camera::default();
        self.enter_mode(GameMode::Mission);
    }

    fn update_mission(&mut self) -> Result<(), Error> {
        match self.state.mission.visit {
            MissionVisit::OpeningEngagement => self.update_opening_mission(),
            MissionVisit::Reengagement => self.update_reengagement_mission(),
            MissionVisit::MissileInterception => self.update_missile_interception(),
            MissionVisit::FighterIntercept => self.update_fighter_intercept(),
            MissionVisit::PigmaDuel => self.update_pigma_duel(),
            MissionVisit::LeonDuel => self.update_leon_duel(),
            MissionVisit::MirageDragon => self.update_mirage_dragon(),
            MissionVisit::RecurringAttackers => self.update_pressure_fighter_encounter(),
            MissionVisit::LeonPressure => self.update_leon_pressure_encounter(),
            MissionVisit::FinalPursuer | MissionVisit::WolfBlockade => {
                self.update_final_rival_encounter()
            }
            MissionVisit::AstropolisAssault => self.update_astropolis_assault(),
            MissionVisit::TitaniaBase => self.update_titania_base(),
            MissionVisit::EladardBase => self.update_eladard_base(),
            MissionVisit::FirstBattleCarrier | MissionVisit::SecondBattleCarrier => {
                self.update_carrier_assault()
            }
        }
    }

    fn update_opening_mission(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let primary_flight_craft_shape = self.primary_flight_craft_shape();
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);
        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                if let Some(primary) = self.state.mission.primary_player {
                    if let Some(object) = self.state.objects.get_mut(primary) {
                        object.base.position = SORTIE_ENTRY_PRIMARY_POSITION;
                        object.base.yaw = super::object::Angle::from_units(227);
                    }
                }
                if let Some(wingmate) = self.state.mission.wingmate {
                    if let Some(object) = self.state.objects.get_mut(wingmate) {
                        object.base.position = SORTIE_ENTRY_WINGMATE_POSITION;
                    }
                }
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                if let Some(primary) = self.state.mission.primary_player {
                    if let Some(object) = self.state.objects.get_mut(primary) {
                        object.base.shape = primary_flight_craft_shape;
                        object.base.position = ACTIVE_PRIMARY_POSITION;
                        object.base.roll = Angle::from_units(6);
                        object.base.speed = 6;
                        object.base.flags.visible = true;
                        object.base.flags.collision_disabled = false;
                    }
                }
                if let Some(wingmate) = self.state.mission.wingmate {
                    if let Some(object) = self.state.objects.get_mut(wingmate) {
                        object.base.position = ACTIVE_WINGMATE_POSITION;
                    }
                }
                self.state.camera.position = ACTIVE_CAMERA_POSITION;
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= OPENING_SORTIE_RETURN_TRIGGER_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= OPENING_SORTIE_STRATEGIC_MAP_RETURN_RETAIL_FRAME =>
            {
                self.finish_sortie();
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        if self.state.mode_frame >= MISSION_ENTRY_FORMATION_TICKS
            && self.mission_entry_flyby.iter().all(Option::is_none)
        {
            self.spawn_mission_entry_flyby()?;
        }
        if retail_frame <= MISSION_CONTROL_HANDOFF_RETAIL_FRAME {
            self.update_mission_camera(retail_frame);
        }
        if matches!(
            self.state.mission.phase,
            MissionPhase::Active | MissionPhase::ReturningToStrategicMap
        ) {
            if retail_frame <= MISSION_PLAYER_INPUT_START_RETAIL_FRAME {
                self.update_mission_player_entry(retail_frame);
            } else {
                let weapons_enabled = self.state.mission.phase == MissionPhase::Active;
                self.update_active_flight(retail_frame, weapons_enabled)?;
            }
        }
        if retail_frame >= MISSION_PLAYER_CRAFT_HIDE_RETAIL_FRAME {
            if let Some(primary) = self.state.mission.primary_player {
                if let Some(object) = self.state.objects.get_mut(primary) {
                    object.base.flags.visible = false;
                }
            }
        }
        self.update_mission_entry_flyby(retail_frame);
        self.update_mission_projectiles(retail_frame)?;
        Ok(())
    }

    fn update_reengagement_mission(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let primary_flight_craft_shape = self.primary_flight_craft_shape();
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                if let Some(primary) = self.state.mission.primary_player {
                    if let Some(object) = self.state.objects.get_mut(primary) {
                        object.base.shape = primary_flight_craft_shape;
                        object.base.flags.visible = true;
                        object.base.flags.collision_disabled = false;
                    }
                }
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= second_sortie::MAP_READY_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
                self.finish_sortie();
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        if matches!(
            self.state.mission.phase,
            MissionPhase::Active | MissionPhase::ReturningToStrategicMap
        ) && retail_frame > MISSION_PLAYER_INPUT_START_RETAIL_FRAME
        {
            let weapons_enabled = self.state.mission.phase == MissionPhase::Active;
            self.update_active_flight(retail_frame, weapons_enabled)?;
        } else {
            self.update_reengagement_presentation(retail_frame);
        }
        if retail_frame >= MISSION_PLAYER_CRAFT_HIDE_RETAIL_FRAME {
            if let Some(primary) = self.state.mission.primary_player {
                if let Some(object) = self.state.objects.get_mut(primary) {
                    object.base.flags.visible = false;
                }
            }
        }
        let current_player_position = self
            .state
            .mission
            .primary_player
            .and_then(|id| self.state.objects.get(id))
            .map(|object| object.base.position);
        let previous_player_position = current_player_position
            .map(|current| self.previous_mission_player_position.unwrap_or(current));
        self.update_reengagement_targets(retail_frame);
        if let (Some(current), Some(previous)) = (current_player_position, previous_player_position)
        {
            self.update_reengagement_projectiles(retail_frame, current, previous)?;
        }
        Ok(())
    }

    fn update_missile_interception(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = retail_frame
            .saturating_sub(INTERCEPTION_TIMER_START_RETAIL_FRAME)
            / INTERCEPTION_RETAIL_FRAMES_PER_TENTH;

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= missile_interception::RETURN_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= missile_interception::MAP_READY_RETAIL_FRAME =>
            {
                self.finish_sortie();
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        if retail_frame > MISSION_PLAYER_INPUT_START_RETAIL_FRAME {
            self.update_active_flight(
                retail_frame,
                self.state.mission.phase == MissionPhase::Active,
            )?;
        } else {
            self.update_interception_presentation(retail_frame);
        }
        self.update_interception_missiles(retail_frame);
        Ok(())
    }

    fn update_fighter_intercept(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= fighter_intercept::RETURN_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= fighter_intercept::MAP_READY_RETAIL_FRAME =>
            {
                self.finish_sortie();
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        if retail_frame > MISSION_PLAYER_INPUT_START_RETAIL_FRAME {
            self.update_active_flight(
                retail_frame,
                self.state.mission.phase == MissionPhase::Active,
            )?;
        } else {
            self.update_fighter_intercept_presentation(retail_frame);
        }
        let current_player_position = self
            .state
            .mission
            .primary_player
            .and_then(|id| self.state.objects.get(id))
            .map(|object| object.base.position);
        let previous_player_position = current_player_position
            .map(|current| self.previous_mission_player_position.unwrap_or(current));
        if let (Some(current), Some(previous)) = (current_player_position, previous_player_position)
        {
            self.update_fighter_intercept_targets(retail_frame, current, previous);
            self.update_fighter_intercept_projectiles(retail_frame, current, previous)?;
            self.previous_mission_player_position = Some(current);
        } else {
            self.update_fighter_intercept_targets(
                retail_frame,
                Vector3::default(),
                Vector3::default(),
            );
        }
        Ok(())
    }

    fn update_pigma_duel(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= pigma_duel::RETURN_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= pigma_duel::MAP_READY_RETAIL_FRAME =>
            {
                self.finish_sortie();
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        if retail_frame > MISSION_PLAYER_INPUT_START_RETAIL_FRAME {
            self.update_active_flight(
                retail_frame,
                self.state.mission.phase == MissionPhase::Active,
            )?;
        } else {
            self.update_pigma_presentation(retail_frame);
        }
        let current_player_position = self
            .state
            .mission
            .primary_player
            .and_then(|id| self.state.objects.get(id))
            .map(|object| object.base.position);
        let previous_player_position = current_player_position
            .map(|current| self.previous_mission_player_position.unwrap_or(current));
        if let (Some(current), Some(previous)) = (current_player_position, previous_player_position)
        {
            self.update_pigma_rival(retail_frame, current, previous);
            self.update_pigma_projectiles(retail_frame, current, previous)?;
            self.previous_mission_player_position = Some(current);
        } else {
            self.update_pigma_rival(retail_frame, Vector3::default(), Vector3::default());
            self.update_pigma_projectiles(retail_frame, Vector3::default(), Vector3::default())?;
        }
        Ok(())
    }

    fn update_leon_duel(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= leon_duel::RETURN_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= leon_duel::MAP_READY_RETAIL_FRAME =>
            {
                self.finish_sortie();
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        if retail_frame > MISSION_PLAYER_INPUT_START_RETAIL_FRAME {
            self.update_active_flight(
                retail_frame,
                self.state.mission.phase == MissionPhase::Active,
            )?;
        } else {
            self.update_leon_presentation(retail_frame);
        }
        self.update_leon_rival(retail_frame);
        self.update_leon_projectiles(retail_frame)?;
        Ok(())
    }

    fn update_pressure_fighter_encounter(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= pressure_fighters::RETURN_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= pressure_fighters::MAP_READY_RETAIL_FRAME =>
            {
                self.finish_pressure_encounter(RECURRING_ATTACKERS_ELAPSED_DISPLAY_SECONDS);
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        if retail_frame > MISSION_PLAYER_INPUT_START_RETAIL_FRAME {
            self.update_active_flight(
                retail_frame,
                self.state.mission.phase == MissionPhase::Active,
            )?;
        } else {
            self.update_pressure_fighter_presentation(retail_frame);
        }
        self.update_pressure_fighter_actors(retail_frame);
        self.update_pressure_fighter_projectiles(retail_frame)?;
        Ok(())
    }

    fn update_leon_pressure_encounter(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= leon_pressure::RETURN_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= leon_pressure::MAP_READY_RETAIL_FRAME =>
            {
                self.finish_pressure_encounter(LEON_PRESSURE_ELAPSED_DISPLAY_SECONDS);
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        if retail_frame > MISSION_PLAYER_INPUT_START_RETAIL_FRAME {
            self.update_active_flight(
                retail_frame,
                self.state.mission.phase == MissionPhase::Active,
            )?;
        } else {
            self.update_leon_pressure_presentation(retail_frame);
        }
        self.update_leon_pressure_rival(retail_frame);
        self.update_leon_pressure_projectiles(retail_frame)?;
        Ok(())
    }

    fn update_final_rival_encounter(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);
        let (return_frame, map_ready_frame) = match self.state.mission.visit {
            MissionVisit::FinalPursuer => (
                final_pursuer::RETURN_RETAIL_FRAME,
                final_pursuer::MAP_READY_RETAIL_FRAME,
            ),
            MissionVisit::WolfBlockade => (
                wolf_blockade::RETURN_RETAIL_FRAME,
                wolf_blockade::MAP_READY_RETAIL_FRAME,
            ),
            _ => unreachable!("final rival update requires a final rival visit"),
        };

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= return_frame => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap if retail_frame >= map_ready_frame => {
                self.finish_final_rival_encounter();
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        if retail_frame > MISSION_PLAYER_INPUT_START_RETAIL_FRAME {
            self.update_active_flight(
                retail_frame,
                self.state.mission.phase == MissionPhase::Active,
            )?;
        } else {
            self.update_final_rival_presentation(retail_frame);
        }
        let current_player_position = self
            .state
            .mission
            .primary_player
            .and_then(|id| self.state.objects.get(id))
            .map(|object| object.base.position);
        let previous_player_position = current_player_position
            .map(|current| self.previous_mission_player_position.unwrap_or(current));
        if let (Some(current), Some(previous)) =
            (current_player_position, previous_player_position)
        {
            self.update_final_rival_actor(retail_frame, current, previous);
            self.previous_mission_player_position = Some(current);
        } else {
            self.update_final_rival_actor(
                retail_frame,
                Vector3::default(),
                Vector3::default(),
            );
        }
        self.update_final_rival_projectiles(retail_frame)?;
        Ok(())
    }

    fn update_astropolis_assault(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        if self.state.mission.phase == MissionPhase::Loading
            && self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS
        {
            self.state.mission.phase = MissionPhase::EntryCinematic;
        }
        if self.state.mission.phase == MissionPhase::EntryCinematic
            && retail_frame >= ASTROPOLIS_INTERIOR_CORRIDOR_RETAIL_FRAME
        {
            self.state.mission.phase = MissionPhase::Active;
        }

        if matches!(
            self.state.mission.astropolis.phase,
            AstropolisPhase::ExteriorApproach
                | AstropolisPhase::BaseEntry
                | AstropolisPhase::InteriorCorridor
        ) {
            let (phase, phase_started_retail_frame) =
                if retail_frame >= ASTROPOLIS_INTERIOR_CORRIDOR_RETAIL_FRAME {
                    (
                        AstropolisPhase::InteriorCorridor,
                        ASTROPOLIS_INTERIOR_CORRIDOR_RETAIL_FRAME,
                    )
                } else if retail_frame >= ASTROPOLIS_BASE_ENTRY_RETAIL_FRAME {
                    (
                        AstropolisPhase::BaseEntry,
                        ASTROPOLIS_BASE_ENTRY_RETAIL_FRAME,
                    )
                } else {
                    (AstropolisPhase::ExteriorApproach, 0)
                };
            self.state.mission.astropolis.phase = phase;
            self.state.mission.astropolis.phase_started_retail_frame = phase_started_retail_frame;
            self.state.mission.astropolis.corridor_progress =
                retail_frame.saturating_sub(ASTROPOLIS_INTERIOR_CORRIDOR_RETAIL_FRAME);
        }

        if self.state.mission.astropolis.phase == AstropolisPhase::FinalCore {
            self.state
                .mission
                .astropolis
                .advance_core_exposure(RETAIL_PRESENTATION_FRAMES_PER_TICK as u16, retail_frame);
        }
        if self.state.mission.astropolis.phase == AstropolisPhase::CoreDestruction
            && retail_frame.saturating_sub(self.state.mission.astropolis.phase_started_retail_frame)
                >= astropolis_assault::CORE_DESTRUCTION_RETAIL_FRAMES
        {
            self.state.mission.astropolis.begin_escape(retail_frame);
            self.state.campaign.objectives.astropolis = AstropolisStatus::Assaulted;
            self.clear_sortie_runtime();
            self.state.ending = EndingState::default();
            self.enter_mode(GameMode::Ending);
            return Ok(());
        }

        self.update_active_flight(
            retail_frame,
            self.state.mission.phase == MissionPhase::Active,
        )
    }

    fn update_ending(&mut self) {
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.ending.retail_frame = retail_frame;
        self.state.ending.phase =
            if retail_frame >= astropolis_assault::ENDING_END_SCREEN_SAMPLE_RETAIL_FRAME {
                EndingPhase::EndScreen
            } else if retail_frame >= astropolis_assault::ENDING_CREDITS_SAMPLE_RETAIL_FRAME {
                EndingPhase::Credits
            } else {
                EndingPhase::EscapeFlash
            };
    }

    fn update_mirage_dragon(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= mirage_dragon::RETURN_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= mirage_dragon::MAP_READY_RETAIL_FRAME =>
            {
                self.finish_sortie();
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        if retail_frame > MISSION_PLAYER_INPUT_START_RETAIL_FRAME {
            self.update_active_flight(
                retail_frame,
                self.state.mission.phase == MissionPhase::Active,
            )?;
        } else {
            self.update_mirage_dragon_presentation(retail_frame);
        }
        self.update_mirage_dragon_actor(retail_frame);
        self.update_mirage_dragon_segments(retail_frame);
        Ok(())
    }

    fn update_eladard_base(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= ELADARD_RETURN_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= ELADARD_MAP_READY_RETAIL_FRAME =>
            {
                self.finish_sortie();
                return Ok(());
            }
            _ => {}
        }

        let next_phase = if retail_frame >= ELADARD_RETURN_RETAIL_FRAME {
            EladardPhase::ReturnFlight
        } else if retail_frame >= ELADARD_GENERATOR_DESTROYED_RETAIL_FRAME {
            EladardPhase::BaseDestruction
        } else if retail_frame >= ELADARD_GENERATOR_RETAIL_FRAME {
            EladardPhase::Generator
        } else if retail_frame >= ELADARD_WALL_SPIDER_RETAIL_FRAME {
            EladardPhase::WallSpider
        } else if retail_frame >= ELADARD_PLATFORM_SWITCH_RETAIL_FRAME {
            EladardPhase::PlatformSwitch
        } else if retail_frame >= ELADARD_WALKER_TRANSFORMATION_RETAIL_FRAME {
            EladardPhase::WalkerTransformation
        } else if retail_frame >= ELADARD_BASE_ENTRANCE_RETAIL_FRAME {
            EladardPhase::BaseEntrance
        } else if retail_frame >= MISSION_ACTIVE_RETAIL_FRAMES as u16 {
            EladardPhase::SurfaceBarriers
        } else {
            EladardPhase::SurfaceApproach
        };
        let previous_eladard_phase = self.state.mission.eladard.phase;
        if previous_eladard_phase != next_phase {
            self.state.mission.eladard.phase = next_phase;
            self.state.mission.eladard.phase_started_retail_frame = retail_frame;
            match next_phase {
                EladardPhase::WalkerTransformation
                    if self.state.mission.player_craft_form == PlayerCraftForm::Flight =>
                {
                    self.begin_player_transformation(PlayerCraftTransformationDirection::ToWalker);
                }
                EladardPhase::ReturnFlight
                    if self.state.mission.player_craft_form == PlayerCraftForm::Walker =>
                {
                    self.begin_player_transformation(PlayerCraftTransformationDirection::ToFlight);
                }
                EladardPhase::SurfaceApproach
                | EladardPhase::SurfaceBarriers
                | EladardPhase::BaseEntrance
                | EladardPhase::WalkerTransformation
                | EladardPhase::PlatformSwitch
                | EladardPhase::WallSpider
                | EladardPhase::Generator
                | EladardPhase::BaseDestruction
                | EladardPhase::ReturnFlight => {}
            }
        }

        {
            let eladard = &mut self.state.mission.eladard;
            eladard.surface_barriers_remaining =
                if retail_frame < ELADARD_SURFACE_BARRIERS_RETAIL_FRAME {
                    ELADARD_SURFACE_BARRIER_COUNT
                } else {
                    0
                };
            eladard.platform_switch_pressed = retail_frame >= ELADARD_WALL_SPIDER_RETAIL_FRAME;
            eladard.wall_spider_hit_points = if retail_frame < ELADARD_GENERATOR_RETAIL_FRAME {
                ELADARD_WALL_SPIDER_HEALTH
            } else {
                0
            };
            eladard.generator_active = retail_frame >= ELADARD_GENERATOR_RETAIL_FRAME
                && retail_frame < ELADARD_GENERATOR_DESTROYED_RETAIL_FRAME;
            eladard.generator_hit_points = if retail_frame < ELADARD_GENERATOR_RETAIL_FRAME {
                ELADARD_GENERATOR_HEALTH
            } else if retail_frame >= ELADARD_GENERATOR_DESTROYED_RETAIL_FRAME {
                0
            } else {
                let remaining = u32::from(
                    ELADARD_GENERATOR_DESTROYED_RETAIL_FRAME.saturating_sub(retail_frame),
                );
                let duration = u32::from(
                    ELADARD_GENERATOR_DESTROYED_RETAIL_FRAME - ELADARD_GENERATOR_RETAIL_FRAME,
                );
                ((remaining * u32::from(ELADARD_GENERATOR_HEALTH)).div_ceil(duration)) as u8
            };
        }
        self.update_eladard_player_presentation(retail_frame);
        if self.state.mission.phase == MissionPhase::Active {
            self.update_active_flight(retail_frame, true)?;
        }
        Ok(())
    }

    fn update_eladard_player_presentation(&mut self, retail_frame: u16) {
        let visible = retail_frame >= MISSION_STAGE_LOAD_RETAIL_FRAMES as u16;
        if let Some(primary) = self
            .state
            .mission
            .primary_player
            .and_then(|id| self.state.objects.get_mut(id))
        {
            primary.base.flags.visible = visible;
            primary.base.flags.collision_disabled = true;
        }
    }

    fn update_titania_base(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= TITANIA_RETURN_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= TITANIA_MAP_READY_RETAIL_FRAME =>
            {
                self.finish_sortie();
                return Ok(());
            }
            MissionPhase::Loading
            | MissionPhase::EntryCinematic
            | MissionPhase::Active
            | MissionPhase::ReturningToStrategicMap => {}
        }

        let next_phase = if retail_frame >= TITANIA_RETURN_RETAIL_FRAME {
            TitaniaPhase::ReturnFlight
        } else if retail_frame >= TITANIA_REACTOR_RETAIL_FRAME {
            TitaniaPhase::Reactor
        } else if retail_frame >= TITANIA_INTERIOR_RETAIL_FRAME {
            TitaniaPhase::Interior
        } else if retail_frame >= TITANIA_BASE_ENTRY_RETAIL_FRAME {
            TitaniaPhase::BaseEntry
        } else if retail_frame >= MISSION_ACTIVE_RETAIL_FRAMES as u16 {
            TitaniaPhase::SurfaceSwitches
        } else {
            TitaniaPhase::SurfaceApproach
        };
        {
            let titania = &mut self.state.mission.titania;
            if titania.phase != next_phase {
                titania.phase = next_phase;
                titania.phase_started_retail_frame = retail_frame;
            }
            if retail_frame >= TITANIA_BASE_ENTRY_RETAIL_FRAME {
                titania.surface_switches =
                    [TitaniaSurfaceSwitchStatus::Disabled; TITANIA_SURFACE_SWITCH_COUNT];
            }
            titania.reactor = if retail_frame >= TITANIA_RETURN_RETAIL_FRAME {
                TitaniaReactorStatus::Destroyed
            } else if retail_frame >= TITANIA_REACTOR_RETAIL_FRAME {
                TitaniaReactorStatus::Exposed
            } else {
                TitaniaReactorStatus::Shielded
            };
        }
        if let Some(primary) = self
            .state
            .mission
            .primary_player
            .and_then(|id| self.state.objects.get_mut(id))
        {
            primary.base.flags.visible = retail_frame >= MISSION_STAGE_LOAD_RETAIL_FRAMES as u16;
            primary.base.flags.collision_disabled = true;
        }
        if self.state.mission.phase == MissionPhase::Active {
            self.update_active_flight(retail_frame, true)?;
        }
        Ok(())
    }

    fn update_carrier_assault(&mut self) -> Result<(), Error> {
        self.state.mission.active = true;
        let retail_frame = self
            .state
            .mode_frame
            .saturating_mul(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        self.state.mission.elapsed_time_tenths = mission_elapsed_time_tenths(retail_frame);

        match self.state.mission.phase {
            MissionPhase::Loading if self.state.mode_frame >= MISSION_STAGE_LOAD_TICKS => {
                self.state.mission.phase = MissionPhase::EntryCinematic;
            }
            MissionPhase::EntryCinematic if self.state.mode_frame >= MISSION_ACTIVE_TICKS => {
                self.state.mission.phase = MissionPhase::Active;
            }
            MissionPhase::Active if retail_frame >= CARRIER_RETURN_FLIGHT_RETAIL_FRAME => {
                self.state.mission.phase = MissionPhase::ReturningToStrategicMap;
            }
            MissionPhase::ReturningToStrategicMap
                if retail_frame >= CARRIER_MAP_READY_RETAIL_FRAME =>
            {
                self.finish_sortie();
                return Ok(());
            }
            _ => {}
        }

        let next_phase = if retail_frame >= CARRIER_RETURN_FLIGHT_RETAIL_FRAME {
            CarrierAssaultPhase::ReturnFlight
        } else if retail_frame >= CARRIER_CORE_DESTROYED_RETAIL_FRAME {
            CarrierAssaultPhase::CoreDestruction
        } else if retail_frame >= CARRIER_REACTOR_OPEN_RETAIL_FRAME {
            CarrierAssaultPhase::ReactorCombat
        } else if retail_frame >= CARRIER_REACTOR_APPROACH_RETAIL_FRAME {
            CarrierAssaultPhase::ReactorApproach
        } else if retail_frame >= CARRIER_EXTERIOR_END_RETAIL_FRAME {
            CarrierAssaultPhase::InteriorCorridor
        } else {
            CarrierAssaultPhase::ExteriorApproach
        };
        let previous_phase = self.state.mission.carrier_assault.phase;
        if previous_phase != next_phase {
            match next_phase {
                CarrierAssaultPhase::InteriorCorridor => self.spawn_carrier_corridor_scene()?,
                CarrierAssaultPhase::ReactorCombat => {
                    self.spawn_carrier_reactor_scene()?;
                    self.state.mission.player_craft_form = PlayerCraftForm::Walker;
                }
                CarrierAssaultPhase::ReturnFlight => {
                    self.state.mission.player_craft_form = PlayerCraftForm::Flight;
                }
                CarrierAssaultPhase::ExteriorApproach
                | CarrierAssaultPhase::ReactorApproach
                | CarrierAssaultPhase::CoreDestruction => {}
            }
            self.state.mission.carrier_assault.phase = next_phase;
            self.state
                .mission
                .carrier_assault
                .phase_started_retail_frame = retail_frame;
        }

        let corridor_progress = if retail_frame <= CARRIER_EXTERIOR_END_RETAIL_FRAME {
            0
        } else if retail_frame >= CARRIER_REACTOR_OPEN_RETAIL_FRAME {
            CARRIER_CORRIDOR_LENGTH
        } else {
            let elapsed = u32::from(retail_frame.saturating_sub(CARRIER_EXTERIOR_END_RETAIL_FRAME));
            let duration =
                u32::from(CARRIER_REACTOR_OPEN_RETAIL_FRAME - CARRIER_EXTERIOR_END_RETAIL_FRAME);
            ((elapsed * u32::from(CARRIER_CORRIDOR_LENGTH)) / duration) as u16
        };
        let carrier = &mut self.state.mission.carrier_assault;
        carrier.corridor_progress = corridor_progress;
        carrier.reactor_room_open = retail_frame >= CARRIER_REACTOR_OPEN_RETAIL_FRAME;
        carrier.starboard_panel.integrity = carrier_starboard_panel_integrity(retail_frame);
        carrier.starboard_panel.active = retail_frame < CARRIER_STARBOARD_DESTROYED_RETAIL_FRAME;
        carrier.port_panel.integrity = carrier_port_panel_integrity(retail_frame);
        carrier.port_panel.active = retail_frame < CARRIER_PORT_DESTROYED_RETAIL_FRAME;
        let starboard_panel_destroyed = !carrier.starboard_panel.active;
        let port_panel_destroyed = !carrier.port_panel.active;

        if starboard_panel_destroyed {
            self.set_carrier_panel_destroyed(CARRIER_STARBOARD_PANEL_INDEX);
        }
        if port_panel_destroyed {
            self.set_carrier_panel_destroyed(CARRIER_PORT_PANEL_INDEX);
        }
        self.update_carrier_player_presentation(retail_frame);
        Ok(())
    }

    fn update_carrier_player_presentation(&mut self, retail_frame: u16) {
        let walker_shape = self.primary_walker_shape();
        let (position, shape, camera) = if retail_frame < CARRIER_EXTERIOR_END_RETAIL_FRAME {
            let x = interpolate_i16(
                CARRIER_EXTERIOR_START_POSITION.x,
                CARRIER_EXTERIOR_ENTRY_POSITION.x,
                retail_frame,
                CARRIER_EXTERIOR_END_RETAIL_FRAME,
            );
            let z = interpolate_i16(
                CARRIER_EXTERIOR_START_POSITION.z,
                CARRIER_EXTERIOR_ENTRY_POSITION.z,
                retail_frame,
                CARRIER_EXTERIOR_END_RETAIL_FRAME,
            );
            let position = Vector3 { x, y: 0, z };
            (
                position,
                ShapeId::CARRIER_ASSAULT_CRAFT,
                Vector3 {
                    x,
                    y: CARRIER_EXTERIOR_CAMERA_HEIGHT,
                    z,
                },
            )
        } else if retail_frame < CARRIER_REACTOR_OPEN_RETAIL_FRAME {
            let progress = self.state.mission.carrier_assault.corridor_progress as i16;
            let position = Vector3 {
                x: CARRIER_CORRIDOR_START_POSITION.x,
                y: CARRIER_CORRIDOR_START_POSITION.y,
                z: CARRIER_CORRIDOR_START_POSITION.z.saturating_add(progress),
            };
            (
                position,
                ShapeId::CARRIER_ASSAULT_CRAFT,
                Vector3 {
                    x: position.x,
                    y: CARRIER_CORRIDOR_CAMERA_HEIGHT,
                    z: position
                        .z
                        .saturating_add(CARRIER_CORRIDOR_CAMERA_FORWARD_OFFSET),
                },
            )
        } else {
            (
                CARRIER_REACTOR_PLAYER_POSITION,
                walker_shape,
                CARRIER_REACTOR_CAMERA_POSITION,
            )
        };
        let visible = retail_frame >= MISSION_STAGE_LOAD_RETAIL_FRAMES as u16;
        if let Some(primary) = self
            .state
            .mission
            .primary_player
            .and_then(|id| self.state.objects.get_mut(id))
        {
            primary.base.position = position;
            primary.base.shape = shape;
            primary.base.flags.visible = visible;
            primary.base.flags.collision_disabled = true;
            primary.base.velocity = Vector3::default();
        }
        self.state.camera.position = camera;
    }

    fn clear_carrier_scene(&mut self) {
        for object in self.carrier_scenery.drain(..) {
            self.state.objects.remove(object);
        }
        for panel in &mut self.carrier_panels {
            if let Some(object) = panel.take() {
                self.state.objects.remove(object);
            }
        }
    }

    fn spawn_carrier_scene_objects(
        &mut self,
        scene: &[(ShapeId, Vector3, Angle)],
    ) -> Result<(), Error> {
        for (shape, position, yaw) in scene.iter().copied() {
            let mut object = Object::new(ObjectKind::Scenery, shape, Behavior::Effect);
            object.base.position = position;
            object.base.yaw = yaw;
            object.base.flags.collision_disabled = true;
            object.base.flags.casts_shadow = false;
            let id = self
                .state
                .objects
                .allocate(object)
                .ok_or(Error::ObjectCapacityReached)?;
            self.carrier_scenery.push(id);
        }
        Ok(())
    }

    fn spawn_carrier_exterior_scene(&mut self) -> Result<(), Error> {
        self.clear_carrier_scene();
        self.spawn_carrier_scene_objects(&CARRIER_EXTERIOR_SCENE)
    }

    fn spawn_carrier_corridor_scene(&mut self) -> Result<(), Error> {
        self.clear_carrier_scene();
        self.spawn_carrier_scene_objects(&CARRIER_CORRIDOR_SCENE)
    }

    fn spawn_carrier_reactor_scene(&mut self) -> Result<(), Error> {
        self.clear_carrier_scene();
        self.spawn_carrier_scene_objects(&CARRIER_REACTOR_SCENE)?;
        for (index, (position, yaw)) in CARRIER_PANEL_SCENE.into_iter().enumerate() {
            let mut panel = Object::new(
                ObjectKind::Scenery,
                ShapeId::CARRIER_REACTOR_PANEL,
                Behavior::Effect,
            );
            panel.base.position = position;
            panel.base.yaw = yaw;
            panel.base.flags.collision_disabled = true;
            panel.base.flags.casts_shadow = false;
            let id = self
                .state
                .objects
                .allocate(panel)
                .ok_or(Error::ObjectCapacityReached)?;
            self.carrier_panels[index] = Some(id);
        }
        Ok(())
    }

    fn set_carrier_panel_destroyed(&mut self, index: usize) {
        let Some(panel) = self.carrier_panels[index] else {
            return;
        };
        if let Some(object) = self.state.objects.get_mut(panel) {
            object.base.shape = ShapeId::CARRIER_REACTOR_PANEL_DESTROYED;
        }
    }

    fn clear_sortie_runtime(&mut self) {
        self.clear_carrier_scene();
        self.previous_mission_player_position = None;
        for projectile in self.mission_projectiles.drain(..) {
            self.state.objects.remove(projectile.object);
        }
        for projectile in self.reengagement_projectiles.drain(..) {
            self.state.objects.remove(projectile.object);
        }
        for projectile in self.fighter_intercept_projectiles.drain(..) {
            self.state.objects.remove(projectile.object);
        }
        for projectile in self.pigma_projectiles.drain(..) {
            self.state.objects.remove(projectile.object);
        }
        for projectile in self.leon_projectiles.drain(..) {
            self.state.objects.remove(projectile.object);
        }
        for projectile in self.pressure_fighter_projectiles.drain(..) {
            self.state.objects.remove(projectile.object);
        }
        for projectile in self.leon_pressure_projectiles.drain(..) {
            self.state.objects.remove(projectile.object);
        }
        for projectile in self.final_rival_projectiles.drain(..) {
            self.state.objects.remove(projectile.object);
        }
        for missile in &mut self.interception_missiles {
            if let Some(object) = missile.take() {
                self.state.objects.remove(object);
            }
        }
        for actor in &mut self.mission_entry_flyby {
            if let Some(object) = actor.take() {
                self.state.objects.remove(object);
            }
        }
        self.fighter_intercept_actors
            .remove_all(&mut self.state.objects);
        self.pressure_fighter_actors
            .remove_all(&mut self.state.objects);
        if let Some(rival) = self.pigma_rival.take() {
            self.state.objects.remove(rival);
        }
        if let Some(rival) = self.leon_rival.take() {
            self.state.objects.remove(rival);
        }
        if let Some(rival) = self.final_rival.take() {
            self.state.objects.remove(rival);
        }
        if let Some(boss) = self.mirage_dragon.take() {
            self.state.objects.remove(boss);
        }
        for segment in &mut self.mirage_dragon_body {
            if let Some(object) = segment.take() {
                self.state.objects.remove(object);
            }
        }
        if let Some(tail) = self.mirage_dragon_tail.take() {
            self.state.objects.remove(tail);
        }
        let player_projectiles: Vec<_> = self
            .state
            .objects
            .active_objects()
            .filter_map(|(id, object)| (object.base.behavior == Behavior::Projectile).then_some(id))
            .collect();
        for projectile in player_projectiles {
            self.state.objects.remove(projectile);
        }
        for player in [
            self.state.mission.primary_player,
            self.state.mission.wingmate,
        ]
        .into_iter()
        .flatten()
        {
            if let Some(object) = self.state.objects.get_mut(player) {
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
                object.base.velocity = Vector3::default();
            }
        }
        self.state.mission.active = false;
        self.state.mission.mission = None;
        self.state.mission.player_blaster = PlayerBlasterState::Ready;
    }

    fn finish_sortie(&mut self) {
        let completed_visit = self.state.mission.visit;
        self.clear_sortie_runtime();
        match completed_visit {
            MissionVisit::MissileInterception => {
                self.state.campaign.objectives.missiles = StrategicThreatCount::NONE;
            }
            MissionVisit::EladardBase => {
                self.state.campaign.objectives.eladard = PlanetObjectiveStatus::Rescued;
            }
            MissionVisit::TitaniaBase => {
                self.state.campaign.objectives.titania = PlanetObjectiveStatus::Rescued;
            }
            MissionVisit::FirstBattleCarrier => {
                self.state.campaign.objectives.first_carrier = CarrierObjectiveStatus::Destroyed;
            }
            MissionVisit::SecondBattleCarrier => {
                self.state.campaign.objectives.second_carrier = CarrierObjectiveStatus::Destroyed;
                self.state.mission.objects_destroyed = self
                    .state
                    .mission
                    .objects_destroyed
                    .saturating_add(CARRIER_REACTOR_PANEL_COUNT);
            }
            MissionVisit::OpeningEngagement
            | MissionVisit::Reengagement
            | MissionVisit::FighterIntercept
            | MissionVisit::PigmaDuel
            | MissionVisit::LeonDuel
            | MissionVisit::MirageDragon
            | MissionVisit::RecurringAttackers
            | MissionVisit::LeonPressure
            | MissionVisit::FinalPursuer
            | MissionVisit::WolfBlockade
            | MissionVisit::AstropolisAssault => {}
        }
        self.state.campaign.route_step = self.state.campaign.route_step.after_completion();
        self.state.campaign.objectives.refresh_final_access();
        match self.state.campaign.route_step {
            CampaignRouteStep::Reengagement => {
                self.state.campaign.elapsed_frames =
                    FIRST_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
                self.set_campaign_craft_shields(
                    FIRST_RETURN_PRIMARY_SHIELD,
                    FIRST_RETURN_WINGMATE_SHIELD,
                );
                self.state.strategic_map.actors = OPENING_ASSAULT_MAP_ACTORS;
                self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
                self.state.strategic_map.recommended_destination = FIRST_REENGAGEMENT_DESTINATION;
            }
            CampaignRouteStep::MissileInterception => {
                self.state.campaign.elapsed_frames =
                    SECOND_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
                self.state.campaign.corneria_damage_percent = SECOND_RETURN_CORNERIA_DAMAGE_PERCENT;
                self.set_campaign_craft_shields(
                    SECOND_RETURN_PRIMARY_SHIELD,
                    SECOND_RETURN_WINGMATE_SHIELD,
                );
                self.state.strategic_map.actors = ESCALATED_ASSAULT_MAP_ACTORS;
                self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
                self.state.strategic_map.recommended_destination = MISSILE_INTERCEPTION_DESTINATION;
            }
            CampaignRouteStep::FighterIntercept => {
                self.state.campaign.elapsed_frames =
                    MISSILE_INTERCEPTION_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
                self.state.campaign.corneria_damage_percent =
                    MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT;
                self.state.strategic_map.actors = POST_INTERCEPTION_MAP_ACTORS;
                self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
                self.state.strategic_map.recommended_destination = FIGHTER_INTERCEPT_DESTINATION;
            }
            CampaignRouteStep::PigmaDuel => {
                self.state.campaign.elapsed_frames =
                    FIGHTER_INTERCEPT_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
                self.state.campaign.corneria_damage_percent =
                    MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT;
                self.state.strategic_map.actors = POST_FIGHTER_INTERCEPT_MAP_ACTORS;
                self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
                self.state.strategic_map.recommended_destination = PIGMA_DUEL_DESTINATION;
            }
            CampaignRouteStep::EladardBase => {
                self.state.campaign.elapsed_frames =
                    PIGMA_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
                self.state.campaign.corneria_damage_percent =
                    MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT;
                self.state.mission.item_count = 2;
                self.state.strategic_map.actors = POST_PIGMA_MAP_ACTORS;
                self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
                self.state.strategic_map.recommended_destination = ELADARD_BASE_DESTINATION;
            }
            CampaignRouteStep::FirstBattleCarrier => {
                self.state.campaign.elapsed_frames =
                    ELADARD_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
                self.state.campaign.corneria_damage_percent =
                    ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT;
                self.state.mission.item_count = ELADARD_RETURN_ITEM_COUNT;
                self.state.mission.score = ELADARD_RETURN_SCORE;
                self.set_campaign_craft_shields(ELADARD_RETURN_SHIELD, ELADARD_RETURN_SHIELD);
                self.state.strategic_map.actors = POST_ELADARD_MAP_ACTORS;
                self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
                self.state.strategic_map.recommended_destination =
                    POST_ELADARD_RECOMMENDED_DESTINATION;
            }
            CampaignRouteStep::LeonDuel => {
                self.state.campaign.elapsed_frames =
                    CARRIER_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
                self.state.campaign.corneria_damage_percent =
                    ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT;
                self.state.mission.item_count = CARRIER_RETURN_ITEM_COUNT;
                self.state.mission.score = CARRIER_RETURN_SCORE;
                self.state.mission.objects_destroyed = self
                    .state
                    .mission
                    .objects_destroyed
                    .saturating_add(CARRIER_REACTOR_PANEL_COUNT);
                self.set_campaign_craft_shields(
                    CARRIER_RETURN_PRIMARY_SHIELD,
                    CARRIER_RETURN_WINGMATE_SHIELD,
                );
                self.state.strategic_map.actors = POST_CARRIER_MAP_ACTORS;
                self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
                self.state.strategic_map.recommended_destination = LEON_DUEL_DESTINATION;
            }
            CampaignRouteStep::MirageDragon => {
                self.state.campaign.elapsed_frames =
                    LEON_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
                self.state.campaign.corneria_damage_percent =
                    ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT;
                self.state.mission.item_count = LEON_RETURN_ITEM_COUNT;
                self.state.mission.score = LEON_RETURN_SCORE;
                self.set_campaign_craft_shields(LEON_RETURN_SHIELD, LEON_RETURN_SHIELD);
                self.state.campaign.corneria_defense = CorneriaDefenseState::post_leon();
                self.state.strategic_map.actors = POST_LEON_MAP_ACTORS;
                self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
                self.state.strategic_map.recommended_destination = MIRAGE_DRAGON_DESTINATION;
            }
            CampaignRouteStep::StrategicPressure => {
                if completed_visit == MissionVisit::MirageDragon {
                    self.state.campaign.elapsed_frames =
                        MIRAGE_DRAGON_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
                    self.state.mission.item_count = MIRAGE_DRAGON_RETURN_ITEM_COUNT;
                    self.state.mission.score = MIRAGE_DRAGON_RETURN_SCORE;
                    self.set_campaign_craft_shields(
                        MIRAGE_DRAGON_RETURN_SHIELD,
                        MIRAGE_DRAGON_RETURN_SHIELD,
                    );
                    self.state.campaign.objectives.live_attackers = StrategicThreatCount::new(1);
                }
                self.state.strategic_map.actors = POST_MIRAGE_MAP_ACTORS;
                self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
                self.state.strategic_map.recommended_destination =
                    if self.state.campaign.objectives.titania == PlanetObjectiveStatus::Occupied {
                        TITANIA_BASE_DESTINATION
                    } else if self.state.campaign.objectives.second_carrier
                        == CarrierObjectiveStatus::Operational
                    {
                        SECOND_BATTLE_CARRIER_DESTINATION
                    } else if self.state.campaign.objectives.live_attackers.remaining() == 1 {
                        FINAL_PURSUER_DESTINATION
                    } else {
                        INITIAL_PLAYER_MAP_POSITION
                    };
            }
            CampaignRouteStep::WolfBlockade => {
                self.state.strategic_map.actors = POST_MIRAGE_MAP_ACTORS;
                self.state.strategic_map.recommended_destination = WOLF_BLOCKADE_DESTINATION;
            }
            CampaignRouteStep::AstropolisAssault => {
                self.state.strategic_map.actors = POST_MIRAGE_MAP_ACTORS;
                self.state.strategic_map.recommended_destination = ASTROPOLIS_DESTINATION;
            }
            CampaignRouteStep::OpeningEngagement => {}
        }
        let recommended_encounter = match self.state.campaign.route_step {
            CampaignRouteStep::StrategicPressure
                if self.state.campaign.objectives.titania == PlanetObjectiveStatus::Occupied =>
            {
                Some(StrategicEncounter::TitaniaBase)
            }
            CampaignRouteStep::StrategicPressure
                if self.state.campaign.objectives.second_carrier
                    == CarrierObjectiveStatus::Operational =>
            {
                Some(StrategicEncounter::SecondBattleCarrier)
            }
            CampaignRouteStep::StrategicPressure
                if self.state.campaign.objectives.major_objectives_complete()
                    && self.state.campaign.objectives.live_attackers.remaining() == 1 =>
            {
                Some(StrategicEncounter::FinalPursuer)
            }
            CampaignRouteStep::OpeningEngagement
            | CampaignRouteStep::Reengagement
            | CampaignRouteStep::MissileInterception
            | CampaignRouteStep::FighterIntercept
            | CampaignRouteStep::PigmaDuel
            | CampaignRouteStep::EladardBase
            | CampaignRouteStep::FirstBattleCarrier
            | CampaignRouteStep::LeonDuel
            | CampaignRouteStep::MirageDragon
            | CampaignRouteStep::StrategicPressure
            | CampaignRouteStep::WolfBlockade
            | CampaignRouteStep::AstropolisAssault => None,
        };
        self.state.strategic_map.selected_encounter = recommended_encounter;
        self.state.strategic_map.destination = if recommended_encounter.is_some() {
            self.state.strategic_map.recommended_destination
        } else {
            self.state.strategic_map.player_map_position
        };
        self.state.strategic_map.travel_ticks_remaining = 0;
        self.state.strategic_map.travel_total_ticks = 0;
        self.state.strategic_map.phase = StrategicMapPhase::Planning;
        self.state.camera = Camera::default();
        self.enter_mode(GameMode::StrategicMap);
    }

    fn finish_pressure_encounter(&mut self, elapsed_display_seconds: u64) {
        self.clear_sortie_runtime();
        self.state.campaign.elapsed_frames = self.state.campaign.elapsed_frames.saturating_add(
            elapsed_display_seconds.saturating_mul(CAMPAIGN_TICKS_PER_DISPLAY_SECOND),
        );
        self.state.strategic_map.actors = POST_MIRAGE_MAP_ACTORS;
        self.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        self.state.strategic_map.destination = INITIAL_PLAYER_MAP_POSITION;
        self.state.strategic_map.recommended_destination = INITIAL_PLAYER_MAP_POSITION;
        self.state.strategic_map.selected_encounter = None;
        self.state.strategic_map.travel_ticks_remaining = 0;
        self.state.strategic_map.travel_total_ticks = 0;
        self.state.strategic_map.phase = StrategicMapPhase::Planning;
        self.state.camera = Camera::default();
        self.enter_mode(GameMode::StrategicMap);
    }

    fn finish_final_rival_encounter(&mut self) {
        let completed_visit = self.state.mission.visit;
        self.finish_pressure_encounter(0);
        match completed_visit {
            MissionVisit::FinalPursuer => {
                self.state.campaign.objectives.live_attackers.remove_one();
                self.state.campaign.objectives.refresh_final_access();
                if self.state.campaign.objectives.wolf_blockade == WolfBlockadeStatus::Active {
                    self.state.campaign.route_step = CampaignRouteStep::WolfBlockade;
                    self.state.strategic_map.destination = WOLF_BLOCKADE_DESTINATION;
                    self.state.strategic_map.recommended_destination = WOLF_BLOCKADE_DESTINATION;
                }
            }
            MissionVisit::WolfBlockade => {
                self.state.campaign.objectives.record_wolf_defeated();
                if self.state.campaign.objectives.astropolis == AstropolisStatus::Vulnerable {
                    self.state.campaign.route_step = CampaignRouteStep::AstropolisAssault;
                    self.state.strategic_map.destination = ASTROPOLIS_DESTINATION;
                    self.state.strategic_map.recommended_destination = ASTROPOLIS_DESTINATION;
                }
            }
            _ => unreachable!("final rival completion requires a final rival visit"),
        }
    }

    fn set_campaign_craft_shields(&mut self, primary_shield: u8, wingmate_shield: u8) {
        for (craft, shield) in [
            (self.state.mission.primary_player, primary_shield),
            (self.state.mission.wingmate, wingmate_shield),
        ] {
            if let Some(object) = craft.and_then(|id| self.state.objects.get_mut(id)) {
                object.base.hit_points = shield;
            }
        }
    }

    fn spawn_mission_entry_flyby(&mut self) -> Result<(), Error> {
        for (index, craft) in MISSION_ENTRY_CRAFTS.into_iter().enumerate() {
            let mut object = Object::new(
                ObjectKind::Scenery,
                craft.shape,
                Behavior::MissionEntryFlyby,
            );
            object.base.position = MISSION_FORMATION_KEYFRAMES[0].positions[index];
            object.base.yaw = Angle::from_units(craft.yaw);
            object.base.hit_points = MISSION_ENCOUNTER_HEALTH;
            object.base.attack_power = MISSION_ENCOUNTER_ATTACK_POWER;
            object.base.flags.casts_shadow = false;
            let Some(id) = self.state.objects.allocate(object) else {
                for allocated_id in self.mission_entry_flyby.iter().flatten().copied() {
                    self.state.objects.remove(allocated_id);
                }
                self.mission_entry_flyby = [None; MISSION_ENCOUNTER_ACTOR_COUNT];
                return Err(Error::ObjectCapacityReached);
            };
            self.mission_entry_flyby[index] = Some(id);
        }
        Ok(())
    }

    fn spawn_reengagement_targets(&mut self) -> Result<(), Error> {
        for (index, craft) in MISSION_ENTRY_CRAFTS.into_iter().enumerate() {
            let mut object = Object::new(ObjectKind::Scenery, craft.shape, Behavior::EnemyFlight);
            object.base.hit_points = MISSION_ENCOUNTER_HEALTH;
            object.base.attack_power = MISSION_ENCOUNTER_ATTACK_POWER;
            object.base.flags.active = false;
            object.base.flags.visible = false;
            object.base.flags.collision_disabled = true;
            object.base.flags.casts_shadow = false;
            let Some(id) = self.state.objects.allocate(object) else {
                for allocated_id in self.mission_entry_flyby.iter().flatten().copied() {
                    self.state.objects.remove(allocated_id);
                }
                self.mission_entry_flyby = [None; MISSION_ENCOUNTER_ACTOR_COUNT];
                return Err(Error::ObjectCapacityReached);
            };
            self.mission_entry_flyby[index] = Some(id);
        }
        Ok(())
    }

    fn spawn_interception_missiles(&mut self) -> Result<(), Error> {
        for (index, slot) in self.interception_missiles.iter_mut().enumerate() {
            let pose = missile_interception_targets::INITIAL_POSES[index];
            let mut missile = Object::new(
                ObjectKind::Enemy,
                ShapeId::CAMPAIGN_MISSILE,
                Behavior::EnemyFlight,
            );
            missile.base.hit_points = MISSION_ENCOUNTER_HEALTH;
            missile.base.flags.active = false;
            missile.base.flags.visible = false;
            missile.base.flags.collision_disabled = true;
            missile.base.flags.casts_shadow = false;
            missile.base.position = pose.position;
            missile.base.pitch = Angle::from_units(pose.pitch);
            missile.base.yaw = Angle::from_units(pose.yaw);
            missile.base.roll = Angle::from_units(pose.roll);
            missile.base.speed = pose.speed;
            missile.extension.activity =
                ObjectActivity::InterceptionMissileFlight(InterceptionMissileFlightState {
                    last_steering_adjustment: InterceptionMissileSteering::Straight,
                });
            let Some(id) = self.state.objects.allocate(missile) else {
                for allocated in &mut self.interception_missiles {
                    if let Some(object) = allocated.take() {
                        self.state.objects.remove(object);
                    }
                }
                return Err(Error::ObjectCapacityReached);
            };
            *slot = Some(id);
        }
        Ok(())
    }

    fn spawn_fighter_intercept_targets(&mut self) -> Result<(), Error> {
        for actor in FighterInterceptActor::ALL {
            let actor_index = actor.index();
            let pose = fighter_intercept_fighters::INITIAL_POSES[actor_index];
            let mut fighter = Object::new(
                ObjectKind::Enemy,
                ShapeId::INTERCEPT_FIGHTER,
                Behavior::EnemyFlight,
            );
            fighter.base.hit_points = MISSION_ENCOUNTER_HEALTH;
            fighter.base.attack_power = MISSION_ENCOUNTER_ATTACK_POWER;
            fighter.base.collision_class = CollisionClass::Enemy;
            fighter.base.flags.active = true;
            fighter.base.flags.visible = true;
            fighter.base.flags.collision_disabled = false;
            fighter.base.flags.casts_shadow = false;
            fighter.base.position = pose.position;
            fighter.base.pitch = Angle::from_units(pose.pitch);
            fighter.base.yaw = Angle::from_units(pose.yaw);
            fighter.base.roll = Angle::from_units(pose.roll);
            fighter.base.speed = pose.speed;
            fighter.extension.activity =
                ObjectActivity::FighterInterceptFlight(FighterInterceptFlightState {
                    vertical_wave_phase: Angle::from_units(
                        fighter_intercept_fighters::INITIAL_WAVE_PHASES[actor_index],
                    ),
                    cruise_target_speed: 0,
                    cruise_acceleration: 0,
                    corridor_drift_x: 0,
                    corridor_altitude: 0,
                    corridor_drift_z: 0,
                    pending_velocity: Vector3::default(),
                    movement_phase: FighterInterceptMovementPhase::Ready,
                    weapon_phase: FighterInterceptWeaponPhase::Flight,
                });
            let Some(id) = self.state.objects.allocate(fighter) else {
                self.fighter_intercept_actors
                    .remove_all(&mut self.state.objects);
                return Err(Error::ObjectCapacityReached);
            };
            *self.fighter_intercept_actors.slot_mut(actor) = Some(id);
        }
        Ok(())
    }

    fn spawn_pressure_fighters(&mut self) -> Result<(), Error> {
        self.pressure_fighter_actors
            .remove_all(&mut self.state.objects);
        for fighter_kind in PressureFighter::ALL {
            let mut fighter = Object::new(
                ObjectKind::Enemy,
                fighter_kind.shape(),
                Behavior::EnemyFlight,
            );
            fighter.base.hit_points = PRESSURE_FIGHTER_HEALTH;
            fighter.base.attack_power = PRESSURE_FIGHTER_ATTACK_POWER;
            fighter.base.collision_class = CollisionClass::Enemy;
            fighter.base.flags.active = false;
            fighter.base.flags.visible = false;
            fighter.base.flags.collision_disabled = true;
            fighter.base.flags.casts_shadow = false;
            let Some(id) = self.state.objects.allocate(fighter) else {
                self.pressure_fighter_actors
                    .remove_all(&mut self.state.objects);
                return Err(Error::ObjectCapacityReached);
            };
            *self.pressure_fighter_actors.slot_mut(fighter_kind) = Some(id);
        }
        Ok(())
    }

    fn spawn_pigma_rival(&mut self) -> Result<(), Error> {
        let mut rival = Object::new(
            ObjectKind::Enemy,
            ShapeId::PIGMA_CRAFT,
            Behavior::EnemyFlight,
        );
        rival.base.hit_points = PIGMA_HEALTH;
        rival.base.attack_power = PIGMA_ATTACK_POWER;
        rival.base.collision_class = CollisionClass::Enemy;
        rival.base.flags.active = false;
        rival.base.flags.visible = false;
        rival.base.flags.collision_disabled = true;
        rival.base.flags.casts_shadow = false;
        rival.extension.activity = ObjectActivity::PigmaRivalFlight(PigmaRivalFlightState {
            phase: PigmaRivalFlightPhase::AwaitingEntrance,
            target_speed: 0,
            acceleration: 0,
            motion_steps_elapsed: 0,
            second_approach_wave_step: 0,
            escape_wobble_step: 0,
            earlier_player_altitude: pigma_duel::PLAYER_KEYFRAMES[0].position.y,
        });
        self.pigma_rival = Some(
            self.state
                .objects
                .allocate(rival)
                .ok_or(Error::ObjectCapacityReached)?,
        );
        Ok(())
    }

    fn spawn_leon_rival(&mut self) -> Result<(), Error> {
        let mut rival = Object::new(
            ObjectKind::Enemy,
            ShapeId::LEON_CRAFT,
            Behavior::EnemyFlight,
        );
        rival.base.hit_points = LEON_HEALTH;
        rival.base.attack_power = LEON_ATTACK_POWER;
        rival.base.collision_class = CollisionClass::Enemy;
        rival.base.flags.active = false;
        rival.base.flags.visible = false;
        rival.base.flags.collision_disabled = true;
        rival.base.flags.casts_shadow = false;
        rival.extension.activity = ObjectActivity::LeonRivalFlight(LeonRivalFlightState {
            phase: LeonRivalFlightPhase::AwaitingEntrance,
            movement_phase: LeonRivalMovementPhase::Ready,
            target_speed: 0,
            acceleration: 0,
            motion_steps_elapsed: 0,
        });
        self.leon_rival = Some(
            self.state
                .objects
                .allocate(rival)
                .ok_or(Error::ObjectCapacityReached)?,
        );
        Ok(())
    }

    fn spawn_final_rival(&mut self, shape: ShapeId) -> Result<(), Error> {
        if let Some(previous) = self.final_rival.take() {
            self.state.objects.remove(previous);
        }
        let mut rival = Object::new(ObjectKind::Enemy, shape, Behavior::EnemyFlight);
        rival.base.hit_points = LEON_HEALTH;
        rival.base.attack_power = LEON_ATTACK_POWER;
        rival.base.collision_class = CollisionClass::Enemy;
        rival.base.flags.active = false;
        rival.base.flags.visible = false;
        rival.base.flags.collision_disabled = true;
        rival.base.flags.casts_shadow = false;
        rival.extension.activity = ObjectActivity::FinalRivalFlight(FinalRivalFlightState {
            phase: FinalRivalFlightPhase::AwaitingEntrance,
            target_speed: 0,
            acceleration: 0,
            motion_steps_elapsed: 0,
        });
        self.final_rival = Some(
            self.state
                .objects
                .allocate(rival)
                .ok_or(Error::ObjectCapacityReached)?,
        );
        Ok(())
    }

    fn spawn_mirage_dragon(&mut self) -> Result<(), Error> {
        let mut boss = Object::new(
            ObjectKind::Enemy,
            ShapeId::MIRAGE_DRAGON_HEAD,
            Behavior::EnemyFlight,
        );
        boss.base.hit_points = MIRAGE_DRAGON_HEALTH;
        boss.base.attack_power = MIRAGE_DRAGON_ATTACK_POWER;
        boss.base.collision_class = CollisionClass::Enemy;
        boss.base.flags.active = false;
        boss.base.flags.visible = false;
        boss.base.flags.collision_disabled = true;
        boss.base.flags.casts_shadow = false;
        self.mirage_dragon = Some(
            self.state
                .objects
                .allocate(boss)
                .ok_or(Error::ObjectCapacityReached)?,
        );

        for segment in &mut self.mirage_dragon_body {
            let mut body = Object::new(
                ObjectKind::Enemy,
                ShapeId::MIRAGE_DRAGON_BODY,
                Behavior::EnemyFlight,
            );
            body.base.hit_points = MIRAGE_DRAGON_SEGMENT_HEALTH;
            body.base.attack_power = MIRAGE_DRAGON_SEGMENT_ATTACK_POWER;
            body.base.collision_class = CollisionClass::Enemy;
            body.base.flags.active = false;
            body.base.flags.visible = false;
            body.base.flags.collision_disabled = true;
            body.base.flags.casts_shadow = false;
            *segment = Some(
                self.state
                    .objects
                    .allocate(body)
                    .ok_or(Error::ObjectCapacityReached)?,
            );
        }

        let mut tail = Object::new(
            ObjectKind::Enemy,
            ShapeId::MIRAGE_DRAGON_TAIL,
            Behavior::EnemyFlight,
        );
        tail.base.hit_points = MIRAGE_DRAGON_SEGMENT_HEALTH;
        tail.base.attack_power = MIRAGE_DRAGON_SEGMENT_ATTACK_POWER;
        tail.base.collision_class = CollisionClass::Enemy;
        tail.base.flags.active = false;
        tail.base.flags.visible = false;
        tail.base.flags.collision_disabled = true;
        tail.base.flags.casts_shadow = false;
        self.mirage_dragon_tail = Some(
            self.state
                .objects
                .allocate(tail)
                .ok_or(Error::ObjectCapacityReached)?,
        );
        Ok(())
    }

    fn update_mission_camera(&mut self, retail_frame: u16) {
        let (start, end) = enclosing_camera_keyframes(retail_frame);
        self.state.camera.position = interpolate_vector(
            start.position,
            end.position,
            retail_frame.saturating_sub(start.retail_frame),
            end.retail_frame.saturating_sub(start.retail_frame),
        );
        self.state.camera.rotation = Rotation {
            pitch: interpolate_angle(
                start.pitch,
                end.pitch,
                retail_frame.saturating_sub(start.retail_frame),
                end.retail_frame.saturating_sub(start.retail_frame),
            ),
            yaw: interpolate_angle(
                start.yaw,
                end.yaw,
                retail_frame.saturating_sub(start.retail_frame),
                end.retail_frame.saturating_sub(start.retail_frame),
            ),
            roll: interpolate_angle(
                start.roll,
                end.roll,
                retail_frame.saturating_sub(start.retail_frame),
                end.retail_frame.saturating_sub(start.retail_frame),
            ),
        };
    }

    fn update_mission_entry_flyby(&mut self, retail_frame: u16) {
        if self.mission_entry_flyby.iter().all(Option::is_none) {
            return;
        }
        if retail_frame >= MISSION_ENCOUNTER_START_RETAIL_FRAME {
            if retail_frame >= MISSION_PLAYER_CONTROL_START_RETAIL_FRAME {
                self.activate_mission_encounter_targets();
            }
            if retail_frame < MISSION_FIGHTER_COMBAT_HANDOFF_RETAIL_FRAME {
                self.update_certified_mission_encounter(retail_frame);
            } else {
                self.update_certified_capital_craft(retail_frame);
                if retail_frame == MISSION_FIGHTER_COMBAT_HANDOFF_RETAIL_FRAME {
                    self.initialize_mission_fighter_combat();
                } else {
                    self.update_mission_fighter_combat(retail_frame);
                }
                if retail_frame > MISSION_ENCOUNTER_CERTIFIED_END_RETAIL_FRAME {
                    self.update_post_opening_mission_encounter(retail_frame);
                }
            }
            return;
        }
        let (start, end) = enclosing_formation_keyframes(retail_frame);
        let numerator = retail_frame.saturating_sub(start.retail_frame);
        let denominator = end.retail_frame.saturating_sub(start.retail_frame);
        for (index, id) in self.mission_entry_flyby.iter().copied().enumerate() {
            let Some(id) = id else {
                continue;
            };
            if let Some(object) = self.state.objects.get_mut(id) {
                object.base.position = interpolate_vector(
                    start.positions[index],
                    end.positions[index],
                    numerator,
                    denominator,
                );
            }
        }
    }

    fn depart_mission_encounter_actor(&mut self, actor: MissionEncounterActor) {
        let Some(object) = self.mission_entry_flyby[actor.index()].take() else {
            return;
        };
        self.state.objects.remove(object);
    }

    fn activate_mission_encounter_targets(&mut self) {
        for id in self.mission_entry_flyby.iter().flatten().copied() {
            if let Some(object) = self.state.objects.get_mut(id) {
                object.base.kind = ObjectKind::Enemy;
                object.base.collision_class = CollisionClass::Enemy;
            }
        }
    }

    fn update_certified_mission_encounter(&mut self, retail_frame: u16) {
        let (start, end) = enclosing_encounter_keyframes(retail_frame);
        let numerator = retail_frame.saturating_sub(start.retail_frame);
        let denominator = end.retail_frame.saturating_sub(start.retail_frame);
        for (index, id) in self.mission_entry_flyby.iter().copied().enumerate() {
            let Some(id) = id else {
                continue;
            };
            if let Some(object) = self.state.objects.get_mut(id) {
                let start_pose = start.poses[index];
                let end_pose = end.poses[index];
                object.base.position = interpolate_vector(
                    start_pose.position,
                    end_pose.position,
                    numerator,
                    denominator,
                );
                object.base.pitch =
                    interpolate_angle(start_pose.pitch, end_pose.pitch, numerator, denominator);
                object.base.yaw =
                    interpolate_angle(start_pose.yaw, end_pose.yaw, numerator, denominator);
                object.base.roll =
                    interpolate_angle(start_pose.roll, end_pose.roll, numerator, denominator);
                object.base.speed =
                    interpolate_u8(start_pose.speed, end_pose.speed, numerator, denominator);
                object.base.velocity = Vector3::default();
            }
        }
    }

    fn update_certified_capital_craft(&mut self, retail_frame: u16) {
        if retail_frame > CAPITAL_FLIGHT_HANDOFF_RETAIL_FRAME {
            self.update_mission_capital_flight(retail_frame);
            return;
        }

        let (start, end) = enclosing_encounter_keyframes(retail_frame);
        let numerator = retail_frame.saturating_sub(start.retail_frame);
        let denominator = end.retail_frame.saturating_sub(start.retail_frame);
        for actor in [
            MissionEncounterActor::FirstCapital,
            MissionEncounterActor::SecondCapital,
        ] {
            let Some(id) = self.mission_entry_flyby[actor.index()] else {
                continue;
            };
            let Some(object) = self.state.objects.get_mut(id) else {
                continue;
            };
            let start_pose = start.poses[actor.index()];
            let end_pose = end.poses[actor.index()];
            object.base.position = interpolate_vector(
                start_pose.position,
                end_pose.position,
                numerator,
                denominator,
            );
            object.base.pitch =
                interpolate_angle(start_pose.pitch, end_pose.pitch, numerator, denominator);
            object.base.yaw =
                interpolate_angle(start_pose.yaw, end_pose.yaw, numerator, denominator);
            object.base.roll =
                interpolate_angle(start_pose.roll, end_pose.roll, numerator, denominator);
            object.base.speed =
                interpolate_u8(start_pose.speed, end_pose.speed, numerator, denominator);
            object.base.velocity = Vector3::default();
        }

        if retail_frame == CAPITAL_FLIGHT_HANDOFF_RETAIL_FRAME {
            self.initialize_mission_capital_flight();
        }
    }

    fn initialize_mission_capital_flight(&mut self) {
        let handoff_states = [
            CapitalFlightState {
                vertical_wave_phase: Angle::from_units(FIRST_CAPITAL_VERTICAL_WAVE_PHASE),
                pending_velocity: Vector3::default(),
                movement_phase: CapitalMovementPhase::Ready,
                weapon_phase: CapitalWeaponPhase::Ready,
            },
            CapitalFlightState {
                vertical_wave_phase: Angle::from_units(SECOND_CAPITAL_VERTICAL_WAVE_PHASE),
                pending_velocity: Vector3::default(),
                movement_phase: CapitalMovementPhase::Ready,
                weapon_phase: CapitalWeaponPhase::Ready,
            },
        ];
        for ((actor, flight), velocity) in [
            MissionEncounterActor::FirstCapital,
            MissionEncounterActor::SecondCapital,
        ]
        .into_iter()
        .zip(handoff_states)
        .zip(CAPITAL_FLIGHT_HANDOFF_VELOCITIES)
        {
            let Some(id) = self.mission_entry_flyby[actor.index()] else {
                continue;
            };
            let Some(object) = self.state.objects.get_mut(id) else {
                continue;
            };
            object.base.velocity = velocity;
            object.base.kind = ObjectKind::Enemy;
            object.base.behavior = Behavior::EnemyFlight;
            object.base.collision_class = CollisionClass::Enemy;
            object.extension.activity = ObjectActivity::CapitalFlight(flight);
        }
    }

    fn update_mission_capital_flight(&mut self, retail_frame: u16) {
        let Some(player_position) = self
            .state
            .mission
            .primary_player
            .and_then(|id| self.state.objects.get(id))
            .map(|object| object.base.position)
        else {
            return;
        };

        for (actor, first) in [
            (MissionEncounterActor::FirstCapital, true),
            (MissionEncounterActor::SecondCapital, false),
        ] {
            let Some(id) = self.mission_entry_flyby[actor.index()] else {
                continue;
            };
            let Some(object) = self.state.objects.get_mut(id) else {
                continue;
            };
            let ObjectActivity::CapitalFlight(mut flight) = object.extension.activity else {
                continue;
            };
            for &action in capital_continuation::actions(retail_frame, first) {
                apply_capital_flight_action(
                    object,
                    &mut flight,
                    action,
                    player_position,
                    player_position,
                );
            }
            object.extension.activity = ObjectActivity::CapitalFlight(flight);
        }
    }

    fn initialize_mission_fighter_combat(&mut self) {
        let (before_handoff, at_handoff) =
            enclosing_encounter_keyframes(MISSION_FIGHTER_COMBAT_HANDOFF_RETAIL_FRAME);
        let handoff = if at_handoff.retail_frame == MISSION_FIGHTER_COMBAT_HANDOFF_RETAIL_FRAME {
            at_handoff
        } else {
            before_handoff
        };
        let fighter_states = [
            FighterFlightState {
                logic_credit: UPPER_FIGHTER_INITIAL_LOGIC_CREDIT,
                logic_cadence: FighterLogicCadence::EntryChase,
                vertical_wave_phase: Angle::from_units(138),
                vertical_pitch_target: Angle::ZERO,
                vertical_wave_direction: FighterWaveDirection::Forward,
                vertical_wave_polarity: FighterWavePolarity::Standard,
                vertical_wave_order: FighterWaveOrder::BeforeSteering,
                centering_target_order: FighterCenteringTargetOrder::BeforeSteering,
                pending_velocity: Vector3::default(),
                pending_vertical_displacement: 0,
                altitude_phase: FighterAltitudePhase::Wave,
                maneuver_bank: Angle::from_units(244),
                maneuver_ticks_remaining: FIGHTER_INITIAL_ACTIVITY_TICKS_REMAINING,
                fire_ticks_remaining: FIGHTER_INITIAL_ACTIVITY_TICKS_REMAINING,
                weapon_phase: FighterWeaponPhase::Ready,
            },
            FighterFlightState {
                logic_credit: 0,
                logic_cadence: FighterLogicCadence::EntryChase,
                vertical_wave_phase: Angle::from_units(138),
                vertical_pitch_target: Angle::ZERO,
                vertical_wave_direction: FighterWaveDirection::Forward,
                vertical_wave_polarity: FighterWavePolarity::Mirrored,
                vertical_wave_order: FighterWaveOrder::BeforeSteering,
                centering_target_order: FighterCenteringTargetOrder::BeforeSteering,
                pending_velocity: Vector3::default(),
                pending_vertical_displacement: 0,
                altitude_phase: FighterAltitudePhase::Wave,
                maneuver_bank: Angle::from_units(244),
                maneuver_ticks_remaining: FIGHTER_INITIAL_ACTIVITY_TICKS_REMAINING,
                fire_ticks_remaining: FIGHTER_INITIAL_ACTIVITY_TICKS_REMAINING,
                weapon_phase: FighterWeaponPhase::Ready,
            },
        ];

        for (actor, fighter_state) in [
            MissionEncounterActor::UpperFighter,
            MissionEncounterActor::LowerFighter,
        ]
        .into_iter()
        .zip(fighter_states)
        {
            let Some(id) = self.mission_entry_flyby[actor.index()] else {
                continue;
            };
            let Some(object) = self.state.objects.get_mut(id) else {
                continue;
            };
            let pose = handoff.poses[actor.index()];
            object.base.position = pose.position;
            object.base.pitch = Angle::from_units(pose.pitch);
            object.base.yaw = Angle::from_units(pose.yaw);
            object.base.roll = Angle::from_units(pose.roll);
            object.base.speed = pose.speed;
            object.base.velocity = Vector3::default();
            object.base.kind = ObjectKind::Enemy;
            object.base.behavior = Behavior::EnemyFlight;
            object.base.collision_class = CollisionClass::Enemy;
            object.extension.activity = ObjectActivity::FighterFlight(fighter_state);
        }
        self.state.random = super::state::RandomState::new(FIGHTER_HANDOFF_RANDOM_STATE);
    }

    fn update_mission_fighter_combat(&mut self, retail_frame: u16) {
        let Some(player_position) = self
            .state
            .mission
            .primary_player
            .and_then(|id| self.state.objects.get(id))
            .map(|object| object.base.position)
        else {
            return;
        };

        let cooperative_dispatch = fighter_logic_dispatch(retail_frame);
        let random_cadence = fighter_random_cadence(retail_frame);
        let logic_due = cooperative_dispatch
            .map(FighterLogicDispatchPair::has_work)
            .unwrap_or_else(|| {
                [
                    MissionEncounterActor::UpperFighter,
                    MissionEncounterActor::LowerFighter,
                ]
                .into_iter()
                .filter_map(|actor| self.mission_entry_flyby[actor.index()])
                .filter_map(|id| self.state.objects.get(id))
                .find_map(|object| {
                    let ObjectActivity::FighterFlight(fighter) = object.extension.activity else {
                        return None;
                    };
                    Some(
                        fighter.logic_credit + FIGHTER_LOGIC_CREDIT_PER_TICK
                            >= fighter_logic_credit_threshold(fighter.logic_cadence),
                    )
                })
                .unwrap_or(false)
            });
        if let Some(cadence) = random_cadence {
            for _ in 0..cadence.ambient_before {
                self.state.random.next_byte();
            }
        } else if logic_due {
            self.state.random.next_byte();
        }
        'fighters: for actor in [
            MissionEncounterActor::UpperFighter,
            MissionEncounterActor::LowerFighter,
        ] {
            let Some(id) = self.mission_entry_flyby[actor.index()] else {
                continue;
            };

            let (maneuver_due, fire_due, within_fire_range) = 'fighter_update: {
                let Some(object) = self.state.objects.get_mut(id) else {
                    continue 'fighters;
                };
                if object.base.flags.collision_disabled {
                    continue 'fighters;
                }
                let ObjectActivity::FighterFlight(mut fighter) = object.extension.activity else {
                    continue 'fighters;
                };

                fighter.logic_credit += FIGHTER_LOGIC_CREDIT_PER_TICK;
                let logic_credit_threshold = fighter_logic_credit_threshold(fighter.logic_cadence);
                let credit_dispatch = if fighter.logic_credit < logic_credit_threshold {
                    FighterLogicDispatch::Wait
                } else {
                    fighter.logic_credit -= logic_credit_threshold;
                    FighterLogicDispatch::Complete
                };
                let dispatch = cooperative_dispatch
                    .map(|pair| pair.for_actor(actor))
                    .unwrap_or(credit_dispatch);
                if dispatch == FighterLogicDispatch::Wait {
                    object.base.velocity = Vector3::default();
                    object.extension.activity = ObjectActivity::FighterFlight(fighter);
                    continue 'fighters;
                }

                if dispatch == FighterLogicDispatch::ApplyWave {
                    object.base.velocity = Vector3::default();
                    object.base.position.y = object
                        .base
                        .position
                        .y
                        .wrapping_add(fighter.pending_vertical_displacement);
                    fighter.pending_vertical_displacement = 0;
                    object.extension.activity = ObjectActivity::FighterFlight(fighter);
                    continue 'fighters;
                }

                if dispatch == FighterLogicDispatch::AltitudeCenteringOnly {
                    object.base.velocity = Vector3::default();
                    if matches!(
                        fighter.altitude_phase,
                        FighterAltitudePhase::Centering { .. }
                    ) {
                        object.base.position.y = chase_proportional(object.base.position.y, 0, 3);
                    }
                    object.extension.activity = ObjectActivity::FighterFlight(fighter);
                    continue 'fighters;
                }

                if dispatch.includes_movement()
                    || matches!(
                        dispatch,
                        FighterLogicDispatch::TurnOnly | FighterLogicDispatch::AltitudeAndTurnOnly
                    )
                {
                    if let FighterWeaponPhase::Restoring {
                        flight_angles,
                        ticks_remaining,
                    } = fighter.weapon_phase
                    {
                        if ticks_remaining <= 1 {
                            object.base.pitch = flight_angles.pitch;
                            object.base.yaw = flight_angles.yaw;
                            object.base.roll = flight_angles.roll;
                            fighter.weapon_phase = FighterWeaponPhase::Ready;
                        } else {
                            fighter.weapon_phase = FighterWeaponPhase::Restoring {
                                flight_angles,
                                ticks_remaining: ticks_remaining - 1,
                            };
                        }
                    }
                }

                if dispatch == FighterLogicDispatch::AltitudeAndTurnOnly {
                    if matches!(
                        fighter.altitude_phase,
                        FighterAltitudePhase::Centering { .. }
                    ) {
                        object.base.position.y = chase_proportional(object.base.position.y, 0, 3);
                    }
                    let bank_turn = (object.base.roll.units() as i8) / 4;
                    object.base.yaw = object.base.yaw.wrapping_add(bank_turn);
                    fighter.pending_velocity = flight_velocity(
                        object.base.pitch,
                        object.base.yaw,
                        object.base.speed,
                        MISSION_ENCOUNTER_POSITION_SCALE,
                    );
                    object.base.velocity = Vector3::default();
                    object.extension.activity = ObjectActivity::FighterFlight(fighter);
                    continue 'fighters;
                }

                if dispatch == FighterLogicDispatch::PrepareMovement {
                    if matches!(
                        fighter.altitude_phase,
                        FighterAltitudePhase::Centering { .. }
                    ) {
                        object.base.position.y = chase_proportional(object.base.position.y, 0, 3);
                    }
                    let bank_turn = (object.base.roll.units() as i8) / 4;
                    let pending_yaw = object.base.yaw.wrapping_add(bank_turn);
                    fighter.pending_velocity = flight_velocity(
                        object.base.pitch,
                        pending_yaw,
                        object.base.speed,
                        MISSION_ENCOUNTER_POSITION_SCALE,
                    );
                    object.base.velocity = Vector3::default();
                    object.extension.activity = ObjectActivity::FighterFlight(fighter);
                    continue 'fighters;
                }

                if dispatch == FighterLogicDispatch::FinishPreparedAndBeginMovement {
                    let bank_turn = (object.base.roll.units() as i8) / 4;
                    object.base.yaw = object.base.yaw.wrapping_add(bank_turn);
                    object.base.position =
                        add_vectors(object.base.position, fighter.pending_velocity);
                    fighter.pending_velocity = Vector3::default();
                    let decisions = finish_fighter_steering(
                        object,
                        &mut fighter,
                        FighterLogicDispatch::SteeringAfterEarlyAltitude,
                        player_position,
                    );
                    if matches!(
                        fighter.altitude_phase,
                        FighterAltitudePhase::Centering { .. }
                    ) {
                        object.base.position.y = chase_proportional(object.base.position.y, 0, 3);
                    }
                    let bank_turn = (object.base.roll.units() as i8) / 4;
                    object.base.yaw = object.base.yaw.wrapping_add(bank_turn);
                    object.base.velocity = flight_velocity(
                        object.base.pitch,
                        object.base.yaw,
                        object.base.speed,
                        MISSION_ENCOUNTER_POSITION_SCALE,
                    );
                    object.extension.activity = ObjectActivity::FighterFlight(fighter);
                    break 'fighter_update decisions;
                }

                if dispatch == FighterLogicDispatch::TurnOnly {
                    let bank_turn = (object.base.roll.units() as i8) / 4;
                    object.base.yaw = object.base.yaw.wrapping_add(bank_turn);
                    fighter.pending_velocity = flight_velocity(
                        object.base.pitch,
                        object.base.yaw,
                        object.base.speed,
                        MISSION_ENCOUNTER_POSITION_SCALE,
                    );
                    object.base.velocity = Vector3::default();
                    object.extension.activity = ObjectActivity::FighterFlight(fighter);
                    continue 'fighters;
                }

                if matches!(
                    dispatch,
                    FighterLogicDispatch::MovementContinuation
                        | FighterLogicDispatch::MovementContinuationAfterEarlyAltitude
                ) {
                    object.base.velocity = fighter.pending_velocity;
                    fighter.pending_velocity = Vector3::default();
                } else if dispatch.includes_movement() {
                    let bank_turn = (object.base.roll.units() as i8) / 4;
                    object.base.yaw = object.base.yaw.wrapping_add(bank_turn);
                    object.base.velocity = flight_velocity(
                        object.base.pitch,
                        object.base.yaw,
                        object.base.speed,
                        MISSION_ENCOUNTER_POSITION_SCALE,
                    );
                } else {
                    object.base.velocity = Vector3::default();
                }

                if dispatch == FighterLogicDispatch::MovementAndRoll {
                    if matches!(
                        fighter.altitude_phase,
                        FighterAltitudePhase::Centering { .. }
                    ) {
                        object.base.position.y = chase_proportional(object.base.position.y, 0, 3);
                    }
                    object.base.roll = chase_fighter_angle(object.base.roll, fighter.maneuver_bank);
                    object.extension.activity = ObjectActivity::FighterFlight(fighter);
                    continue 'fighters;
                }

                if !dispatch.includes_steering() {
                    object.extension.activity = ObjectActivity::FighterFlight(fighter);
                    continue 'fighters;
                }

                let decisions =
                    finish_fighter_steering(object, &mut fighter, dispatch, player_position);
                object.extension.activity = ObjectActivity::FighterFlight(fighter);
                decisions
            };

            if maneuver_due {
                let selection =
                    self.state.random.next_byte() as usize % FIGHTER_MANEUVER_BANKS.len();
                if let Some(object) = self.state.objects.get_mut(id) {
                    if let ObjectActivity::FighterFlight(ref mut fighter) =
                        object.extension.activity
                    {
                        fighter.centering_target_order = match fighter.logic_cadence {
                            FighterLogicCadence::EntryChase => {
                                FighterCenteringTargetOrder::BeforeSteering
                            }
                            FighterLogicCadence::Combat => {
                                FighterCenteringTargetOrder::AfterSteering
                            }
                        };
                        fighter.maneuver_bank = FIGHTER_MANEUVER_BANKS[selection];
                        fighter.maneuver_ticks_remaining = FIGHTER_MANEUVER_PERIOD_TICKS_REMAINING;
                        fighter.logic_cadence = FighterLogicCadence::Combat;
                        // The two formation fighters enter the cooperative
                        // combat queue at different points. The upper craft
                        // waits one slice after installing its new task; the
                        // lower craft inherits the already credited slice.
                        fighter.logic_credit = post_maneuver_logic_credit(actor);
                        fighter.vertical_wave_order = FighterWaveOrder::AfterSteering;
                        fighter.altitude_phase = FighterAltitudePhase::Centering {
                            ticks_remaining: FIGHTER_ALTITUDE_CENTERING_TICKS,
                        };
                    }
                }
            }

            if fire_due {
                let passes_random_check =
                    self.state.random.next_byte() >= FIGHTER_FIRE_RANDOM_THRESHOLD;
                let fire = passes_random_check && within_fire_range;
                if let Some(object) = self.state.objects.get_mut(id) {
                    if let ObjectActivity::FighterFlight(ref mut fighter) =
                        object.extension.activity
                    {
                        fighter.fire_ticks_remaining = FIGHTER_FIRE_PERIOD_TICKS_REMAINING;
                        if fire {
                            if matches!(
                                fighter.centering_target_order,
                                FighterCenteringTargetOrder::BeforeSteering
                            ) && matches!(
                                fighter.altitude_phase,
                                FighterAltitudePhase::Centering { .. }
                            ) {
                                fighter.vertical_pitch_target = fighter_fire_pitch_target(
                                    player_position,
                                    object.base.position,
                                );
                                fighter.weapon_phase = FighterWeaponPhase::Restoring {
                                    flight_angles: FighterAngles {
                                        pitch: object.base.pitch,
                                        yaw: object.base.yaw,
                                        roll: object.base.roll,
                                    },
                                    ticks_remaining: FIGHTER_AIM_RESTORE_TICKS,
                                };
                            } else {
                                // Combat weapon aim exists only while
                                // constructing the shot; the altitude task
                                // retains its flight target.
                                fighter.weapon_phase = FighterWeaponPhase::Ready;
                            }
                        }
                    }
                }
            }
            if actor == MissionEncounterActor::UpperFighter {
                if let Some(cadence) = random_cadence {
                    for _ in 0..cadence.ambient_between_fighters {
                        self.state.random.next_byte();
                    }
                }
            }
        }
        if let Some(cadence) = random_cadence {
            for _ in 0..cadence.ambient_after {
                self.state.random.next_byte();
            }
            if let Some(resulting_state) = cadence.resulting_state {
                self.state.random = super::state::RandomState::new(resulting_state);
            }
        }
    }

    fn update_post_opening_mission_encounter(&mut self, retail_frame: u16) {
        let tracks: [(MissionEncounterActor, &'static [MissionActorKeyframe]); 4] = [
            (
                MissionEncounterActor::FirstCapital,
                &opening_continuation::FIRST_CAPITAL_MISSION_KEYFRAMES,
            ),
            (
                MissionEncounterActor::SecondCapital,
                &opening_continuation::SECOND_CAPITAL_MISSION_KEYFRAMES,
            ),
            (
                MissionEncounterActor::UpperFighter,
                &opening_continuation::UPPER_FIGHTER_MISSION_KEYFRAMES,
            ),
            (
                MissionEncounterActor::LowerFighter,
                &opening_continuation::LOWER_FIGHTER_MISSION_KEYFRAMES,
            ),
        ];
        self.update_scripted_mission_actors(retail_frame, tracks);
    }

    fn update_reengagement_targets(&mut self, retail_frame: u16) {
        let defeated_target_was_present =
            self.mission_entry_flyby[MissionEncounterActor::FirstCapital.index()].is_some();

        if retail_frame == second_sortie_fighters::INITIAL_RETAIL_FRAME {
            for (actor, pose) in [
                MissionEncounterActor::UpperFighter,
                MissionEncounterActor::LowerFighter,
            ]
            .into_iter()
            .zip(second_sortie_fighters::INITIAL_POSES)
            {
                self.initialize_reengagement_fighter(actor, pose);
            }
        } else {
            for (actor, upper) in [
                (MissionEncounterActor::UpperFighter, true),
                (MissionEncounterActor::LowerFighter, false),
            ] {
                self.update_reengagement_fighter(retail_frame, actor, upper);
            }
        }

        let player_position = self
            .state
            .mission
            .primary_player
            .and_then(|id| self.state.objects.get(id))
            .map(|object| object.base.position);
        if retail_frame == second_sortie_capital::INITIAL_RETAIL_FRAME {
            for ((actor, _), pose) in [
                (MissionEncounterActor::FirstCapital, true),
                (MissionEncounterActor::SecondCapital, false),
            ]
            .into_iter()
            .zip(second_sortie_capital::INITIAL_POSES)
            {
                self.initialize_reengagement_capital(actor, pose);
            }
        } else if let Some(player_position) = player_position {
            let previous_player_position = self
                .previous_mission_player_position
                .unwrap_or(player_position);
            for (actor, first) in [
                (MissionEncounterActor::FirstCapital, true),
                (MissionEncounterActor::SecondCapital, false),
            ] {
                self.update_reengagement_capital(
                    retail_frame,
                    actor,
                    first,
                    player_position,
                    previous_player_position,
                );
            }
        }

        if retail_frame == second_sortie_capital::SECOND_DEPARTURE_RETAIL_FRAME {
            self.depart_mission_encounter_actor(MissionEncounterActor::SecondCapital);
        }
        if retail_frame == second_sortie_capital::FIRST_DEPARTURE_RETAIL_FRAME {
            self.depart_mission_encounter_actor(MissionEncounterActor::FirstCapital);
        }
        if retail_frame == second_sortie_fighters::LOWER_DEPARTURE_RETAIL_FRAME {
            self.depart_mission_encounter_actor(MissionEncounterActor::LowerFighter);
        }
        if retail_frame == second_sortie_fighters::UPPER_DEPARTURE_RETAIL_FRAME {
            self.depart_mission_encounter_actor(MissionEncounterActor::UpperFighter);
        }
        if retail_frame == SECOND_SORTIE_DEFEATED_TARGET_RETAIL_FRAME
            && defeated_target_was_present
            && self.mission_entry_flyby[MissionEncounterActor::FirstCapital.index()].is_none()
        {
            // Retail's surviving target is destroyed at the end of the first
            // re-engagement and contributes the `00100` shown on the next map
            // frame. Earlier departures are escapes and do not score.
            self.state.mission.score = self
                .state
                .mission
                .score
                .saturating_add(u32::from(MISSION_ENCOUNTER_HEALTH));
            self.state.mission.objects_destroyed =
                self.state.mission.objects_destroyed.saturating_add(1);
        }
        if let Some(player_position) = player_position {
            self.previous_mission_player_position = Some(player_position);
        }
    }

    fn initialize_reengagement_capital(
        &mut self,
        actor: MissionEncounterActor,
        pose: MissionEncounterPose,
    ) {
        let Some(id) = self.mission_entry_flyby[actor.index()] else {
            return;
        };
        let Some(object) = self.state.objects.get_mut(id) else {
            return;
        };
        if object.base.explosion_timer > 0 || object.base.flags.exploding {
            return;
        }
        object.base.flags.active = true;
        object.base.flags.visible = true;
        object.base.flags.collision_disabled = false;
        object.base.kind = ObjectKind::Enemy;
        object.base.behavior = Behavior::EnemyFlight;
        object.base.collision_class = CollisionClass::Enemy;
        object.base.position = pose.position;
        object.base.pitch = Angle::from_units(pose.pitch);
        object.base.yaw = Angle::from_units(pose.yaw);
        object.base.roll = Angle::from_units(pose.roll);
        object.base.speed = pose.speed;
        object.base.velocity = Vector3::default();
        object.extension.activity = ObjectActivity::CapitalFlight(CapitalFlightState {
            vertical_wave_phase: Angle::ZERO,
            pending_velocity: Vector3::default(),
            movement_phase: CapitalMovementPhase::Ready,
            weapon_phase: CapitalWeaponPhase::Ready,
        });
    }

    fn initialize_reengagement_fighter(
        &mut self,
        actor: MissionEncounterActor,
        pose: MissionEncounterPose,
    ) {
        let Some(id) = self.mission_entry_flyby[actor.index()] else {
            return;
        };
        let Some(object) = self.state.objects.get_mut(id) else {
            return;
        };
        if object.base.explosion_timer > 0 || object.base.flags.exploding {
            return;
        }
        object.base.flags.active = true;
        object.base.flags.visible = true;
        object.base.flags.collision_disabled = false;
        object.base.kind = ObjectKind::Enemy;
        object.base.behavior = Behavior::EnemyFlight;
        object.base.collision_class = CollisionClass::Enemy;
        object.base.position = pose.position;
        object.base.pitch = Angle::from_units(pose.pitch);
        object.base.yaw = Angle::from_units(pose.yaw);
        object.base.roll = Angle::from_units(pose.roll);
        object.base.speed = pose.speed;
        object.base.velocity = Vector3::default();
        object.extension.activity =
            ObjectActivity::ReengagementFighterFlight(ReengagementFighterFlightState {
                vertical_wave_phase: Angle::from_units(REENGAGEMENT_FIGHTER_INITIAL_WAVE_PHASE),
                vertical_wave_sample: 0,
                vertical_wave_quarters_applied: 0,
                vertical_pitch_target: Angle::ZERO,
                maneuver_bank: Angle::ZERO,
                altitude_phase: FighterAltitudePhase::Wave,
                pending_velocity: Vector3::default(),
                movement_phase: ReengagementFighterMovementPhase::Ready,
            });
    }

    fn update_reengagement_fighter(
        &mut self,
        retail_frame: u16,
        actor: MissionEncounterActor,
        upper: bool,
    ) {
        let Some(id) = self.mission_entry_flyby[actor.index()] else {
            return;
        };
        let Some(object) = self.state.objects.get_mut(id) else {
            return;
        };
        if object.base.explosion_timer > 0 || object.base.flags.exploding {
            return;
        }
        let ObjectActivity::ReengagementFighterFlight(mut flight) = object.extension.activity
        else {
            return;
        };
        for &action in second_sortie_fighters::actions(retail_frame, upper) {
            apply_reengagement_fighter_action(object, &mut flight, action);
        }
        object.extension.activity = ObjectActivity::ReengagementFighterFlight(flight);
    }

    fn update_reengagement_capital(
        &mut self,
        retail_frame: u16,
        actor: MissionEncounterActor,
        first: bool,
        player_position: Vector3,
        previous_player_position: Vector3,
    ) {
        let Some(id) = self.mission_entry_flyby[actor.index()] else {
            return;
        };
        let Some(object) = self.state.objects.get_mut(id) else {
            return;
        };
        if object.base.explosion_timer > 0 || object.base.flags.exploding {
            return;
        }
        let ObjectActivity::CapitalFlight(mut flight) = object.extension.activity else {
            return;
        };
        for &action in second_sortie_capital::actions(retail_frame, first) {
            apply_capital_flight_action(
                object,
                &mut flight,
                action,
                player_position,
                previous_player_position,
            );
        }
        object.extension.activity = ObjectActivity::CapitalFlight(flight);

        let temporarily_inactive =
            first && second_sortie_capital::FIRST_INACTIVE_RETAIL_FRAMES.contains(&retail_frame);
        object.base.flags.active = !temporarily_inactive;
        object.base.flags.visible = !temporarily_inactive;
        object.base.flags.collision_disabled = temporarily_inactive;
    }

    fn update_scripted_mission_actors<const TRACK_COUNT: usize>(
        &mut self,
        retail_frame: u16,
        tracks: [(MissionEncounterActor, &'static [MissionActorKeyframe]); TRACK_COUNT],
    ) {
        for (actor, keyframes) in tracks {
            if retail_frame < keyframes[0].retail_frame {
                continue;
            }
            let (start, end) =
                enclosing_keyframes(keyframes, retail_frame, |keyframe| keyframe.retail_frame);
            let presentation = if retail_frame >= end.retail_frame {
                end.presentation
            } else {
                match (start.presentation, end.presentation) {
                    (
                        MissionActorPresentation::Present(start_pose),
                        MissionActorPresentation::Present(end_pose),
                    ) => MissionActorPresentation::Present(interpolate_encounter_pose(
                        start_pose,
                        end_pose,
                        retail_frame.saturating_sub(start.retail_frame),
                        end.retail_frame.saturating_sub(start.retail_frame),
                    )),
                    (presentation, _) => presentation,
                }
            };
            self.apply_mission_actor_presentation(actor, presentation);
        }
    }

    fn apply_mission_actor_presentation(
        &mut self,
        actor: MissionEncounterActor,
        presentation: MissionActorPresentation,
    ) {
        let Some(id) = self.mission_entry_flyby[actor.index()] else {
            return;
        };
        if self
            .state
            .objects
            .get(id)
            .is_some_and(|object| object.base.explosion_timer > 0 || object.base.flags.exploding)
        {
            return;
        }
        if presentation == MissionActorPresentation::Departed {
            self.depart_mission_encounter_actor(actor);
            return;
        }
        let Some(object) = self.state.objects.get_mut(id) else {
            return;
        };
        match presentation {
            MissionActorPresentation::Present(pose) => {
                object.base.flags.active = true;
                object.base.flags.visible = true;
                object.base.flags.collision_disabled = false;
                object.base.kind = ObjectKind::Enemy;
                object.base.behavior = Behavior::EnemyFlight;
                object.base.collision_class = CollisionClass::Enemy;
                object.base.position = pose.position;
                object.base.pitch = Angle::from_units(pose.pitch);
                object.base.yaw = Angle::from_units(pose.yaw);
                object.base.roll = Angle::from_units(pose.roll);
                object.base.speed = pose.speed;
                object.base.velocity = Vector3::default();
            }
            MissionActorPresentation::Inactive => {
                object.base.flags.active = false;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
                object.base.velocity = Vector3::default();
            }
            MissionActorPresentation::Departed => unreachable!(),
        }
    }

    fn update_mission_projectiles(&mut self, retail_frame: u16) -> Result<(), Error> {
        for trajectory in MISSION_PROJECTILE_TRAJECTORIES {
            let keyframes = trajectory.keyframes(retail_frame);
            let start_frame = keyframes
                .first()
                .expect("mission projectile trajectory is not empty")
                .retail_frame;
            let end_frame = keyframes
                .last()
                .expect("mission projectile trajectory is not empty")
                .retail_frame;
            let active_index = self
                .mission_projectiles
                .iter()
                .position(|projectile| projectile.trajectory == trajectory);

            if retail_frame < start_frame || retail_frame > end_frame {
                if retail_frame > end_frame {
                    if let Some(index) = active_index {
                        let projectile = self.mission_projectiles.swap_remove(index);
                        self.state.objects.remove(projectile.object);
                    }
                }
                continue;
            }

            let projectile_id = if let Some(index) = active_index {
                self.mission_projectiles[index].object
            } else {
                let mut projectile = Object::new(
                    ObjectKind::Projectile,
                    ShapeId::ENEMY_LASER,
                    Behavior::MissionScriptedProjectile,
                );
                projectile.base.weapon = WeaponKind::EnemyLaser;
                projectile.base.attack_power = ENEMY_LASER_ATTACK_POWER;
                projectile.base.collision_class = CollisionClass::EnemyWeapon;
                projectile.base.flags.casts_shadow = false;
                projectile.base.linked_object = self
                    .mission_entry_flyby
                    .get(trajectory.firing_actor().index())
                    .copied()
                    .flatten();
                let projectile_id = self
                    .state
                    .objects
                    .allocate(projectile)
                    .ok_or(Error::ObjectCapacityReached)?;
                self.mission_projectiles.push(ActiveMissionProjectile {
                    trajectory,
                    object: projectile_id,
                });
                projectile_id
            };

            let pose = mission_projectile_pose(keyframes, retail_frame);
            if let Some(projectile) = self.state.objects.get_mut(projectile_id) {
                projectile.base.position = pose.position;
                projectile.base.pitch = Angle::from_units(pose.pitch);
                projectile.base.yaw = Angle::from_units(pose.yaw);
                projectile.base.roll = Angle::from_units(pose.roll);
                projectile.base.speed = pose.speed;
                projectile.base.velocity = Vector3::default();
            }
        }
        Ok(())
    }

    fn update_reengagement_projectiles(
        &mut self,
        retail_frame: u16,
        player_position: Vector3,
        previous_player_position: Vector3,
    ) -> Result<(), Error> {
        for track_index in 0..second_sortie_projectiles::PROJECTILE_COUNT {
            let descriptor = second_sortie_projectiles::descriptor(track_index)
                .expect("re-engagement projectile descriptor exists");
            let active_index = self
                .reengagement_projectiles
                .iter()
                .position(|projectile| projectile.track_index == track_index);
            if retail_frame < descriptor.start_retail_frame
                || retail_frame > descriptor.end_retail_frame
            {
                if retail_frame > descriptor.end_retail_frame {
                    if let Some(index) = active_index {
                        let projectile = self.reengagement_projectiles.swap_remove(index);
                        self.state.objects.remove(projectile.object);
                    }
                }
                continue;
            }

            let projectile_id = if let Some(index) = active_index {
                self.reengagement_projectiles[index].object
            } else {
                let mut projectile = Object::new(
                    ObjectKind::Projectile,
                    ShapeId::ENEMY_LASER,
                    Behavior::Projectile,
                );
                projectile.base.weapon = WeaponKind::EnemyLaser;
                projectile.base.hit_points = SF2_HOSTILE_LASER_HEALTH;
                projectile.base.attack_power = SF2_HOSTILE_LASER_ATTACK_POWER;
                projectile.base.collision_class = CollisionClass::EnemyWeapon;
                projectile.base.flags.casts_shadow = false;
                projectile.base.position = descriptor.initial_pose.position;
                projectile.base.pitch = Angle::from_units(descriptor.initial_pose.pitch);
                projectile.base.yaw = Angle::from_units(descriptor.initial_pose.yaw);
                projectile.base.roll = Angle::from_units(descriptor.initial_pose.roll);
                projectile.base.speed = descriptor.initial_pose.speed;
                projectile.extension.activity =
                    ObjectActivity::HostileProjectileFlight(HostileProjectileFlightState {
                        phase: HostileProjectileFlightPhase::Homing,
                        motion_steps_elapsed: 0,
                        movement_phase: HostileProjectileMovementPhase::Ready,
                    });
                let projectile_id = self
                    .state
                    .objects
                    .allocate(projectile)
                    .ok_or(Error::ObjectCapacityReached)?;
                self.reengagement_projectiles
                    .push(ActiveReengagementProjectile {
                        track_index,
                        object: projectile_id,
                    });
                projectile_id
            };

            if let Some(projectile) = self.state.objects.get_mut(projectile_id) {
                let ObjectActivity::HostileProjectileFlight(mut flight) =
                    projectile.extension.activity
                else {
                    continue;
                };
                for &action in second_sortie_projectiles::actions(track_index, retail_frame) {
                    apply_hostile_projectile_action(
                        projectile,
                        &mut flight,
                        action,
                        player_position,
                        previous_player_position,
                    );
                }
                projectile.extension.activity = ObjectActivity::HostileProjectileFlight(flight);
            }
        }
        Ok(())
    }

    fn update_mission_player_entry(&mut self, retail_frame: u16) {
        let (start, end) = enclosing_player_keyframes(retail_frame);
        let numerator = retail_frame.saturating_sub(start.retail_frame);
        let denominator = end.retail_frame.saturating_sub(start.retail_frame);
        let position = interpolate_vector(start.position, end.position, numerator, denominator);
        let pitch = interpolate_angle(start.pitch, end.pitch, numerator, denominator);
        let yaw = interpolate_angle(start.yaw, end.yaw, numerator, denominator);
        let roll = interpolate_angle(start.roll, end.roll, numerator, denominator);
        let speed = interpolate_u8(start.speed, end.speed, numerator, denominator);

        if let Some(primary) = self.state.mission.primary_player {
            if let Some(object) = self.state.objects.get_mut(primary) {
                object.base.position = position;
                object.base.pitch = pitch;
                object.base.yaw = yaw;
                object.base.roll = roll;
                object.base.speed = speed;
                object.base.velocity = Vector3::default();
            }
        }
        if let Some(wingmate) = self.state.mission.wingmate {
            if let Some(object) = self.state.objects.get_mut(wingmate) {
                object.base.position = add_vectors(position, ACTIVE_WINGMATE_OFFSET);
                object.base.pitch = pitch;
                object.base.yaw = yaw;
                object.base.roll = roll;
                object.base.speed = speed;
                object.base.velocity = Vector3::default();
            }
        }
        if retail_frame > MISSION_CONTROL_HANDOFF_RETAIL_FRAME {
            self.update_active_camera(retail_frame, position, pitch, yaw);
        }
    }

    fn update_reengagement_presentation(&mut self, retail_frame: u16) {
        let camera = interpolated_camera_keyframe(&second_sortie::CAMERA_KEYFRAMES, retail_frame);
        self.state.camera.position = camera.position;
        self.state.camera.rotation.pitch = Angle::from_units(camera.pitch);
        self.state.camera.rotation.yaw = Angle::from_units(camera.yaw);
        self.state.camera.rotation.roll = Angle::from_units(camera.roll);

        let player = interpolated_player_keyframe(&second_sortie::PLAYER_KEYFRAMES, retail_frame);
        if let Some(primary) = self.state.mission.primary_player {
            if let Some(object) = self.state.objects.get_mut(primary) {
                apply_player_keyframe(object, player);
            }
        }
        let wingmate =
            interpolated_player_keyframe(&second_sortie::WINGMATE_KEYFRAMES, retail_frame);
        if let Some(id) = self.state.mission.wingmate {
            if let Some(object) = self.state.objects.get_mut(id) {
                apply_player_keyframe(object, wingmate);
            }
        }
    }

    fn update_interception_presentation(&mut self, retail_frame: u16) {
        let primary_flight_craft_shape = self.primary_flight_craft_shape();
        let camera =
            interpolated_camera_keyframe(&missile_interception::CAMERA_KEYFRAMES, retail_frame);
        self.state.camera.position = camera.position;
        self.state.camera.rotation.pitch = Angle::from_units(camera.pitch);
        self.state.camera.rotation.yaw = Angle::from_units(camera.yaw);
        self.state.camera.rotation.roll = Angle::from_units(camera.roll);

        let player =
            interpolated_player_keyframe(&missile_interception::PLAYER_KEYFRAMES, retail_frame);
        if let Some(primary) = self.state.mission.primary_player {
            if let Some(object) = self.state.objects.get_mut(primary) {
                apply_player_keyframe(object, player);
                let visible = retail_frame >= INTERCEPTION_PLAYER_REVEAL_RETAIL_FRAME;
                object.base.shape = if visible {
                    primary_flight_craft_shape
                } else {
                    ShapeId::EMPTY
                };
                object.base.flags.visible = visible;
                object.base.flags.collision_disabled = !visible;
            }
        }
        let wingmate =
            interpolated_player_keyframe(&missile_interception::WINGMATE_KEYFRAMES, retail_frame);
        if let Some(id) = self.state.mission.wingmate {
            if let Some(object) = self.state.objects.get_mut(id) {
                apply_player_keyframe(object, wingmate);
                object.base.shape = ShapeId::EMPTY;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
            }
        }
    }

    fn update_interception_missiles(&mut self, retail_frame: u16) {
        for index in 0..INTERCEPTION_MISSILE_COUNT {
            for &action in missile_interception_targets::actions(retail_frame, index) {
                if action == InterceptionMissileAction::Depart {
                    if let Some(object) = self.interception_missiles[index].take() {
                        self.state.objects.remove(object);
                    }
                    break;
                }
                let Some(id) = self.interception_missiles[index] else {
                    continue;
                };
                let Some(object) = self.state.objects.get_mut(id) else {
                    continue;
                };
                let ObjectActivity::InterceptionMissileFlight(mut flight) =
                    object.extension.activity
                else {
                    continue;
                };
                apply_interception_missile_action(object, &mut flight, action);
                object.extension.activity = ObjectActivity::InterceptionMissileFlight(flight);
            }
        }
    }

    fn update_fighter_intercept_presentation(&mut self, retail_frame: u16) {
        let primary_flight_craft_shape = self.primary_flight_craft_shape();
        let camera =
            interpolated_camera_keyframe(&fighter_intercept::CAMERA_KEYFRAMES, retail_frame);
        self.state.camera.position = camera.position;
        self.state.camera.rotation.pitch = Angle::from_units(camera.pitch);
        self.state.camera.rotation.yaw = Angle::from_units(camera.yaw);
        self.state.camera.rotation.roll = Angle::from_units(camera.roll);

        let player =
            interpolated_player_keyframe(&fighter_intercept::PLAYER_KEYFRAMES, retail_frame);
        if let Some(primary) = self.state.mission.primary_player {
            if let Some(object) = self.state.objects.get_mut(primary) {
                apply_player_keyframe(object, player);
                let visible = retail_frame >= FIGHTER_INTERCEPT_PLAYER_REVEAL_RETAIL_FRAME
                    && !FIGHTER_INTERCEPT_PLAYER_HIDDEN_RETAIL_FRAMES.contains(&retail_frame);
                object.base.shape = if visible {
                    primary_flight_craft_shape
                } else {
                    ShapeId::EMPTY
                };
                object.base.flags.visible = visible;
                object.base.flags.collision_disabled = !visible;
            }
        }
        let wingmate =
            interpolated_player_keyframe(&fighter_intercept::WINGMATE_KEYFRAMES, retail_frame);
        if let Some(id) = self.state.mission.wingmate {
            if let Some(object) = self.state.objects.get_mut(id) {
                apply_player_keyframe(object, wingmate);
                object.base.shape = ShapeId::EMPTY;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
            }
        }
    }

    fn update_fighter_intercept_targets(
        &mut self,
        retail_frame: u16,
        player_position: Vector3,
        previous_player_position: Vector3,
    ) {
        for actor in FighterInterceptActor::ALL {
            let actor_index = actor.index();
            if retail_frame >= fighter_intercept_fighters::DEPARTURE_RETAIL_FRAMES[actor_index] {
                let actor_id = *self.fighter_intercept_actors.slot_mut(actor);
                let destruction_in_progress = actor_id
                    .and_then(|id| self.state.objects.get(id))
                    .is_some_and(|object| {
                        object.base.explosion_timer > 0 || object.base.flags.exploding
                    });
                if destruction_in_progress {
                    continue;
                }
                if let Some(object) = self.fighter_intercept_actors.slot_mut(actor).take() {
                    self.state.objects.remove(object);
                    self.state.mission.score = self
                        .state
                        .mission
                        .score
                        .saturating_add(u32::from(MISSION_ENCOUNTER_HEALTH));
                    self.state.mission.objects_destroyed =
                        self.state.mission.objects_destroyed.saturating_add(1);
                }
                continue;
            }
            let Some(id) = *self.fighter_intercept_actors.slot_mut(actor) else {
                continue;
            };
            let Some(object) = self.state.objects.get_mut(id) else {
                continue;
            };
            if object.base.explosion_timer > 0 || object.base.flags.exploding {
                continue;
            }
            let ObjectActivity::FighterInterceptFlight(mut flight) = object.extension.activity
            else {
                debug_assert!(
                    false,
                    "fighter-interception target lacks typed flight state"
                );
                continue;
            };
            for &action in fighter_intercept_fighters::actions(retail_frame, actor_index) {
                apply_fighter_intercept_action(
                    object,
                    &mut flight,
                    action,
                    previous_player_position,
                    player_position,
                );
            }
            object.extension.activity = ObjectActivity::FighterInterceptFlight(flight);
        }
    }

    fn update_fighter_intercept_projectiles(
        &mut self,
        retail_frame: u16,
        player_position: Vector3,
        previous_player_position: Vector3,
    ) -> Result<(), Error> {
        for track_index in 0..fighter_intercept_projectiles::PROJECTILE_COUNT {
            let descriptor = fighter_intercept_projectiles::descriptor(track_index)
                .expect("fighter-intercept projectile descriptor exists");
            let active_index = self
                .fighter_intercept_projectiles
                .iter()
                .position(|projectile| projectile.track_index == track_index);
            if retail_frame < descriptor.start_retail_frame
                || retail_frame > descriptor.end_retail_frame
            {
                if retail_frame > descriptor.end_retail_frame {
                    if let Some(index) = active_index {
                        let projectile = self.fighter_intercept_projectiles.swap_remove(index);
                        self.state.objects.remove(projectile.object);
                    }
                }
                continue;
            }

            let projectile_id = if let Some(index) = active_index {
                self.fighter_intercept_projectiles[index].object
            } else {
                let mut projectile = Object::new(
                    ObjectKind::Projectile,
                    ShapeId::ENEMY_LASER,
                    Behavior::Projectile,
                );
                projectile.base.weapon = WeaponKind::EnemyLaser;
                projectile.base.hit_points = SF2_HOSTILE_LASER_HEALTH;
                projectile.base.attack_power = SF2_HOSTILE_LASER_ATTACK_POWER;
                projectile.base.collision_class = CollisionClass::EnemyWeapon;
                projectile.base.flags.casts_shadow = false;
                projectile.base.position = descriptor.initial_pose.position;
                projectile.base.pitch = Angle::from_units(descriptor.initial_pose.pitch);
                projectile.base.yaw = Angle::from_units(descriptor.initial_pose.yaw);
                projectile.base.roll = Angle::from_units(descriptor.initial_pose.roll);
                projectile.base.speed = descriptor.initial_pose.speed;
                projectile.extension.activity =
                    ObjectActivity::HostileProjectileFlight(HostileProjectileFlightState {
                        phase: HostileProjectileFlightPhase::Homing,
                        motion_steps_elapsed: 0,
                        movement_phase: HostileProjectileMovementPhase::Ready,
                    });
                let projectile_id = self
                    .state
                    .objects
                    .allocate(projectile)
                    .ok_or(Error::ObjectCapacityReached)?;
                self.fighter_intercept_projectiles
                    .push(ActiveFighterInterceptProjectile {
                        track_index,
                        object: projectile_id,
                    });
                projectile_id
            };

            if let Some(projectile) = self.state.objects.get_mut(projectile_id) {
                let ObjectActivity::HostileProjectileFlight(mut flight) =
                    projectile.extension.activity
                else {
                    continue;
                };
                for &action in fighter_intercept_projectiles::actions(track_index, retail_frame) {
                    apply_hostile_projectile_action(
                        projectile,
                        &mut flight,
                        action,
                        player_position,
                        previous_player_position,
                    );
                }
                projectile.extension.activity = ObjectActivity::HostileProjectileFlight(flight);
            }
        }
        Ok(())
    }

    fn update_pigma_presentation(&mut self, retail_frame: u16) {
        let primary_flight_craft_shape = self.primary_flight_craft_shape();
        let camera = interpolated_camera_keyframe(&pigma_duel::CAMERA_KEYFRAMES, retail_frame);
        self.state.camera.position = camera.position;
        self.state.camera.rotation.pitch = Angle::from_units(camera.pitch);
        self.state.camera.rotation.yaw = Angle::from_units(camera.yaw);
        self.state.camera.rotation.roll = Angle::from_units(camera.roll);

        let player = interpolated_player_keyframe(&pigma_duel::PLAYER_KEYFRAMES, retail_frame);
        if let Some(primary) = self.state.mission.primary_player {
            if let Some(object) = self.state.objects.get_mut(primary) {
                apply_player_keyframe(object, player);
                let visible = retail_frame >= PIGMA_PLAYER_REVEAL_RETAIL_FRAME;
                object.base.shape = if visible {
                    primary_flight_craft_shape
                } else {
                    ShapeId::EMPTY
                };
                object.base.flags.visible = visible;
                object.base.flags.collision_disabled = !visible;
            }
        }
        let wingmate = interpolated_player_keyframe(&pigma_duel::WINGMATE_KEYFRAMES, retail_frame);
        if let Some(id) = self.state.mission.wingmate {
            if let Some(object) = self.state.objects.get_mut(id) {
                apply_player_keyframe(object, wingmate);
                object.base.shape = ShapeId::EMPTY;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
            }
        }
    }

    fn update_pigma_rival(
        &mut self,
        retail_frame: u16,
        player_position: Vector3,
        previous_player_position: Vector3,
    ) {
        if retail_frame < pigma_duel_rival::PRESENTATION_START_RETAIL_FRAME {
            return;
        }
        if retail_frame >= pigma_duel_rival::DEPARTURE_RETAIL_FRAME {
            let destruction_in_progress = self
                .pigma_rival
                .and_then(|id| self.state.objects.get(id))
                .is_some_and(|object| {
                    object.base.explosion_timer > 0 || object.base.flags.exploding
                });
            if destruction_in_progress {
                return;
            }
            if let Some(rival) = self.pigma_rival.take() {
                self.state.objects.remove(rival);
                self.state.mission.score =
                    self.state.mission.score.saturating_add(PIGMA_SCORE_AWARD);
                self.state.mission.objects_destroyed =
                    self.state.mission.objects_destroyed.saturating_add(1);
            }
            return;
        }
        let Some(rival) = self.pigma_rival else {
            return;
        };
        let Some(object) = self.state.objects.get_mut(rival) else {
            return;
        };
        if object.base.explosion_timer > 0 || object.base.flags.exploding {
            return;
        }
        let ObjectActivity::PigmaRivalFlight(mut flight) = object.extension.activity else {
            return;
        };
        if retail_frame == pigma_duel_rival::PRESENTATION_START_RETAIL_FRAME {
            object.base.flags.active = true;
            object.base.flags.visible = true;
            object.base.flags.collision_disabled = false;
            object.base.position = Vector3::default();
            object.base.pitch = Angle::ZERO;
            object.base.yaw = Angle::ZERO;
            object.base.roll = Angle::ZERO;
            object.base.speed = 0;
            object.base.velocity = Vector3::default();
        }
        if retail_frame == pigma_duel_rival::FLIGHT_START_RETAIL_FRAME
            && flight.phase == PigmaRivalFlightPhase::AwaitingEntrance
        {
            let pose = pigma_duel_rival::INITIAL_POSE;
            object.base.flags.active = true;
            object.base.flags.visible = true;
            object.base.flags.collision_disabled = false;
            object.base.position = pose.position;
            object.base.pitch = Angle::from_units(pose.pitch);
            object.base.yaw = Angle::from_units(pose.yaw);
            object.base.roll = Angle::from_units(pose.roll);
            object.base.speed = pose.speed;
            object.base.velocity = Vector3::default();
        }
        for &action in pigma_duel_rival::actions(retail_frame) {
            apply_pigma_rival_action(
                object,
                &mut flight,
                action,
                player_position,
                previous_player_position,
            );
        }
        flight.earlier_player_altitude = previous_player_position.y;
        object.extension.activity = ObjectActivity::PigmaRivalFlight(flight);
    }

    fn update_pigma_projectiles(
        &mut self,
        retail_frame: u16,
        player_position: Vector3,
        previous_player_position: Vector3,
    ) -> Result<(), Error> {
        for track_index in 0..pigma_duel_projectiles::PROJECTILE_COUNT {
            let descriptor = pigma_duel_projectiles::descriptor(track_index)
                .expect("Pigma projectile descriptor exists");
            let active_index = self
                .pigma_projectiles
                .iter()
                .position(|projectile| projectile.track_index == track_index);
            if retail_frame < descriptor.start_retail_frame
                || retail_frame > descriptor.end_retail_frame
            {
                if retail_frame > descriptor.end_retail_frame {
                    if let Some(index) = active_index {
                        let projectile = self.pigma_projectiles.swap_remove(index);
                        self.state.objects.remove(projectile.object);
                    }
                }
                continue;
            }

            let projectile_id = if let Some(index) = active_index {
                self.pigma_projectiles[index].object
            } else {
                let mut projectile = Object::new(
                    ObjectKind::Projectile,
                    ShapeId::ENEMY_LASER,
                    Behavior::Projectile,
                );
                projectile.base.weapon = WeaponKind::EnemyLaser;
                projectile.base.hit_points = SF2_HOSTILE_LASER_HEALTH;
                projectile.base.attack_power = SF2_HOSTILE_LASER_ATTACK_POWER;
                projectile.base.collision_class = CollisionClass::EnemyWeapon;
                projectile.base.flags.casts_shadow = false;
                projectile.base.position = descriptor.initial_pose.position;
                projectile.base.pitch = Angle::from_units(descriptor.initial_pose.pitch);
                projectile.base.yaw = Angle::from_units(descriptor.initial_pose.yaw);
                projectile.base.roll = Angle::from_units(descriptor.initial_pose.roll);
                projectile.base.speed = descriptor.initial_pose.speed;
                projectile.extension.activity =
                    ObjectActivity::HostileProjectileFlight(HostileProjectileFlightState {
                        phase: HostileProjectileFlightPhase::Homing,
                        motion_steps_elapsed: 0,
                        movement_phase: HostileProjectileMovementPhase::Ready,
                    });
                let projectile_id = self
                    .state
                    .objects
                    .allocate(projectile)
                    .ok_or(Error::ObjectCapacityReached)?;
                self.pigma_projectiles.push(ActivePigmaProjectile {
                    track_index,
                    object: projectile_id,
                });
                projectile_id
            };

            if let Some(projectile) = self.state.objects.get_mut(projectile_id) {
                let ObjectActivity::HostileProjectileFlight(mut flight) =
                    projectile.extension.activity
                else {
                    continue;
                };
                for &action in pigma_duel_projectiles::actions(track_index, retail_frame) {
                    apply_hostile_projectile_action(
                        projectile,
                        &mut flight,
                        action,
                        player_position,
                        previous_player_position,
                    );
                }
                projectile.extension.activity = ObjectActivity::HostileProjectileFlight(flight);
            }
        }
        Ok(())
    }

    fn update_leon_presentation(&mut self, retail_frame: u16) {
        let primary_flight_craft_shape = self.primary_flight_craft_shape();
        let camera = interpolated_camera_keyframe(&leon_duel::CAMERA_KEYFRAMES, retail_frame);
        self.state.camera.position = camera.position;
        self.state.camera.rotation.pitch = Angle::from_units(camera.pitch);
        self.state.camera.rotation.yaw = Angle::from_units(camera.yaw);
        self.state.camera.rotation.roll = Angle::from_units(camera.roll);

        let player = interpolated_player_keyframe(&leon_duel::PLAYER_KEYFRAMES, retail_frame);
        if let Some(primary) = self.state.mission.primary_player {
            if let Some(object) = self.state.objects.get_mut(primary) {
                apply_player_keyframe(object, player);
                let visible = retail_frame >= LEON_PLAYER_REVEAL_RETAIL_FRAME;
                object.base.shape = if visible {
                    primary_flight_craft_shape
                } else {
                    ShapeId::EMPTY
                };
                object.base.flags.visible = visible;
                object.base.flags.collision_disabled = !visible;
            }
        }
        let wingmate = interpolated_player_keyframe(&leon_duel::WINGMATE_KEYFRAMES, retail_frame);
        if let Some(id) = self.state.mission.wingmate {
            if let Some(object) = self.state.objects.get_mut(id) {
                apply_player_keyframe(object, wingmate);
                object.base.shape = ShapeId::EMPTY;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
            }
        }
    }

    fn update_pressure_fighter_presentation(&mut self, retail_frame: u16) {
        self.update_pressure_player_presentation(
            &pressure_fighters::CAMERA_KEYFRAMES,
            &pressure_fighters::PLAYER_KEYFRAMES,
            &pressure_fighters::WINGMATE_KEYFRAMES,
            FIGHTER_INTERCEPT_PLAYER_REVEAL_RETAIL_FRAME,
            retail_frame,
        );
    }

    fn update_leon_pressure_presentation(&mut self, retail_frame: u16) {
        self.update_pressure_player_presentation(
            &leon_pressure::CAMERA_KEYFRAMES,
            &leon_pressure::PLAYER_KEYFRAMES,
            &leon_pressure::WINGMATE_KEYFRAMES,
            LEON_PLAYER_REVEAL_RETAIL_FRAME,
            retail_frame,
        );
    }

    fn update_final_rival_presentation(&mut self, retail_frame: u16) {
        let (camera, player, wingmate) = match self.state.mission.visit {
            MissionVisit::FinalPursuer => (
                final_pursuer::CAMERA_KEYFRAMES.as_slice(),
                final_pursuer::PLAYER_KEYFRAMES.as_slice(),
                final_pursuer::WINGMATE_KEYFRAMES.as_slice(),
            ),
            MissionVisit::WolfBlockade => (
                wolf_blockade::CAMERA_KEYFRAMES.as_slice(),
                wolf_blockade::PLAYER_KEYFRAMES.as_slice(),
                wolf_blockade::WINGMATE_KEYFRAMES.as_slice(),
            ),
            _ => unreachable!("final rival presentation requires a final rival visit"),
        };
        self.update_pressure_player_presentation(
            camera,
            player,
            wingmate,
            LEON_PLAYER_REVEAL_RETAIL_FRAME,
            retail_frame,
        );
    }

    fn update_astropolis_presentation(&mut self, retail_frame: u16) {
        let primary_flight_craft_shape = self.primary_flight_craft_shape();
        let camera =
            interpolated_camera_keyframe(&astropolis_entry::CAMERA_KEYFRAMES, retail_frame);
        self.state.camera.position = camera.position;
        self.state.camera.rotation.pitch = Angle::from_units(camera.pitch);
        self.state.camera.rotation.yaw = Angle::from_units(camera.yaw);
        self.state.camera.rotation.roll = Angle::from_units(camera.roll);

        let player =
            interpolated_player_keyframe(&astropolis_entry::PLAYER_KEYFRAMES, retail_frame);
        if let Some(primary) = self.state.mission.primary_player {
            if let Some(object) = self.state.objects.get_mut(primary) {
                apply_player_keyframe(object, player);
                let visible = retail_frame >= ASTROPOLIS_BASE_ENTRY_RETAIL_FRAME;
                object.base.shape = if visible {
                    primary_flight_craft_shape
                } else {
                    ShapeId::EMPTY
                };
                object.base.flags.visible = visible;
                object.base.flags.collision_disabled = !visible;
            }
        }
        let wingmate =
            interpolated_player_keyframe(&astropolis_entry::WINGMATE_KEYFRAMES, retail_frame);
        if let Some(id) = self.state.mission.wingmate {
            if let Some(object) = self.state.objects.get_mut(id) {
                apply_player_keyframe(object, wingmate);
                object.base.shape = ShapeId::EMPTY;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
            }
        }
    }

    fn update_pressure_player_presentation(
        &mut self,
        camera_keyframes: &[MissionCameraKeyframe],
        player_keyframes: &[MissionPlayerKeyframe],
        wingmate_keyframes: &[MissionPlayerKeyframe],
        reveal_retail_frame: u16,
        retail_frame: u16,
    ) {
        let primary_flight_craft_shape = self.primary_flight_craft_shape();
        let camera = interpolated_camera_keyframe(camera_keyframes, retail_frame);
        self.state.camera.position = camera.position;
        self.state.camera.rotation.pitch = Angle::from_units(camera.pitch);
        self.state.camera.rotation.yaw = Angle::from_units(camera.yaw);
        self.state.camera.rotation.roll = Angle::from_units(camera.roll);

        let player = interpolated_player_keyframe(player_keyframes, retail_frame);
        if let Some(primary) = self.state.mission.primary_player {
            if let Some(object) = self.state.objects.get_mut(primary) {
                apply_player_keyframe(object, player);
                let visible = retail_frame >= reveal_retail_frame;
                object.base.shape = if visible {
                    primary_flight_craft_shape
                } else {
                    ShapeId::EMPTY
                };
                object.base.flags.visible = visible;
                object.base.flags.collision_disabled = !visible;
            }
        }
        let wingmate = interpolated_player_keyframe(wingmate_keyframes, retail_frame);
        if let Some(id) = self.state.mission.wingmate {
            if let Some(object) = self.state.objects.get_mut(id) {
                apply_player_keyframe(object, wingmate);
                object.base.shape = ShapeId::EMPTY;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
            }
        }
    }

    fn update_pressure_fighter_actors(&mut self, retail_frame: u16) {
        for (slot, keyframes) in self
            .pressure_fighter_actors
            .slots_mut()
            .into_iter()
            .zip(pressure_fighters::ATTACKER_KEYFRAME_TRACKS)
        {
            Self::update_pressure_actor(&mut self.state, slot, keyframes, retail_frame);
        }
    }

    fn update_leon_pressure_rival(&mut self, retail_frame: u16) {
        Self::update_pressure_actor(
            &mut self.state,
            &mut self.leon_rival,
            &leon_pressure::RIVAL_KEYFRAMES,
            retail_frame,
        );
    }

    fn update_final_rival_actor(
        &mut self,
        retail_frame: u16,
        player_position: Vector3,
        previous_player_position: Vector3,
    ) {
        let plan = match self.state.mission.visit {
            MissionVisit::FinalPursuer => final_rivals_flight::FINAL_PURSUER,
            MissionVisit::WolfBlockade => final_rivals_flight::WOLF_BLOCKADE,
            _ => unreachable!("final rival actor requires a final rival visit"),
        };
        if retail_frame < plan.presentation_start_retail_frame {
            return;
        }
        if retail_frame >= plan.departure_retail_frame {
            let destruction_in_progress = self
                .final_rival
                .and_then(|id| self.state.objects.get(id))
                .is_some_and(|object| {
                    object.base.explosion_timer > 0 || object.base.flags.exploding
                });
            if destruction_in_progress {
                return;
            }
            if let Some(rival) = self.final_rival.take() {
                self.state.objects.remove(rival);
            }
            return;
        }
        let Some(rival) = self.final_rival else {
            return;
        };
        let Some(object) = self.state.objects.get_mut(rival) else {
            return;
        };
        if object.base.explosion_timer > 0 || object.base.flags.exploding {
            return;
        }
        let ObjectActivity::FinalRivalFlight(mut flight) = object.extension.activity else {
            return;
        };
        if plan.is_hidden(retail_frame) {
            object.base.flags.active = false;
            object.base.flags.visible = false;
            object.base.flags.collision_disabled = true;
            object.base.velocity = Vector3::default();
            return;
        }
        if retail_frame == plan.presentation_start_retail_frame || !object.base.flags.active {
            object.base.flags.active = true;
            object.base.flags.visible = true;
            object.base.flags.collision_disabled = false;
            object.base.position = Vector3::default();
            object.base.pitch = Angle::ZERO;
            object.base.yaw = Angle::ZERO;
            object.base.roll = Angle::ZERO;
            object.base.speed = 0;
            object.base.velocity = Vector3::default();
        }
        if retail_frame == plan.flight_start_retail_frame
            && flight.phase == FinalRivalFlightPhase::AwaitingEntrance
        {
            let pose = plan.initial_pose;
            object.base.flags.active = true;
            object.base.flags.visible = true;
            object.base.flags.collision_disabled = false;
            object.base.position = pose.position;
            object.base.pitch = Angle::from_units(pose.pitch);
            object.base.yaw = Angle::from_units(pose.yaw);
            object.base.roll = Angle::from_units(pose.roll);
            object.base.speed = pose.speed;
            object.base.velocity = Vector3::default();
        }
        for &action in plan.actions(retail_frame) {
            apply_final_rival_action(
                object,
                &mut flight,
                action,
                player_position,
                previous_player_position,
            );
        }
        object.extension.activity = ObjectActivity::FinalRivalFlight(flight);
    }

    fn update_pressure_actor(
        state: &mut GameState,
        actor: &mut Option<ObjectId>,
        keyframes: &[MissionActorKeyframe],
        retail_frame: u16,
    ) {
        if retail_frame < keyframes[0].retail_frame {
            return;
        }
        let (start, end) =
            enclosing_keyframes(keyframes, retail_frame, |keyframe| keyframe.retail_frame);
        let presentation = if retail_frame >= end.retail_frame {
            end.presentation
        } else {
            match (start.presentation, end.presentation) {
                (
                    MissionActorPresentation::Present(start_pose),
                    MissionActorPresentation::Present(end_pose),
                ) => MissionActorPresentation::Present(interpolate_encounter_pose(
                    start_pose,
                    end_pose,
                    retail_frame.saturating_sub(start.retail_frame),
                    end.retail_frame.saturating_sub(start.retail_frame),
                )),
                (presentation, _) => presentation,
            }
        };
        if presentation == MissionActorPresentation::Departed {
            let destruction_in_progress =
                actor
                    .and_then(|id| state.objects.get(id))
                    .is_some_and(|object| {
                        object.base.explosion_timer > 0 || object.base.flags.exploding
                    });
            if destruction_in_progress {
                return;
            }
            if let Some(object) = actor.take() {
                state.objects.remove(object);
            }
            return;
        }
        let Some(object) = actor.and_then(|id| state.objects.get_mut(id)) else {
            return;
        };
        if object.base.explosion_timer > 0 || object.base.flags.exploding {
            return;
        }
        match presentation {
            MissionActorPresentation::Present(pose) => {
                object.base.flags.active = true;
                object.base.flags.visible = true;
                object.base.flags.collision_disabled = false;
                object.base.position = pose.position;
                object.base.pitch = Angle::from_units(pose.pitch);
                object.base.yaw = Angle::from_units(pose.yaw);
                object.base.roll = Angle::from_units(pose.roll);
                object.base.speed = pose.speed;
                object.base.velocity = Vector3::default();
            }
            MissionActorPresentation::Inactive => {
                object.base.flags.active = false;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
                object.base.velocity = Vector3::default();
            }
            MissionActorPresentation::Departed => unreachable!(),
        }
    }

    fn update_leon_rival(&mut self, retail_frame: u16) {
        if retail_frame < leon_duel_rival::PRESENTATION_START_RETAIL_FRAME {
            return;
        }
        if retail_frame >= leon_duel_rival::DEPARTURE_RETAIL_FRAME {
            let destruction_in_progress = self
                .leon_rival
                .and_then(|id| self.state.objects.get(id))
                .is_some_and(|object| {
                    object.base.explosion_timer > 0 || object.base.flags.exploding
                });
            if destruction_in_progress {
                return;
            }
            if let Some(rival) = self.leon_rival.take() {
                self.state.objects.remove(rival);
                self.state.mission.score =
                    self.state.mission.score.saturating_add(LEON_SCORE_AWARD);
                self.state.mission.objects_destroyed =
                    self.state.mission.objects_destroyed.saturating_add(1);
            }
            return;
        }
        let Some(rival) = self.leon_rival else {
            return;
        };
        let Some(object) = self.state.objects.get_mut(rival) else {
            return;
        };
        if object.base.explosion_timer > 0 || object.base.flags.exploding {
            return;
        }
        let ObjectActivity::LeonRivalFlight(mut flight) = object.extension.activity else {
            return;
        };
        if retail_frame == leon_duel_rival::PRESENTATION_START_RETAIL_FRAME {
            object.base.flags.active = true;
            object.base.flags.visible = true;
            object.base.flags.collision_disabled = false;
            object.base.position = Vector3::default();
            object.base.pitch = Angle::ZERO;
            object.base.yaw = Angle::ZERO;
            object.base.roll = Angle::ZERO;
            object.base.speed = 0;
            object.base.velocity = Vector3::default();
        }
        if retail_frame == leon_duel_rival::FLIGHT_START_RETAIL_FRAME
            && flight.phase == LeonRivalFlightPhase::AwaitingEntrance
        {
            let pose = leon_duel_rival::INITIAL_POSE;
            object.base.flags.active = true;
            object.base.flags.visible = true;
            object.base.flags.collision_disabled = false;
            object.base.position = pose.position;
            object.base.pitch = Angle::from_units(pose.pitch);
            object.base.yaw = Angle::from_units(pose.yaw);
            object.base.roll = Angle::from_units(pose.roll);
            object.base.speed = pose.speed;
            object.base.velocity = Vector3::default();
        }
        for &action in leon_duel_rival::actions(retail_frame) {
            apply_leon_rival_action(object, &mut flight, action);
        }
        object.extension.activity = ObjectActivity::LeonRivalFlight(flight);
    }

    fn update_mirage_dragon_presentation(&mut self, retail_frame: u16) {
        let primary_flight_craft_shape = self.primary_flight_craft_shape();
        let camera = interpolated_camera_keyframe(&mirage_dragon::CAMERA_KEYFRAMES, retail_frame);
        self.state.camera.position = camera.position;
        self.state.camera.rotation.pitch = Angle::from_units(camera.pitch);
        self.state.camera.rotation.yaw = Angle::from_units(camera.yaw);
        self.state.camera.rotation.roll = Angle::from_units(camera.roll);

        let player = interpolated_player_keyframe(&mirage_dragon::PLAYER_KEYFRAMES, retail_frame);
        if let Some(primary) = self.state.mission.primary_player {
            if let Some(object) = self.state.objects.get_mut(primary) {
                apply_player_keyframe(object, player);
                let visible = retail_frame >= MIRAGE_DRAGON_PLAYER_REVEAL_RETAIL_FRAME;
                object.base.shape = if visible {
                    primary_flight_craft_shape
                } else {
                    ShapeId::EMPTY
                };
                object.base.flags.visible = visible;
                object.base.flags.collision_disabled = !visible;
            }
        }
        let wingmate =
            interpolated_player_keyframe(&mirage_dragon::WINGMATE_KEYFRAMES, retail_frame);
        if let Some(id) = self.state.mission.wingmate {
            if let Some(object) = self.state.objects.get_mut(id) {
                apply_player_keyframe(object, wingmate);
                object.base.shape = ShapeId::EMPTY;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
            }
        }
    }

    fn update_mirage_dragon_actor(&mut self, retail_frame: u16) {
        let keyframes = &mirage_dragon::RIVAL_KEYFRAMES;
        if retail_frame < keyframes[0].retail_frame {
            return;
        }
        let (start, end) =
            enclosing_keyframes(keyframes, retail_frame, |keyframe| keyframe.retail_frame);
        let presentation = if retail_frame >= end.retail_frame {
            end.presentation
        } else {
            match (start.presentation, end.presentation) {
                (
                    MissionActorPresentation::Present(start_pose),
                    MissionActorPresentation::Present(end_pose),
                ) => MissionActorPresentation::Present(interpolate_encounter_pose(
                    start_pose,
                    end_pose,
                    retail_frame.saturating_sub(start.retail_frame),
                    end.retail_frame.saturating_sub(start.retail_frame),
                )),
                (presentation, _) => presentation,
            }
        };
        if presentation == MissionActorPresentation::Departed {
            let destruction_in_progress = self
                .mirage_dragon
                .and_then(|id| self.state.objects.get(id))
                .is_some_and(|object| {
                    object.base.explosion_timer > 0 || object.base.flags.exploding
                });
            if destruction_in_progress {
                return;
            }
            if let Some(boss) = self.mirage_dragon.take() {
                self.state.objects.remove(boss);
                self.state.mission.score = self
                    .state
                    .mission
                    .score
                    .saturating_add(MIRAGE_DRAGON_SCORE_AWARD);
                self.state.mission.objects_destroyed =
                    self.state.mission.objects_destroyed.saturating_add(1);
            }
            return;
        }
        let Some(boss) = self.mirage_dragon else {
            return;
        };
        let Some(object) = self.state.objects.get_mut(boss) else {
            return;
        };
        if object.base.explosion_timer > 0 || object.base.flags.exploding {
            return;
        }
        match presentation {
            MissionActorPresentation::Present(pose) => {
                object.base.flags.active = true;
                object.base.flags.visible = true;
                object.base.flags.collision_disabled = false;
                object.base.position = pose.position;
                object.base.pitch = Angle::from_units(pose.pitch);
                object.base.yaw = Angle::from_units(pose.yaw);
                object.base.roll = Angle::from_units(pose.roll);
                object.base.speed = pose.speed;
                object.base.velocity = Vector3::default();
            }
            MissionActorPresentation::Inactive => {
                object.base.flags.active = false;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
                object.base.velocity = Vector3::default();
            }
            MissionActorPresentation::Departed => unreachable!(),
        }
    }

    fn update_mirage_dragon_segments(&mut self, retail_frame: u16) {
        for (segment, keyframes) in self
            .mirage_dragon_body
            .iter_mut()
            .zip(mirage_dragon_segments::BODY_SEGMENT_KEYFRAME_TRACKS)
        {
            Self::update_mirage_dragon_part(&mut self.state, segment, keyframes, retail_frame);
        }
        Self::update_mirage_dragon_part(
            &mut self.state,
            &mut self.mirage_dragon_tail,
            mirage_dragon_segments::TAIL_KEYFRAMES,
            retail_frame,
        );
    }

    fn update_mirage_dragon_part(
        state: &mut GameState,
        part: &mut Option<ObjectId>,
        keyframes: &[MissionActorKeyframe],
        retail_frame: u16,
    ) {
        if retail_frame < keyframes[0].retail_frame {
            return;
        }
        let (start, end) =
            enclosing_keyframes(keyframes, retail_frame, |keyframe| keyframe.retail_frame);
        let presentation = if retail_frame >= end.retail_frame {
            end.presentation
        } else {
            match (start.presentation, end.presentation) {
                (
                    MissionActorPresentation::Present(start_pose),
                    MissionActorPresentation::Present(end_pose),
                ) => MissionActorPresentation::Present(interpolate_encounter_pose(
                    start_pose,
                    end_pose,
                    retail_frame.saturating_sub(start.retail_frame),
                    end.retail_frame.saturating_sub(start.retail_frame),
                )),
                (presentation, _) => presentation,
            }
        };
        if presentation == MissionActorPresentation::Departed {
            if let Some(object) = part.take() {
                state.objects.remove(object);
            }
            return;
        }
        let Some(object) = part.and_then(|id| state.objects.get_mut(id)) else {
            return;
        };
        if object.base.explosion_timer > 0 || object.base.flags.exploding {
            return;
        }
        match presentation {
            MissionActorPresentation::Present(pose) => {
                object.base.flags.active = true;
                object.base.flags.visible = true;
                object.base.flags.collision_disabled = false;
                object.base.position = pose.position;
                object.base.pitch = Angle::from_units(pose.pitch);
                object.base.yaw = Angle::from_units(pose.yaw);
                object.base.roll = Angle::from_units(pose.roll);
                object.base.speed = pose.speed;
                object.base.velocity = Vector3::default();
            }
            MissionActorPresentation::Inactive => {
                object.base.flags.active = false;
                object.base.flags.visible = false;
                object.base.flags.collision_disabled = true;
                object.base.velocity = Vector3::default();
            }
            MissionActorPresentation::Departed => unreachable!(),
        }
    }

    fn update_leon_projectiles(&mut self, retail_frame: u16) -> Result<(), Error> {
        for (track_index, keyframes) in leon_duel::ENEMY_LASER_KEYFRAME_TRACKS
            .iter()
            .copied()
            .enumerate()
        {
            let start_frame = keyframes
                .first()
                .expect("Leon projectile trajectory is not empty")
                .retail_frame;
            let end_frame = keyframes
                .last()
                .expect("Leon projectile trajectory is not empty")
                .retail_frame;
            let active_index = self
                .leon_projectiles
                .iter()
                .position(|projectile| projectile.track_index == track_index);
            if retail_frame < start_frame || retail_frame > end_frame {
                if retail_frame > end_frame {
                    if let Some(index) = active_index {
                        let projectile = self.leon_projectiles.swap_remove(index);
                        self.state.objects.remove(projectile.object);
                    }
                }
                continue;
            }

            let projectile_id = if let Some(index) = active_index {
                self.leon_projectiles[index].object
            } else {
                let mut projectile = Object::new(
                    ObjectKind::Projectile,
                    ShapeId::ENEMY_LASER,
                    Behavior::MissionScriptedProjectile,
                );
                projectile.base.weapon = WeaponKind::EnemyLaser;
                projectile.base.hit_points = SF2_HOSTILE_LASER_HEALTH;
                projectile.base.attack_power = SF2_HOSTILE_LASER_ATTACK_POWER;
                projectile.base.collision_class = CollisionClass::EnemyWeapon;
                projectile.base.flags.casts_shadow = false;
                let projectile_id = self
                    .state
                    .objects
                    .allocate(projectile)
                    .ok_or(Error::ObjectCapacityReached)?;
                self.leon_projectiles.push(ActiveLeonProjectile {
                    track_index,
                    object: projectile_id,
                });
                projectile_id
            };

            let pose = mission_projectile_pose(keyframes, retail_frame);
            if let Some(projectile) = self.state.objects.get_mut(projectile_id) {
                projectile.base.position = pose.position;
                projectile.base.pitch = Angle::from_units(pose.pitch);
                projectile.base.yaw = Angle::from_units(pose.yaw);
                projectile.base.roll = Angle::from_units(pose.roll);
                projectile.base.speed = pose.speed;
                projectile.base.velocity = Vector3::default();
            }
        }
        Ok(())
    }

    fn update_pressure_fighter_projectiles(&mut self, retail_frame: u16) -> Result<(), Error> {
        Self::update_pressure_projectile_tracks(
            &mut self.state,
            &mut self.pressure_fighter_projectiles,
            &pressure_fighters::ENEMY_LASER_KEYFRAME_TRACKS,
            retail_frame,
        )
    }

    fn update_leon_pressure_projectiles(&mut self, retail_frame: u16) -> Result<(), Error> {
        Self::update_pressure_projectile_tracks(
            &mut self.state,
            &mut self.leon_pressure_projectiles,
            &leon_pressure::ENEMY_LASER_KEYFRAME_TRACKS,
            retail_frame,
        )
    }

    fn update_final_rival_projectiles(&mut self, retail_frame: u16) -> Result<(), Error> {
        let tracks = match self.state.mission.visit {
            MissionVisit::FinalPursuer => final_pursuer::ENEMY_LASER_KEYFRAME_TRACKS.as_slice(),
            MissionVisit::WolfBlockade => wolf_blockade::ENEMY_LASER_KEYFRAME_TRACKS.as_slice(),
            _ => unreachable!("final rival projectiles require a final rival visit"),
        };
        Self::update_pressure_projectile_tracks(
            &mut self.state,
            &mut self.final_rival_projectiles,
            tracks,
            retail_frame,
        )
    }

    fn update_pressure_projectile_tracks(
        state: &mut GameState,
        active_projectiles: &mut Vec<ActivePressureProjectile>,
        tracks: &[&[MissionProjectileKeyframe]],
        retail_frame: u16,
    ) -> Result<(), Error> {
        for (track_index, keyframes) in tracks.iter().copied().enumerate() {
            let start_frame = keyframes
                .first()
                .expect("pressure projectile trajectory is not empty")
                .retail_frame;
            let end_frame = keyframes
                .last()
                .expect("pressure projectile trajectory is not empty")
                .retail_frame;
            let active_index = active_projectiles
                .iter()
                .position(|projectile| projectile.track_index == track_index);
            if retail_frame < start_frame || retail_frame > end_frame {
                if retail_frame > end_frame {
                    if let Some(index) = active_index {
                        let projectile = active_projectiles.swap_remove(index);
                        state.objects.remove(projectile.object);
                    }
                }
                continue;
            }

            let projectile_id = if let Some(index) = active_index {
                active_projectiles[index].object
            } else {
                let mut projectile = Object::new(
                    ObjectKind::Projectile,
                    ShapeId::ENEMY_LASER,
                    Behavior::MissionScriptedProjectile,
                );
                projectile.base.weapon = WeaponKind::EnemyLaser;
                projectile.base.hit_points = SF2_HOSTILE_LASER_HEALTH;
                projectile.base.attack_power = SF2_HOSTILE_LASER_ATTACK_POWER;
                projectile.base.collision_class = CollisionClass::EnemyWeapon;
                projectile.base.flags.casts_shadow = false;
                let projectile_id = state
                    .objects
                    .allocate(projectile)
                    .ok_or(Error::ObjectCapacityReached)?;
                active_projectiles.push(ActivePressureProjectile {
                    track_index,
                    object: projectile_id,
                });
                projectile_id
            };

            let pose = mission_projectile_pose(keyframes, retail_frame);
            if let Some(projectile) = state.objects.get_mut(projectile_id) {
                projectile.base.position = pose.position;
                projectile.base.pitch = Angle::from_units(pose.pitch);
                projectile.base.yaw = Angle::from_units(pose.yaw);
                projectile.base.roll = Angle::from_units(pose.roll);
                projectile.base.speed = pose.speed;
                projectile.base.velocity = Vector3::default();
            }
        }
        Ok(())
    }

    fn update_active_flight(
        &mut self,
        retail_frame: u16,
        weapons_enabled: bool,
    ) -> Result<(), Error> {
        let left = self.state.input.held.contains(Button::Left);
        let right = self.state.input.held.contains(Button::Right);
        let up = self.state.input.held.contains(Button::Up);
        let down = self.state.input.held.contains(Button::Down);
        let Some(primary_id) = self.state.mission.primary_player else {
            return Ok(());
        };

        if self.update_player_transformation(primary_id) {
            return Ok(());
        }
        if self.state.mission.player_craft_form == PlayerCraftForm::Walker {
            return self.update_active_walker(primary_id, retail_frame, weapons_enabled);
        }

        if left || right || up || down {
            self.state.mission.departed_certified_neutral_path = true;
        }
        let certified_end = match self.state.mission.visit {
            MissionVisit::OpeningEngagement => {
                opening_continuation::PLAYER_CERTIFIED_END_RETAIL_FRAME
            }
            MissionVisit::Reengagement => second_sortie::RETURN_RETAIL_FRAME,
            MissionVisit::MissileInterception => missile_interception::RETURN_RETAIL_FRAME,
            MissionVisit::FighterIntercept => fighter_intercept::RETURN_RETAIL_FRAME,
            MissionVisit::PigmaDuel => pigma_duel::RETURN_RETAIL_FRAME,
            MissionVisit::LeonDuel => leon_duel::RETURN_RETAIL_FRAME,
            MissionVisit::LeonPressure => leon_pressure::RETURN_RETAIL_FRAME,
            MissionVisit::MirageDragon => mirage_dragon::RETURN_RETAIL_FRAME,
            MissionVisit::RecurringAttackers => pressure_fighters::RETURN_RETAIL_FRAME,
            MissionVisit::FinalPursuer => final_pursuer::RETURN_RETAIL_FRAME,
            MissionVisit::WolfBlockade => wolf_blockade::RETURN_RETAIL_FRAME,
            MissionVisit::AstropolisAssault => astropolis_entry::LAST_RETAIL_FRAME,
            MissionVisit::TitaniaBase => TITANIA_MAP_READY_RETAIL_FRAME,
            MissionVisit::EladardBase => ELADARD_RETURN_RETAIL_FRAME,
            MissionVisit::FirstBattleCarrier | MissionVisit::SecondBattleCarrier => {
                CARRIER_MAP_READY_RETAIL_FRAME
            }
        };
        if retail_frame > certified_end {
            self.state.mission.departed_certified_neutral_path = true;
        }
        if !self.state.mission.departed_certified_neutral_path {
            match self.state.mission.visit {
                MissionVisit::OpeningEngagement => {
                    self.update_certified_neutral_flight(retail_frame)
                }
                MissionVisit::Reengagement => self.update_reengagement_presentation(retail_frame),
                MissionVisit::MissileInterception => {
                    self.update_interception_presentation(retail_frame)
                }
                MissionVisit::FighterIntercept => {
                    self.update_fighter_intercept_presentation(retail_frame)
                }
                MissionVisit::PigmaDuel => self.update_pigma_presentation(retail_frame),
                MissionVisit::LeonDuel => self.update_leon_presentation(retail_frame),
                MissionVisit::LeonPressure => self.update_leon_pressure_presentation(retail_frame),
                MissionVisit::MirageDragon => self.update_mirage_dragon_presentation(retail_frame),
                MissionVisit::RecurringAttackers => {
                    self.update_pressure_fighter_presentation(retail_frame)
                }
                MissionVisit::FinalPursuer | MissionVisit::WolfBlockade => {
                    self.update_final_rival_presentation(retail_frame)
                }
                MissionVisit::AstropolisAssault => {
                    self.update_astropolis_presentation(retail_frame)
                }
                MissionVisit::TitaniaBase
                | MissionVisit::EladardBase
                | MissionVisit::FirstBattleCarrier
                | MissionVisit::SecondBattleCarrier => {}
            }
            self.update_player_blaster(primary_id, weapons_enabled)?;
            return Ok(());
        }

        let position = self
            .state
            .objects
            .get(primary_id)
            .map(|player| player.base.position)
            .unwrap_or_default();
        let at_upper_boundary = position.y >= PLAYER_VERTICAL_UPPER_BOUND;
        let at_lower_boundary = position.y <= PLAYER_VERTICAL_LOWER_BOUND;
        let pitch_target = if up != down {
            if up {
                if at_upper_boundary {
                    PLAYER_BOUNDARY_PITCH_TARGET
                } else {
                    PLAYER_PITCH_TARGET
                }
            } else if at_lower_boundary {
                -PLAYER_BOUNDARY_PITCH_TARGET
            } else {
                -PLAYER_PITCH_TARGET
            }
        } else {
            0
        };
        let pitch_lean_target = if up != down {
            if up {
                if at_upper_boundary {
                    -PLAYER_BOUNDARY_PITCH_LEAN
                } else {
                    -PLAYER_PITCH_LEAN_LIMIT
                }
            } else if at_lower_boundary {
                PLAYER_BOUNDARY_PITCH_LEAN
            } else {
                PLAYER_PITCH_LEAN_LIMIT
            }
        } else {
            0
        };
        let flight = &mut self.state.mission.player_flight;
        flight.pitch_accumulator = chase_proportional(
            flight.pitch_accumulator,
            pitch_target,
            PLAYER_CONTROL_RESPONSE_SHIFT,
        );
        flight.pitch_lean =
            approach_i8(flight.pitch_lean, pitch_lean_target, PLAYER_PITCH_LEAN_RATE);
        if left != right {
            flight.yaw_accumulator = flight.yaw_accumulator.wrapping_add(if left {
                PLAYER_YAW_ACCUMULATOR_STEP
            } else {
                -PLAYER_YAW_ACCUMULATOR_STEP
            });
        }
        let movement_pitch = accumulator_angle(flight.pitch_accumulator);
        let yaw = accumulator_angle(flight.yaw_accumulator);
        let visual_pitch = visible_pitch_from_lean(flight.pitch_lean);

        let (pitch, velocity) = {
            let Some(player) = self.state.objects.get_mut(primary_id) else {
                return Ok(());
            };
            let bank_target = if left != right {
                if left {
                    PLAYER_LEFT_BANK
                } else {
                    PLAYER_RIGHT_BANK
                }
            } else {
                Angle::ZERO
            };
            player.base.roll = approach_angle(player.base.roll, bank_target, PLAYER_BANK_RATE);
            let target_speed = if left != right {
                PLAYER_TURN_SPEED
            } else {
                PLAYER_CRUISE_SPEED
            };
            player.base.speed = approach_u8(
                player.base.speed,
                target_speed,
                PLAYER_SPEED_CHANGE_PER_TICK,
            );
            player.base.pitch = visual_pitch;
            player.base.yaw = yaw;
            player.base.velocity =
                flight_velocity(movement_pitch, player.base.yaw, player.base.speed, 1);
            let next_y = player.base.position.y.wrapping_add(player.base.velocity.y);
            if next_y > PLAYER_VERTICAL_UPPER_BOUND {
                player.base.velocity.y =
                    PLAYER_VERTICAL_UPPER_BOUND.wrapping_sub(player.base.position.y);
            } else if next_y < PLAYER_VERTICAL_LOWER_BOUND {
                player.base.velocity.y =
                    PLAYER_VERTICAL_LOWER_BOUND.wrapping_sub(player.base.position.y);
            }
            (player.base.pitch, player.base.velocity)
        };

        let next_position = add_vectors(position, velocity);
        self.update_active_camera(retail_frame, next_position, pitch, yaw);

        if let Some(wingmate_id) = self.state.mission.wingmate {
            if let Some(wingmate) = self.state.objects.get_mut(wingmate_id) {
                wingmate.base.pitch = pitch;
                wingmate.base.yaw = yaw;
                wingmate.base.velocity = velocity;
            }
        }
        self.update_player_blaster(primary_id, weapons_enabled)?;
        Ok(())
    }

    fn update_active_walker(
        &mut self,
        player_id: ObjectId,
        retail_frame: u16,
        weapons_enabled: bool,
    ) -> Result<(), Error> {
        let turn_left = self.state.input.held.contains(Button::LeftShoulder);
        let turn_right = self.state.input.held.contains(Button::RightShoulder);
        let walking = self.state.input.held.contains(Button::Up);
        let jump_held = self.state.input.held.contains(Button::Y);
        let jump_pressed = self.state.input.pressed.contains(Button::Y);
        if turn_left || turn_right || walking || jump_pressed {
            self.state.mission.departed_certified_neutral_path = true;
        }

        let turn_input = if turn_left {
            WalkerTurnInput::Left
        } else if turn_right {
            WalkerTurnInput::Right
        } else {
            WalkerTurnInput::Neutral
        };
        let motion_profile = self.primary_pilot().walker_motion_profile();
        let mut walker = self.state.mission.player_walker;
        let (next_position, yaw, velocity) = {
            let Some(player) = self.state.objects.get_mut(player_id) else {
                return Ok(());
            };
            let mut heading_offset = walker.heading_offset.unwrap_or(player.base.yaw);
            let (spring_target, velocity_target, spring_divisor, velocity_divisor) =
                match turn_input {
                    WalkerTurnInput::Left => (
                        WALKER_TURN_SPRING_TARGET,
                        WALKER_TURN_VELOCITY_TARGET,
                        WALKER_ACTIVE_SPRING_RESPONSE_DIVISOR,
                        WALKER_ACTIVE_VELOCITY_RESPONSE_DIVISOR,
                    ),
                    WalkerTurnInput::Right => (
                        -WALKER_TURN_SPRING_TARGET,
                        -WALKER_TURN_VELOCITY_TARGET,
                        WALKER_ACTIVE_SPRING_RESPONSE_DIVISOR,
                        WALKER_ACTIVE_VELOCITY_RESPONSE_DIVISOR,
                    ),
                    WalkerTurnInput::Neutral => (
                        0,
                        0,
                        WALKER_NEUTRAL_RESPONSE_DIVISOR,
                        WALKER_NEUTRAL_RESPONSE_DIVISOR,
                    ),
                };
            walker.turn_spring =
                approach_proportional_i16(walker.turn_spring, spring_target, spring_divisor);
            walker.turn_velocity =
                approach_proportional_i8(walker.turn_velocity, velocity_target, velocity_divisor);
            heading_offset = heading_offset.wrapping_add(walker.turn_velocity);
            walker.heading_offset = Some(heading_offset);
            player.base.yaw = heading_offset.wrapping_add((walker.turn_spring >> 8) as i8);
            player.base.pitch = Angle::ZERO;
            player.base.roll = approach_angle(player.base.roll, Angle::ZERO, PLAYER_BANK_RATE);
            player.base.speed = if walking { WALKER_FORWARD_SPEED } else { 0 };
            let mut velocity = flight_velocity(Angle::ZERO, player.base.yaw, player.base.speed, 1);
            velocity.y = 0;

            let jump_started = jump_pressed && walker.jump == WalkerJumpState::Grounded;
            if jump_started {
                walker.jump = WalkerJumpState::Active(WalkerJumpMotion {
                    launch_ticks_remaining: motion_profile.launch_ticks,
                    ascent_impulse: motion_profile.initial_ascent_impulse,
                    pose_extension: WALKER_JUMP_INITIAL_POSE_EXTENSION,
                    motion_ticks_elapsed: 0,
                    height_offset: 0,
                    fall_velocity: 0,
                    ascent_velocity: 0,
                    surface_height: player.base.position.y,
                });
            }

            if let WalkerJumpState::Active(mut jump) = walker.jump {
                let mut landed = false;
                if !jump_started {
                    let ascent_impulse_for_motion = jump.ascent_impulse;
                    jump.motion_ticks_elapsed = jump.motion_ticks_elapsed.saturating_add(1);
                    let next_height_offset = jump
                        .height_offset
                        .saturating_add(walker_fall_step(jump.fall_velocity))
                        .saturating_add(jump.ascent_velocity);
                    if jump.height_offset < 0 && next_height_offset >= 0 {
                        velocity.y = jump.surface_height.saturating_sub(player.base.position.y);
                        walker.jump = WalkerJumpState::Grounded;
                        landed = true;
                    } else {
                        jump.height_offset = next_height_offset;
                        let acceleration = i16::from(jump.motion_ticks_elapsed)
                            .min(WALKER_MAXIMUM_FALL_ACCELERATION);
                        if jump.motion_ticks_elapsed > WALKER_TAKEOFF_COLLISION_TICKS
                            || jump.height_offset < 0
                        {
                            jump.fall_velocity = jump.fall_velocity.saturating_add(acceleration);
                        } else {
                            jump.fall_velocity = 0;
                        }
                        jump.ascent_velocity = walker_ascent_velocity(ascent_impulse_for_motion);
                    }
                }

                if !landed {
                    if !jump_started && jump.launch_ticks_remaining > 0 {
                        jump.launch_ticks_remaining -= 1;
                    }
                    if jump_held {
                        jump.ascent_impulse = approach_i16(
                            jump.ascent_impulse,
                            motion_profile.maximum_ascent_impulse,
                            motion_profile.held_ascent_step,
                        );
                    } else if jump.launch_ticks_remaining == 0 {
                        jump.ascent_impulse =
                            approach_i16(jump.ascent_impulse, 0, WALKER_JUMP_RELEASE_RECOVERY_STEP);
                    }
                    jump.pose_extension = approach_u16(
                        jump.pose_extension,
                        WALKER_JUMP_FULL_POSE_EXTENSION,
                        motion_profile.pose_extension_step,
                    );
                    let next_height = jump.surface_height.saturating_add(jump.height_offset);
                    velocity.y = next_height.saturating_sub(player.base.position.y);
                    walker.jump = WalkerJumpState::Active(jump);
                }
            }
            player.base.velocity = velocity;
            (
                add_vectors(player.base.position, velocity),
                player.base.yaw,
                velocity,
            )
        };
        self.state.mission.player_walker = walker;
        self.update_active_camera(retail_frame, next_position, Angle::ZERO, yaw);
        if let Some(wingmate) = self.state.mission.wingmate {
            if let Some(wingmate) = self.state.objects.get_mut(wingmate) {
                wingmate.base.yaw = yaw;
                wingmate.base.velocity = velocity;
            }
        }
        self.update_player_blaster(player_id, weapons_enabled)
    }

    fn update_certified_neutral_flight(&mut self, retail_frame: u16) {
        let (start, end) = enclosing_player_keyframes(retail_frame);
        let numerator = retail_frame.saturating_sub(start.retail_frame);
        let denominator = end.retail_frame.saturating_sub(start.retail_frame);
        let position = interpolate_vector(start.position, end.position, numerator, denominator);
        let pitch = interpolate_angle(start.pitch, end.pitch, numerator, denominator);
        let yaw = interpolate_angle(start.yaw, end.yaw, numerator, denominator);
        let roll = interpolate_angle(start.roll, end.roll, numerator, denominator);
        let speed = interpolate_u8(start.speed, end.speed, numerator, denominator);

        if let Some(primary) = self.state.mission.primary_player {
            if let Some(object) = self.state.objects.get_mut(primary) {
                object.base.position = position;
                object.base.pitch = pitch;
                object.base.yaw = yaw;
                object.base.roll = roll;
                object.base.speed = speed;
                object.base.velocity = Vector3::default();
            }
        }
        if let Some(wingmate) = self.state.mission.wingmate {
            if let Some(object) = self.state.objects.get_mut(wingmate) {
                object.base.position = add_vectors(position, ACTIVE_WINGMATE_OFFSET);
                object.base.pitch = pitch;
                object.base.yaw = yaw;
                object.base.roll = roll;
                object.base.speed = speed;
                object.base.velocity = Vector3::default();
            }
        }
        self.update_mission_camera(retail_frame);
    }

    fn update_active_camera(
        &mut self,
        retail_frame: u16,
        player_position: Vector3,
        pitch: Angle,
        yaw: Angle,
    ) {
        if retail_frame
            <= MISSION_CAMERA_FOLLOW_KEYFRAMES
                .last()
                .expect("camera follow path is not empty")
                .retail_frame
        {
            let (start, end) = enclosing_camera_follow_keyframes(retail_frame);
            self.state.mission.camera_follow_offset = interpolate_vector(
                start.offset,
                end.offset,
                retail_frame.saturating_sub(start.retail_frame),
                end.retail_frame.saturating_sub(start.retail_frame),
            );
        } else {
            self.state.mission.camera_follow_offset.x =
                approach_i16(self.state.mission.camera_follow_offset.x, 0, 1);
            self.state.mission.camera_follow_offset.z =
                approach_i16(self.state.mission.camera_follow_offset.z, 0, 1);
        }
        self.state.camera.position =
            add_vectors(player_position, self.state.mission.camera_follow_offset);
        self.state.camera.rotation = Rotation {
            pitch: Angle::from_units(pitch.units().wrapping_neg()),
            yaw: Angle::from_units(yaw.units().wrapping_neg()),
            roll: Angle::ZERO,
        };
    }

    fn player_charge_ready_tick(&self) -> u8 {
        match self.primary_pilot().craft_profile().class {
            PilotCraftClass::FoxFalco => FOX_FALCO_CHARGE_READY_TICK,
            PilotCraftClass::PeppySlippy => PEPPY_SLIPPY_CHARGE_READY_TICK,
            PilotCraftClass::MiyuFay => MIYU_FAY_CHARGE_READY_TICK,
        }
    }

    fn update_player_blaster(
        &mut self,
        player: ObjectId,
        weapons_enabled: bool,
    ) -> Result<(), Error> {
        let fire_held = weapons_enabled && self.state.input.held.contains(Button::B);
        match self.state.mission.player_blaster {
            PlayerBlasterState::Ready => {
                if !fire_held {
                    return Ok(());
                }
                self.spawn_player_rapid_laser(player)?;
                self.state.mission.player_blaster = PlayerBlasterState::Holding {
                    held_ticks: 1,
                    charge_orb: None,
                };
            }
            PlayerBlasterState::Holding {
                mut held_ticks,
                mut charge_orb,
            } => {
                let ready_tick = self.player_charge_ready_tick();
                if fire_held {
                    held_ticks = held_ticks.saturating_add(1);
                    if held_ticks == PLAYER_CHARGE_ORB_SPAWN_TICK {
                        charge_orb = Some(self.spawn_player_charge_orb(player)?);
                    }
                    if held_ticks >= ready_tick {
                        self.set_charge_orb_phase(charge_orb, PlayerChargeOrbPhase::Ready);
                    }
                    self.state.mission.player_blaster = PlayerBlasterState::Holding {
                        held_ticks,
                        charge_orb,
                    };
                } else {
                    let reached_release_window =
                        held_ticks.saturating_add(PLAYER_CHARGE_RELEASE_GRACE_TICKS) >= ready_tick;
                    if reached_release_window {
                        self.spawn_player_charged_laser(player)?;
                    }
                    self.set_charge_orb_phase(
                        charge_orb,
                        PlayerChargeOrbPhase::Releasing {
                            ticks_remaining: PLAYER_CHARGE_ORB_RELEASE_TICKS,
                            ready: reached_release_window,
                        },
                    );
                    self.state.mission.player_blaster = PlayerBlasterState::Ready;
                }
            }
        }
        Ok(())
    }

    fn set_charge_orb_phase(&mut self, charge_orb: Option<ObjectId>, phase: PlayerChargeOrbPhase) {
        let Some(charge_orb) = charge_orb else {
            return;
        };
        let Some(object) = self.state.objects.get_mut(charge_orb) else {
            return;
        };
        let ObjectActivity::PlayerChargeOrb(mut state) = object.extension.activity else {
            return;
        };
        state.phase = phase;
        object.extension.activity = ObjectActivity::PlayerChargeOrb(state);
    }

    fn player_weapon_origin(&self, player: ObjectId) -> (Vector3, Angle, Angle) {
        self.state
            .objects
            .get(player)
            .map(|object| (object.base.position, object.base.pitch, object.base.yaw))
            .unwrap_or_default()
    }

    fn new_player_projectile(&self, player: ObjectId, kind: PlayerProjectileKind) -> Object {
        let (position, pitch, yaw) = self.player_weapon_origin(player);
        let (speed, attack_power, weapon) = match kind {
            PlayerProjectileKind::Rapid => (
                PLAYER_RAPID_LASER_SPEED,
                PLAYER_RAPID_LASER_ATTACK_POWER,
                WeaponKind::Laser,
            ),
            PlayerProjectileKind::Charged => (
                PLAYER_CHARGED_LASER_SPEED,
                PLAYER_CHARGED_LASER_ATTACK_POWER,
                WeaponKind::ChargedLaser,
            ),
        };
        let mut projectile =
            Object::new(ObjectKind::Projectile, ShapeId::EMPTY, Behavior::Projectile);
        projectile.base.position = position;
        projectile.base.pitch = pitch;
        projectile.base.yaw = yaw;
        projectile.base.speed = speed;
        projectile.base.hit_points = PLAYER_PROJECTILE_DURABILITY;
        projectile.base.attack_power = attack_power;
        projectile.base.weapon = weapon;
        projectile.base.collision_class = CollisionClass::PlayerWeapon;
        projectile.base.linked_object = Some(player);
        projectile.base.flags.casts_shadow = false;
        projectile.base.flags.visible = false;
        projectile.base.flags.collision_disabled = true;
        projectile.extension.activity =
            ObjectActivity::PlayerProjectile(PlayerProjectileState { kind, age_ticks: 0 });
        projectile
    }

    fn spawn_player_rapid_laser(&mut self, player: ObjectId) -> Result<ObjectId, Error> {
        let laser = self.new_player_projectile(player, PlayerProjectileKind::Rapid);
        self.state
            .objects
            .allocate(laser)
            .ok_or(Error::ObjectCapacityReached)
    }

    fn spawn_player_charged_laser(&mut self, player: ObjectId) -> Result<ObjectId, Error> {
        let laser = self.new_player_projectile(player, PlayerProjectileKind::Charged);
        self.state
            .objects
            .allocate(laser)
            .ok_or(Error::ObjectCapacityReached)
    }

    fn spawn_player_charge_orb(&mut self, player: ObjectId) -> Result<ObjectId, Error> {
        let (position, pitch, yaw) = self.player_weapon_origin(player);
        let mut orb = Object::new(
            ObjectKind::Effect,
            ShapeId::PLAYER_CHARGE_ORB_BUILDING,
            Behavior::Effect,
        );
        orb.base.position = position;
        orb.base.pitch = pitch;
        orb.base.yaw = yaw;
        orb.base.hit_points = 1;
        orb.base.linked_object = Some(player);
        orb.base.flags.casts_shadow = false;
        orb.base.flags.collision_disabled = true;
        orb.extension.activity = ObjectActivity::PlayerChargeOrb(PlayerChargeOrbState {
            phase: PlayerChargeOrbPhase::Building,
            age_ticks: 0,
        });
        self.state
            .objects
            .allocate(orb)
            .ok_or(Error::ObjectCapacityReached)
    }

    fn update_title(&mut self) {
        use super::state::Difficulty;

        match self.state.title.page {
            TitlePage::MainMenu => {
                if self.state.input.pressed.contains(Button::Up) {
                    self.state.title.menu_item = self.state.title.menu_item.previous();
                }
                if self.state.input.pressed.contains(Button::Down) {
                    self.state.title.menu_item = self.state.title.menu_item.next();
                }
                if self.state.title.menu_item == TitleMenuItem::SoundMode
                    && (self.state.input.pressed.contains(Button::Left)
                        || self.state.input.pressed.contains(Button::Right))
                {
                    self.state.title.audio_output = self.state.title.audio_output.toggled();
                }
                if self.confirm_pressed() {
                    match self.state.title.menu_item {
                        TitleMenuItem::Mission => {
                            self.state.title.page = TitlePage::Difficulty;
                            self.state.campaign.difficulty = Difficulty::Normal;
                        }
                        TitleMenuItem::Records => self.enter_mode(GameMode::Records),
                        TitleMenuItem::SoundMode => {}
                    }
                }
            }
            TitlePage::Difficulty => {
                if self.cancel_pressed() {
                    self.state.title.page = TitlePage::MainMenu;
                    return;
                }
                if self.state.input.pressed.contains(Button::Up) {
                    self.state.campaign.difficulty = match self.state.campaign.difficulty {
                        Difficulty::Normal => Difficulty::Expert,
                        Difficulty::Hard => Difficulty::Normal,
                        Difficulty::Expert => Difficulty::Hard,
                    };
                }
                if self.state.input.pressed.contains(Button::Down) {
                    self.state.campaign.difficulty = match self.state.campaign.difficulty {
                        Difficulty::Normal => Difficulty::Hard,
                        Difficulty::Hard => Difficulty::Expert,
                        Difficulty::Expert => Difficulty::Normal,
                    };
                }
                if self.confirm_pressed() {
                    self.enter_mode(GameMode::Briefing);
                }
            }
        }
    }

    fn confirm_pressed(&self) -> bool {
        self.state.input.pressed.contains(Button::B)
            || self.state.input.pressed.contains(Button::Start)
    }

    fn cancel_pressed(&self) -> bool {
        self.state.input.pressed.contains(Button::X) || self.state.input.pressed.contains(Button::Y)
    }

    fn enter_mode(&mut self, mode: GameMode) {
        self.state.mode = mode;
        self.state.mode_frame = 0;
    }

    fn spawn_title_flyby(&mut self) -> Result<(), Error> {
        for pose in TITLE_CRAFT_POSES {
            let mut craft = Object::new(
                ObjectKind::Scenery,
                ShapeId::TITLE_CRAFT,
                Behavior::TitleFlyby,
            );
            craft.base.position = pose.position;
            craft.base.velocity = pose.velocity;
            craft.base.pitch = super::object::Angle::from_units(pose.pitch);
            craft.base.yaw = super::object::Angle::from_units(pose.yaw);
            craft.base.roll = super::object::Angle::from_units(pose.roll);
            craft.base.flags.casts_shadow = false;
            let id = self
                .state
                .objects
                .allocate(craft)
                .ok_or(Error::ObjectCapacityReached)?;
            self.title_flyby.push(id);
        }
        for pose in TITLE_EFFECT_POSES {
            let mut effect = Object::new(
                ObjectKind::Effect,
                ShapeId::TITLE_FORMATION_EFFECT,
                Behavior::TitleFlyby,
            );
            effect.base.position = pose.position;
            effect.base.velocity = pose.velocity;
            effect.base.flags.casts_shadow = false;
            let id = self
                .state
                .objects
                .allocate(effect)
                .ok_or(Error::ObjectCapacityReached)?;
            self.title_flyby.push(id);
        }
        self.state.camera.position.z = TITLE_CAMERA_START_Z;
        Ok(())
    }

    fn remove_title_flyby(&mut self) {
        let ids = std::mem::take(&mut self.title_flyby);
        for id in ids {
            self.state.objects.remove(id);
        }
    }

    fn resolve_mission_collisions(&mut self) {
        if self.state.mode != GameMode::Mission || self.state.mission.phase != MissionPhase::Active
        {
            return;
        }

        let player_weapons: Vec<_> = self
            .state
            .objects
            .active_objects()
            .filter_map(|(id, object)| {
                (object.base.collision_class == CollisionClass::PlayerWeapon
                    && !object.base.flags.collision_disabled
                    && !object.base.flags.remove_after_tick)
                    .then_some(id)
            })
            .collect();
        let enemies: Vec<_> = self
            .state
            .objects
            .active_objects()
            .filter_map(|(id, object)| {
                (object.base.collision_class == CollisionClass::Enemy
                    && object.base.hit_points > 0
                    && !object.base.flags.collision_disabled)
                    .then_some(id)
            })
            .collect();

        let mut collisions = Vec::new();
        for weapon_id in player_weapons {
            let Some(weapon) = self.state.objects.get(weapon_id) else {
                continue;
            };
            for enemy_id in enemies.iter().copied() {
                let Some(enemy) = self.state.objects.get(enemy_id) else {
                    continue;
                };
                if objects_overlap(weapon, enemy) {
                    collisions.push((weapon_id, enemy_id));
                    break;
                }
            }
        }

        for (weapon_id, enemy_id) in collisions {
            let damage = self
                .state
                .objects
                .get(weapon_id)
                .map(|weapon| weapon.base.attack_power)
                .unwrap_or_default();
            let weapon_can_hit = self
                .state
                .objects
                .get(weapon_id)
                .is_some_and(|weapon| !weapon.base.flags.collision_disabled);
            let enemy_can_be_hit = self.state.objects.get(enemy_id).is_some_and(|enemy| {
                enemy.base.hit_points > 0 && !enemy.base.flags.collision_disabled
            });
            if !weapon_can_hit || !enemy_can_be_hit {
                continue;
            }

            if let Some(weapon) = self.state.objects.get_mut(weapon_id) {
                weapon.base.flags.visible = false;
                weapon.base.flags.collided = true;
                weapon.base.flags.collision_disabled = true;
                weapon.base.flags.remove_after_tick = true;
            }
            if let Some(enemy) = self.state.objects.get_mut(enemy_id) {
                enemy.base.flags.collided = true;
                if damage >= enemy.base.hit_points {
                    enemy.base.flags.collision_disabled = true;
                    enemy.base.explosion_timer = ENEMY_DESTRUCTION_TICKS;
                } else {
                    enemy.base.hit_points -= damage;
                }
            }
        }
    }

    fn update_objects(&mut self) {
        let formation_just_started =
            matches!(self.state.mode, GameMode::Intro(IntroPhase::Formation))
                && self.state.mode_frame == 0;
        if matches!(self.state.mode, GameMode::Intro(IntroPhase::Formation))
            && !formation_just_started
        {
            self.state.camera.position.z = self
                .state
                .camera
                .position
                .z
                .wrapping_add(TITLE_CAMERA_VELOCITY_Z);
        }
        let active = self.state.objects.active_ids().to_vec();
        let scoreless_pressure_encounter = matches!(
            self.state.mission.visit,
            MissionVisit::RecurringAttackers
                | MissionVisit::LeonPressure
                | MissionVisit::FinalPursuer
                | MissionVisit::WolfBlockade
        );
        let mut score_award = 0u32;
        let mut destroyed_objects = 0u32;
        for id in active {
            let linked_player_pose = self
                .state
                .objects
                .get(id)
                .and_then(|object| object.base.linked_object)
                .and_then(|player| self.state.objects.get(player))
                .map(|player| (player.base.position, player.base.pitch, player.base.yaw));
            let Some(object) = self.state.objects.get_mut(id) else {
                continue;
            };
            object.base.flags.collided = false;
            if object.base.kind == ObjectKind::Enemy
                && object.base.flags.collision_disabled
                && object.base.explosion_timer > 0
            {
                object.base.explosion_timer -= 1;
                match object.base.explosion_timer {
                    ENEMY_SCORE_AWARD_TIMER => {
                        if !scoreless_pressure_encounter {
                            score_award =
                                score_award.saturating_add(u32::from(object.base.hit_points));
                        }
                    }
                    ENEMY_EXPLOSION_START_TIMER => {
                        object.base.hit_points = 0;
                        object.base.flags.exploding = true;
                    }
                    0 => {
                        object.base.flags.remove_after_tick = true;
                        destroyed_objects = destroyed_objects.saturating_add(1);
                    }
                    _ => {}
                }
            }
            if formation_just_started && object.base.behavior == Behavior::TitleFlyby {
                continue;
            }
            match object.extension.activity {
                ObjectActivity::PlayerProjectile(projectile) => {
                    update_player_projectile_object(object, projectile);
                    continue;
                }
                ObjectActivity::PlayerChargeOrb(orb) => {
                    update_player_charge_orb_object(object, orb, linked_player_pose);
                    continue;
                }
                ObjectActivity::CapitalFlight(_)
                | ObjectActivity::ReengagementFighterFlight(_)
                | ObjectActivity::FighterInterceptFlight(_)
                | ObjectActivity::InterceptionMissileFlight(_)
                | ObjectActivity::HostileProjectileFlight(_)
                | ObjectActivity::PigmaRivalFlight(_)
                | ObjectActivity::LeonRivalFlight(_)
                | ObjectActivity::FinalRivalFlight(_) => continue,
                ObjectActivity::None | ObjectActivity::FighterFlight(_) => {}
            }
            if matches!(
                object.base.behavior,
                Behavior::MissionEntryFlyby | Behavior::MissionScriptedProjectile
            ) {
                continue;
            }
            object.base.position.x = object.base.position.x.wrapping_add(object.base.velocity.x);
            object.base.position.y = object.base.position.y.wrapping_add(object.base.velocity.y);
            object.base.position.z = object.base.position.z.wrapping_add(object.base.velocity.z);
        }

        self.state.mission.score = self.state.mission.score.saturating_add(score_award);
        self.state.mission.objects_destroyed = self
            .state
            .mission
            .objects_destroyed
            .saturating_add(destroyed_objects);

        let removals: Vec<_> = self
            .state
            .objects
            .active_objects()
            .filter_map(|(id, object)| object.base.flags.remove_after_tick.then_some(id))
            .collect();
        for id in removals {
            let defeated_pigma = self.pigma_rival == Some(id);
            let defeated_leon = self.leon_rival == Some(id);
            let defeated_final_rival = self.final_rival == Some(id);
            let defeated_mirage_dragon = self.mirage_dragon == Some(id);
            self.state.objects.remove(id);
            self.title_flyby.retain(|candidate| *candidate != id);
            self.fighter_intercept_actors.forget(id);
            self.pressure_fighter_actors.forget(id);
            if defeated_pigma {
                self.pigma_rival = None;
                self.state.mission.score = self
                    .state
                    .mission
                    .score
                    .saturating_add(PIGMA_SCORE_AWARD.saturating_sub(u32::from(PIGMA_HEALTH)));
            }
            if defeated_leon && self.state.mission.visit == MissionVisit::LeonDuel {
                self.leon_rival = None;
                self.state.mission.score = self
                    .state
                    .mission
                    .score
                    .saturating_add(LEON_SCORE_AWARD.saturating_sub(u32::from(LEON_HEALTH)));
            }
            if defeated_final_rival {
                self.final_rival = None;
            }
            if defeated_mirage_dragon {
                self.mirage_dragon = None;
                self.state.mission.score = self.state.mission.score.saturating_add(
                    MIRAGE_DRAGON_SCORE_AWARD.saturating_sub(u32::from(MIRAGE_DRAGON_HEALTH)),
                );
            }
            for segment in &mut self.mirage_dragon_body {
                if *segment == Some(id) {
                    *segment = None;
                }
            }
            if self.mirage_dragon_tail == Some(id) {
                self.mirage_dragon_tail = None;
            }
            for actor in &mut self.mission_entry_flyby {
                if *actor == Some(id) {
                    *actor = None;
                }
            }
        }
    }

    /// Place the campaign craft relative to its selected strategic-map
    /// target. This is the typed form of the retail cardinal-placement
    /// service; map and target positions remain ordinary fields throughout.
    pub fn position_strategic_map_player(&mut self, approach_distance: u16) {
        let map = &mut self.state.strategic_map;
        map.marker_phase = (map.marker_phase + 1) % MAP_MARKER_PHASE_COUNT;

        let delta_x = map.target_position.x.wrapping_sub(map.player_position.x);
        let delta_z = map.target_position.z.wrapping_sub(map.player_position.z);
        let target_bearing = sf_core::aim_angle::yanglexy(delta_x, delta_z).wrapping_neg();
        let mut position = map.player_position;
        if approach_distance != 0 {
            let (offset_x, offset_z) =
                sf_core::snes_trig::rotate_16xz(target_bearing, 0, -(approach_distance as i16));
            position.x = position.x.wrapping_add(offset_x);
            position.z = position.z.wrapping_add(offset_z);
        }

        if let Some(target) = map.selected_target {
            if let Some(object) = self.state.objects.get_mut(target) {
                object.base.position = map.target_position;
            }
        }
        if let Some(player) = map.primary_player {
            if let Some(object) = self.state.objects.get_mut(player) {
                object.base.position = position;
                object.base.yaw = map.formation_yaw;
            }
        }
    }

    fn build_render_objects(&mut self) -> Result<(), Error> {
        self.render_objects.clear();
        for (id, object) in self.state.objects.active_objects() {
            if !object.base.flags.visible || object.base.shape == ShapeId::EMPTY {
                continue;
            }
            let shape = object.base.shape;
            let catalog = shape
                .catalog_entry()
                .ok_or(Error::MissingCatalogShape(shape))?;
            let material_set = object.extension.material_set.unwrap_or_else(|| {
                if catalog.color_table == 0 {
                    MaterialSetId::from_catalog_token(TITLE_MATERIAL_FALLBACK)
                } else {
                    MaterialSetId::from_catalog_token(catalog.color_table)
                }
            });
            self.render_objects.push(RenderObject {
                object: id,
                shape,
                material_set,
                position: object.base.position,
                rotation: Rotation {
                    pitch: object.base.pitch,
                    yaw: object.base.yaw,
                    roll: object.base.roll,
                },
                sort_depth: catalog.sort_z,
                animation: AnimationState {
                    shape_frame: object.extension.animation_frame,
                    color_frame: object.extension.color_frame,
                    explosion_frame: object.base.explosion_timer,
                },
                depth_offset: object.extension.depth_offset,
                texture_scroll_x: object.extension.texture_scroll_x,
                texture_scroll_y: object.extension.texture_scroll_y,
                flags: RenderFlags {
                    visible: object.base.flags.visible,
                    casts_shadow: object.base.flags.casts_shadow,
                    highlighted: false,
                },
            });
        }
        Ok(())
    }
}

fn update_player_projectile_object(object: &mut Object, mut state: PlayerProjectileState) {
    state.age_ticks = state.age_ticks.saturating_add(1);
    object.extension.activity = ObjectActivity::PlayerProjectile(state);

    let (visible_tick, end_tick) = match state.kind {
        PlayerProjectileKind::Rapid => {
            if state.age_ticks >= PLAYER_RAPID_LASER_DISTANT_TICK {
                object.base.shape = ShapeId::PLAYER_RAPID_LASER_DISTANT;
                object.base.velocity = flight_velocity(
                    object.base.pitch,
                    object.base.yaw,
                    PLAYER_RAPID_LASER_SPEED,
                    PLAYER_RAPID_LASER_FAST_VELOCITY_SCALE,
                );
            } else if state.age_ticks >= PLAYER_RAPID_LASER_FAST_TICK {
                object.base.shape = ShapeId::PLAYER_RAPID_LASER_FAST;
                object.base.velocity = flight_velocity(
                    object.base.pitch,
                    object.base.yaw,
                    PLAYER_RAPID_LASER_SPEED,
                    PLAYER_RAPID_LASER_FAST_VELOCITY_SCALE,
                );
            } else if state.age_ticks >= PLAYER_RAPID_LASER_EXPANDED_TICK {
                object.base.shape = ShapeId::PLAYER_RAPID_LASER_EXPANDED;
                object.base.velocity = flight_velocity(
                    object.base.pitch,
                    object.base.yaw,
                    PLAYER_RAPID_LASER_SPEED,
                    PLAYER_RAPID_LASER_EXPANDED_VELOCITY_SCALE,
                );
            } else if state.age_ticks >= PLAYER_RAPID_LASER_VISIBLE_TICK {
                object.base.shape = ShapeId::PLAYER_RAPID_LASER_LAUNCH;
                object.base.velocity = flight_velocity(
                    object.base.pitch,
                    object.base.yaw,
                    PLAYER_RAPID_LASER_SPEED,
                    PLAYER_RAPID_LASER_LAUNCH_VELOCITY_SCALE,
                );
            }
            (PLAYER_RAPID_LASER_VISIBLE_TICK, PLAYER_RAPID_LASER_END_TICK)
        }
        PlayerProjectileKind::Charged => {
            if state.age_ticks >= PLAYER_CHARGED_LASER_ACTIVE_TICK {
                object.base.shape = ShapeId::PLAYER_CHARGED_LASER_ACTIVE;
                object.base.velocity = flight_velocity(
                    object.base.pitch,
                    object.base.yaw,
                    PLAYER_CHARGED_LASER_SPEED,
                    PLAYER_CHARGED_LASER_ACTIVE_VELOCITY_SCALE,
                );
            } else if state.age_ticks >= PLAYER_CHARGED_LASER_VISIBLE_TICK {
                object.base.shape = ShapeId::PLAYER_CHARGED_LASER_LAUNCH;
                object.base.velocity = flight_velocity(
                    object.base.pitch,
                    object.base.yaw,
                    PLAYER_CHARGED_LASER_SPEED,
                    PLAYER_CHARGED_LASER_LAUNCH_VELOCITY_SCALE,
                );
            }
            (
                PLAYER_CHARGED_LASER_VISIBLE_TICK,
                PLAYER_CHARGED_LASER_END_TICK,
            )
        }
    };

    if state.age_ticks >= end_tick {
        object.base.flags.visible = false;
        object.base.flags.collision_disabled = true;
        object.base.flags.remove_after_tick = true;
        return;
    }
    if state.age_ticks < visible_tick {
        return;
    }
    object.base.flags.visible = true;
    object.base.flags.collision_disabled = false;
    object.base.position = add_vectors(object.base.position, object.base.velocity);
}

fn update_player_charge_orb_object(
    object: &mut Object,
    mut state: PlayerChargeOrbState,
    linked_player_pose: Option<(Vector3, Angle, Angle)>,
) {
    let Some((player_position, player_pitch, player_yaw)) = linked_player_pose else {
        object.base.flags.remove_after_tick = true;
        return;
    };
    state.age_ticks = state.age_ticks.saturating_add(1);
    object.base.pitch = player_pitch;
    object.base.yaw = player_yaw;
    object.base.velocity = Vector3::default();
    object.base.position = if state.age_ticks == 1 {
        player_position
    } else {
        add_vectors(
            player_position,
            flight_velocity(
                player_pitch,
                player_yaw,
                PLAYER_LASER_MUZZLE_OFFSET_MAGNITUDE,
                1,
            ),
        )
    };
    object.base.shape = match state.phase {
        PlayerChargeOrbPhase::Building | PlayerChargeOrbPhase::Releasing { ready: false, .. } => {
            ShapeId::PLAYER_CHARGE_ORB_BUILDING
        }
        PlayerChargeOrbPhase::Ready | PlayerChargeOrbPhase::Releasing { ready: true, .. } => {
            ShapeId::PLAYER_CHARGE_ORB_READY
        }
    };
    object.base.flags.collision_disabled = true;

    if let PlayerChargeOrbPhase::Releasing {
        ticks_remaining,
        ready,
    } = state.phase
    {
        if ticks_remaining <= 1 {
            object.base.flags.visible = false;
            object.base.flags.remove_after_tick = true;
        } else {
            state.phase = PlayerChargeOrbPhase::Releasing {
                ticks_remaining: ticks_remaining - 1,
                ready,
            };
        }
    }
    object.extension.activity = ObjectActivity::PlayerChargeOrb(state);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerCraftPresentation {
    Flight,
    FlightSideTransition { animation_frame: u8 },
    WalkerSideTransition { animation_frame: u8 },
    Walker,
}

fn sampled_transformation_frame<const FRAME_COUNT: usize>(
    frames: &[u8; FRAME_COUNT],
    elapsed_retail_frames: u8,
    stage_start_retail_frames: u8,
) -> u8 {
    let sample = usize::from(
        elapsed_retail_frames.saturating_sub(stage_start_retail_frames)
            / PLAYER_TRANSFORMATION_RETAIL_FRAMES_PER_TICK,
    );
    frames[sample.min(FRAME_COUNT.saturating_sub(1))]
}

fn transformation_presentation(
    transformation: PlayerCraftTransformation,
) -> (PlayerCraftForm, PlayerCraftPresentation) {
    let elapsed = transformation.elapsed_retail_frames;
    match transformation.direction {
        PlayerCraftTransformationDirection::ToWalker => {
            if elapsed < PLAYER_TRANSFORMATION_START_RETAIL_FRAMES {
                return (
                    PlayerCraftForm::Transforming(transformation),
                    PlayerCraftPresentation::Flight,
                );
            }
            if elapsed < PLAYER_TRANSFORMATION_SECOND_STAGE_RETAIL_FRAMES {
                return (
                    PlayerCraftForm::Transforming(transformation),
                    PlayerCraftPresentation::FlightSideTransition {
                        animation_frame: sampled_transformation_frame(
                            &TO_WALKER_FLIGHT_SIDE_FRAMES,
                            elapsed,
                            PLAYER_TRANSFORMATION_START_RETAIL_FRAMES,
                        ),
                    },
                );
            }
            if elapsed < PLAYER_TRANSFORMATION_TO_WALKER_END_RETAIL_FRAMES {
                return (
                    PlayerCraftForm::Transforming(transformation),
                    PlayerCraftPresentation::WalkerSideTransition {
                        animation_frame: sampled_transformation_frame(
                            &TO_WALKER_WALKER_SIDE_FRAMES,
                            elapsed,
                            PLAYER_TRANSFORMATION_SECOND_STAGE_RETAIL_FRAMES,
                        ),
                    },
                );
            }
            (PlayerCraftForm::Walker, PlayerCraftPresentation::Walker)
        }
        PlayerCraftTransformationDirection::ToFlight => {
            if elapsed < PLAYER_TRANSFORMATION_START_RETAIL_FRAMES {
                return (
                    PlayerCraftForm::Transforming(transformation),
                    PlayerCraftPresentation::Walker,
                );
            }
            if elapsed < PLAYER_TRANSFORMATION_SECOND_STAGE_RETAIL_FRAMES {
                return (
                    PlayerCraftForm::Transforming(transformation),
                    PlayerCraftPresentation::WalkerSideTransition {
                        animation_frame: sampled_transformation_frame(
                            &TO_FLIGHT_WALKER_SIDE_FRAMES,
                            elapsed,
                            PLAYER_TRANSFORMATION_START_RETAIL_FRAMES,
                        ),
                    },
                );
            }
            if elapsed < PLAYER_TRANSFORMATION_TO_FLIGHT_END_RETAIL_FRAMES {
                return (
                    PlayerCraftForm::Transforming(transformation),
                    PlayerCraftPresentation::FlightSideTransition {
                        animation_frame: sampled_transformation_frame(
                            &TO_FLIGHT_FLIGHT_SIDE_FRAMES,
                            elapsed,
                            PLAYER_TRANSFORMATION_SECOND_STAGE_RETAIL_FRAMES,
                        ),
                    },
                );
            }
            (PlayerCraftForm::Flight, PlayerCraftPresentation::Flight)
        }
    }
}

fn pilot_flight_craft_shape(pilot: Pilot) -> ShapeId {
    match pilot.craft_profile().class {
        PilotCraftClass::FoxFalco => ShapeId::FOX_FALCO_FLIGHT_CRAFT,
        PilotCraftClass::PeppySlippy => ShapeId::PEPPY_SLIPPY_FLIGHT_CRAFT,
        PilotCraftClass::MiyuFay => ShapeId::MIYU_FAY_FLIGHT_CRAFT,
    }
}

fn pilot_flight_side_transition_shape(pilot: Pilot) -> ShapeId {
    match pilot.craft_profile().class {
        PilotCraftClass::FoxFalco => ShapeId::FOX_FALCO_FLIGHT_SIDE_TRANSITION,
        PilotCraftClass::PeppySlippy => ShapeId::PEPPY_SLIPPY_FLIGHT_SIDE_TRANSITION,
        PilotCraftClass::MiyuFay => ShapeId::MIYU_FAY_FLIGHT_SIDE_TRANSITION,
    }
}

fn pilot_walker_side_transition_shape(pilot: Pilot) -> ShapeId {
    match pilot.craft_profile().class {
        PilotCraftClass::FoxFalco => ShapeId::FOX_FALCO_WALKER_SIDE_TRANSITION,
        PilotCraftClass::PeppySlippy => ShapeId::PEPPY_SLIPPY_WALKER_SIDE_TRANSITION,
        PilotCraftClass::MiyuFay => ShapeId::MIYU_FAY_WALKER_SIDE_TRANSITION,
    }
}

fn pilot_walker_shape(pilot: Pilot) -> ShapeId {
    match pilot.craft_profile().class {
        PilotCraftClass::FoxFalco => ShapeId::FOX_FALCO_WALKER,
        PilotCraftClass::PeppySlippy => ShapeId::PEPPY_SLIPPY_WALKER,
        PilotCraftClass::MiyuFay => ShapeId::MIYU_FAY_WALKER,
    }
}

fn object_collision_bounds(object: &Object) -> CollisionBounds {
    if matches!(
        object.base.weapon,
        WeaponKind::Laser | WeaponKind::ChargedLaser
    ) {
        return PLAYER_LASER_COLLISION_BOUNDS;
    }
    let [x, y, z] = object
        .base
        .shape
        .catalog_entry()
        .map(|shape| shape.bounds)
        .unwrap_or_default();
    CollisionBounds { x, y, z }
}

fn objects_overlap(first: &Object, second: &Object) -> bool {
    let first_bounds = object_collision_bounds(first);
    let second_bounds = object_collision_bounds(second);
    axis_overlaps(
        first.base.position.x,
        second.base.position.x,
        first_bounds.x,
        second_bounds.x,
    ) && axis_overlaps(
        first.base.position.y,
        second.base.position.y,
        first_bounds.y,
        second_bounds.y,
    ) && axis_overlaps(
        first.base.position.z,
        second.base.position.z,
        first_bounds.z,
        second_bounds.z,
    )
}

fn axis_overlaps(first: i16, second: i16, first_extent: u16, second_extent: u16) -> bool {
    let distance = i32::from(first.wrapping_sub(second)).unsigned_abs();
    distance < u32::from(first_extent) + u32::from(second_extent)
}

fn interpolate_map_coordinate(
    origin: i16,
    destination: i16,
    elapsed_ticks: u16,
    total_ticks: u16,
) -> i16 {
    if total_ticks == 0 {
        return destination;
    }
    let delta = i32::from(destination) - i32::from(origin);
    (i32::from(origin) + delta * i32::from(elapsed_ticks) / i32::from(total_ticks)) as i16
}

fn carrier_starboard_panel_integrity(retail_frame: u16) -> u8 {
    if retail_frame < CARRIER_STARBOARD_FIRST_HIT_RETAIL_FRAME {
        CARRIER_PANEL_INITIAL_INTEGRITY
    } else if retail_frame < CARRIER_STARBOARD_THIRD_HIT_RETAIL_FRAME {
        CARRIER_PANEL_AFTER_ONE_HIT
    } else if retail_frame < CARRIER_STARBOARD_FOURTH_HIT_RETAIL_FRAME {
        CARRIER_PANEL_AFTER_TWO_HITS
    } else if retail_frame < CARRIER_STARBOARD_EIGHTH_HIT_RETAIL_FRAME {
        CARRIER_PANEL_AFTER_THREE_HITS
    } else if retail_frame < CARRIER_STARBOARD_FINAL_HIT_RETAIL_FRAME {
        CARRIER_PANEL_AFTER_FOUR_HITS
    } else {
        CARRIER_PANEL_DESTROYED_INTEGRITY
    }
}

fn carrier_port_panel_integrity(retail_frame: u16) -> u8 {
    if retail_frame < CARRIER_PORT_FIRST_HIT_RETAIL_FRAME {
        CARRIER_PANEL_INITIAL_INTEGRITY
    } else if retail_frame < CARRIER_PORT_SECOND_HIT_RETAIL_FRAME {
        CARRIER_PANEL_AFTER_ONE_HIT
    } else if retail_frame < CARRIER_PORT_THIRD_HIT_RETAIL_FRAME {
        CARRIER_PANEL_AFTER_TWO_HITS
    } else if retail_frame < CARRIER_PORT_FOURTH_HIT_RETAIL_FRAME {
        CARRIER_PANEL_AFTER_THREE_HITS
    } else if retail_frame < CARRIER_PORT_FINAL_HIT_RETAIL_FRAME {
        CARRIER_PANEL_AFTER_FOUR_HITS
    } else {
        CARRIER_PANEL_DESTROYED_INTEGRITY
    }
}

fn interpolate_percent(origin: u8, destination: u8, elapsed_ticks: u16, total_ticks: u16) -> u8 {
    if total_ticks == 0 {
        return destination;
    }
    let delta = i32::from(destination) - i32::from(origin);
    (i32::from(origin) + delta * i32::from(elapsed_ticks) / i32::from(total_ticks)) as u8
}

fn interpolated_camera_keyframe(
    keyframes: &[MissionCameraKeyframe],
    retail_frame: u16,
) -> MissionCameraKeyframe {
    let (start, end) =
        enclosing_keyframes(keyframes, retail_frame, |keyframe| keyframe.retail_frame);
    let numerator = retail_frame.saturating_sub(start.retail_frame);
    let denominator = end.retail_frame.saturating_sub(start.retail_frame);
    MissionCameraKeyframe {
        retail_frame,
        position: interpolate_vector(start.position, end.position, numerator, denominator),
        pitch: interpolate_angle(start.pitch, end.pitch, numerator, denominator).units(),
        yaw: interpolate_angle(start.yaw, end.yaw, numerator, denominator).units(),
        roll: interpolate_angle(start.roll, end.roll, numerator, denominator).units(),
    }
}

fn interpolated_player_keyframe(
    keyframes: &[MissionPlayerKeyframe],
    retail_frame: u16,
) -> MissionPlayerKeyframe {
    let (start, end) =
        enclosing_keyframes(keyframes, retail_frame, |keyframe| keyframe.retail_frame);
    let numerator = retail_frame.saturating_sub(start.retail_frame);
    let denominator = end.retail_frame.saturating_sub(start.retail_frame);
    MissionPlayerKeyframe {
        retail_frame,
        position: interpolate_vector(start.position, end.position, numerator, denominator),
        pitch: interpolate_angle(start.pitch, end.pitch, numerator, denominator).units(),
        yaw: interpolate_angle(start.yaw, end.yaw, numerator, denominator).units(),
        roll: interpolate_angle(start.roll, end.roll, numerator, denominator).units(),
        speed: interpolate_u8(start.speed, end.speed, numerator, denominator),
    }
}

fn apply_player_keyframe(object: &mut Object, keyframe: MissionPlayerKeyframe) {
    object.base.position = keyframe.position;
    object.base.pitch = Angle::from_units(keyframe.pitch);
    object.base.yaw = Angle::from_units(keyframe.yaw);
    object.base.roll = Angle::from_units(keyframe.roll);
    object.base.speed = keyframe.speed;
    object.base.velocity = Vector3::default();
}

impl MissionProjectileTrajectory {
    fn keyframes(self, retail_frame: u16) -> &'static [MissionProjectileKeyframe] {
        match self {
            Self::UpperFighterOpeningShotOne => &UPPER_FIGHTER_OPENING_SHOT_ONE_KEYFRAMES,
            Self::UpperFighterOpeningShotTwo
                if retail_frame <= MISSION_BASE_KEYFRAME_END_RETAIL_FRAME =>
            {
                &UPPER_FIGHTER_OPENING_SHOT_TWO_KEYFRAMES
            }
            Self::UpperFighterOpeningShotTwo => {
                &opening_continuation::UPPER_FIGHTER_OPENING_SHOT_TWO_KEYFRAMES
            }
            Self::LowerFighterOpeningShot
                if retail_frame <= MISSION_BASE_KEYFRAME_END_RETAIL_FRAME =>
            {
                &LOWER_FIGHTER_OPENING_SHOT_KEYFRAMES
            }
            Self::LowerFighterOpeningShot => {
                &opening_continuation::LOWER_FIGHTER_OPENING_SHOT_KEYFRAMES
            }
            Self::SecondCapitalOpeningShotOne => {
                &opening_continuation::SECOND_CAPITAL_OPENING_SHOT_ONE_KEYFRAMES
            }
            Self::UpperFighterOpeningShotThree => {
                &opening_continuation::UPPER_FIGHTER_OPENING_SHOT_THREE_KEYFRAMES
            }
            Self::SecondCapitalOpeningShotTwo => {
                &opening_continuation::SECOND_CAPITAL_OPENING_SHOT_TWO_KEYFRAMES
            }
            Self::SecondCapitalOpeningShotThree => {
                &opening_continuation::SECOND_CAPITAL_OPENING_SHOT_THREE_KEYFRAMES
            }
            Self::FirstCapitalOpeningShot => {
                &opening_continuation::FIRST_CAPITAL_OPENING_SHOT_KEYFRAMES
            }
            Self::UpperFighterOpeningShotFour => {
                &opening_continuation::UPPER_FIGHTER_OPENING_SHOT_FOUR_KEYFRAMES
            }
            Self::UpperFighterOpeningShotFive => {
                &opening_continuation::UPPER_FIGHTER_OPENING_SHOT_FIVE_KEYFRAMES
            }
            Self::SecondCapitalOpeningShotFour => {
                &opening_continuation::SECOND_CAPITAL_OPENING_SHOT_FOUR_KEYFRAMES
            }
            Self::UpperFighterOpeningShotSix => {
                &opening_continuation::UPPER_FIGHTER_OPENING_SHOT_SIX_KEYFRAMES
            }
            Self::SecondCapitalOpeningShotFive => {
                &opening_continuation::SECOND_CAPITAL_OPENING_SHOT_FIVE_KEYFRAMES
            }
            Self::LowerFighterOpeningShotTwo => {
                &opening_continuation::LOWER_FIGHTER_OPENING_SHOT_TWO_KEYFRAMES
            }
            Self::FirstCapitalOpeningShotTwo => {
                &opening_continuation::FIRST_CAPITAL_OPENING_SHOT_TWO_KEYFRAMES
            }
            Self::FirstCapitalOpeningShotThree => {
                &opening_continuation::FIRST_CAPITAL_OPENING_SHOT_THREE_KEYFRAMES
            }
            Self::SecondCapitalMissionShotOne => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_ONE_KEYFRAMES
            }
            Self::SecondCapitalMissionShotTwo => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_TWO_KEYFRAMES
            }
            Self::SecondCapitalMissionShotThree => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_THREE_KEYFRAMES
            }
            Self::SecondCapitalMissionShotFour => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_FOUR_KEYFRAMES
            }
            Self::SecondCapitalMissionShotFive => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_FIVE_KEYFRAMES
            }
            Self::SecondCapitalMissionShotSix => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_SIX_KEYFRAMES
            }
            Self::SecondCapitalMissionShotSeven => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_SEVEN_KEYFRAMES
            }
            Self::SecondCapitalMissionShotEight => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_EIGHT_KEYFRAMES
            }
            Self::SecondCapitalMissionShotNine => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_NINE_KEYFRAMES
            }
            Self::SecondCapitalMissionShotTen => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_TEN_KEYFRAMES
            }
            Self::SecondCapitalMissionShotEleven => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_ELEVEN_KEYFRAMES
            }
            Self::SecondCapitalMissionShotTwelve => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_TWELVE_KEYFRAMES
            }
            Self::SecondCapitalMissionShotThirteen => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_THIRTEEN_KEYFRAMES
            }
            Self::SecondCapitalMissionShotFourteen => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_FOURTEEN_KEYFRAMES
            }
            Self::SecondCapitalMissionShotFifteen => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_FIFTEEN_KEYFRAMES
            }
            Self::SecondCapitalMissionShotSixteen => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_SIXTEEN_KEYFRAMES
            }
            Self::SecondCapitalMissionShotSeventeen => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_SEVENTEEN_KEYFRAMES
            }
            Self::SecondCapitalMissionShotEighteen => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_EIGHTEEN_KEYFRAMES
            }
            Self::SecondCapitalMissionShotNineteen => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_NINETEEN_KEYFRAMES
            }
            Self::SecondCapitalMissionShotTwenty => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_TWENTY_KEYFRAMES
            }
            Self::SecondCapitalMissionShotTwentyOne => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_TWENTY_ONE_KEYFRAMES
            }
            Self::SecondCapitalMissionShotTwentyTwo => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_TWENTY_TWO_KEYFRAMES
            }
            Self::SecondCapitalMissionShotTwentyThree => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_TWENTY_THREE_KEYFRAMES
            }
            Self::SecondCapitalMissionShotTwentyFour => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_TWENTY_FOUR_KEYFRAMES
            }
            Self::SecondCapitalMissionShotTwentyFive => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_TWENTY_FIVE_KEYFRAMES
            }
            Self::SecondCapitalMissionShotTwentySix => {
                &opening_continuation::SECOND_CAPITAL_MISSION_SHOT_TWENTY_SIX_KEYFRAMES
            }
        }
    }

    const fn firing_actor(self) -> MissionEncounterActor {
        match self {
            Self::FirstCapitalOpeningShot
            | Self::FirstCapitalOpeningShotTwo
            | Self::FirstCapitalOpeningShotThree => MissionEncounterActor::FirstCapital,
            Self::SecondCapitalOpeningShotOne
            | Self::SecondCapitalOpeningShotTwo
            | Self::SecondCapitalOpeningShotThree
            | Self::SecondCapitalOpeningShotFour
            | Self::SecondCapitalOpeningShotFive
            | Self::SecondCapitalMissionShotOne
            | Self::SecondCapitalMissionShotTwo
            | Self::SecondCapitalMissionShotThree
            | Self::SecondCapitalMissionShotFour
            | Self::SecondCapitalMissionShotFive
            | Self::SecondCapitalMissionShotSix
            | Self::SecondCapitalMissionShotSeven
            | Self::SecondCapitalMissionShotEight
            | Self::SecondCapitalMissionShotNine
            | Self::SecondCapitalMissionShotTen
            | Self::SecondCapitalMissionShotEleven
            | Self::SecondCapitalMissionShotTwelve
            | Self::SecondCapitalMissionShotThirteen
            | Self::SecondCapitalMissionShotFourteen
            | Self::SecondCapitalMissionShotFifteen
            | Self::SecondCapitalMissionShotSixteen
            | Self::SecondCapitalMissionShotSeventeen
            | Self::SecondCapitalMissionShotEighteen
            | Self::SecondCapitalMissionShotNineteen
            | Self::SecondCapitalMissionShotTwenty
            | Self::SecondCapitalMissionShotTwentyOne
            | Self::SecondCapitalMissionShotTwentyTwo
            | Self::SecondCapitalMissionShotTwentyThree
            | Self::SecondCapitalMissionShotTwentyFour
            | Self::SecondCapitalMissionShotTwentyFive
            | Self::SecondCapitalMissionShotTwentySix => MissionEncounterActor::SecondCapital,
            Self::UpperFighterOpeningShotOne
            | Self::UpperFighterOpeningShotTwo
            | Self::UpperFighterOpeningShotThree
            | Self::UpperFighterOpeningShotFour
            | Self::UpperFighterOpeningShotFive
            | Self::UpperFighterOpeningShotSix => MissionEncounterActor::UpperFighter,
            Self::LowerFighterOpeningShot | Self::LowerFighterOpeningShotTwo => {
                MissionEncounterActor::LowerFighter
            }
        }
    }
}

fn mission_projectile_pose(
    keyframes: &[MissionProjectileKeyframe],
    retail_frame: u16,
) -> MissionEncounterPose {
    let (start, end) =
        enclosing_keyframes(keyframes, retail_frame, |keyframe| keyframe.retail_frame);
    let numerator = retail_frame.saturating_sub(start.retail_frame);
    let denominator = end.retail_frame.saturating_sub(start.retail_frame);
    MissionEncounterPose {
        position: interpolate_vector(
            start.pose.position,
            end.pose.position,
            numerator,
            denominator,
        ),
        pitch: interpolate_angle(start.pose.pitch, end.pose.pitch, numerator, denominator).units(),
        yaw: interpolate_angle(start.pose.yaw, end.pose.yaw, numerator, denominator).units(),
        roll: interpolate_angle(start.pose.roll, end.pose.roll, numerator, denominator).units(),
        speed: interpolate_u8(start.pose.speed, end.pose.speed, numerator, denominator),
    }
}

fn enclosing_camera_keyframes(
    retail_frame: u16,
) -> (
    &'static MissionCameraKeyframe,
    &'static MissionCameraKeyframe,
) {
    if retail_frame > MISSION_BASE_KEYFRAME_END_RETAIL_FRAME {
        enclosing_keyframes(
            &opening_continuation::CAMERA_KEYFRAMES,
            retail_frame,
            |keyframe| keyframe.retail_frame,
        )
    } else {
        enclosing_keyframes(&MISSION_CAMERA_KEYFRAMES, retail_frame, |keyframe| {
            keyframe.retail_frame
        })
    }
}

fn enclosing_formation_keyframes(
    retail_frame: u16,
) -> (
    &'static MissionFormationKeyframe,
    &'static MissionFormationKeyframe,
) {
    enclosing_keyframes(&MISSION_FORMATION_KEYFRAMES, retail_frame, |keyframe| {
        keyframe.retail_frame
    })
}

fn enclosing_player_keyframes(
    retail_frame: u16,
) -> (
    &'static MissionPlayerKeyframe,
    &'static MissionPlayerKeyframe,
) {
    if retail_frame > MISSION_BASE_KEYFRAME_END_RETAIL_FRAME {
        enclosing_keyframes(
            &opening_continuation::PLAYER_KEYFRAMES,
            retail_frame,
            |keyframe| keyframe.retail_frame,
        )
    } else {
        enclosing_keyframes(&MISSION_PLAYER_KEYFRAMES, retail_frame, |keyframe| {
            keyframe.retail_frame
        })
    }
}

fn mission_elapsed_time_tenths(retail_frame: u16) -> u16 {
    let mut elapsed_tenths = 0;
    for keyframe in &opening_continuation::MISSION_TIMER_KEYFRAMES {
        if keyframe.retail_frame > retail_frame {
            break;
        }
        elapsed_tenths = keyframe.elapsed_tenths;
    }
    elapsed_tenths
}

fn enclosing_encounter_keyframes(
    retail_frame: u16,
) -> (
    &'static MissionEncounterKeyframe,
    &'static MissionEncounterKeyframe,
) {
    if retail_frame > MISSION_BASE_KEYFRAME_END_RETAIL_FRAME {
        enclosing_keyframes(
            &opening_continuation::ENCOUNTER_KEYFRAMES,
            retail_frame,
            |keyframe| keyframe.retail_frame,
        )
    } else {
        enclosing_keyframes(&MISSION_ENCOUNTER_KEYFRAMES, retail_frame, |keyframe| {
            keyframe.retail_frame
        })
    }
}

fn enclosing_camera_follow_keyframes(
    retail_frame: u16,
) -> (
    &'static MissionCameraFollowKeyframe,
    &'static MissionCameraFollowKeyframe,
) {
    enclosing_keyframes(&MISSION_CAMERA_FOLLOW_KEYFRAMES, retail_frame, |keyframe| {
        keyframe.retail_frame
    })
}

fn enclosing_keyframes<T>(
    keyframes: &[T],
    retail_frame: u16,
    frame_of: impl Fn(&T) -> u16,
) -> (&T, &T) {
    let first = keyframes
        .first()
        .expect("mission presentation has at least one keyframe");
    if retail_frame <= frame_of(first) {
        return (first, first);
    }
    for pair in keyframes.windows(2) {
        if retail_frame <= frame_of(&pair[1]) {
            return (&pair[0], &pair[1]);
        }
    }
    let last = keyframes
        .last()
        .expect("mission presentation has at least one keyframe");
    (last, last)
}

fn interpolate_vector(start: Vector3, end: Vector3, numerator: u16, denominator: u16) -> Vector3 {
    Vector3 {
        x: interpolate_i16(start.x, end.x, numerator, denominator),
        y: interpolate_i16(start.y, end.y, numerator, denominator),
        z: interpolate_i16(start.z, end.z, numerator, denominator),
    }
}

fn interpolate_encounter_pose(
    start: MissionEncounterPose,
    end: MissionEncounterPose,
    numerator: u16,
    denominator: u16,
) -> MissionEncounterPose {
    MissionEncounterPose {
        position: interpolate_vector(start.position, end.position, numerator, denominator),
        pitch: interpolate_angle(start.pitch, end.pitch, numerator, denominator).units(),
        yaw: interpolate_angle(start.yaw, end.yaw, numerator, denominator).units(),
        roll: interpolate_angle(start.roll, end.roll, numerator, denominator).units(),
        speed: interpolate_u8(start.speed, end.speed, numerator, denominator),
    }
}

fn interpolate_i16(start: i16, end: i16, numerator: u16, denominator: u16) -> i16 {
    if denominator == 0 {
        return start;
    }
    let delta = i32::from(end) - i32::from(start);
    (i32::from(start) + delta * i32::from(numerator) / i32::from(denominator)) as i16
}

fn interpolate_u8(start: u8, end: u8, numerator: u16, denominator: u16) -> u8 {
    if denominator == 0 {
        return start;
    }
    let delta = i32::from(end) - i32::from(start);
    (i32::from(start) + delta * i32::from(numerator) / i32::from(denominator)) as u8
}

fn interpolate_angle(start: u8, end: u8, numerator: u16, denominator: u16) -> Angle {
    if denominator == 0 {
        return Angle::from_units(start);
    }
    let delta = end.wrapping_sub(start) as i8;
    let offset = i32::from(delta) * i32::from(numerator) / i32::from(denominator);
    Angle::from_units(start.wrapping_add_signed(offset as i8))
}

fn add_vectors(left: Vector3, right: Vector3) -> Vector3 {
    Vector3 {
        x: left.x.wrapping_add(right.x),
        y: left.y.wrapping_add(right.y),
        z: left.z.wrapping_add(right.z),
    }
}

fn approach_angle(current: Angle, target: Angle, maximum_step: u8) -> Angle {
    let delta = i16::from(target.units().wrapping_sub(current.units()) as i8);
    if delta.unsigned_abs() <= u16::from(maximum_step) {
        return target;
    }
    current.wrapping_add(if delta > 0 {
        maximum_step as i8
    } else {
        -(maximum_step as i8)
    })
}

fn approach_i16(current: i16, target: i16, maximum_step: i16) -> i16 {
    let delta = i32::from(target) - i32::from(current);
    if delta.unsigned_abs() <= u32::from(maximum_step.unsigned_abs()) {
        return target;
    }
    current.wrapping_add(if delta > 0 {
        maximum_step
    } else {
        maximum_step.wrapping_neg()
    })
}

fn approach_i8(current: i8, target: i8, maximum_step: i8) -> i8 {
    let delta = i16::from(target) - i16::from(current);
    if delta.unsigned_abs() <= u16::from(maximum_step.unsigned_abs()) {
        return target;
    }
    current.wrapping_add(if delta > 0 {
        maximum_step
    } else {
        maximum_step.wrapping_neg()
    })
}

fn approach_u8(current: u8, target: u8, maximum_step: u8) -> u8 {
    if current < target {
        current.saturating_add(maximum_step).min(target)
    } else {
        current.saturating_sub(maximum_step).max(target)
    }
}

fn approach_u16(current: u16, target: u16, maximum_step: u16) -> u16 {
    if current < target {
        current.saturating_add(maximum_step).min(target)
    } else {
        current.saturating_sub(maximum_step).max(target)
    }
}

fn walker_ascent_velocity(ascent_impulse: i16) -> i16 {
    let coarse_impulse = (ascent_impulse >> 8) as i8 as i16;
    coarse_impulse / WALKER_ASCENT_HALF_SCALE_DIVISOR * WALKER_LOCAL_MOTION_SCALE
}

fn walker_fall_step(fall_velocity: i16) -> i16 {
    if (-WALKER_FALL_DEAD_ZONE..WALKER_FALL_DEAD_ZONE).contains(&fall_velocity) {
        0
    } else {
        fall_velocity
    }
}

/// Move by a fixed fraction of the remaining signed distance. A distance
/// smaller than the divisor is widened just enough to preserve one unit of
/// progress, matching the retail Walker spring's convergence behavior.
fn approach_proportional_i16(current: i16, target: i16, divisor: i16) -> i16 {
    debug_assert!(divisor > 0);
    let difference = target.wrapping_sub(current);
    if difference == 0 {
        return current;
    }
    let adjusted_difference = if difference > 0 {
        difference.max(divisor)
    } else {
        difference.min(divisor.wrapping_neg())
    };
    current.wrapping_add(adjusted_difference / divisor)
}

fn approach_proportional_i8(current: i8, target: i8, divisor: i16) -> i8 {
    approach_proportional_i16(i16::from(current), i16::from(target), divisor) as i8
}

/// Proportional wrapped-angle steering used by the first-sortie fighters.
/// Small non-zero errors still move by one angle unit, which prevents a bank
/// or pitch target from stalling as it converges.
fn chase_fighter_angle(current: Angle, target: Angle) -> Angle {
    let difference = target.units().wrapping_sub(current.units()) as i8;
    if difference == 0 {
        return current;
    }
    let adjusted = if difference.unsigned_abs() < 8 {
        if difference.is_positive() {
            8
        } else {
            -8
        }
    } else {
        difference
    };
    current.wrapping_add(adjusted / 8)
}

fn chase_capital_angle(current: Angle, target: Angle, divisor: i8) -> Angle {
    debug_assert!(divisor > 0);
    let difference = target.units().wrapping_sub(current.units()) as i8;
    if difference == 0 {
        return current;
    }
    let adjusted = if difference.unsigned_abs() < divisor as u8 {
        if difference.is_positive() {
            divisor
        } else {
            divisor.wrapping_neg()
        }
    } else {
        difference
    };
    current.wrapping_add(adjusted / divisor)
}

fn hostile_projectile_contracted_position(position: Vector3, target: Vector3) -> Vector3 {
    let delta = [
        position.x.wrapping_sub(target.x),
        position.y.wrapping_sub(target.y),
        position.z.wrapping_sub(target.z),
    ];
    let squared_radius = delta.into_iter().fold(0_u32, |sum, component| {
        let component = i32::from(component);
        sum.wrapping_add((component * component) as u32)
    });
    let radius = squared_radius.isqrt();
    if radius == 0 {
        return position;
    }

    let precision_shift = u32::BITS - 1 - radius.leading_zeros();
    let reciprocal = (NORMALIZED_DIRECTION_SCALE << precision_shift) / i64::from(radius);
    let contracted_radius = (radius as u16).wrapping_sub(HOSTILE_PROJECTILE_CONTRACTION_DISTANCE);
    let contract_component = |component: i16, target_component: i16| {
        let direction = (i64::from(component) * reciprocal) >> precision_shift;
        let offset =
            (direction * i64::from(contracted_radius)) >> NORMALIZED_DIRECTION_FRACTION_BITS;
        target_component.wrapping_add(offset as i16)
    };
    Vector3 {
        x: contract_component(delta[0], target.x),
        y: contract_component(delta[1], target.y),
        z: contract_component(delta[2], target.z),
    }
}

fn contract_hostile_projectile_toward(object: &mut Object, target: Vector3) {
    object.base.position = hostile_projectile_contracted_position(object.base.position, target);
}

fn face_hostile_projectile_toward(object: &mut Object, target: Vector3, smooth: bool) {
    let delta_x = target.x.wrapping_sub(object.base.position.x);
    let delta_y = target.y.wrapping_sub(object.base.position.y);
    let delta_z = target.z.wrapping_sub(object.base.position.z);
    let distance = sf_core::aim_angle::sf2_xz_angle_distance(delta_x, delta_z);
    let target_pitch = sf_core::aim_angle::sf2_pitch_to_target(delta_y, distance);
    let target_yaw = sf_core::aim_angle::sf2_yaw_to_target(delta_x, delta_z);
    if smooth {
        let mut pitch = object.base.pitch.units();
        let mut yaw = object.base.yaw.units();
        sf_core::snes_trig::achase_angle_8(
            &mut pitch,
            target_pitch,
            HOSTILE_PROJECTILE_AIM_CHASE_SHIFT,
        );
        sf_core::snes_trig::achase_angle_8(
            &mut yaw,
            target_yaw,
            HOSTILE_PROJECTILE_AIM_CHASE_SHIFT,
        );
        object.base.pitch = Angle::from_units(pitch);
        object.base.yaw = Angle::from_units(yaw);
    } else {
        object.base.pitch = Angle::from_units(target_pitch);
        object.base.yaw = Angle::from_units(target_yaw);
    }
}

fn advance_hostile_projectile(
    object: &mut Object,
    flight: &mut HostileProjectileFlightState,
    phase: HostileProjectileFlightPhase,
) {
    let velocity = flight_velocity(
        object.base.pitch,
        object.base.yaw,
        object.base.speed,
        MISSION_ENCOUNTER_POSITION_SCALE,
    );
    object.base.velocity = velocity;
    object.base.position.x = object.base.position.x.wrapping_add(velocity.x);
    object.base.position.y = object.base.position.y.wrapping_add(velocity.y);
    object.base.position.z = object.base.position.z.wrapping_add(velocity.z);
    flight.phase = phase;
    flight.motion_steps_elapsed = flight.motion_steps_elapsed.saturating_add(1);
}

fn advance_pigma_rival(object: &mut Object, flight: &mut PigmaRivalFlightState) {
    advance_rival(
        object,
        flight.target_speed,
        flight.acceleration,
        &mut flight.motion_steps_elapsed,
    );
}

fn advance_leon_rival(object: &mut Object, flight: &mut LeonRivalFlightState) {
    debug_assert_eq!(flight.movement_phase, LeonRivalMovementPhase::Ready);
    advance_rival(
        object,
        flight.target_speed,
        flight.acceleration,
        &mut flight.motion_steps_elapsed,
    );
}

fn advance_final_rival(object: &mut Object, flight: &mut FinalRivalFlightState) {
    advance_rival(
        object,
        flight.target_speed,
        flight.acceleration,
        &mut flight.motion_steps_elapsed,
    );
}

fn advance_rival(
    object: &mut Object,
    target_speed: u8,
    acceleration: u8,
    motion_steps_elapsed: &mut u16,
) {
    prepare_rival_advance(object, target_speed, acceleration, motion_steps_elapsed);
    finish_prepared_rival_advance(object);
}

fn prepare_rival_advance(
    object: &mut Object,
    target_speed: u8,
    acceleration: u8,
    motion_steps_elapsed: &mut u16,
) {
    let difference = i16::from(target_speed) - i16::from(object.base.speed);
    let adjustment = difference.unsigned_abs().min(u16::from(acceleration)) as u8;
    object.base.speed = if difference > 0 {
        object.base.speed.saturating_add(adjustment)
    } else if difference < 0 {
        object.base.speed.saturating_sub(adjustment)
    } else {
        object.base.speed
    };
    let velocity = flight_velocity(
        object.base.pitch,
        object.base.yaw,
        object.base.speed,
        MISSION_ENCOUNTER_POSITION_SCALE,
    );
    object.base.velocity = velocity;
    *motion_steps_elapsed = motion_steps_elapsed.saturating_add(1);
}

fn finish_prepared_rival_advance(object: &mut Object) {
    let velocity = object.base.velocity;
    object.base.position.x = object.base.position.x.wrapping_add(velocity.x);
    object.base.position.y = object.base.position.y.wrapping_add(velocity.y);
    object.base.position.z = object.base.position.z.wrapping_add(velocity.z);
}

fn apply_rival_approach_steering(object: &mut Object, steering: RivalApproachSteering) {
    let mut roll = object.base.roll.units();
    let mut pitch = object.base.pitch.units();
    sf_core::snes_trig::achase_angle_8(
        &mut roll,
        steering.roll_target().units(),
        RIVAL_APPROACH_ANGLE_CHASE_SHIFT,
    );
    sf_core::snes_trig::achase_angle_8(
        &mut pitch,
        steering.pitch_target().units(),
        RIVAL_APPROACH_ANGLE_CHASE_SHIFT,
    );
    object.base.roll = Angle::from_units(roll);
    object.base.pitch = Angle::from_units(pitch);
    object.base.yaw = object.base.yaw.wrapping_add(steering.yaw_step());
}

fn chase_rival_roll_to_level(object: &mut Object) {
    let mut roll = object.base.roll.units();
    sf_core::snes_trig::achase_angle_8(
        &mut roll,
        Angle::ZERO.units(),
        RIVAL_APPROACH_ANGLE_CHASE_SHIFT,
    );
    object.base.roll = Angle::from_units(roll);
}

fn chase_pigma_player_altitude(current: i16, target: i16) -> i16 {
    if current == target {
        return current;
    }
    let difference = target.wrapping_sub(current);
    let limited = if (1..8).contains(&difference) {
        8
    } else if (-7..0).contains(&difference) {
        -8
    } else {
        difference
    };
    current.wrapping_add(limited / 8)
}

fn apply_pigma_rival_action(
    object: &mut Object,
    flight: &mut PigmaRivalFlightState,
    action: PigmaRivalAction,
    player_position: Vector3,
    previous_player_position: Vector3,
) {
    match action {
        PigmaRivalAction::BeginApproach => {
            flight.phase = PigmaRivalFlightPhase::Approach;
            flight.target_speed = RIVAL_APPROACH_SPEED;
            flight.acceleration = RIVAL_APPROACH_ACCELERATION;
        }
        PigmaRivalAction::AdvanceApproach(steering) => {
            apply_rival_approach_steering(object, steering);
            advance_pigma_rival(object, flight);
        }
        PigmaRivalAction::BeginCombatManeuver => {
            flight.phase = PigmaRivalFlightPhase::CombatManeuver;
            flight.target_speed = RIVAL_MANEUVER_SPEED;
            flight.acceleration = RIVAL_MANEUVER_ACCELERATION;
        }
        PigmaRivalAction::BeginAttack => {
            flight.phase = PigmaRivalFlightPhase::Attack;
        }
        PigmaRivalAction::MaintainCombatAltitude => {
            object.base.position.y = RIVAL_COMBAT_ALTITUDE;
        }
        PigmaRivalAction::ChaseRollToLevel => {
            chase_rival_roll_to_level(object);
        }
        PigmaRivalAction::FacePlayerYawAndLevelPitch(timing) => {
            let target = timing.select(previous_player_position, player_position);
            let target_yaw = sf_core::aim_angle::sf2_yaw_to_target(
                target.x.wrapping_sub(object.base.position.x),
                target.z.wrapping_sub(object.base.position.z),
            );
            let mut yaw = object.base.yaw.units();
            let mut pitch = object.base.pitch.units();
            sf_core::snes_trig::achase_angle_8(
                &mut yaw,
                target_yaw,
                RIVAL_PLAYER_FACING_CHASE_SHIFT,
            );
            sf_core::snes_trig::achase_angle_8(
                &mut pitch,
                Angle::ZERO.units(),
                PIGMA_PLAYER_PITCH_LEVEL_CHASE_SHIFT,
            );
            object.base.yaw = Angle::from_units(yaw);
            object.base.pitch = Angle::from_units(pitch);
        }
        PigmaRivalAction::FacePlayerSmooth(timing) => {
            let target = timing.select(previous_player_position, player_position);
            let delta_x = target.x.wrapping_sub(object.base.position.x);
            let delta_y = target.y.wrapping_sub(object.base.position.y);
            let delta_z = target.z.wrapping_sub(object.base.position.z);
            let distance = sf_core::aim_angle::sf2_xz_angle_distance(delta_x, delta_z);
            let target_pitch = sf_core::aim_angle::sf2_pitch_to_target(delta_y, distance);
            let target_yaw = sf_core::aim_angle::sf2_yaw_to_target(delta_x, delta_z);
            let mut pitch = object.base.pitch.units();
            let mut yaw = object.base.yaw.units();
            sf_core::snes_trig::achase_angle_8(
                &mut pitch,
                target_pitch,
                RIVAL_PLAYER_FACING_CHASE_SHIFT,
            );
            sf_core::snes_trig::achase_angle_8(
                &mut yaw,
                target_yaw,
                RIVAL_PLAYER_FACING_CHASE_SHIFT,
            );
            object.base.pitch = Angle::from_units(pitch);
            object.base.yaw = Angle::from_units(yaw);
        }
        PigmaRivalAction::Advance => advance_pigma_rival(object, flight),
        PigmaRivalAction::BeginSecondApproach => {
            flight.phase = PigmaRivalFlightPhase::SecondApproach;
            flight.target_speed = RIVAL_APPROACH_SPEED;
            flight.acceleration = RIVAL_APPROACH_ACCELERATION;
            flight.second_approach_wave_step = 0;
            object.base.position.y = object
                .base
                .position
                .y
                .wrapping_add(PIGMA_SECOND_APPROACH_ALTITUDE_OFFSET);
            object.base.roll = object
                .base
                .roll
                .wrapping_add(PIGMA_SECOND_APPROACH_INITIAL_BANK);
        }
        PigmaRivalAction::LaunchSecondApproach => {
            object.base.speed = RIVAL_APPROACH_SPEED;
            advance_pigma_rival(object, flight);
        }
        PigmaRivalAction::ApplySecondApproachWave => {
            let index =
                usize::from(flight.second_approach_wave_step) % PIGMA_SECOND_APPROACH_WAVE.len();
            object.base.roll = object
                .base
                .roll
                .wrapping_add(PIGMA_SECOND_APPROACH_WAVE[index]);
            object.base.position.y = object
                .base
                .position
                .y
                .wrapping_add(PIGMA_SECOND_APPROACH_VERTICAL_STEP);
            flight.second_approach_wave_step = flight.second_approach_wave_step.wrapping_add(1);
        }
        PigmaRivalAction::BeginDeceleration => {
            flight.phase = PigmaRivalFlightPhase::Deceleration;
            flight.target_speed = PIGMA_DECELERATION_SPEED;
            flight.acceleration = PIGMA_DECELERATION_RATE;
        }
        PigmaRivalAction::BeginEscape => {
            flight.phase = PigmaRivalFlightPhase::Escape;
            flight.target_speed = PIGMA_ESCAPE_SPEED;
            flight.acceleration = PIGMA_ESCAPE_DECELERATION;
            flight.escape_wobble_step = 0;
        }
        PigmaRivalAction::TurnAway => {
            object.base.yaw = object.base.yaw.wrapping_add(PIGMA_ESCAPE_YAW_STEP);
        }
        PigmaRivalAction::TurnAwayAndAdvance => {
            object.base.yaw = object.base.yaw.wrapping_add(PIGMA_ESCAPE_YAW_STEP);
            advance_pigma_rival(object, flight);
        }
        PigmaRivalAction::ChasePlayerAltitude(timing) => {
            let target = timing.select(flight.earlier_player_altitude, previous_player_position.y);
            object.base.position.y = chase_pigma_player_altitude(object.base.position.y, target);
        }
        PigmaRivalAction::ApplyEscapeWobble => {
            let index = usize::from(flight.escape_wobble_step) % PIGMA_ESCAPE_WOBBLE.len();
            object.base.roll = object.base.roll.wrapping_add(PIGMA_ESCAPE_WOBBLE[index]);
            flight.escape_wobble_step = flight.escape_wobble_step.wrapping_add(1);
        }
    }
}

fn apply_leon_rival_action(
    object: &mut Object,
    flight: &mut LeonRivalFlightState,
    action: LeonRivalAction,
) {
    match action {
        LeonRivalAction::BeginApproach => {
            flight.phase = LeonRivalFlightPhase::Approach;
            flight.target_speed = RIVAL_APPROACH_SPEED;
            flight.acceleration = RIVAL_APPROACH_ACCELERATION;
        }
        LeonRivalAction::AdvanceApproach(steering) => {
            apply_rival_approach_steering(object, steering);
            advance_leon_rival(object, flight);
        }
        LeonRivalAction::PrepareApproachAdvance(steering) => {
            debug_assert_eq!(flight.movement_phase, LeonRivalMovementPhase::Ready);
            apply_rival_approach_steering(object, steering);
            prepare_rival_advance(
                object,
                flight.target_speed,
                flight.acceleration,
                &mut flight.motion_steps_elapsed,
            );
            flight.movement_phase = LeonRivalMovementPhase::PreparedAdvance;
        }
        LeonRivalAction::FinishPreparedApproachAdvance => {
            debug_assert_eq!(
                flight.movement_phase,
                LeonRivalMovementPhase::PreparedAdvance
            );
            finish_prepared_rival_advance(object);
            flight.movement_phase = LeonRivalMovementPhase::Ready;
        }
        LeonRivalAction::BeginCombatManeuver => {
            flight.phase = LeonRivalFlightPhase::CombatManeuver;
            flight.target_speed = RIVAL_MANEUVER_SPEED;
            flight.acceleration = RIVAL_MANEUVER_ACCELERATION;
        }
        LeonRivalAction::MaintainCombatAltitude => {
            object.base.position.y = RIVAL_COMBAT_ALTITUDE;
        }
        LeonRivalAction::ChaseRollToLevel => chase_rival_roll_to_level(object),
        LeonRivalAction::Advance => advance_leon_rival(object, flight),
    }
}

fn apply_final_rival_action(
    object: &mut Object,
    flight: &mut FinalRivalFlightState,
    action: FinalRivalAction,
    player_position: Vector3,
    previous_player_position: Vector3,
) {
    match action {
        FinalRivalAction::BeginApproach => {
            flight.phase = FinalRivalFlightPhase::Approach;
            flight.target_speed = RIVAL_APPROACH_SPEED;
            flight.acceleration = RIVAL_APPROACH_ACCELERATION;
        }
        FinalRivalAction::AdvanceSteered(steering) => {
            apply_rival_approach_steering(object, steering);
            advance_final_rival(object, flight);
        }
        FinalRivalAction::BeginCombatManeuver => {
            flight.phase = FinalRivalFlightPhase::CombatManeuver;
            flight.target_speed = RIVAL_MANEUVER_SPEED;
            flight.acceleration = RIVAL_MANEUVER_ACCELERATION;
        }
        FinalRivalAction::BeginAttack => {
            flight.phase = FinalRivalFlightPhase::Attack;
        }
        FinalRivalAction::MaintainCombatAltitude => {
            object.base.position.y = RIVAL_COMBAT_ALTITUDE;
        }
        FinalRivalAction::ClampFlightAltitude => {
            object.base.position.y = object
                .base
                .position
                .y
                .clamp(RIVAL_COMBAT_ALTITUDE, -RIVAL_COMBAT_ALTITUDE);
        }
        FinalRivalAction::ChaseRollToLevel => chase_rival_roll_to_level(object),
        FinalRivalAction::FacePlayerYawAndLevelPitch(timing) => {
            let target = timing.select(previous_player_position, player_position);
            let target_yaw = sf_core::aim_angle::sf2_yaw_to_target(
                target.x.wrapping_sub(object.base.position.x),
                target.z.wrapping_sub(object.base.position.z),
            );
            let mut yaw = object.base.yaw.units();
            let mut pitch = object.base.pitch.units();
            sf_core::snes_trig::achase_angle_8(
                &mut yaw,
                target_yaw,
                RIVAL_PLAYER_FACING_CHASE_SHIFT,
            );
            sf_core::snes_trig::achase_angle_8(
                &mut pitch,
                Angle::ZERO.units(),
                FINAL_RIVAL_PITCH_LEVEL_CHASE_SHIFT,
            );
            object.base.yaw = Angle::from_units(yaw);
            object.base.pitch = Angle::from_units(pitch);
        }
        FinalRivalAction::FacePlayerSmooth(timing) => {
            let target = timing.select(previous_player_position, player_position);
            let delta_x = target.x.wrapping_sub(object.base.position.x);
            let delta_y = target.y.wrapping_sub(object.base.position.y);
            let delta_z = target.z.wrapping_sub(object.base.position.z);
            let distance = sf_core::aim_angle::sf2_xz_angle_distance(delta_x, delta_z);
            let target_pitch = sf_core::aim_angle::sf2_pitch_to_target(delta_y, distance);
            let target_yaw = sf_core::aim_angle::sf2_yaw_to_target(delta_x, delta_z);
            let mut pitch = object.base.pitch.units();
            let mut yaw = object.base.yaw.units();
            sf_core::snes_trig::achase_angle_8(
                &mut pitch,
                target_pitch,
                RIVAL_PLAYER_FACING_CHASE_SHIFT,
            );
            sf_core::snes_trig::achase_angle_8(
                &mut yaw,
                target_yaw,
                RIVAL_PLAYER_FACING_CHASE_SHIFT,
            );
            object.base.pitch = Angle::from_units(pitch);
            object.base.yaw = Angle::from_units(yaw);
        }
        FinalRivalAction::Advance => advance_final_rival(object, flight),
        FinalRivalAction::BeginDeparture => {
            flight.phase = FinalRivalFlightPhase::Departure;
            flight.target_speed = RIVAL_APPROACH_SPEED;
            flight.acceleration = RIVAL_APPROACH_ACCELERATION;
        }
        FinalRivalAction::LaunchDeparture => {
            object.base.speed = RIVAL_APPROACH_SPEED;
            advance_final_rival(object, flight);
        }
    }
}

fn apply_hostile_projectile_action(
    object: &mut Object,
    flight: &mut HostileProjectileFlightState,
    action: HostileProjectileAction,
    player_position: Vector3,
    previous_player_position: Vector3,
) {
    let target =
        |timing: HostileProjectileTarget| timing.select(previous_player_position, player_position);
    match action {
        HostileProjectileAction::ContractTowardTarget(timing) => {
            contract_hostile_projectile_toward(object, target(timing));
        }
        HostileProjectileAction::BeginTargetContraction(timing) => {
            let contracted =
                hostile_projectile_contracted_position(object.base.position, target(timing));
            object.base.position.x = contracted.x;
            flight.movement_phase = HostileProjectileMovementPhase::TargetContractionPending {
                altitude: contracted.y,
                depth: contracted.z,
            };
        }
        HostileProjectileAction::FinishTargetContraction => {
            let HostileProjectileMovementPhase::TargetContractionPending { altitude, depth } =
                flight.movement_phase
            else {
                debug_assert!(
                    false,
                    "projectile target contraction has no pending position"
                );
                return;
            };
            object.base.position.y = altitude;
            object.base.position.z = depth;
            flight.movement_phase = HostileProjectileMovementPhase::Ready;
        }
        HostileProjectileAction::FaceTargetImmediate(timing) => {
            face_hostile_projectile_toward(object, target(timing), false);
        }
        HostileProjectileAction::FaceTargetSmooth(timing) => {
            face_hostile_projectile_toward(object, target(timing), true);
        }
        HostileProjectileAction::SetCruiseSpeed => {
            object.base.speed = HOSTILE_PROJECTILE_CRUISE_SPEED;
        }
        HostileProjectileAction::AdvanceHoming => {
            advance_hostile_projectile(object, flight, HostileProjectileFlightPhase::Homing);
        }
        HostileProjectileAction::AdvanceAimCorrection => {
            advance_hostile_projectile(object, flight, HostileProjectileFlightPhase::AimCorrection);
        }
        HostileProjectileAction::AdvanceCruise => {
            advance_hostile_projectile(object, flight, HostileProjectileFlightPhase::Cruise);
        }
    }
}

fn apply_capital_flight_action(
    object: &mut Object,
    flight: &mut CapitalFlightState,
    action: CapitalFlightAction,
    player_position: Vector3,
    previous_player_position: Vector3,
) {
    match action {
        CapitalFlightAction::BeginPitchManeuver(direction) => {
            object.base.roll = direction.bank();
        }
        CapitalFlightAction::ChasePitch(target) => {
            object.base.pitch = chase_capital_angle(
                object.base.pitch,
                target.angle(),
                CAPITAL_ANGLE_CHASE_DIVISOR,
            );
        }
        CapitalFlightAction::ChaseRollToLevel => {
            object.base.roll =
                chase_capital_angle(object.base.roll, Angle::ZERO, CAPITAL_ANGLE_CHASE_DIVISOR);
        }
        CapitalFlightAction::ChaseBank(target) => {
            object.base.roll = chase_capital_angle(
                object.base.roll,
                target.angle(),
                CAPITAL_ANGLE_CHASE_DIVISOR,
            );
        }
        CapitalFlightAction::FacePlayer | CapitalFlightAction::FacePlayerAt(_) => {
            let target_position = match action {
                CapitalFlightAction::FacePlayerAt(timing) => {
                    timing.select(previous_player_position, player_position)
                }
                CapitalFlightAction::FacePlayer => player_position,
                _ => unreachable!(),
            };
            let delta_x = target_position.x.wrapping_sub(object.base.position.x);
            let delta_z = target_position.z.wrapping_sub(object.base.position.z);
            let target = Angle::from_units(sf_core::aim_angle::sf2_yaw_to_target(delta_x, delta_z));
            object.base.yaw =
                chase_capital_angle(object.base.yaw, target, CAPITAL_PLAYER_FACING_DIVISOR);
        }
        CapitalFlightAction::CenterAltitude => {
            object.base.position.y = approach_proportional_i16(
                object.base.position.y,
                0,
                CAPITAL_ALTITUDE_CENTERING_DIVISOR,
            );
        }
        CapitalFlightAction::SetSpeed(speed) => {
            object.base.speed = speed.units();
        }
        CapitalFlightAction::ApplyBankTurn => {
            let bank_turn = (object.base.roll.units() as i8) / CAPITAL_BANK_TURN_DIVISOR;
            object.base.yaw = object.base.yaw.wrapping_add(bank_turn);
        }
        CapitalFlightAction::Move(turn_mode) | CapitalFlightAction::MoveHorizontal(turn_mode) => {
            if turn_mode == CapitalTurnMode::Banked {
                let bank_turn = (object.base.roll.units() as i8) / CAPITAL_BANK_TURN_DIVISOR;
                object.base.yaw = object.base.yaw.wrapping_add(bank_turn);
            }
            let velocity = flight_velocity(
                object.base.pitch,
                object.base.yaw,
                object.base.speed,
                MISSION_ENCOUNTER_POSITION_SCALE,
            );
            object.base.velocity = velocity;
            object.base.position.x = object.base.position.x.wrapping_add(velocity.x);
            if action == CapitalFlightAction::MoveHorizontal(turn_mode) {
                flight.pending_velocity = velocity;
                flight.movement_phase = CapitalMovementPhase::HorizontalApplied;
            } else {
                object.base.position.y = object.base.position.y.wrapping_add(velocity.y);
                object.base.position.z = object.base.position.z.wrapping_add(velocity.z);
                flight.pending_velocity = Vector3::default();
                flight.movement_phase = CapitalMovementPhase::Ready;
            }
        }
        CapitalFlightAction::FinishMovement => {
            debug_assert_eq!(
                flight.movement_phase,
                CapitalMovementPhase::HorizontalApplied
            );
            object.base.position.y = object
                .base
                .position
                .y
                .wrapping_add(flight.pending_velocity.y);
            object.base.position.z = object
                .base
                .position
                .z
                .wrapping_add(flight.pending_velocity.z);
            flight.pending_velocity = Vector3::default();
            flight.movement_phase = CapitalMovementPhase::Ready;
        }
        CapitalFlightAction::ApplyVerticalWave => {
            let displacement =
                i16::from(sf_core::snes_trig::COSTAB[flight.vertical_wave_phase.units() as usize]);
            object.base.position.y = object.base.position.y.wrapping_add(displacement);
            flight.vertical_wave_phase = flight.vertical_wave_phase.wrapping_add(1);
        }
        CapitalFlightAction::ApplyVerticalWaveMode(mode) => {
            object.base.position.y = object
                .base
                .position
                .y
                .wrapping_add(mode.displacement(flight.vertical_wave_phase));
            flight.vertical_wave_phase = flight.vertical_wave_phase.wrapping_add(mode.phase_step());
        }
        CapitalFlightAction::AimWeapon
        | CapitalFlightAction::AimWeaponPitch
        | CapitalFlightAction::AimWeaponAt(_) => {
            let target_position = match action {
                CapitalFlightAction::AimWeaponAt(timing) => {
                    timing.select(previous_player_position, player_position)
                }
                CapitalFlightAction::AimWeapon | CapitalFlightAction::AimWeaponPitch => {
                    player_position
                }
                _ => unreachable!(),
            };
            let flight_angles = CapitalFlightAngles {
                pitch: object.base.pitch,
                yaw: object.base.yaw,
                roll: object.base.roll,
            };
            flight.weapon_phase = CapitalWeaponPhase::Aiming { flight_angles };
            let delta_x = target_position.x.wrapping_sub(object.base.position.x);
            let delta_y = target_position.y.wrapping_sub(object.base.position.y);
            let delta_z = target_position.z.wrapping_sub(object.base.position.z);
            let distance = sf_core::aim_angle::sf2_xz_angle_distance(delta_x, delta_z);
            object.base.pitch =
                Angle::from_units(sf_core::aim_angle::sf2_pitch_to_target(delta_y, distance));
            if matches!(
                action,
                CapitalFlightAction::AimWeapon | CapitalFlightAction::AimWeaponAt(_)
            ) {
                object.base.yaw =
                    Angle::from_units(sf_core::aim_angle::sf2_yaw_to_target(delta_x, delta_z));
            }
        }
        CapitalFlightAction::RestoreFlightAngles => {
            let CapitalWeaponPhase::Aiming { flight_angles } = flight.weapon_phase else {
                debug_assert!(
                    false,
                    "capital weapon restoration requires a saved flight pose"
                );
                return;
            };
            object.base.pitch = flight_angles.pitch;
            object.base.yaw = flight_angles.yaw;
            object.base.roll = flight_angles.roll;
            flight.weapon_phase = CapitalWeaponPhase::Ready;
        }
    }
}

fn half_signed_angle(angle: Angle) -> Angle {
    Angle::from_units(((angle.units() as i8) / 2) as u8)
}

fn fighter_fire_pitch_target(player: Vector3, fighter: Vector3) -> Angle {
    let delta_x = player.x.wrapping_sub(fighter.x);
    let delta_y = player.y.wrapping_sub(fighter.y);
    let delta_z = player.z.wrapping_sub(fighter.z);
    let aim_pitch = sf_core::aim_angle::atan2_to_u8(
        f32::from(delta_y),
        f32::from(sf_core::aim_angle::sf2_xz_angle_distance(delta_x, delta_z)),
    );
    let half = half_signed_angle(Angle::from_units(aim_pitch));
    let quarter = half_signed_angle(half);
    Angle::from_units(half.units().wrapping_add(quarter.units()).wrapping_neg())
}

fn finish_fighter_steering(
    object: &mut Object,
    fighter: &mut FighterFlightState,
    dispatch: FighterLogicDispatch,
    player_position: Vector3,
) -> (bool, bool, bool) {
    let pitch_target = match fighter.altitude_phase {
        FighterAltitudePhase::Wave => {
            let pitch_target = fighter.vertical_pitch_target;
            let wave_sample = fighter
                .vertical_wave_polarity
                .apply(sf_core::snes_trig::COSTAB[fighter.vertical_wave_phase.units() as usize]);
            let vertical_displacement =
                i16::from(wave_sample).wrapping_mul(FIGHTER_VERTICAL_WAVE_POSITION_SCALE);
            match dispatch {
                FighterLogicDispatch::PrepareWave => {
                    fighter.pending_vertical_displacement = vertical_displacement;
                }
                FighterLogicDispatch::SplitWave => {
                    let first_displacement = vertical_displacement / 2;
                    object.base.position.y =
                        object.base.position.y.wrapping_add(first_displacement);
                    fighter.pending_vertical_displacement =
                        vertical_displacement.wrapping_sub(first_displacement);
                }
                FighterLogicDispatch::QuarterWave => {
                    let first_displacement = vertical_displacement / FIGHTER_QUARTER_WAVE_DIVISOR;
                    object.base.position.y =
                        object.base.position.y.wrapping_add(first_displacement);
                    fighter.pending_vertical_displacement =
                        vertical_displacement.wrapping_sub(first_displacement);
                }
                _ => {
                    object.base.position.y =
                        object.base.position.y.wrapping_add(vertical_displacement);
                }
            }
            fighter.vertical_wave_phase = fighter
                .vertical_wave_direction
                .advance(fighter.vertical_wave_phase, FIGHTER_VERTICAL_WAVE_STEP);
            fighter.vertical_pitch_target = Angle::from_units((wave_sample / 2) as u8);
            match fighter.vertical_wave_order {
                FighterWaveOrder::BeforeSteering => fighter.vertical_pitch_target,
                FighterWaveOrder::AfterSteering => pitch_target,
            }
        }
        FighterAltitudePhase::Centering { ticks_remaining } => {
            if !matches!(
                dispatch,
                FighterLogicDispatch::PitchContinuation
                    | FighterLogicDispatch::CompleteAfterEarlyAltitude
                    | FighterLogicDispatch::MovementContinuationAfterEarlyAltitude
                    | FighterLogicDispatch::SteeringAfterEarlyAltitude
            ) {
                object.base.position.y = chase_proportional(object.base.position.y, 0, 3);
            }
            let retained_pitch_target = fighter.vertical_pitch_target;
            fighter.vertical_pitch_target = half_signed_angle(retained_pitch_target);
            let centering_pitch_target = match fighter.centering_target_order {
                FighterCenteringTargetOrder::BeforeSteering => fighter.vertical_pitch_target,
                FighterCenteringTargetOrder::AfterSteering => retained_pitch_target,
            };
            if ticks_remaining <= 1 {
                fighter.altitude_phase = FighterAltitudePhase::Wave;
                let wave_sample = fighter.vertical_wave_polarity.apply(
                    sf_core::snes_trig::COSTAB[fighter.vertical_wave_phase.units() as usize],
                );
                object.base.position.y = object.base.position.y.wrapping_add(
                    i16::from(wave_sample).wrapping_mul(FIGHTER_VERTICAL_WAVE_POSITION_SCALE),
                );
                fighter.vertical_wave_phase = fighter
                    .vertical_wave_direction
                    .advance(fighter.vertical_wave_phase, FIGHTER_VERTICAL_WAVE_STEP);
                fighter.vertical_pitch_target = Angle::from_units((wave_sample / 2) as u8);
            } else {
                fighter.altitude_phase = FighterAltitudePhase::Centering {
                    ticks_remaining: ticks_remaining - 1,
                };
            }
            centering_pitch_target
        }
    };
    object.base.pitch = chase_fighter_angle(object.base.pitch, pitch_target);
    if dispatch != FighterLogicDispatch::PitchContinuation {
        object.base.roll = chase_fighter_angle(object.base.roll, fighter.maneuver_bank);
    }

    let maneuver_due = fighter.maneuver_ticks_remaining == 0;
    if !maneuver_due {
        fighter.maneuver_ticks_remaining -= 1;
    }
    let fire_due = fighter.fire_ticks_remaining == 0;
    if !fire_due {
        fighter.fire_ticks_remaining -= 1;
    }
    let delta_x = i32::from(player_position.x) - i32::from(object.base.position.x);
    let delta_z = i32::from(player_position.z) - i32::from(object.base.position.z);
    let fire_range = i64::from(FIGHTER_FIRE_RANGE);
    let within_fire_range = i64::from(delta_x) * i64::from(delta_x)
        + i64::from(delta_z) * i64::from(delta_z)
        < fire_range * fire_range;
    (maneuver_due, fire_due, within_fire_range)
}

fn chase_proportional(current: i16, target: i16, shift: u32) -> i16 {
    if current == target {
        return current;
    }
    let difference = target.wrapping_sub(current);
    let mut step = if difference >= 0 {
        difference >> shift
    } else {
        -((-(i32::from(difference)) >> shift) as i16)
    };
    if step == 0 {
        step = if difference > 0 { 1 } else { -1 };
    }
    current.wrapping_add(step)
}

const fn fighter_logic_credit_threshold(cadence: FighterLogicCadence) -> u8 {
    match cadence {
        FighterLogicCadence::EntryChase => FIGHTER_ENTRY_LOGIC_CREDIT_THRESHOLD,
        FighterLogicCadence::Combat => FIGHTER_COMBAT_LOGIC_CREDIT_THRESHOLD,
    }
}

fn fighter_logic_dispatch(retail_frame: u16) -> Option<FighterLogicDispatchPair> {
    if retail_frame <= fighter_continuation::END_RETAIL_FRAME {
        if let Some(offset) = retail_frame.checked_sub(fighter_continuation::START_RETAIL_FRAME) {
            if offset % FIGHTER_COOPERATIVE_SCHEDULE_STEP == 0 {
                if let Some(dispatch) = fighter_continuation::DISPATCH
                    .get(usize::from(offset / FIGHTER_COOPERATIVE_SCHEDULE_STEP))
                {
                    return Some(*dispatch);
                }
            }
        }
    }
    if let Some(offset) =
        retail_frame.checked_sub(FIGHTER_COOPERATIVE_CONTINUATION_START_RETAIL_FRAME)
    {
        if offset % FIGHTER_COOPERATIVE_SCHEDULE_STEP == 0 {
            if let Some(dispatch) = FIGHTER_COOPERATIVE_CONTINUATION
                .get(usize::from(offset / FIGHTER_COOPERATIVE_SCHEDULE_STEP))
            {
                return Some(*dispatch);
            }
        }
    }
    let offset = retail_frame.checked_sub(FIGHTER_COOPERATIVE_SCHEDULE_START_RETAIL_FRAME)?;
    (offset % FIGHTER_COOPERATIVE_SCHEDULE_STEP == 0).then_some(())?;
    FIGHTER_COOPERATIVE_SCHEDULE
        .get(usize::from(offset / FIGHTER_COOPERATIVE_SCHEDULE_STEP))
        .copied()
}

fn fighter_random_cadence(retail_frame: u16) -> Option<FighterRandomCadence> {
    if retail_frame <= fighter_continuation::END_RETAIL_FRAME {
        if let Some(offset) = retail_frame.checked_sub(fighter_continuation::START_RETAIL_FRAME) {
            if offset % FIGHTER_COOPERATIVE_SCHEDULE_STEP == 0 {
                if let Some(cadence) = fighter_continuation::RANDOM_CADENCE
                    .get(usize::from(offset / FIGHTER_COOPERATIVE_SCHEDULE_STEP))
                {
                    return Some(*cadence);
                }
            }
        }
    }
    if let Some(offset) = retail_frame.checked_sub(FIGHTER_RANDOM_CONTINUATION_START_RETAIL_FRAME) {
        if offset % FIGHTER_COOPERATIVE_SCHEDULE_STEP == 0 {
            if let Some(cadence) = FIGHTER_RANDOM_CONTINUATION
                .get(usize::from(offset / FIGHTER_COOPERATIVE_SCHEDULE_STEP))
            {
                return Some(*cadence);
            }
        }
    }
    let offset = retail_frame.checked_sub(FIGHTER_RANDOM_CADENCE_START_RETAIL_FRAME)?;
    (offset % FIGHTER_COOPERATIVE_SCHEDULE_STEP == 0).then_some(())?;
    FIGHTER_RANDOM_CADENCE
        .get(usize::from(offset / FIGHTER_COOPERATIVE_SCHEDULE_STEP))
        .copied()
}

const fn post_maneuver_logic_credit(actor: MissionEncounterActor) -> u8 {
    match actor {
        MissionEncounterActor::UpperFighter => 0,
        MissionEncounterActor::LowerFighter => LOWER_FIGHTER_POST_MANEUVER_LOGIC_CREDIT,
        MissionEncounterActor::FirstCapital | MissionEncounterActor::SecondCapital => 0,
    }
}

fn accumulator_angle(accumulator: i16) -> Angle {
    Angle::from_units((accumulator >> FLIGHT_ACCUMULATOR_FRACTION_BITS) as u8)
}

fn visible_pitch_from_lean(pitch_lean: i8) -> Angle {
    let inverted_lean = i16::from(pitch_lean).wrapping_neg();
    Angle::from_units(
        (inverted_lean + (inverted_lean >> PLAYER_VISIBLE_PITCH_LEAN_SHIFT)) as i8 as u8,
    )
}

/// Exact signed-logarithmic direction vector shared by retail flight paths.
/// `position_scale` expresses the source path's world-step multiplier as an
/// ordinary gameplay unit rather than exposing encoded path operands.
fn flight_velocity(pitch: Angle, yaw: Angle, speed: u8, position_scale: i16) -> Vector3 {
    let source_yaw = yaw.units().wrapping_neg();
    let source_pitch = pitch.units();
    let signed_speed = speed as i8;
    let cos_pitch = sf_core::snes_trig::COSTAB[source_pitch as usize];
    let x = sf_core::snes_trig::mulslog_mac8(
        sf_core::snes_trig::mulslog_mac8(
            signed_speed,
            sf_core::snes_trig::SINTAB[source_yaw as usize],
        ),
        cos_pitch,
    );
    let y = sf_core::snes_trig::mulslog_mac8(
        signed_speed,
        sf_core::snes_trig::SINTAB[source_pitch as usize],
    );
    let z = sf_core::snes_trig::mulslog_mac8(
        sf_core::snes_trig::mulslog_mac8(
            signed_speed,
            sf_core::snes_trig::COSTAB[source_yaw as usize],
        ),
        cos_pitch,
    );
    Vector3 {
        x: i16::from(x).wrapping_mul(position_scale),
        y: i16::from(y).wrapping_mul(position_scale),
        z: i16::from(z).wrapping_mul(position_scale),
    }
}

fn apply_reengagement_fighter_action(
    object: &mut Object,
    flight: &mut ReengagementFighterFlightState,
    action: ReengagementFighterAction,
) {
    match action {
        ReengagementFighterAction::EntrySetup => {
            object.base.position.y = object
                .base
                .position
                .y
                .wrapping_add(REENGAGEMENT_FIGHTER_ENTRY_ALTITUDE_OFFSET);
            object.base.yaw = Angle::from_units(REENGAGEMENT_FIGHTER_ENTRY_YAW_UNITS);
            object.base.speed = REENGAGEMENT_FIGHTER_ENTRY_SPEED;
            flight.vertical_wave_phase = Angle::from_units(REENGAGEMENT_FIGHTER_INITIAL_WAVE_PHASE);
        }
        ReengagementFighterAction::SetBankTarget(target) => {
            flight.maneuver_bank = target.angle();
        }
        ReengagementFighterAction::BeginEntryTurn(direction) => {
            flight.vertical_wave_phase = direction.wave_phase();
            flight.maneuver_bank = direction.bank().angle();
        }
        ReengagementFighterAction::BeginManeuver(target) => {
            flight.maneuver_bank = target.angle();
            flight.altitude_phase = FighterAltitudePhase::Centering {
                ticks_remaining: REENGAGEMENT_FIGHTER_ALTITUDE_CENTERING_TICKS,
            };
        }
        ReengagementFighterAction::ChaseRoll(target) => {
            object.base.roll = chase_fighter_angle(object.base.roll, target.angle());
        }
        ReengagementFighterAction::CenterAltitudeDuringManeuver => {
            if matches!(
                flight.altitude_phase,
                FighterAltitudePhase::Centering { .. }
            ) {
                object.base.position.y = chase_proportional(object.base.position.y, 0, 3);
            }
        }
        ReengagementFighterAction::CenterAltitude => {
            object.base.position.y = chase_proportional(object.base.position.y, 0, 3);
        }
        ReengagementFighterAction::Move(acceleration)
        | ReengagementFighterAction::BeginMovement(acceleration) => {
            if acceleration == ReengagementFighterAcceleration::Accelerate {
                object.base.speed = object
                    .base
                    .speed
                    .saturating_add(REENGAGEMENT_FIGHTER_ACCELERATION)
                    .min(REENGAGEMENT_FIGHTER_MAXIMUM_SPEED);
            }
            let bank_turn =
                (object.base.roll.units() as i8) / REENGAGEMENT_FIGHTER_BANK_TURN_DIVISOR;
            object.base.yaw = object.base.yaw.wrapping_add(bank_turn);
            let velocity = flight_velocity(
                object.base.pitch,
                object.base.yaw,
                object.base.speed,
                MISSION_ENCOUNTER_POSITION_SCALE,
            );
            object.base.velocity = velocity;
            object.base.position.x = object.base.position.x.wrapping_add(velocity.x);
            if matches!(action, ReengagementFighterAction::BeginMovement(_)) {
                flight.pending_velocity = velocity;
                flight.movement_phase = ReengagementFighterMovementPhase::HorizontalApplied;
            } else {
                object.base.position.y = object.base.position.y.wrapping_add(velocity.y);
                object.base.position.z = object.base.position.z.wrapping_add(velocity.z);
                flight.pending_velocity = Vector3::default();
                flight.movement_phase = ReengagementFighterMovementPhase::Ready;
            }
        }
        ReengagementFighterAction::FinishMovement => {
            debug_assert_eq!(
                flight.movement_phase,
                ReengagementFighterMovementPhase::HorizontalApplied
            );
            object.base.position.y = object
                .base
                .position
                .y
                .wrapping_add(flight.pending_velocity.y);
            object.base.position.z = object
                .base
                .position
                .z
                .wrapping_add(flight.pending_velocity.z);
            flight.pending_velocity = Vector3::default();
            flight.movement_phase = ReengagementFighterMovementPhase::Ready;
        }
        ReengagementFighterAction::ApplyEntryWave => {
            let displacement =
                i16::from(sf_core::snes_trig::COSTAB[flight.vertical_wave_phase.units() as usize])
                    / REENGAGEMENT_FIGHTER_ENTRY_WAVE_DIVISOR;
            object.base.position.y = object.base.position.y.wrapping_add(displacement);
            flight.vertical_wave_phase = flight
                .vertical_wave_phase
                .wrapping_add(REENGAGEMENT_FIGHTER_ENTRY_WAVE_STEP);
        }
        ReengagementFighterAction::ApplyWaveQuarter => {
            if flight.vertical_wave_quarters_applied == 0 {
                flight.vertical_wave_sample =
                    sf_core::snes_trig::COSTAB[flight.vertical_wave_phase.units() as usize];
            }
            object.base.position.y = object
                .base
                .position
                .y
                .wrapping_add(i16::from(flight.vertical_wave_sample));
            flight.vertical_wave_quarters_applied += 1;
            if flight.vertical_wave_quarters_applied == REENGAGEMENT_FIGHTER_WAVE_QUARTERS {
                flight.vertical_wave_quarters_applied = 0;
                flight.vertical_wave_phase = flight
                    .vertical_wave_phase
                    .wrapping_add(REENGAGEMENT_FIGHTER_COMBAT_WAVE_STEP);
                flight.vertical_pitch_target = Angle::from_units(
                    (flight.vertical_wave_sample / REENGAGEMENT_FIGHTER_PITCH_TARGET_DIVISOR) as u8,
                );
            }
        }
        ReengagementFighterAction::ChasePitch => {
            object.base.pitch =
                chase_fighter_angle(object.base.pitch, flight.vertical_pitch_target);
            if let FighterAltitudePhase::Centering { ticks_remaining } = flight.altitude_phase {
                flight.vertical_pitch_target = half_signed_angle(flight.vertical_pitch_target);
                flight.altitude_phase = if ticks_remaining <= 1 {
                    FighterAltitudePhase::Wave
                } else {
                    FighterAltitudePhase::Centering {
                        ticks_remaining: ticks_remaining - 1,
                    }
                };
            }
        }
        ReengagementFighterAction::ChaseBank => {
            object.base.roll = chase_fighter_angle(object.base.roll, flight.maneuver_bank);
        }
        ReengagementFighterAction::SetWeaponPitchTarget(target) => {
            flight.vertical_pitch_target = target.angle();
        }
    }
}

fn apply_fighter_intercept_action(
    object: &mut Object,
    flight: &mut FighterInterceptFlightState,
    action: FighterInterceptAction,
    previous_player_position: Vector3,
    player_position: Vector3,
) {
    match action {
        FighterInterceptAction::SetCruise(cruise) => {
            flight.cruise_target_speed = cruise.target_speed();
            flight.cruise_acceleration = cruise.acceleration();
        }
        FighterInterceptAction::SetCorridor(corridor) => {
            flight.corridor_drift_x = corridor.drift_x;
            flight.corridor_altitude = corridor.altitude;
            flight.corridor_drift_z = corridor.drift_z;
        }
        FighterInterceptAction::SetSpeed(speed) => {
            object.base.speed = speed.units();
        }
        FighterInterceptAction::SetPresentation(presentation) => {
            let visible = presentation == FighterInterceptPresentation::Visible;
            object.base.flags.active = visible;
            object.base.flags.visible = visible;
            object.base.flags.collision_disabled = !visible;
        }
        FighterInterceptAction::ChaseBank(target) => {
            object.base.roll = chase_fighter_angle(object.base.roll, target.angle());
        }
        FighterInterceptAction::ChaseRollToLevel => {
            object.base.roll = chase_fighter_angle(object.base.roll, Angle::ZERO);
        }
        FighterInterceptAction::FacePlayer(timing) => {
            let target = timing.select(previous_player_position, player_position);
            let delta_x = target.x.wrapping_sub(object.base.position.x);
            let delta_z = target.z.wrapping_sub(object.base.position.z);
            let target_yaw =
                Angle::from_units(sf_core::aim_angle::sf2_yaw_to_target(delta_x, delta_z));
            object.base.yaw = chase_capital_angle(
                object.base.yaw,
                target_yaw,
                FIGHTER_INTERCEPT_PLAYER_FACING_DIVISOR,
            );
        }
        FighterInterceptAction::AimWeaponPitch(timing) => {
            let target = timing.select(previous_player_position, player_position);
            flight.weapon_phase = FighterInterceptWeaponPhase::Aiming {
                flight_pitch: object.base.pitch,
            };
            let delta_x = target.x.wrapping_sub(object.base.position.x);
            let delta_y = target.y.wrapping_sub(object.base.position.y);
            let delta_z = target.z.wrapping_sub(object.base.position.z);
            let distance = sf_core::aim_angle::sf2_xz_angle_distance(delta_x, delta_z);
            object.base.pitch =
                Angle::from_units(sf_core::aim_angle::sf2_pitch_to_target(delta_y, distance));
        }
        FighterInterceptAction::RestoreFlightPitch => {
            let FighterInterceptWeaponPhase::Aiming { flight_pitch } = flight.weapon_phase else {
                debug_assert!(false, "fighter weapon restoration lacks saved flight pitch");
                return;
            };
            object.base.pitch = flight_pitch;
            flight.weapon_phase = FighterInterceptWeaponPhase::Flight;
        }
        FighterInterceptAction::ApplyBankTurn => {
            let bank_turn = (object.base.roll.units() as i8) / FIGHTER_INTERCEPT_BANK_TURN_DIVISOR;
            object.base.yaw = object.base.yaw.wrapping_add(bank_turn);
        }
        FighterInterceptAction::Move(turn_mode)
        | FighterInterceptAction::MoveHorizontal(turn_mode) => {
            let difference = i16::from(flight.cruise_target_speed) - i16::from(object.base.speed);
            let adjustment = difference
                .unsigned_abs()
                .min(u16::from(flight.cruise_acceleration)) as u8;
            object.base.speed = if difference > 0 {
                object.base.speed.saturating_add(adjustment)
            } else if difference < 0 {
                object.base.speed.saturating_sub(adjustment)
            } else {
                object.base.speed
            };
            if turn_mode == FighterInterceptTurnMode::Banked {
                let bank_turn =
                    (object.base.roll.units() as i8) / FIGHTER_INTERCEPT_BANK_TURN_DIVISOR;
                object.base.yaw = object.base.yaw.wrapping_add(bank_turn);
            }
            let velocity = flight_velocity(
                object.base.pitch,
                object.base.yaw,
                object.base.speed,
                MISSION_ENCOUNTER_POSITION_SCALE,
            );
            object.base.velocity = velocity;
            object.base.position.x = object.base.position.x.wrapping_add(velocity.x);
            if matches!(action, FighterInterceptAction::MoveHorizontal(_)) {
                flight.pending_velocity = velocity;
                flight.movement_phase = FighterInterceptMovementPhase::HorizontalApplied;
            } else {
                object.base.position.y = object.base.position.y.wrapping_add(velocity.y);
                object.base.position.z = object.base.position.z.wrapping_add(velocity.z);
                flight.pending_velocity = Vector3::default();
                flight.movement_phase = FighterInterceptMovementPhase::Ready;
            }
        }
        FighterInterceptAction::FinishMovement => {
            debug_assert_eq!(
                flight.movement_phase,
                FighterInterceptMovementPhase::HorizontalApplied
            );
            object.base.position.y = object
                .base
                .position
                .y
                .wrapping_add(flight.pending_velocity.y);
            object.base.position.z = object
                .base
                .position
                .z
                .wrapping_add(flight.pending_velocity.z);
            flight.pending_velocity = Vector3::default();
            flight.movement_phase = FighterInterceptMovementPhase::Ready;
        }
        FighterInterceptAction::ApplyVerticalWave(mode) => {
            let displacement =
                i16::from(sf_core::snes_trig::COSTAB[flight.vertical_wave_phase.units() as usize])
                    / mode.divisor();
            object.base.position.y = object.base.position.y.wrapping_add(displacement);
            flight.vertical_wave_phase = flight
                .vertical_wave_phase
                .wrapping_add(FIGHTER_INTERCEPT_WAVE_PHASE_STEP);
        }
        FighterInterceptAction::ShiftCorridorX => {
            object.base.position.x = object.base.position.x.wrapping_add(flight.corridor_drift_x);
        }
        FighterInterceptAction::ApproachCorridorAltitude => {
            object.base.position.y = approach_proportional_i16(
                object.base.position.y,
                flight.corridor_altitude,
                FIGHTER_INTERCEPT_ALTITUDE_DIVISOR,
            );
        }
        FighterInterceptAction::ShiftCorridorZ => {
            object.base.position.z = object.base.position.z.wrapping_add(flight.corridor_drift_z);
        }
    }
}

fn apply_interception_missile_action(
    object: &mut Object,
    flight: &mut InterceptionMissileFlightState,
    action: InterceptionMissileAction,
) {
    match action {
        InterceptionMissileAction::Present => {
            object.base.flags.active = true;
            object.base.flags.visible = true;
            object.base.flags.collision_disabled = true;
        }
        InterceptionMissileAction::BeginLowerFlight => {
            let pose = missile_interception_targets::LOWER_FLIGHT_POSE;
            object.base.position = pose.position;
            object.base.pitch = Angle::from_units(pose.pitch);
            object.base.yaw = Angle::from_units(pose.yaw);
            object.base.roll = Angle::from_units(pose.roll);
            object.base.speed = pose.speed;
            object.base.velocity = Vector3::default();
            flight.last_steering_adjustment = InterceptionMissileSteering::Straight;
        }
        InterceptionMissileAction::Steer(steering) => {
            flight.last_steering_adjustment = steering;
            match steering {
                InterceptionMissileSteering::Straight => {}
                InterceptionMissileSteering::Climb => {
                    object.base.pitch = object
                        .base
                        .pitch
                        .wrapping_add(INTERCEPTION_MISSILE_STEERING_STEP);
                }
                InterceptionMissileSteering::Dive => {
                    object.base.pitch = object
                        .base
                        .pitch
                        .wrapping_add(-INTERCEPTION_MISSILE_STEERING_STEP);
                }
                InterceptionMissileSteering::Clockwise => {
                    object.base.yaw = object
                        .base
                        .yaw
                        .wrapping_add(INTERCEPTION_MISSILE_STEERING_STEP);
                }
                InterceptionMissileSteering::CounterClockwise => {
                    object.base.yaw = object
                        .base
                        .yaw
                        .wrapping_add(-INTERCEPTION_MISSILE_STEERING_STEP);
                }
            }
        }
        InterceptionMissileAction::Spin => {
            object.base.roll = object
                .base
                .roll
                .wrapping_add(INTERCEPTION_MISSILE_SPIN_STEP);
        }
        InterceptionMissileAction::Move => {
            let velocity = flight_velocity(
                object.base.pitch,
                object.base.yaw,
                object.base.speed,
                INTERCEPTION_MISSILE_POSITION_SCALE,
            );
            object.base.velocity = velocity;
            object.base.position.x = object.base.position.x.wrapping_add(velocity.x);
            object.base.position.y = object.base.position.y.wrapping_add(velocity.y);
            object.base.position.z = object.base.position.z.wrapping_add(velocity.z);
        }
        InterceptionMissileAction::Depart => {
            debug_assert!(
                false,
                "missile departure must be handled by its owning slot"
            );
        }
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(game: &mut Game, button: Button) {
        game.tick(button as u16).unwrap();
        game.tick(0).unwrap();
    }

    fn assert_mission_actor_presentation(
        game: &Game,
        actor: MissionEncounterActor,
        expected: MissionActorPresentation,
        retail_frame: u16,
    ) {
        match expected {
            MissionActorPresentation::Present(pose) => {
                let id = game.mission_entry_flyby[actor.index()]
                    .unwrap_or_else(|| panic!("actor departed before retail frame {retail_frame}"));
                let object = game.state().objects.get(id).unwrap();
                assert_eq!(object.base.position, pose.position, "frame {retail_frame}");
                assert_eq!(
                    object.base.pitch.units(),
                    pose.pitch,
                    "frame {retail_frame}"
                );
                assert_eq!(object.base.yaw.units(), pose.yaw, "frame {retail_frame}");
                assert_eq!(object.base.roll.units(), pose.roll, "frame {retail_frame}");
                assert_eq!(object.base.speed, pose.speed, "frame {retail_frame}");
                assert!(object.base.flags.active, "frame {retail_frame}");
                assert!(object.base.flags.visible, "frame {retail_frame}");
                assert!(
                    !object.base.flags.collision_disabled,
                    "frame {retail_frame}"
                );
                let has_typed_flight_state = match actor {
                    MissionEncounterActor::FirstCapital | MissionEncounterActor::SecondCapital => {
                        matches!(object.extension.activity, ObjectActivity::CapitalFlight(_))
                    }
                    MissionEncounterActor::UpperFighter | MissionEncounterActor::LowerFighter => {
                        matches!(
                            object.extension.activity,
                            ObjectActivity::ReengagementFighterFlight(_)
                        )
                    }
                };
                assert!(has_typed_flight_state, "frame {retail_frame}");
            }
            MissionActorPresentation::Inactive => {
                let id = game.mission_entry_flyby[actor.index()]
                    .unwrap_or_else(|| panic!("actor departed before retail frame {retail_frame}"));
                let object = game.state().objects.get(id).unwrap();
                assert!(!object.base.flags.active, "frame {retail_frame}");
                assert!(!object.base.flags.visible, "frame {retail_frame}");
                assert!(object.base.flags.collision_disabled, "frame {retail_frame}");
                assert!(matches!(
                    object.extension.activity,
                    ObjectActivity::CapitalFlight(_)
                ));
            }
            MissionActorPresentation::Departed => {
                assert!(
                    game.mission_entry_flyby[actor.index()].is_none(),
                    "actor remains allocated at retail frame {retail_frame}"
                );
            }
        }
    }

    fn step_player_blaster(game: &mut Game, player: ObjectId, held_input: u16) {
        game.state.input.sample(Buttons::from_bits(held_input));
        game.update_player_blaster(player, true).unwrap();
        game.update_objects();
    }

    fn rendered_object(game: &Game, object: ObjectId) -> &RenderObject {
        game.render_objects()
            .iter()
            .find(|entry| entry.object == object)
            .expect("active player craft is absent from the native render boundary")
    }

    fn active_walker_game() -> (Game, ObjectId, i16) {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.begin_eladard_sortie().unwrap();
        game.state.mission.phase = MissionPhase::Active;
        game.state.mode_frame = MISSION_ACTIVE_TICKS;
        let player = game.state.mission.primary_player.unwrap();
        game.state.mission.player_craft_form = PlayerCraftForm::Walker;
        game.apply_player_craft_presentation(player, PlayerCraftPresentation::Walker);
        let surface_height = game.state.objects.get(player).unwrap().base.position.y;
        (game, player, surface_height)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct WalkerJumpOracleFrame {
        launch_ticks_remaining: u8,
        ascent_impulse: i16,
        pose_extension: u16,
        height_offset: i16,
        fall_velocity: i16,
        ascent_velocity: i16,
    }

    fn assert_walker_jump_frame(
        game: &Game,
        player: ObjectId,
        surface_height: i16,
        expected: WalkerJumpOracleFrame,
        frame_index: usize,
    ) {
        let WalkerJumpState::Active(jump) = game.state.mission.player_walker.jump else {
            panic!("Walker jump ended before oracle frame {frame_index}");
        };
        assert_eq!(jump.launch_ticks_remaining, expected.launch_ticks_remaining);
        assert_eq!(jump.ascent_impulse, expected.ascent_impulse);
        assert_eq!(jump.pose_extension, expected.pose_extension);
        assert_eq!(jump.motion_ticks_elapsed, frame_index as u8);
        assert_eq!(jump.height_offset, expected.height_offset);
        assert_eq!(jump.fall_velocity, expected.fall_velocity);
        assert_eq!(jump.ascent_velocity, expected.ascent_velocity);
        assert_eq!(jump.surface_height, surface_height);
        assert_eq!(
            game.state.objects.get(player).unwrap().base.position.y,
            surface_height.saturating_add(expected.height_offset)
        );
    }

    #[test]
    fn pilot_profiles_select_the_retail_craft_and_shield_tables() {
        let cases = [
            (
                Pilot::Fox,
                ShapeId::FOX_FALCO_FLIGHT_CRAFT,
                ShapeId::FOX_FALCO_WALKER,
                32,
                25,
                FOX_FALCO_CHARGE_READY_TICK,
            ),
            (
                Pilot::Falco,
                ShapeId::FOX_FALCO_FLIGHT_CRAFT,
                ShapeId::FOX_FALCO_WALKER,
                32,
                25,
                FOX_FALCO_CHARGE_READY_TICK,
            ),
            (
                Pilot::Peppy,
                ShapeId::PEPPY_SLIPPY_FLIGHT_CRAFT,
                ShapeId::PEPPY_SLIPPY_WALKER,
                40,
                35,
                PEPPY_SLIPPY_CHARGE_READY_TICK,
            ),
            (
                Pilot::Slippy,
                ShapeId::PEPPY_SLIPPY_FLIGHT_CRAFT,
                ShapeId::PEPPY_SLIPPY_WALKER,
                40,
                35,
                PEPPY_SLIPPY_CHARGE_READY_TICK,
            ),
            (
                Pilot::Miyu,
                ShapeId::MIYU_FAY_FLIGHT_CRAFT,
                ShapeId::MIYU_FAY_WALKER,
                24,
                10,
                MIYU_FAY_CHARGE_READY_TICK,
            ),
            (
                Pilot::Fay,
                ShapeId::MIYU_FAY_FLIGHT_CRAFT,
                ShapeId::MIYU_FAY_WALKER,
                24,
                10,
                MIYU_FAY_CHARGE_READY_TICK,
            ),
        ];

        for (
            pilot,
            expected_craft,
            expected_walker,
            expected_shield,
            expected_charge_threshold,
            expected_charge_ready_tick,
        ) in cases
        {
            assert_eq!(pilot_flight_craft_shape(pilot), expected_craft);
            assert_eq!(pilot_walker_shape(pilot), expected_walker);
            assert_eq!(pilot.craft_profile().maximum_shield, expected_shield);
            assert_eq!(
                pilot.craft_profile().charge_threshold,
                expected_charge_threshold
            );

            let wingmate = if pilot == Pilot::Slippy {
                Pilot::Fox
            } else {
                Pilot::Slippy
            };
            let mut game = Game::new();
            game.state.roster.selected = [Some(pilot), Some(wingmate)];
            assert_eq!(game.player_charge_ready_tick(), expected_charge_ready_tick);
            game.begin_opening_sortie().unwrap();
            let primary_id = game.state.mission.primary_player.unwrap();
            assert_eq!(
                game.state.objects.get(primary_id).unwrap().base.hit_points,
                expected_shield
            );
            while game.state.mission.phase != MissionPhase::Active {
                game.tick(0).unwrap();
            }
            assert_eq!(
                game.state.objects.get(primary_id).unwrap().base.shape,
                expected_craft
            );
        }
    }

    #[test]
    fn pilot_walkers_are_applied_to_eladard_and_carrier_presentations() {
        let cases = [
            (Pilot::Fox, ShapeId::FOX_FALCO_WALKER),
            (Pilot::Falco, ShapeId::FOX_FALCO_WALKER),
            (Pilot::Peppy, ShapeId::PEPPY_SLIPPY_WALKER),
            (Pilot::Slippy, ShapeId::PEPPY_SLIPPY_WALKER),
            (Pilot::Miyu, ShapeId::MIYU_FAY_WALKER),
            (Pilot::Fay, ShapeId::MIYU_FAY_WALKER),
        ];

        for (pilot, expected_walker) in cases {
            let wingmate = if pilot == Pilot::Slippy {
                Pilot::Fox
            } else {
                Pilot::Slippy
            };

            let mut eladard = Game::new();
            eladard.state.roster.selected = [Some(pilot), Some(wingmate)];
            eladard.begin_opening_sortie().unwrap();
            eladard.begin_eladard_sortie().unwrap();
            let player = eladard.state.mission.primary_player.unwrap();
            eladard.state.mission.player_craft_form = PlayerCraftForm::Walker;
            eladard.apply_player_craft_presentation(player, PlayerCraftPresentation::Walker);
            assert_eq!(
                eladard.state.objects.get(player).unwrap().base.shape,
                expected_walker
            );

            let mut carrier = Game::new();
            carrier.state.roster.selected = [Some(pilot), Some(wingmate)];
            carrier.begin_opening_sortie().unwrap();
            carrier
                .begin_carrier_assault(MissionVisit::FirstBattleCarrier)
                .unwrap();
            carrier.state.mode_frame = u32::from(CARRIER_REACTOR_OPEN_RETAIL_FRAME)
                .div_ceil(RETAIL_PRESENTATION_FRAMES_PER_TICK);
            carrier.update_carrier_assault().unwrap();
            let player = carrier.state.mission.primary_player.unwrap();
            assert_eq!(
                carrier.state.objects.get(player).unwrap().base.shape,
                expected_walker
            );
        }
    }

    #[test]
    fn select_drives_the_oracle_sampled_ground_transformation_round_trip() {
        let cases = [
            (
                Pilot::Fox,
                ShapeId::FOX_FALCO_FLIGHT_CRAFT,
                ShapeId::FOX_FALCO_FLIGHT_SIDE_TRANSITION,
                ShapeId::FOX_FALCO_WALKER_SIDE_TRANSITION,
                ShapeId::FOX_FALCO_WALKER,
            ),
            (
                Pilot::Peppy,
                ShapeId::PEPPY_SLIPPY_FLIGHT_CRAFT,
                ShapeId::PEPPY_SLIPPY_FLIGHT_SIDE_TRANSITION,
                ShapeId::PEPPY_SLIPPY_WALKER_SIDE_TRANSITION,
                ShapeId::PEPPY_SLIPPY_WALKER,
            ),
            (
                Pilot::Miyu,
                ShapeId::MIYU_FAY_FLIGHT_CRAFT,
                ShapeId::MIYU_FAY_FLIGHT_SIDE_TRANSITION,
                ShapeId::MIYU_FAY_WALKER_SIDE_TRANSITION,
                ShapeId::MIYU_FAY_WALKER,
            ),
        ];

        for (pilot, flight, flight_side, walker_side, walker) in cases {
            let mut game = Game::new();
            game.state.roster.selected = [Some(pilot), Some(Pilot::Slippy)];
            game.begin_opening_sortie().unwrap();
            game.begin_eladard_sortie().unwrap();
            game.state.mission.phase = MissionPhase::Active;
            game.state.mode_frame = MISSION_ACTIVE_TICKS;
            let player = game.state.mission.primary_player.unwrap();
            game.apply_player_craft_presentation(player, PlayerCraftPresentation::Flight);

            game.tick(Button::Select as u16).unwrap();
            assert_eq!(game.state.objects.get(player).unwrap().base.shape, flight);
            assert_eq!(
                game.state.mission.player_craft_form,
                PlayerCraftForm::Transforming(PlayerCraftTransformation {
                    direction: PlayerCraftTransformationDirection::ToWalker,
                    elapsed_retail_frames: PLAYER_TRANSFORMATION_RETAIL_FRAMES_PER_TICK,
                })
            );

            game.tick(0).unwrap();
            let object = game.state.objects.get(player).unwrap();
            assert_eq!(object.base.shape, flight_side);
            assert_eq!(object.extension.animation_frame, 6);
            let rendered = rendered_object(&game, player);
            assert_eq!(rendered.shape, flight_side);
            assert_eq!(rendered.animation.shape_frame, 6);
            for _ in 0..5 {
                game.tick(0).unwrap();
            }
            let object = game.state.objects.get(player).unwrap();
            assert_eq!(object.base.shape, flight_side);
            assert_eq!(object.extension.animation_frame, 1);
            game.tick(0).unwrap();
            let object = game.state.objects.get(player).unwrap();
            assert_eq!(object.base.shape, walker_side);
            assert_eq!(object.extension.animation_frame, 6);
            for _ in 0..6 {
                game.tick(0).unwrap();
            }
            assert_eq!(
                game.state.mission.player_craft_form,
                PlayerCraftForm::Walker
            );
            assert_eq!(game.state.objects.get(player).unwrap().base.shape, walker);
            assert_eq!(rendered_object(&game, player).shape, walker);

            game.tick(Button::Select as u16).unwrap();
            assert_eq!(game.state.objects.get(player).unwrap().base.shape, walker);
            game.tick(0).unwrap();
            let object = game.state.objects.get(player).unwrap();
            assert_eq!(object.base.shape, walker_side);
            assert_eq!(object.extension.animation_frame, 1);
            for _ in 0..5 {
                game.tick(0).unwrap();
            }
            game.tick(0).unwrap();
            let object = game.state.objects.get(player).unwrap();
            assert_eq!(object.base.shape, flight_side);
            assert_eq!(object.extension.animation_frame, 1);
            for _ in 0..4 {
                game.tick(0).unwrap();
            }
            assert_eq!(
                game.state.mission.player_craft_form,
                PlayerCraftForm::Flight
            );
            assert_eq!(game.state.objects.get(player).unwrap().base.shape, flight);
            assert_eq!(rendered_object(&game, player).shape, flight);
        }
    }

    #[test]
    fn walker_controls_follow_the_retail_typed_motion_state() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.begin_eladard_sortie().unwrap();
        game.state.mission.phase = MissionPhase::Active;
        game.state.mode_frame = MISSION_ACTIVE_TICKS;
        let player = game.state.mission.primary_player.unwrap();
        game.state.mission.player_craft_form = PlayerCraftForm::Walker;
        game.apply_player_craft_presentation(player, PlayerCraftPresentation::Walker);
        let initial_yaw = game.state.objects.get(player).unwrap().base.yaw;

        game.tick(Button::LeftShoulder as u16).unwrap();
        let walker = game.state.mission.player_walker;
        assert_eq!(walker.turn_spring, 2_176);
        assert_eq!(walker.turn_velocity, 1);
        assert_eq!(
            walker.heading_offset,
            Some(Angle::from_units(initial_yaw.units().wrapping_add(1)))
        );
        assert_eq!(
            game.state.objects.get(player).unwrap().base.yaw,
            Angle::from_units(initial_yaw.units().wrapping_add(9))
        );

        game.tick(Button::LeftShoulder as u16).unwrap();
        let walker = game.state.mission.player_walker;
        assert_eq!(walker.turn_spring, 3_808);
        assert_eq!(walker.turn_velocity, 2);
        assert_eq!(
            game.state.objects.get(player).unwrap().base.yaw,
            Angle::from_units(initial_yaw.units().wrapping_add(17))
        );

        game.tick(0).unwrap();
        let walker = game.state.mission.player_walker;
        assert_eq!(walker.turn_spring, 1_904);
        assert_eq!(walker.turn_velocity, 1);
        assert_eq!(
            game.state.objects.get(player).unwrap().base.yaw,
            Angle::from_units(initial_yaw.units().wrapping_add(11))
        );

        let stopped_position = game.state.objects.get(player).unwrap().base.position;
        game.tick(Button::Up as u16).unwrap();
        assert_eq!(game.state.objects.get(player).unwrap().base.speed, 36);
        let walking_position = game.state.objects.get(player).unwrap().base.position;
        assert_ne!(walking_position, stopped_position);
        game.tick(0).unwrap();
        assert_eq!(game.state.objects.get(player).unwrap().base.speed, 0);
        assert_eq!(
            game.state.objects.get(player).unwrap().base.position,
            walking_position
        );

        let landing_height = game.state.objects.get(player).unwrap().base.position.y;
        game.tick(Button::Y as u16).unwrap();
        let WalkerJumpState::Active(jump) = game.state.mission.player_walker.jump else {
            panic!("Walker jump did not enter its active typed state");
        };
        assert_eq!(jump.launch_ticks_remaining, 8);
        assert_eq!(jump.ascent_impulse, -864);
        assert_eq!(jump.pose_extension, 276);
        assert_eq!(jump.surface_height, landing_height);
        assert_eq!(jump.motion_ticks_elapsed, 0);
        assert_eq!(jump.height_offset, 0);
        assert_eq!(jump.fall_velocity, 0);
        assert_eq!(jump.ascent_velocity, 0);
        assert_eq!(
            game.state.objects.get(player).unwrap().base.position.y,
            landing_height
        );

        game.tick(0).unwrap();
        let WalkerJumpState::Active(jump) = game.state.mission.player_walker.jump else {
            panic!("Walker jump ended during its retail takeoff delay");
        };
        assert_eq!(jump.launch_ticks_remaining, 7);
        assert_eq!(jump.ascent_impulse, -864);
        assert_eq!(jump.pose_extension, 532);
        assert_eq!(jump.motion_ticks_elapsed, 1);
        assert_eq!(jump.height_offset, 0);
        assert_eq!(jump.fall_velocity, 0);
        assert_eq!(jump.ascent_velocity, -16);
        assert_eq!(
            game.state.objects.get(player).unwrap().base.position.y,
            landing_height
        );

        game.tick(0).unwrap();
        let WalkerJumpState::Active(jump) = game.state.mission.player_walker.jump else {
            panic!("Walker jump ended before world motion began");
        };
        assert_eq!(jump.launch_ticks_remaining, 6);
        assert_eq!(jump.pose_extension, 788);
        assert!(game.state.objects.get(player).unwrap().base.position.y < landing_height);

        let mut remaining_guard_ticks = 64;
        while game.state.mission.player_walker.jump != WalkerJumpState::Grounded {
            game.tick(0).unwrap();
            remaining_guard_ticks -= 1;
            assert!(
                remaining_guard_ticks > 0,
                "Walker jump did not return to its surface"
            );
        }
        assert_eq!(
            game.state.objects.get(player).unwrap().base.position.y,
            landing_height
        );
    }

    #[test]
    fn released_walker_jump_matches_the_retail_local_motion_trace() {
        const ORACLE_FRAMES: [WalkerJumpOracleFrame; 11] = [
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 8,
                ascent_impulse: -864,
                pose_extension: 276,
                height_offset: 0,
                fall_velocity: 0,
                ascent_velocity: 0,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 7,
                ascent_impulse: -864,
                pose_extension: 532,
                height_offset: 0,
                fall_velocity: 0,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 6,
                ascent_impulse: -864,
                pose_extension: 788,
                height_offset: -16,
                fall_velocity: 2,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 5,
                ascent_impulse: -864,
                pose_extension: 1_024,
                height_offset: -32,
                fall_velocity: 5,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 4,
                ascent_impulse: -864,
                pose_extension: 1_024,
                height_offset: -43,
                fall_velocity: 9,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 3,
                ascent_impulse: -864,
                pose_extension: 1_024,
                height_offset: -50,
                fall_velocity: 13,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 2,
                ascent_impulse: -864,
                pose_extension: 1_024,
                height_offset: -53,
                fall_velocity: 17,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 1,
                ascent_impulse: -864,
                pose_extension: 1_024,
                height_offset: -52,
                fall_velocity: 21,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 0,
                ascent_impulse: -416,
                pose_extension: 1_024,
                height_offset: -47,
                fall_velocity: 25,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 0,
                ascent_impulse: 0,
                pose_extension: 1_024,
                height_offset: -38,
                fall_velocity: 29,
                ascent_velocity: -8,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 0,
                ascent_impulse: 0,
                pose_extension: 1_024,
                height_offset: -17,
                fall_velocity: 33,
                ascent_velocity: 0,
            },
        ];

        let (mut game, player, surface_height) = active_walker_game();
        for (frame_index, expected) in ORACLE_FRAMES.into_iter().enumerate() {
            let input = if frame_index == 0 {
                Button::Y as u16
            } else {
                0
            };
            game.tick(input).unwrap();
            assert_walker_jump_frame(&game, player, surface_height, expected, frame_index);
        }
        game.tick(0).unwrap();
        assert_eq!(
            game.state.mission.player_walker.jump,
            WalkerJumpState::Grounded
        );
        assert_eq!(
            game.state.objects.get(player).unwrap().base.position.y,
            surface_height
        );
    }

    #[test]
    fn held_walker_jump_matches_the_retail_local_motion_trace() {
        const HELD_INPUT_FRAMES: usize = 8;
        const ORACLE_FRAMES: [WalkerJumpOracleFrame; 14] = [
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 8,
                ascent_impulse: -864,
                pose_extension: 276,
                height_offset: 0,
                fall_velocity: 0,
                ascent_velocity: 0,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 7,
                ascent_impulse: -1_056,
                pose_extension: 532,
                height_offset: 0,
                fall_velocity: 0,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 6,
                ascent_impulse: -1_248,
                pose_extension: 788,
                height_offset: -16,
                fall_velocity: 2,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 5,
                ascent_impulse: -1_440,
                pose_extension: 1_024,
                height_offset: -32,
                fall_velocity: 5,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 4,
                ascent_impulse: -1_632,
                pose_extension: 1_024,
                height_offset: -43,
                fall_velocity: 9,
                ascent_velocity: -24,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 3,
                ascent_impulse: -1_824,
                pose_extension: 1_024,
                height_offset: -58,
                fall_velocity: 13,
                ascent_velocity: -24,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 2,
                ascent_impulse: -2_016,
                pose_extension: 1_024,
                height_offset: -69,
                fall_velocity: 17,
                ascent_velocity: -32,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 1,
                ascent_impulse: -2_048,
                pose_extension: 1_024,
                height_offset: -84,
                fall_velocity: 21,
                ascent_velocity: -32,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 0,
                ascent_impulse: -1_600,
                pose_extension: 1_024,
                height_offset: -95,
                fall_velocity: 25,
                ascent_velocity: -32,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 0,
                ascent_impulse: -1_152,
                pose_extension: 1_024,
                height_offset: -102,
                fall_velocity: 29,
                ascent_velocity: -24,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 0,
                ascent_impulse: -704,
                pose_extension: 1_024,
                height_offset: -97,
                fall_velocity: 33,
                ascent_velocity: -16,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 0,
                ascent_impulse: -256,
                pose_extension: 1_024,
                height_offset: -80,
                fall_velocity: 37,
                ascent_velocity: -8,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 0,
                ascent_impulse: 0,
                pose_extension: 1_024,
                height_offset: -51,
                fall_velocity: 41,
                ascent_velocity: 0,
            },
            WalkerJumpOracleFrame {
                launch_ticks_remaining: 0,
                ascent_impulse: 0,
                pose_extension: 1_024,
                height_offset: -10,
                fall_velocity: 45,
                ascent_velocity: 0,
            },
        ];

        let (mut game, player, surface_height) = active_walker_game();
        for (frame_index, expected) in ORACLE_FRAMES.into_iter().enumerate() {
            let input = if frame_index < HELD_INPUT_FRAMES {
                Button::Y as u16
            } else {
                0
            };
            game.tick(input).unwrap();
            assert_walker_jump_frame(&game, player, surface_height, expected, frame_index);
        }
        game.tick(0).unwrap();
        assert_eq!(
            game.state.mission.player_walker.jump,
            WalkerJumpState::Grounded
        );
        assert_eq!(
            game.state.objects.get(player).unwrap().base.position.y,
            surface_height
        );
    }

    #[test]
    fn walker_right_turn_uses_signed_spring_and_left_has_button_priority() {
        let mut right = Game::new();
        right.begin_opening_sortie().unwrap();
        right.begin_eladard_sortie().unwrap();
        right.state.mission.phase = MissionPhase::Active;
        right.state.mode_frame = MISSION_ACTIVE_TICKS;
        let right_player = right.state.mission.primary_player.unwrap();
        right.state.mission.player_craft_form = PlayerCraftForm::Walker;
        let initial_yaw = right.state.objects.get(right_player).unwrap().base.yaw;

        right.tick(Button::RightShoulder as u16).unwrap();
        assert_eq!(right.state.mission.player_walker.turn_spring, -2_176);
        assert_eq!(right.state.mission.player_walker.turn_velocity, -1);
        assert_eq!(
            right.state.objects.get(right_player).unwrap().base.yaw,
            Angle::from_units(initial_yaw.units().wrapping_sub(10))
        );

        let mut both = Game::new();
        both.begin_opening_sortie().unwrap();
        both.begin_eladard_sortie().unwrap();
        both.state.mission.phase = MissionPhase::Active;
        both.state.mode_frame = MISSION_ACTIVE_TICKS;
        let both_player = both.state.mission.primary_player.unwrap();
        both.state.mission.player_craft_form = PlayerCraftForm::Walker;
        both.tick(Button::LeftShoulder as u16 | Button::RightShoulder as u16)
            .unwrap();
        assert_eq!(both.state.mission.player_walker.turn_spring, 2_176);
        assert_eq!(both.state.mission.player_walker.turn_velocity, 1);
        assert_eq!(
            both.state.objects.get(both_player).unwrap().base.yaw,
            Angle::from_units(
                both.state
                    .mission
                    .player_walker
                    .heading_offset
                    .unwrap()
                    .units()
                    .wrapping_add(8)
            )
        );
    }

    #[test]
    fn walker_jump_profiles_cover_all_six_pilots() {
        let cases = [
            (Pilot::Fox, -672, 256),
            (Pilot::Falco, -512, 192),
            (Pilot::Peppy, -512, 192),
            (Pilot::Slippy, -512, 192),
            (Pilot::Miyu, -672, 256),
            (Pilot::Fay, -672, 256),
        ];
        for (pilot, initial_ascent_impulse, pose_extension_step) in cases {
            let profile = pilot.walker_motion_profile();
            assert_eq!(profile.maximum_ascent_impulse, -2_048);
            assert_eq!(profile.initial_ascent_impulse, initial_ascent_impulse);
            assert_eq!(profile.held_ascent_step, 192);
            assert_eq!(profile.launch_ticks, 8);
            assert_eq!(profile.pose_extension_step, pose_extension_step);
        }
    }

    #[test]
    fn entering_walker_discards_stale_motion_and_seeds_the_current_heading() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.begin_eladard_sortie().unwrap();
        game.state.mission.phase = MissionPhase::Active;
        game.state.mode_frame = MISSION_ACTIVE_TICKS;
        let player = game.state.mission.primary_player.unwrap();
        game.state.mission.player_walker = super::super::state::PlayerWalkerState {
            heading_offset: Some(Angle::from_units(91)),
            turn_spring: 3_200,
            turn_velocity: 4,
            jump: WalkerJumpState::Active(WalkerJumpMotion {
                launch_ticks_remaining: 3,
                ascent_impulse: -1_024,
                pose_extension: 700,
                motion_ticks_elapsed: 5,
                height_offset: -40,
                fall_velocity: 5,
                ascent_velocity: -16,
                surface_height: -40,
            }),
        };

        game.tick(Button::Select as u16).unwrap();
        assert_eq!(
            game.state.mission.player_walker,
            super::super::state::PlayerWalkerState::default()
        );
        while game.state.mission.player_craft_form != PlayerCraftForm::Walker {
            game.tick(0).unwrap();
        }

        let walker_entry_yaw = game.state.objects.get(player).unwrap().base.yaw;
        game.tick(0).unwrap();
        assert_eq!(
            game.state.mission.player_walker.heading_offset,
            Some(walker_entry_yaw)
        );
        assert_eq!(game.state.mission.player_walker.turn_spring, 0);
        assert_eq!(game.state.mission.player_walker.turn_velocity, 0);
        assert_eq!(
            game.state.mission.player_walker.jump,
            WalkerJumpState::Grounded
        );
    }

    #[test]
    fn start_skips_intro_without_executing_machine_code() {
        let mut game = Game::new();
        press(&mut game, Button::Start);
        assert_eq!(game.mode(), GameMode::Title);
        assert!(game.state().objects.is_empty());
    }

    #[test]
    fn title_menu_matches_retail_main_difficulty_and_briefing_hierarchy() {
        let mut game = Game::new();
        press(&mut game, Button::Start);
        assert_eq!(game.state().title.page, TitlePage::MainMenu);
        press(&mut game, Button::B);
        assert_eq!(game.mode(), GameMode::Title);
        assert_eq!(game.state().title.page, TitlePage::Difficulty);
        press(&mut game, Button::Down);
        assert_eq!(
            game.state().campaign.difficulty,
            super::super::state::Difficulty::Hard
        );
        press(&mut game, Button::B);
        assert_eq!(game.mode(), GameMode::Briefing);
        press(&mut game, Button::B);
        assert_eq!(game.mode(), GameMode::StrategicMap);
        assert_eq!(
            game.state().strategic_map.phase,
            StrategicMapPhase::OpeningOverview
        );
    }

    #[test]
    fn retail_default_campaign_selects_two_pilots_and_launches_typed_players() {
        let mut game = Game::new();
        press(&mut game, Button::Start);
        press(&mut game, Button::B);
        press(&mut game, Button::B);
        press(&mut game, Button::B);
        assert_eq!(game.mode(), GameMode::StrategicMap);

        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }
        assert_eq!(game.mode(), GameMode::PilotSelection);
        assert_eq!(
            game.state().pilot_selection.phase,
            PilotSelectionPhase::Revealing
        );

        while game.state().pilot_selection.phase == PilotSelectionPhase::Revealing {
            game.tick(0).unwrap();
        }
        assert_eq!(
            game.state().pilot_selection.phase,
            PilotSelectionPhase::ChoosingPrimary
        );
        press(&mut game, Button::B);
        assert_eq!(game.state().roster.selected[0], Some(Pilot::Fox));
        assert_eq!(
            game.state().pilot_selection.phase,
            PilotSelectionPhase::ChoosingWingmate
        );
        assert_eq!(game.state().pilot_selection.cursor, Pilot::Slippy);
        press(&mut game, Button::B);
        assert_eq!(game.state().roster.selected[1], Some(Pilot::Slippy));
        assert_eq!(
            game.state().pilot_selection.phase,
            PilotSelectionPhase::Ready
        );
        press(&mut game, Button::B);
        while game.mode() == GameMode::PilotSelection {
            game.tick(0).unwrap();
        }

        assert_eq!(game.mode(), GameMode::StrategicMap);
        assert_eq!(
            game.state().strategic_map.phase,
            StrategicMapPhase::Tutorial(StrategicMapTutorialPage::Movement)
        );
        press(&mut game, Button::B);
        assert_eq!(
            game.state().strategic_map.phase,
            StrategicMapPhase::Tutorial(StrategicMapTutorialPage::Engagement)
        );
        press(&mut game, Button::B);
        assert_eq!(
            game.state().strategic_map.phase,
            StrategicMapPhase::Planning
        );
        press(&mut game, Button::Up);
        press(&mut game, Button::B);
        assert_eq!(
            game.state().strategic_map.phase,
            StrategicMapPhase::Traveling
        );
        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }

        assert_eq!(game.mode(), GameMode::Mission);
        assert_eq!(game.mission(), Some(MissionId::OPENING_SORTIE));
        assert_eq!(game.camera(), Camera::default());
        assert_eq!(game.state().mission.phase, MissionPhase::Loading);
        let primary_id = game.state().mission.primary_player.unwrap();
        let wingmate_id = game.state().mission.wingmate.unwrap();
        let primary = game.state().objects.get(primary_id).unwrap();
        let wingmate = game.state().objects.get(wingmate_id).unwrap();
        assert_eq!(primary.base.kind, ObjectKind::Player);
        assert_eq!(primary.base.position, OPENING_PRIMARY_POSITION);
        assert_eq!(primary.base.hit_points, 32);
        assert_eq!(primary.base.linked_object, Some(wingmate_id));
        assert!(!primary.base.flags.visible);
        assert!(primary.base.flags.collision_disabled);
        assert_eq!(wingmate.base.kind, ObjectKind::Wingmate);
        assert_eq!(wingmate.base.position, OPENING_WINGMATE_POSITION);
        assert_eq!(wingmate.base.hit_points, 40);
        assert_eq!(wingmate.base.linked_object, Some(primary_id));
        assert!(!wingmate.base.flags.visible);
        assert!(wingmate.base.flags.collision_disabled);

        while game.state().mode_frame < MISSION_ENTRY_FORMATION_TICKS {
            game.tick(0).unwrap();
        }
        assert_eq!(game.camera(), Camera::default());
        assert_eq!(game.mission_entry_flyby.len(), MISSION_ENTRY_CRAFT_COUNT);
        for (index, id) in game.mission_entry_flyby.iter().copied().enumerate() {
            let id = id.expect("mission encounter actor remains allocated");
            let object = game.state().objects.get(id).unwrap();
            assert_eq!(object.base.behavior, Behavior::MissionEntryFlyby);
            assert_eq!(object.base.shape, MISSION_ENTRY_CRAFTS[index].shape);
            assert_eq!(
                object.base.position,
                MISSION_FORMATION_KEYFRAMES[0].positions[index]
            );
            assert_eq!(object.base.yaw, Angle::from_units(MISSION_ENTRY_YAW));
        }

        const FIRST_EXACT_CAMERA_TICK: u32 = 25;
        while game.state().mode_frame < FIRST_EXACT_CAMERA_TICK {
            game.tick(0).unwrap();
        }
        let expected_camera = MISSION_CAMERA_KEYFRAMES[3];
        assert_eq!(game.camera().position, expected_camera.position);
        assert_eq!(game.camera().rotation.pitch.units(), expected_camera.pitch);
        assert_eq!(game.camera().rotation.yaw.units(), expected_camera.yaw);
        for (index, id) in game.mission_entry_flyby.iter().copied().enumerate() {
            let id = id.expect("mission encounter actor remains allocated");
            assert_eq!(
                game.state().objects.get(id).unwrap().base.position,
                MISSION_FORMATION_KEYFRAMES[2].positions[index]
            );
        }

        while game.state().mission.phase != MissionPhase::Active {
            game.tick(0).unwrap();
        }
        let primary = game.state().objects.get(primary_id).unwrap();
        let wingmate = game.state().objects.get(wingmate_id).unwrap();
        assert_eq!(primary.base.shape, ShapeId::FOX_FALCO_FLIGHT_CRAFT);
        assert_eq!(primary.base.position, ACTIVE_PRIMARY_POSITION);
        assert!(primary.base.flags.visible);
        assert!(!primary.base.flags.collision_disabled);
        assert_eq!(primary.base.roll, Angle::from_units(6));
        assert_eq!(primary.base.speed, 6);
        assert_eq!(wingmate.base.position, ACTIVE_WINGMATE_POSITION);
        assert_eq!(
            game.camera(),
            Camera {
                position: ACTIVE_CAMERA_POSITION,
                rotation: Rotation::default(),
            }
        );

        const CONTROL_HANDOFF_TICK: u32 =
            MISSION_CONTROL_HANDOFF_RETAIL_FRAME as u32 / RETAIL_PRESENTATION_FRAMES_PER_TICK;
        while game.state().mode_frame < CONTROL_HANDOFF_TICK {
            game.tick(0).unwrap();
        }
        let primary = game.state().objects.get(primary_id).unwrap();
        assert_eq!(primary.base.position, MISSION_PLAYER_KEYFRAMES[8].position);
        assert_eq!(primary.base.speed, PLAYER_CRUISE_SPEED);
        assert!(!primary.base.flags.visible);
        assert_eq!(primary.base.shape, ShapeId::FOX_FALCO_FLIGHT_CRAFT);
        assert_eq!(
            game.mission_entry_flyby.len(),
            MISSION_ENCOUNTER_ACTOR_COUNT
        );
        let handoff_keyframe = &MISSION_ENCOUNTER_KEYFRAMES[7];
        for (index, id) in game.mission_entry_flyby.iter().copied().enumerate() {
            let id = id.expect("mission encounter actor remains allocated");
            let object = game.state().objects.get(id).unwrap();
            let expected = handoff_keyframe.poses[index];
            assert_eq!(object.base.position, expected.position);
            assert_eq!(object.base.pitch.units(), expected.pitch);
            assert_eq!(object.base.yaw.units(), expected.yaw);
            assert_eq!(object.base.roll.units(), expected.roll);
            assert_eq!(object.base.speed, expected.speed);
        }
        assert_eq!(
            game.camera().position,
            MISSION_CAMERA_KEYFRAMES[33].position
        );
        let wingmate = game.state().objects.get(wingmate_id).unwrap();
        assert_eq!(
            wingmate.base.position,
            add_vectors(primary.base.position, ACTIVE_WINGMATE_OFFSET)
        );

        const CAMERA_FOLLOW_CHECKPOINT_TICK: u32 = 460 / RETAIL_PRESENTATION_FRAMES_PER_TICK;
        while game.state().mode_frame < CAMERA_FOLLOW_CHECKPOINT_TICK {
            game.tick(0).unwrap();
        }
        const CERTIFIED_PLAYER_AT_FRAME_460: Vector3 = Vector3 {
            x: -7_223,
            y: -2_881,
            z: -5_690,
        };
        const CERTIFIED_CAMERA_AT_FRAME_460: Vector3 = Vector3 {
            x: -7_229,
            y: -2_900,
            z: -5_692,
        };
        assert_eq!(
            game.state().objects.get(primary_id).unwrap().base.position,
            CERTIFIED_PLAYER_AT_FRAME_460
        );
        assert_eq!(game.camera().position, CERTIFIED_CAMERA_AT_FRAME_460);

        const PLAYER_CONTROL_START_TICK: u32 = (MISSION_PLAYER_CONTROL_START_RETAIL_FRAME as u32)
            .div_ceil(RETAIL_PRESENTATION_FRAMES_PER_TICK);
        while game.state().mode_frame < PLAYER_CONTROL_START_TICK {
            game.tick(0).unwrap();
        }
        for id in game.mission_entry_flyby.iter().flatten().copied() {
            let object = game.state().objects.get(id).unwrap();
            assert_eq!(object.base.kind, ObjectKind::Enemy);
            assert_eq!(object.base.behavior, Behavior::MissionEntryFlyby);
            assert_eq!(object.base.collision_class, CollisionClass::Enemy);
        }
        game.tick(Button::Left as u16).unwrap();
        let primary = game.state().objects.get(primary_id).unwrap();
        assert!(!primary.base.flags.visible);
        assert_eq!(primary.base.shape, ShapeId::FOX_FALCO_FLIGHT_CRAFT);
        assert_eq!(primary.base.yaw, Angle::from_units(229));
        assert_eq!(
            primary.base.speed,
            PLAYER_CRUISE_SPEED - PLAYER_SPEED_CHANGE_PER_TICK
        );
        assert_eq!(
            primary.base.velocity,
            flight_velocity(Angle::ZERO, primary.base.yaw, primary.base.speed, 1,)
        );

        game.tick(0).unwrap();
        game.tick(Button::B as u16).unwrap();
        let rapid_id = game
            .state()
            .objects
            .active_objects()
            .find(|(_, object)| {
                matches!(
                    object.extension.activity,
                    ObjectActivity::PlayerProjectile(PlayerProjectileState {
                        kind: PlayerProjectileKind::Rapid,
                        ..
                    })
                )
            })
            .map(|(id, _)| id)
            .expect("B launches the independent rapid shot");
        let rapid = game.state().objects.get(rapid_id).unwrap();
        assert_eq!(rapid.base.shape, ShapeId::EMPTY);
        assert_eq!(rapid.base.weapon, WeaponKind::Laser);
        assert_eq!(rapid.base.speed, PLAYER_RAPID_LASER_SPEED);
        assert_eq!(rapid.base.collision_class, CollisionClass::PlayerWeapon);
        assert_eq!(rapid.base.linked_object, Some(primary_id));
        assert!(!rapid.base.flags.visible);
        assert!(rapid.base.flags.collision_disabled);

        game.tick(Button::B as u16).unwrap();
        assert_eq!(
            game.state().objects.get(rapid_id).unwrap().base.shape,
            ShapeId::PLAYER_RAPID_LASER_LAUNCH
        );
        game.tick(Button::B as u16).unwrap();
        assert_eq!(
            game.state().objects.get(rapid_id).unwrap().base.shape,
            ShapeId::PLAYER_RAPID_LASER_EXPANDED
        );
        game.tick(Button::B as u16).unwrap();
        assert_eq!(
            game.state().objects.get(rapid_id).unwrap().base.shape,
            ShapeId::PLAYER_RAPID_LASER_FAST
        );
        for _ in 4..PLAYER_RAPID_LASER_DISTANT_TICK {
            game.tick(Button::B as u16).unwrap();
        }
        assert_eq!(
            game.state().objects.get(rapid_id).unwrap().base.shape,
            ShapeId::PLAYER_RAPID_LASER_DISTANT
        );
        for _ in PLAYER_RAPID_LASER_DISTANT_TICK..PLAYER_RAPID_LASER_END_TICK {
            game.tick(Button::B as u16).unwrap();
        }
        assert!(game.state().objects.get(rapid_id).is_none());

        game.tick(Button::B as u16).unwrap();
        let charge_orb = match game.state().mission.player_blaster {
            PlayerBlasterState::Holding {
                held_ticks,
                charge_orb: Some(charge_orb),
            } => {
                assert_eq!(held_ticks, PLAYER_CHARGE_ORB_SPAWN_TICK);
                charge_orb
            }
            state => panic!("charge orb was not created at the retail tick: {state:?}"),
        };
        assert_eq!(
            game.state().objects.get(charge_orb).unwrap().base.shape,
            ShapeId::PLAYER_CHARGE_ORB_BUILDING
        );
        assert!(
            game.state()
                .objects
                .get(charge_orb)
                .unwrap()
                .base
                .flags
                .collision_disabled
        );

        for _ in PLAYER_CHARGE_ORB_SPAWN_TICK..FOX_FALCO_CHARGE_READY_TICK {
            game.tick(Button::B as u16).unwrap();
        }
        assert_eq!(
            game.state().objects.get(charge_orb).unwrap().base.shape,
            ShapeId::PLAYER_CHARGE_ORB_READY
        );

        game.tick(0).unwrap();
        let charged_id = game
            .state()
            .objects
            .active_objects()
            .find(|(_, object)| object.base.weapon == WeaponKind::ChargedLaser)
            .map(|(id, _)| id)
            .expect("releasing the ready orb launches a distinct charged shot");
        assert_eq!(
            game.state().objects.get(charged_id).unwrap().base.shape,
            ShapeId::EMPTY
        );
        game.tick(0).unwrap();
        assert_eq!(
            game.state().objects.get(charged_id).unwrap().base.shape,
            ShapeId::PLAYER_CHARGED_LASER_LAUNCH
        );
        game.tick(0).unwrap();
        assert!(game.state().objects.get(charge_orb).is_none());
        for _ in 3..PLAYER_CHARGED_LASER_ACTIVE_TICK {
            game.tick(0).unwrap();
        }
        assert_eq!(
            game.state().objects.get(charged_id).unwrap().base.shape,
            ShapeId::PLAYER_CHARGED_LASER_ACTIVE
        );

        const CERTIFIED_ENCOUNTER_END_TICK: u32 = MISSION_ENCOUNTER_CERTIFIED_END_RETAIL_FRAME
            as u32
            / RETAIL_PRESENTATION_FRAMES_PER_TICK;
        while game.state().mode_frame < CERTIFIED_ENCOUNTER_END_TICK {
            game.tick(0).unwrap();
        }
        assert!(game.state().mission.departed_certified_neutral_path);
        for (actor, id) in game.mission_entry_flyby.iter().copied().enumerate() {
            let id = id.expect("mission encounter actor remains allocated");
            let object = game.state().objects.get(id).unwrap();
            if actor <= MissionEncounterActor::SecondCapital.index() {
                assert!(matches!(
                    object.extension.activity,
                    ObjectActivity::CapitalFlight(_)
                ));
            } else {
                assert!(matches!(
                    object.extension.activity,
                    ObjectActivity::FighterFlight(_)
                ));
            }
        }
        let departing_lower_fighter = game.mission_entry_flyby
            [MissionEncounterActor::LowerFighter.index()]
        .expect("lower fighter exists at its final retail pose");
        game.tick(0).unwrap();
        assert_eq!(
            game.mission_entry_flyby[MissionEncounterActor::LowerFighter.index()],
            None
        );
        assert!(game.state().objects.get(departing_lower_fighter).is_none());
        assert_eq!(
            game.mission_entry_flyby.iter().flatten().count(),
            MISSION_ENCOUNTER_ACTOR_COUNT - 1
        );
        for id in game.mission_entry_flyby.iter().flatten().copied() {
            let object = game.state().objects.get(id).unwrap();
            assert_eq!(object.base.kind, ObjectKind::Enemy);
            assert_eq!(object.base.behavior, Behavior::EnemyFlight);
            assert_eq!(object.base.collision_class, CollisionClass::Enemy);
        }
    }

    #[test]
    fn player_laser_shapes_and_muzzle_offset_match_the_reengagement_oracle() {
        const REENGAGEMENT_LASER_YAW: u8 = 64;
        const REENGAGEMENT_MUZZLE_OFFSET: Vector3 = Vector3 { x: -70, y: 0, z: 0 };
        const WEAPON_SHAPES: [(ShapeId, u16); 8] = [
            (ShapeId::PLAYER_RAPID_LASER_LAUNCH, 0xE3E0),
            (ShapeId::PLAYER_RAPID_LASER_EXPANDED, 0xCBB4),
            (ShapeId::PLAYER_RAPID_LASER_FAST, 0xE904),
            (ShapeId::PLAYER_RAPID_LASER_DISTANT, 0xE920),
            (ShapeId::PLAYER_CHARGE_ORB_BUILDING, 0xBE40),
            (ShapeId::PLAYER_CHARGE_ORB_READY, 0xBE78),
            (ShapeId::PLAYER_CHARGED_LASER_LAUNCH, 0xCA64),
            (ShapeId::PLAYER_CHARGED_LASER_ACTIVE, 0xCA80),
        ];
        for (shape, expected_source_shape) in WEAPON_SHAPES {
            assert_eq!(
                shape
                    .catalog_entry()
                    .expect("weapon shape is present")
                    .shape_id,
                expected_source_shape
            );
        }
        assert_eq!(
            flight_velocity(
                Angle::ZERO,
                Angle::from_units(REENGAGEMENT_LASER_YAW),
                PLAYER_LASER_MUZZLE_OFFSET_MAGNITUDE,
                1,
            ),
            REENGAGEMENT_MUZZLE_OFFSET
        );
    }

    #[test]
    fn releasing_before_charge_ready_cancels_the_orb_without_a_charged_shot() {
        const EARLY_RELEASE_TICK: u8 = 15;

        let mut game = Game::new();
        let mut player = Object::new(
            ObjectKind::Player,
            ShapeId::FOX_FALCO_FLIGHT_CRAFT,
            Behavior::PlayerFlight,
        );
        player.base.position = Vector3 {
            x: -7_400,
            y: -2_881,
            z: -5_900,
        };
        player.base.yaw = Angle::from_units(227);
        let player_id = game.state.objects.allocate(player).unwrap();
        game.state.mission.primary_player = Some(player_id);

        for _ in 0..EARLY_RELEASE_TICK {
            step_player_blaster(&mut game, player_id, Button::B as u16);
        }
        let charge_orb = match game.state.mission.player_blaster {
            PlayerBlasterState::Holding {
                charge_orb: Some(charge_orb),
                ..
            } => charge_orb,
            state => panic!("early-release setup has no charge orb: {state:?}"),
        };

        step_player_blaster(&mut game, player_id, 0);
        assert!(game
            .state
            .objects
            .active_objects()
            .all(|(_, object)| object.base.weapon != WeaponKind::ChargedLaser));
        assert_eq!(
            game.state.objects.get(charge_orb).unwrap().base.shape,
            ShapeId::PLAYER_CHARGE_ORB_BUILDING
        );
        for _ in 1..PLAYER_CHARGE_ORB_RELEASE_TICKS {
            step_player_blaster(&mut game, player_id, 0);
        }
        assert!(game.state.objects.get(charge_orb).is_none());
        assert_eq!(game.state.mission.player_blaster, PlayerBlasterState::Ready);
    }

    #[test]
    fn first_encounter_ai_checkpoint_matches_the_laser_hold_oracle() {
        const ORACLE_RETAIL_FRAME: u32 = 580;
        const ORACLE_PLAYER_POSITION: Vector3 = Vector3 {
            x: -6_737,
            y: -2_881,
            z: -5_123,
        };
        const ORACLE_CAMERA_POSITION: Vector3 = Vector3 {
            x: -6_737,
            y: -2_903,
            z: -5_123,
        };
        const ORACLE_POSES: [MissionEncounterPose; MISSION_ENCOUNTER_ACTOR_COUNT] = [
            mission_encounter_pose([-16_988, 10_707, -7_752, 0, 82, 0, 60]),
            mission_encounter_pose([-15_472, 7_928, -4_044, 0, 86, 0, 60]),
            mission_encounter_pose([144, -9_577, -2_384, 12, 114, 244, 63]),
            mission_encounter_pose([5_144, 13_099, -1_384, 244, 114, 244, 63]),
        ];

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        let oracle_tick = ORACLE_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK;
        while game.state().mode_frame < oracle_tick {
            game.tick(Button::B as u16).unwrap();
        }

        let player = game
            .state()
            .mission
            .primary_player
            .and_then(|id| game.state().objects.get(id))
            .expect("opening sortie retains its player");
        assert_eq!(player.base.position, ORACLE_PLAYER_POSITION);
        assert_eq!(player.base.roll, Angle::from_units(2));
        assert_eq!(game.camera().position, ORACLE_CAMERA_POSITION);
        assert!(!game.state().mission.departed_certified_neutral_path);

        for (id, expected) in game
            .mission_entry_flyby
            .iter()
            .flatten()
            .copied()
            .zip(ORACLE_POSES)
        {
            let object = game.state().objects.get(id).unwrap();
            assert_eq!(object.base.position, expected.position);
            assert_eq!(object.base.pitch.units(), expected.pitch);
            assert_eq!(object.base.yaw.units(), expected.yaw);
            assert_eq!(object.base.roll.units(), expected.roll);
            assert_eq!(object.base.speed, expected.speed);
        }
    }

    #[test]
    fn typed_fighter_dynamics_match_the_oracle_after_the_first_shot() {
        const FIGHTER_FIRST_SHOT_RANDOM_STATE: [u8; 4] = [146, 142, 249, 117];
        const FIGHTER_SECOND_FIRE_APPROACH_RANDOM_STATE: [u8; 4] = [227, 118, 191, 68];
        const FIGHTER_SECOND_FIRE_CHECK_RANDOM_STATE: [u8; 4] = [170, 20, 27, 36];
        const FIGHTER_THIRD_FIRE_CHECK_RANDOM_STATE: [u8; 4] = [55, 146, 51, 220];
        const FIGHTER_WAVE_HANDOFF_RANDOM_STATE: [u8; 4] = [91, 16, 197, 66];
        const FIGHTER_LATE_FIRE_CHECK_RANDOM_STATE: [u8; 4] = [130, 113, 74, 228];
        const FIGHTER_CAPTURE_END_RANDOM_STATE: [u8; 4] = [224, 8, 222, 134];
        const SHARED_RANDOM_PARTIAL_STATE: [u8; 4] = [221, 230, 194, 199];
        const SHARED_RANDOM_COMPLETION_STATE: [u8; 4] = [56, 246, 51, 108];
        const CERTIFIED_FIGHTER_END_RANDOM_STATE: [u8; 4] = [93, 96, 93, 175];
        const CHECKPOINTS: [(u32, [MissionEncounterPose; 2]); 252] = [
            (
                584,
                [
                    mission_encounter_pose([52, -9_101, -2_596, 16, 111, 244, 63]),
                    mission_encounter_pose([5_144, 13_099, -1_384, 244, 114, 244, 63]),
                ],
            ),
            (
                588,
                [
                    mission_encounter_pose([52, -9_101, -2_596, 16, 111, 244, 63]),
                    mission_encounter_pose([5_052, 12_623, -1_596, 240, 111, 244, 63]),
                ],
            ),
            (
                592,
                [
                    mission_encounter_pose([-52, -7_872, -2_796, 13, 108, 245, 63]),
                    mission_encounter_pose([4_948, 10_954, -1_796, 239, 108, 245, 63]),
                ],
            ),
            (
                596,
                [
                    mission_encounter_pose([-168, -6_812, -2_992, 11, 106, 246, 63]),
                    mission_encounter_pose([4_836, 9_485, -1_988, 240, 106, 246, 63]),
                ],
            ),
            (
                600,
                [
                    mission_encounter_pose([-296, -5_897, -3_184, 10, 104, 247, 63]),
                    mission_encounter_pose([4_712, 8_208, -2_172, 241, 104, 247, 63]),
                ],
            ),
            (
                604,
                [
                    mission_encounter_pose([-432, -5_104, -3_376, 9, 102, 248, 63]),
                    mission_encounter_pose([4_580, 7_094, -2_356, 242, 102, 248, 63]),
                ],
            ),
            (
                608,
                [
                    mission_encounter_pose([-580, -4_414, -3_560, 8, 100, 248, 63]),
                    mission_encounter_pose([4_436, 6_128, -2_532, 243, 100, 249, 63]),
                ],
            ),
            (
                612,
                [
                    mission_encounter_pose([-736, -3_819, -3_736, 7, 98, 248, 63]),
                    mission_encounter_pose([4_288, 5_286, -2_708, 244, 99, 250, 63]),
                ],
            ),
            (
                616,
                [
                    mission_encounter_pose([-900, -3_302, -3_900, 6, 96, 248, 63]),
                    mission_encounter_pose([4_288, 5_286, -2_708, 244, 99, 250, 63]),
                ],
            ),
            (
                620,
                [
                    mission_encounter_pose([-900, -3_302, -3_900, 6, 96, 248, 63]),
                    mission_encounter_pose([4_136, 4_558, -2_880, 245, 98, 251, 63]),
                ],
            ),
            (
                624,
                [
                    mission_encounter_pose([-1_076, -2_858, -4_060, 5, 94, 248, 63]),
                    mission_encounter_pose([3_976, 3_925, -3_044, 246, 97, 252, 63]),
                ],
            ),
            (
                628,
                [
                    mission_encounter_pose([-1_264, -2_473, -4_212, 4, 92, 248, 63]),
                    mission_encounter_pose([3_812, 3_379, -3_208, 247, 96, 252, 63]),
                ],
            ),
            (
                632,
                [
                    mission_encounter_pose([-1_460, -2_144, -4_352, 3, 90, 248, 63]),
                    mission_encounter_pose([3_644, 2_905, -3_368, 248, 95, 252, 63]),
                ],
            ),
            (
                636,
                [
                    mission_encounter_pose([-1_660, -1_860, -4_484, 2, 88, 248, 63]),
                    mission_encounter_pose([3_468, 2_498, -3_524, 249, 94, 252, 63]),
                ],
            ),
            (
                640,
                [
                    mission_encounter_pose([-1_868, -1_620, -4_604, 1, 86, 248, 63]),
                    mission_encounter_pose([3_288, 2_146, -3_680, 250, 93, 252, 63]),
                ],
            ),
            (
                644,
                [
                    mission_encounter_pose([-2_084, -1_414, -4_716, 0, 84, 248, 63]),
                    mission_encounter_pose([3_288, 2_146, -3_680, 250, 93, 252, 63]),
                ],
            ),
            (
                648,
                [
                    mission_encounter_pose([-2_084, -1_414, -4_716, 0, 84, 248, 63]),
                    mission_encounter_pose([3_104, 1_846, -3_832, 251, 92, 252, 63]),
                ],
            ),
            (
                652,
                [
                    mission_encounter_pose([-2_304, -1_238, -4_816, 0, 82, 248, 63]),
                    mission_encounter_pose([2_912, 1_588, -3_980, 252, 91, 252, 63]),
                ],
            ),
            (
                656,
                [
                    mission_encounter_pose([-2_528, -1_084, -4_904, 0, 80, 248, 63]),
                    mission_encounter_pose([2_716, 1_370, -4_120, 253, 90, 252, 63]),
                ],
            ),
            (
                660,
                [
                    mission_encounter_pose([-2_756, -949, -4_980, 0, 78, 248, 63]),
                    mission_encounter_pose([2_520, 1_183, -4_256, 254, 89, 252, 63]),
                ],
            ),
            (
                664,
                [
                    mission_encounter_pose([-2_988, -831, -5_044, 0, 76, 248, 63]),
                    mission_encounter_pose([2_320, 1_028, -4_388, 255, 88, 252, 63]),
                ],
            ),
            (
                668,
                [
                    mission_encounter_pose([-3_224, -728, -5_096, 0, 74, 248, 63]),
                    mission_encounter_pose([2_116, 896, -4_512, 0, 87, 252, 63]),
                ],
            ),
            (
                672,
                [
                    mission_encounter_pose([-3_464, -637, -5_136, 0, 72, 248, 63]),
                    mission_encounter_pose([1_908, 784, -4_632, 0, 86, 252, 63]),
                ],
            ),
            (
                676,
                [
                    mission_encounter_pose([-3_704, -558, -5_164, 0, 70, 248, 63]),
                    mission_encounter_pose([1_908, 784, -4_632, 0, 86, 252, 63]),
                ],
            ),
            (
                680,
                [
                    mission_encounter_pose([-3_704, -558, -5_164, 0, 70, 248, 63]),
                    mission_encounter_pose([1_696, 686, -4_748, 0, 85, 252, 63]),
                ],
            ),
            (
                684,
                [
                    mission_encounter_pose([-3_948, -489, -5_180, 0, 68, 248, 63]),
                    mission_encounter_pose([1_480, 601, -4_860, 0, 84, 252, 63]),
                ],
            ),
            (
                688,
                [
                    mission_encounter_pose([-4_192, -428, -5_184, 0, 66, 248, 63]),
                    mission_encounter_pose([1_264, 526, -4_968, 0, 83, 252, 63]),
                ],
            ),
            (
                692,
                [
                    mission_encounter_pose([-4_436, -375, -5_184, 0, 64, 248, 63]),
                    mission_encounter_pose([1_044, 461, -5_068, 0, 82, 252, 63]),
                ],
            ),
            (
                696,
                [
                    mission_encounter_pose([-4_680, -329, -5_180, 0, 62, 248, 63]),
                    mission_encounter_pose([820, 404, -5_164, 0, 81, 252, 63]),
                ],
            ),
            (
                700,
                [
                    mission_encounter_pose([-4_924, -288, -5_164, 0, 60, 248, 63]),
                    mission_encounter_pose([596, 354, -5_252, 0, 80, 252, 63]),
                ],
            ),
            (
                704,
                [
                    mission_encounter_pose([-5_164, -252, -5_136, 0, 58, 248, 63]),
                    mission_encounter_pose([368, 310, -5_336, 0, 79, 252, 63]),
                ],
            ),
            (
                708,
                [
                    mission_encounter_pose([-5_404, -221, -5_096, 0, 56, 248, 63]),
                    mission_encounter_pose([140, 272, -5_412, 0, 78, 252, 63]),
                ],
            ),
            (
                712,
                [
                    mission_encounter_pose([-5_640, -194, -5_044, 0, 54, 248, 63]),
                    mission_encounter_pose([-92, 238, -5_484, 0, 77, 252, 63]),
                ],
            ),
            (
                716,
                [
                    mission_encounter_pose([-5_872, -170, -4_980, 0, 52, 248, 63]),
                    mission_encounter_pose([-324, 209, -5_548, 0, 76, 252, 63]),
                ],
            ),
            (
                720,
                [
                    mission_encounter_pose([-6_100, -149, -4_904, 0, 50, 248, 63]),
                    mission_encounter_pose([-324, 209, -5_548, 0, 76, 252, 63]),
                ],
            ),
            (
                724,
                [
                    mission_encounter_pose([-6_100, -149, -4_904, 0, 50, 248, 63]),
                    mission_encounter_pose([-560, 183, -5_608, 0, 75, 252, 63]),
                ],
            ),
            (
                728,
                [
                    mission_encounter_pose([-6_324, -131, -4_816, 0, 48, 248, 63]),
                    mission_encounter_pose([-796, 161, -5_660, 0, 74, 252, 63]),
                ],
            ),
            (
                732,
                [
                    mission_encounter_pose([-6_544, 317, -4_716, 0, 46, 248, 63]),
                    mission_encounter_pose([-1_032, -291, -5_708, 0, 73, 252, 63]),
                ],
            ),
            (
                736,
                [
                    mission_encounter_pose([-6_760, 773, -4_604, 6, 44, 248, 63]),
                    mission_encounter_pose([-1_272, -747, -5_748, 250, 72, 252, 63]),
                ],
            ),
            (
                740,
                [
                    mission_encounter_pose([-6_964, 1_281, -4_484, 12, 42, 248, 63]),
                    mission_encounter_pose([-1_508, -1_255, -5_784, 244, 71, 252, 63]),
                ],
            ),
            (
                744,
                [
                    mission_encounter_pose([-7_156, 1_841, -4_356, 17, 40, 248, 63]),
                    mission_encounter_pose([-1_736, -1_815, -5_812, 239, 70, 252, 63]),
                ],
            ),
            (
                748,
                [
                    mission_encounter_pose([-7_336, 2_441, -4_228, 22, 38, 248, 63]),
                    mission_encounter_pose([-1_960, -2_415, -5_836, 234, 69, 252, 63]),
                ],
            ),
            (
                752,
                [
                    mission_encounter_pose([-7_496, 2_565, -4_100, 22, 36, 248, 63]),
                    mission_encounter_pose([-1_960, -2_415, -5_836, 234, 69, 252, 63]),
                ],
            ),
            (
                756,
                [
                    mission_encounter_pose([-7_496, 3_069, -4_100, 27, 36, 248, 63]),
                    mission_encounter_pose([-2_168, -3_043, -5_852, 229, 68, 252, 63]),
                ],
            ),
            (
                760,
                [
                    mission_encounter_pose([-7_636, 3_725, -3_972, 31, 34, 248, 63]),
                    mission_encounter_pose([-2_360, -3_699, -5_864, 225, 67, 252, 63]),
                ],
            ),
            (
                764,
                [
                    mission_encounter_pose([-7_756, 4_393, -3_852, 35, 32, 248, 63]),
                    mission_encounter_pose([-2_536, -4_367, -5_868, 221, 66, 252, 63]),
                ],
            ),
            (
                768,
                [
                    mission_encounter_pose([-7_860, 5_073, -3_736, 38, 30, 248, 63]),
                    mission_encounter_pose([-2_692, -5_047, -5_868, 218, 65, 252, 63]),
                ],
            ),
            (
                772,
                [
                    mission_encounter_pose([-7_948, 5_749, -3_624, 40, 28, 248, 63]),
                    mission_encounter_pose([-2_836, -5_723, -5_868, 216, 64, 252, 63]),
                ],
            ),
            (
                776,
                [
                    mission_encounter_pose([-8_024, 6_409, -3_516, 42, 26, 248, 63]),
                    mission_encounter_pose([-2_968, -6_383, -5_868, 214, 63, 252, 63]),
                ],
            ),
            (
                780,
                [
                    mission_encounter_pose([-8_092, 7_053, -3_416, 43, 24, 248, 63]),
                    mission_encounter_pose([-2_968, -6_383, -5_868, 214, 63, 252, 63]),
                ],
            ),
            (
                784,
                [
                    mission_encounter_pose([-8_092, 7_053, -3_416, 43, 24, 248, 63]),
                    mission_encounter_pose([-3_092, -7_027, -5_864, 213, 62, 252, 63]),
                ],
            ),
            (
                788,
                [
                    mission_encounter_pose([-8_152, 7_677, -3_316, 44, 22, 248, 63]),
                    mission_encounter_pose([-3_212, -7_651, -5_860, 212, 61, 252, 63]),
                ],
            ),
            (
                792,
                [
                    mission_encounter_pose([-8_204, 8_273, -3_216, 45, 20, 248, 63]),
                    mission_encounter_pose([-3_324, -8_247, -5_852, 211, 60, 252, 63]),
                ],
            ),
            (
                796,
                [
                    mission_encounter_pose([-8_248, 8_833, -3_120, 46, 18, 248, 63]),
                    mission_encounter_pose([-3_432, -8_807, -5_840, 210, 59, 252, 63]),
                ],
            ),
            (
                800,
                [
                    mission_encounter_pose([-8_284, 9_357, -3_024, 45, 16, 248, 63]),
                    mission_encounter_pose([-3_532, -9_331, -5_828, 211, 58, 252, 63]),
                ],
            ),
            (
                804,
                [
                    mission_encounter_pose([-8_316, 9_837, -2_924, 44, 14, 248, 63]),
                    mission_encounter_pose([-3_640, -9_811, -5_812, 212, 57, 252, 63]),
                ],
            ),
            (
                808,
                [
                    mission_encounter_pose([-8_344, 10_273, -2_816, 43, 12, 248, 63]),
                    mission_encounter_pose([-3_752, -10_247, -5_792, 213, 56, 252, 63]),
                ],
            ),
            (
                812,
                [
                    mission_encounter_pose([-8_368, 10_657, -2_700, 41, 10, 248, 63]),
                    mission_encounter_pose([-3_752, -10_247, -5_792, 213, 56, 252, 63]),
                ],
            ),
            (
                816,
                [
                    mission_encounter_pose([-8_368, 10_657, -2_700, 41, 10, 248, 63]),
                    mission_encounter_pose([-3_868, -10_631, -5_768, 215, 55, 252, 63]),
                ],
            ),
            (
                820,
                [
                    mission_encounter_pose([-8_388, 10_985, -2_576, 39, 8, 248, 63]),
                    mission_encounter_pose([-3_992, -10_959, -5_740, 217, 54, 252, 63]),
                ],
            ),
            (
                824,
                [
                    mission_encounter_pose([-8_404, 11_257, -2_440, 36, 6, 248, 63]),
                    mission_encounter_pose([-4_128, -11_231, -5_704, 220, 53, 252, 63]),
                ],
            ),
            (
                828,
                [
                    mission_encounter_pose([-8_416, 11_473, -2_288, 33, 4, 248, 63]),
                    mission_encounter_pose([-4_272, -11_447, -5_664, 223, 52, 252, 63]),
                ],
            ),
            (
                832,
                [
                    mission_encounter_pose([-8_420, 11_625, -2_120, 30, 2, 248, 63]),
                    mission_encounter_pose([-4_432, -11_599, -5_616, 226, 51, 252, 63]),
                ],
            ),
            (
                836,
                [
                    mission_encounter_pose([-8_420, 11_717, -1_940, 26, 0, 248, 63]),
                    mission_encounter_pose([-4_600, -11_691, -5_560, 230, 50, 252, 63]),
                ],
            ),
            (
                840,
                [
                    mission_encounter_pose([-8_416, 11_741, -1_744, 22, 254, 248, 63]),
                    mission_encounter_pose([-4_784, -11_715, -5_492, 234, 49, 252, 63]),
                ],
            ),
            (
                844,
                [
                    mission_encounter_pose([-8_400, 11_697, -1_536, 18, 252, 248, 63]),
                    mission_encounter_pose([-4_976, -11_671, -5_416, 238, 48, 252, 63]),
                ],
            ),
            (
                848,
                [
                    mission_encounter_pose([-8_372, 11_585, -1_320, 14, 250, 248, 63]),
                    mission_encounter_pose([-5_176, -11_559, -5_328, 242, 47, 252, 63]),
                ],
            ),
            (
                852,
                [
                    mission_encounter_pose([-8_332, 11_405, -1_096, 9, 248, 248, 63]),
                    mission_encounter_pose([-5_384, -11_379, -5_232, 247, 46, 252, 63]),
                ],
            ),
            (
                856,
                [
                    mission_encounter_pose([-8_280, 11_157, -868, 4, 246, 248, 63]),
                    mission_encounter_pose([-5_592, -11_131, -5_128, 252, 45, 252, 63]),
                ],
            ),
            (
                860,
                [
                    mission_encounter_pose([-8_216, 10_837, -636, 255, 244, 248, 63]),
                    mission_encounter_pose([-5_592, -11_131, -5_128, 252, 44, 252, 63]),
                ],
            ),
            (
                864,
                [
                    mission_encounter_pose([-8_140, 10_833, -408, 250, 242, 248, 63]),
                    mission_encounter_pose([-5_808, -10_811, -5_016, 1, 44, 252, 63]),
                ],
            ),
            (
                868,
                [
                    mission_encounter_pose([-8_140, 10_457, -408, 250, 242, 248, 63]),
                    mission_encounter_pose([-6_020, -10_431, -4_900, 6, 43, 252, 63]),
                ],
            ),
            (
                872,
                [
                    mission_encounter_pose([-8_052, 10_017, -188, 245, 240, 248, 63]),
                    mission_encounter_pose([-6_224, -9_991, -4_780, 11, 42, 252, 63]),
                ],
            ),
            (
                876,
                [
                    mission_encounter_pose([-7_956, 9_521, 24, 240, 238, 248, 63]),
                    mission_encounter_pose([-6_420, -9_495, -4_660, 16, 41, 252, 63]),
                ],
            ),
            (
                880,
                [
                    mission_encounter_pose([-7_852, 8_973, 224, 236, 236, 248, 63]),
                    mission_encounter_pose([-6_604, -8_947, -4_536, 20, 40, 252, 63]),
                ],
            ),
            (
                884,
                [
                    mission_encounter_pose([-7_744, 8_857, 408, 236, 234, 248, 63]),
                    mission_encounter_pose([-6_604, -8_947, -4_536, 20, 40, 252, 63]),
                ],
            ),
            (
                888,
                [
                    mission_encounter_pose([-7_744, 8_381, 408, 232, 234, 248, 63]),
                    mission_encounter_pose([-6_776, -8_355, -4_416, 24, 39, 252, 63]),
                ],
            ),
            (
                892,
                [
                    mission_encounter_pose([-7_636, 7_753, 572, 228, 232, 248, 63]),
                    mission_encounter_pose([-6_940, -7_727, -4_300, 28, 38, 252, 63]),
                ],
            ),
            (
                896,
                [
                    mission_encounter_pose([-7_528, 7_097, 724, 224, 230, 248, 63]),
                    mission_encounter_pose([-7_088, -7_071, -4_184, 32, 37, 252, 63]),
                ],
            ),
            (
                900,
                [
                    mission_encounter_pose([-7_420, 6_421, 856, 221, 228, 248, 63]),
                    mission_encounter_pose([-7_220, -6_395, -4_076, 35, 36, 252, 63]),
                ],
            ),
            (
                904,
                [
                    mission_encounter_pose([-7_316, 5_729, 972, 218, 226, 248, 63]),
                    mission_encounter_pose([-7_220, -6_395, -4_076, 35, 36, 252, 63]),
                ],
            ),
            (
                908,
                [
                    mission_encounter_pose([-7_316, 5_729, 972, 218, 226, 248, 63]),
                    mission_encounter_pose([-7_340, -5_703, -3_976, 38, 35, 252, 63]),
                ],
            ),
            (
                912,
                [
                    mission_encounter_pose([-7_216, 5_029, 1_072, 215, 224, 248, 63]),
                    mission_encounter_pose([-7_444, -5_003, -3_880, 41, 34, 252, 63]),
                ],
            ),
            (
                916,
                [
                    mission_encounter_pose([-7_120, 4_329, 1_156, 213, 222, 248, 63]),
                    mission_encounter_pose([-7_536, -4_303, -3_796, 43, 33, 252, 63]),
                ],
            ),
            (
                920,
                [
                    mission_encounter_pose([-7_028, 3_637, 1_228, 211, 220, 248, 63]),
                    mission_encounter_pose([-7_616, -3_611, -3_716, 45, 32, 252, 63]),
                ],
            ),
            (
                924,
                [
                    mission_encounter_pose([-6_940, 2_961, 1_292, 210, 218, 248, 63]),
                    mission_encounter_pose([-7_688, -2_935, -3_640, 46, 31, 252, 63]),
                ],
            ),
            (
                928,
                [
                    mission_encounter_pose([-6_856, 2_305, 1_348, 209, 216, 248, 63]),
                    mission_encounter_pose([-7_756, -2_495, -3_564, 47, 30, 252, 63]),
                ],
            ),
            (
                932,
                [
                    mission_encounter_pose([-6_772, 1_669, 1_396, 208, 214, 248, 63]),
                    mission_encounter_pose([-7_756, -2_279, -3_564, 47, 30, 252, 63]),
                ],
            ),
            (
                936,
                [
                    mission_encounter_pose([-6_772, 1_669, 1_396, 208, 214, 248, 63]),
                    mission_encounter_pose([-7_816, -1_643, -3_492, 48, 29, 252, 63]),
                ],
            ),
            (
                940,
                [
                    mission_encounter_pose([-6_692, 1_065, 1_436, 207, 212, 248, 63]),
                    mission_encounter_pose([-7_872, -1_039, -3_420, 49, 28, 252, 63]),
                ],
            ),
            (
                944,
                [
                    mission_encounter_pose([-6_616, 493, 1_472, 208, 210, 248, 63]),
                    mission_encounter_pose([-7_924, -467, -3_352, 48, 27, 252, 63]),
                ],
            ),
            (
                948,
                [
                    mission_encounter_pose([-6_532, -35, 1_504, 209, 208, 248, 63]),
                    mission_encounter_pose([-7_976, 61, -3_280, 47, 26, 252, 63]),
                ],
            ),
            (
                952,
                [
                    mission_encounter_pose([-6_440, -523, 1_532, 210, 206, 248, 63]),
                    mission_encounter_pose([-8_028, 549, -3_204, 46, 25, 252, 63]),
                ],
            ),
            (
                956,
                [
                    mission_encounter_pose([-6_344, -963, 1_560, 211, 204, 248, 63]),
                    mission_encounter_pose([-8_028, 549, -3_204, 46, 25, 252, 63]),
                ],
            ),
            (
                960,
                [
                    mission_encounter_pose([-6_344, -963, 1_560, 211, 204, 248, 63]),
                    mission_encounter_pose([-8_084, 989, -3_120, 45, 24, 252, 63]),
                ],
            ),
            (
                964,
                [
                    mission_encounter_pose([-6_240, -1_351, 1_584, 213, 202, 248, 63]),
                    mission_encounter_pose([-8_140, 1_377, -3_028, 43, 23, 252, 63]),
                ],
            ),
            (
                968,
                [
                    mission_encounter_pose([-6_124, -1_687, 1_604, 215, 200, 248, 63]),
                    mission_encounter_pose([-8_200, 1_713, -2_928, 41, 22, 252, 63]),
                ],
            ),
            (
                972,
                [
                    mission_encounter_pose([-6_000, -1_967, 1_620, 218, 198, 248, 63]),
                    mission_encounter_pose([-8_260, 1_993, -2_816, 38, 21, 252, 63]),
                ],
            ),
            (
                976,
                [
                    mission_encounter_pose([-5_856, -2_191, 1_628, 221, 196, 248, 63]),
                    mission_encounter_pose([-8_324, 2_217, -2_688, 35, 20, 252, 63]),
                ],
            ),
            (
                980,
                [
                    mission_encounter_pose([-5_700, -2_355, 1_632, 225, 194, 248, 63]),
                    mission_encounter_pose([-8_392, 2_381, -2_548, 31, 19, 252, 63]),
                ],
            ),
            (
                984,
                [
                    mission_encounter_pose([-5_524, -2_451, 1_632, 229, 192, 248, 63]),
                    mission_encounter_pose([-8_392, 2_381, -2_548, 31, 19, 252, 63]),
                ],
            ),
            (
                988,
                [
                    mission_encounter_pose([-5_524, -2_451, 1_632, 229, 192, 248, 63]),
                    mission_encounter_pose([-8_464, 2_477, -2_392, 27, 18, 252, 63]),
                ],
            ),
            (
                992,
                [
                    mission_encounter_pose([-5_332, -2_483, 1_628, 233, 190, 248, 63]),
                    mission_encounter_pose([-8_540, 2_509, -2_216, 23, 17, 252, 63]),
                ],
            ),
            (
                996,
                [
                    mission_encounter_pose([-5_128, -2_443, 1_612, 237, 188, 248, 63]),
                    mission_encounter_pose([-8_616, 2_469, -2_028, 19, 16, 252, 63]),
                ],
            ),
            (
                1_000,
                [
                    mission_encounter_pose([-4_916, -2_339, 1_584, 242, 186, 248, 63]),
                    mission_encounter_pose([-8_692, 2_365, -1_824, 14, 15, 252, 63]),
                ],
            ),
            (
                1_004,
                [
                    mission_encounter_pose([-4_692, -2_159, 1_544, 247, 184, 248, 63]),
                    mission_encounter_pose([-8_764, 2_185, -1_612, 9, 14, 252, 63]),
                ],
            ),
            (
                1_008,
                [
                    mission_encounter_pose([-4_464, -1_911, 1_492, 252, 182, 248, 63]),
                    mission_encounter_pose([-8_836, 1_937, -1_388, 4, 13, 252, 63]),
                ],
            ),
            (
                1_012,
                [
                    mission_encounter_pose([-4_232, -1_591, 1_428, 1, 180, 248, 63]),
                    mission_encounter_pose([-8_900, 1_617, -1_156, 255, 12, 252, 63]),
                ],
            ),
            (
                1_016,
                [
                    mission_encounter_pose([-4_004, -1_211, 1_352, 6, 178, 248, 63]),
                    mission_encounter_pose([-8_960, 1_613, -920, 250, 11, 252, 63]),
                ],
            ),
            (
                1_020,
                [
                    mission_encounter_pose([-3_784, -771, 1_264, 11, 176, 248, 63]),
                    mission_encounter_pose([-8_960, 1_237, -920, 250, 11, 252, 63]),
                ],
            ),
            (
                1_024,
                [
                    mission_encounter_pose([-3_784, -771, 1_264, 11, 176, 248, 63]),
                    mission_encounter_pose([-9_012, 797, -688, 245, 10, 252, 63]),
                ],
            ),
            (
                1_028,
                [
                    mission_encounter_pose([-3_572, -275, 1_168, 16, 174, 248, 63]),
                    mission_encounter_pose([-9_060, 301, -460, 240, 9, 252, 63]),
                ],
            ),
            (
                1_032,
                [
                    mission_encounter_pose([-3_372, 273, 1_064, 20, 172, 248, 63]),
                    mission_encounter_pose([-9_100, 209, -240, 236, 8, 252, 63]),
                ],
            ),
            (
                1_036,
                [
                    mission_encounter_pose([-3_372, 273, 1_064, 20, 172, 248, 63]),
                    mission_encounter_pose([-9_100, -247, -240, 236, 8, 252, 63]),
                ],
            ),
            (
                1_040,
                [
                    mission_encounter_pose([-3_188, 865, 956, 24, 170, 248, 63]),
                    mission_encounter_pose([-9_132, -839, -28, 232, 7, 252, 63]),
                ],
            ),
            (
                1_044,
                [
                    mission_encounter_pose([-3_024, 1_493, 848, 28, 168, 248, 63]),
                    mission_encounter_pose([-9_156, -1_467, 172, 228, 6, 252, 63]),
                ],
            ),
            (
                1_048,
                [
                    mission_encounter_pose([-2_872, 2_149, 740, 32, 166, 248, 63]),
                    mission_encounter_pose([-9_176, -2_123, 360, 224, 5, 252, 63]),
                ],
            ),
            (
                1_052,
                [
                    mission_encounter_pose([-2_740, 2_825, 632, 35, 164, 248, 63]),
                    mission_encounter_pose([-9_188, -2_295, 532, 221, 4, 252, 63]),
                ],
            ),
            (
                1_056,
                [
                    mission_encounter_pose([-2_624, 3_013, 528, 35, 162, 248, 63]),
                    mission_encounter_pose([-9_188, -2_799, 532, 221, 4, 252, 63]),
                ],
            ),
            (
                1_060,
                [
                    mission_encounter_pose([-2_624, 3_517, 528, 38, 162, 248, 63]),
                    mission_encounter_pose([-9_196, -3_491, 688, 218, 3, 252, 63]),
                ],
            ),
            (
                1_064,
                [
                    mission_encounter_pose([-2_524, 4_217, 428, 41, 160, 248, 63]),
                    mission_encounter_pose([-9_200, -4_191, 832, 215, 2, 252, 63]),
                ],
            ),
            (
                1_068,
                [
                    mission_encounter_pose([-2_440, 4_917, 332, 43, 158, 248, 63]),
                    mission_encounter_pose([-9_200, -4_891, 960, 213, 1, 252, 63]),
                ],
            ),
            (
                1_072,
                [
                    mission_encounter_pose([-2_368, 5_609, 240, 45, 156, 248, 63]),
                    mission_encounter_pose([-9_200, -5_583, 1_080, 211, 0, 252, 63]),
                ],
            ),
            (
                1_076,
                [
                    mission_encounter_pose([-2_304, 5_829, 152, 45, 154, 248, 63]),
                    mission_encounter_pose([-9_200, -5_583, 1_080, 211, 0, 252, 63]),
                ],
            ),
            (
                1_080,
                [
                    mission_encounter_pose([-2_304, 6_285, 152, 46, 154, 248, 63]),
                    mission_encounter_pose([-9_200, -6_259, 1_188, 210, 255, 252, 63]),
                ],
            ),
            (
                1_084,
                [
                    mission_encounter_pose([-2_248, 6_941, 68, 47, 152, 248, 63]),
                    mission_encounter_pose([-9_200, -6_915, 1_292, 209, 254, 252, 63]),
                ],
            ),
            (
                1_088,
                [
                    mission_encounter_pose([-2_200, 7_577, -16, 48, 150, 248, 63]),
                    mission_encounter_pose([-9_196, -7_551, 1_388, 208, 253, 252, 63]),
                ],
            ),
            (
                1_092,
                [
                    mission_encounter_pose([-2_160, 8_181, -96, 49, 148, 248, 63]),
                    mission_encounter_pose([-9_192, -8_155, 1_480, 207, 252, 252, 63]),
                ],
            ),
            (
                1_096,
                [
                    mission_encounter_pose([-2_124, 8_753, -172, 48, 146, 248, 63]),
                    mission_encounter_pose([-9_184, -8_727, 1_564, 208, 251, 252, 63]),
                ],
            ),
            (
                1_100,
                [
                    mission_encounter_pose([-2_092, 9_281, -256, 47, 144, 248, 63]),
                    mission_encounter_pose([-9_172, -9_255, 1_652, 209, 250, 252, 63]),
                ],
            ),
            (
                1_104,
                [
                    mission_encounter_pose([-2_092, 9_281, -256, 47, 144, 248, 63]),
                    mission_encounter_pose([-9_172, -9_255, 1_652, 209, 250, 252, 63]),
                ],
            ),
            (
                1_108,
                [
                    mission_encounter_pose([-2_064, 9_769, -348, 46, 142, 248, 63]),
                    mission_encounter_pose([-9_160, -9_743, 1_748, 210, 249, 252, 63]),
                ],
            ),
            (
                1_112,
                [
                    mission_encounter_pose([-2_036, 10_209, -444, 45, 140, 248, 63]),
                    mission_encounter_pose([-9_144, -10_183, 1_848, 211, 248, 252, 63]),
                ],
            ),
            (
                1_116,
                [
                    mission_encounter_pose([-2_012, 10_597, -548, 43, 138, 248, 63]),
                    mission_encounter_pose([-9_124, -10_571, 1_952, 213, 247, 252, 63]),
                ],
            ),
            (
                1_120,
                [
                    mission_encounter_pose([-1_992, 10_933, -664, 41, 136, 248, 63]),
                    mission_encounter_pose([-9_124, -10_571, 1_952, 213, 247, 252, 63]),
                ],
            ),
            (
                1_124,
                [
                    mission_encounter_pose([-1_992, 10_933, -664, 41, 136, 248, 63]),
                    mission_encounter_pose([-9_100, -10_907, 2_068, 215, 246, 252, 63]),
                ],
            ),
            (
                1_128,
                [
                    mission_encounter_pose([-1_976, 11_213, -788, 38, 134, 248, 63]),
                    mission_encounter_pose([-9_068, -11_187, 2_192, 218, 245, 252, 63]),
                ],
            ),
            (
                1_132,
                [
                    mission_encounter_pose([-1_968, 11_437, -932, 35, 132, 248, 63]),
                    mission_encounter_pose([-9_032, -11_411, 2_328, 221, 244, 252, 63]),
                ],
            ),
            (
                1_136,
                [
                    mission_encounter_pose([-1_964, 11_601, -1_088, 31, 130, 248, 63]),
                    mission_encounter_pose([-8_984, -11_575, 2_476, 225, 243, 252, 63]),
                ],
            ),
            (
                1_140,
                [
                    mission_encounter_pose([-1_964, 11_697, -1_264, 27, 128, 248, 63]),
                    mission_encounter_pose([-8_928, -11_743, 2_640, 225, 242, 252, 63]),
                ],
            ),
            (
                1_144,
                [
                    mission_encounter_pose([-1_964, 11_697, -1_264, 27, 128, 248, 63]),
                    mission_encounter_pose([-8_928, -11_671, 2_640, 229, 242, 252, 63]),
                ],
            ),
            (
                1_148,
                [
                    mission_encounter_pose([-1_968, 11_729, -1_456, 23, 126, 248, 63]),
                    mission_encounter_pose([-8_860, -11_703, 2_820, 233, 241, 252, 63]),
                ],
            ),
            (
                1_152,
                [
                    mission_encounter_pose([-1_984, 11_689, -1_660, 19, 124, 248, 63]),
                    mission_encounter_pose([-8_784, -11_663, 3_008, 237, 240, 252, 63]),
                ],
            ),
            (
                1_156,
                [
                    mission_encounter_pose([-2_012, 11_585, -1_872, 14, 122, 248, 63]),
                    mission_encounter_pose([-8_696, -11_559, 3_208, 242, 239, 252, 63]),
                ],
            ),
            (
                1_160,
                [
                    mission_encounter_pose([-2_052, 11_405, -2_096, 9, 120, 248, 63]),
                    mission_encounter_pose([-8_600, -11_379, 3_416, 247, 238, 252, 63]),
                ],
            ),
            (
                1_164,
                [
                    mission_encounter_pose([-2_104, 11_157, -2_324, 4, 118, 248, 63]),
                    mission_encounter_pose([-8_496, -11_131, 3_624, 252, 237, 252, 63]),
                ],
            ),
            (
                1_168,
                [
                    mission_encounter_pose([-2_168, 10_837, -2_556, 255, 116, 248, 63]),
                    mission_encounter_pose([-8_496, -11_131, 3_624, 252, 237, 252, 63]),
                ],
            ),
            (
                1_172,
                [
                    mission_encounter_pose([-2_168, 10_837, -2_556, 255, 116, 248, 63]),
                    mission_encounter_pose([-8_384, -10_811, 3_840, 1, 236, 252, 63]),
                ],
            ),
            (
                1_176,
                [
                    mission_encounter_pose([-2_244, 10_457, -2_784, 250, 114, 248, 63]),
                    mission_encounter_pose([-8_268, -10_431, 4_052, 6, 235, 252, 63]),
                ],
            ),
            (
                1_180,
                [
                    mission_encounter_pose([-2_332, 10_017, -3_004, 245, 112, 248, 63]),
                    mission_encounter_pose([-8_148, -9_991, 4_256, 11, 234, 252, 63]),
                ],
            ),
            (
                1_184,
                [
                    mission_encounter_pose([-2_428, 9_521, -3_216, 240, 110, 248, 63]),
                    mission_encounter_pose([-8_148, -9_991, 4_256, 11, 234, 252, 63]),
                ],
            ),
            (
                1_188,
                [
                    mission_encounter_pose([-2_428, 9_521, -3_216, 240, 110, 248, 63]),
                    mission_encounter_pose([-8_028, -9_495, 4_452, 16, 233, 252, 63]),
                ],
            ),
            (
                1_192,
                [
                    mission_encounter_pose([-2_532, 8_239, -3_416, 236, 108, 250, 63]),
                    mission_encounter_pose([-7_904, -8_217, 4_636, 20, 232, 253, 63]),
                ],
            ),
            (
                1_196,
                [
                    mission_encounter_pose([-2_636, 7_094, -3_604, 235, 107, 251, 63]),
                    mission_encounter_pose([-7_904, -8_217, 4_636, 20, 232, 253, 63]),
                ],
            ),
            (
                1_200,
                [
                    mission_encounter_pose([-2_636, 7_094, -3_604, 235, 107, 251, 63]),
                    mission_encounter_pose([-7_788, -7_074, 4_812, 21, 232, 254, 63]),
                ],
            ),
            (
                1_204,
                [
                    mission_encounter_pose([-2_740, 6_088, -3_784, 236, 106, 252, 63]),
                    mission_encounter_pose([-7_672, -6_070, 4_984, 20, 232, 255, 63]),
                ],
            ),
            (
                1_208,
                [
                    mission_encounter_pose([-2_852, 5_211, -3_964, 237, 105, 253, 63]),
                    mission_encounter_pose([-7_556, -5_196, 5_160, 19, 232, 0, 63]),
                ],
            ),
            (
                1_212,
                [
                    mission_encounter_pose([-2_964, 4_448, -4_144, 239, 105, 254, 63]),
                    mission_encounter_pose([-7_436, -4_435, 5_340, 17, 232, 1, 63]),
                ],
            ),
            (
                1_216,
                [
                    mission_encounter_pose([-2_964, 4_448, -4_144, 239, 105, 254, 63]),
                    mission_encounter_pose([-7_436, -4_435, 5_340, 17, 232, 1, 63]),
                ],
            ),
            (
                1_220,
                [
                    mission_encounter_pose([-3_080, 3_792, -4_332, 241, 105, 255, 63]),
                    mission_encounter_pose([-7_316, -3_781, 5_524, 15, 232, 2, 63]),
                ],
            ),
            (
                1_224,
                [
                    mission_encounter_pose([-3_196, 3_230, -4_520, 242, 105, 0, 63]),
                    mission_encounter_pose([-7_192, -3_221, 5_712, 14, 232, 3, 63]),
                ],
            ),
            (
                1_228,
                [
                    mission_encounter_pose([-3_312, 2_747, -4_712, 243, 105, 1, 63]),
                    mission_encounter_pose([-7_068, -2_739, 5_900, 13, 232, 4, 63]),
                ],
            ),
            (
                1_232,
                [
                    mission_encounter_pose([-3_432, 2_328, -4_904, 244, 105, 2, 63]),
                    mission_encounter_pose([-6_948, -2_321, 6_092, 12, 233, 4, 63]),
                ],
            ),
            (
                1_236,
                [
                    mission_encounter_pose([-3_552, 1_969, -5_100, 245, 105, 3, 63]),
                    mission_encounter_pose([-6_832, -1_963, 6_292, 11, 234, 4, 63]),
                ],
            ),
            (
                1_240,
                [
                    mission_encounter_pose([-3_672, 1_659, -5_296, 246, 105, 4, 63]),
                    mission_encounter_pose([-6_720, -1_654, 6_496, 11, 235, 4, 63]),
                ],
            ),
            (
                1_244,
                [
                    mission_encounter_pose([-3_672, 1_659, -5_296, 246, 105, 4, 63]),
                    mission_encounter_pose([-6_720, -1_654, 6_496, 10, 235, 4, 63]),
                ],
            ),
            (
                1_248,
                [
                    mission_encounter_pose([-3_788, 1_396, -5_496, 247, 106, 5, 63]),
                    mission_encounter_pose([-6_612, -1_392, 6_704, 9, 236, 4, 63]),
                ],
            ),
            (
                1_252,
                [
                    mission_encounter_pose([-3_900, 1_170, -5_700, 248, 107, 6, 63]),
                    mission_encounter_pose([-6_508, -1_166, 6_912, 8, 237, 4, 63]),
                ],
            ),
            (
                1_256,
                [
                    mission_encounter_pose([-4_012, 980, -5_912, 249, 108, 7, 63]),
                    mission_encounter_pose([-6_408, -977, 7_128, 7, 238, 4, 63]),
                ],
            ),
            (
                1_260,
                [
                    mission_encounter_pose([-4_120, 818, -6_124, 250, 109, 8, 63]),
                    mission_encounter_pose([-6_312, -815, 7_348, 6, 239, 4, 63]),
                ],
            ),
            (
                1_264,
                [
                    mission_encounter_pose([-4_120, 818, -6_124, 250, 109, 8, 63]),
                    mission_encounter_pose([-6_312, -815, 7_348, 6, 239, 4, 63]),
                ],
            ),
            (
                1_268,
                [
                    mission_encounter_pose([-4_216, 684, -6_344, 251, 111, 8, 63]),
                    mission_encounter_pose([-6_224, -682, 7_568, 5, 240, 4, 63]),
                ],
            ),
            (
                1_272,
                [
                    mission_encounter_pose([-4_300, 571, -6_572, 252, 113, 8, 63]),
                    mission_encounter_pose([-6_140, -569, 7_796, 4, 241, 4, 63]),
                ],
            ),
            (
                1_276,
                [
                    mission_encounter_pose([-4_372, 480, -6_804, 253, 115, 8, 63]),
                    mission_encounter_pose([-6_064, -478, 8_024, 3, 242, 4, 63]),
                ],
            ),
            (
                1_280,
                [
                    mission_encounter_pose([-4_432, 404, -7_040, 254, 117, 8, 63]),
                    mission_encounter_pose([-5_992, -403, 8_256, 2, 243, 4, 63]),
                ],
            ),
            (
                1_284,
                [
                    mission_encounter_pose([-4_480, 346, -7_276, 255, 119, 8, 63]),
                    mission_encounter_pose([-5_928, -345, 8_488, 1, 244, 4, 63]),
                ],
            ),
            (
                1_288,
                [
                    mission_encounter_pose([-4_516, 299, -7_516, 0, 121, 8, 63]),
                    mission_encounter_pose([-5_868, -298, 8_724, 0, 245, 4, 63]),
                ],
            ),
            (
                1_292,
                [
                    mission_encounter_pose([-4_540, 262, -7_760, 0, 123, 8, 63]),
                    mission_encounter_pose([-5_816, -261, 8_960, 0, 246, 4, 63]),
                ],
            ),
            (
                1_296,
                [
                    mission_encounter_pose([-4_552, 230, -8_004, 0, 125, 8, 63]),
                    mission_encounter_pose([-5_768, -229, 9_196, 0, 247, 4, 63]),
                ],
            ),
            (
                1_300,
                [
                    mission_encounter_pose([-4_552, 202, -8_248, 0, 127, 8, 63]),
                    mission_encounter_pose([-5_728, -201, 9_436, 0, 248, 4, 63]),
                ],
            ),
            (
                1_304,
                [
                    mission_encounter_pose([-4_552, 177, -8_492, 0, 129, 8, 63]),
                    mission_encounter_pose([-5_692, -176, 9_676, 0, 249, 4, 63]),
                ],
            ),
            (
                1_308,
                [
                    mission_encounter_pose([-4_540, 155, -8_736, 0, 131, 8, 63]),
                    mission_encounter_pose([-5_692, -176, 9_676, 0, 249, 4, 63]),
                ],
            ),
            (
                1_312,
                [
                    mission_encounter_pose([-4_540, 155, -8_736, 0, 131, 8, 63]),
                    mission_encounter_pose([-5_664, -154, 9_916, 0, 250, 4, 63]),
                ],
            ),
            (
                1_316,
                [
                    mission_encounter_pose([-4_516, 136, -8_980, 0, 133, 8, 63]),
                    mission_encounter_pose([-5_640, -135, 10_160, 0, 251, 4, 63]),
                ],
            ),
            (
                1_320,
                [
                    mission_encounter_pose([-4_480, 119, -9_220, 0, 135, 8, 63]),
                    mission_encounter_pose([-5_624, -119, 10_404, 0, 252, 4, 63]),
                ],
            ),
            (
                1_324,
                [
                    mission_encounter_pose([-4_432, 105, -9_456, 0, 137, 8, 63]),
                    mission_encounter_pose([-5_612, -105, 10_648, 0, 253, 4, 63]),
                ],
            ),
            (
                1_328,
                [
                    mission_encounter_pose([-4_372, 92, -9_692, 0, 139, 8, 63]),
                    mission_encounter_pose([-5_608, -92, 10_892, 0, 254, 4, 63]),
                ],
            ),
            (
                1_332,
                [
                    mission_encounter_pose([-4_300, 81, -9_924, 0, 141, 8, 63]),
                    mission_encounter_pose([-5_608, -81, 11_136, 0, 255, 4, 63]),
                ],
            ),
            (
                1_336,
                [
                    mission_encounter_pose([-4_216, -385, -10_152, 0, 143, 8, 63]),
                    mission_encounter_pose([-5_608, 385, 11_380, 0, 0, 4, 63]),
                ],
            ),
            (
                1_340,
                [
                    mission_encounter_pose([-4_120, -861, -10_376, 249, 145, 8, 63]),
                    mission_encounter_pose([-5_608, 861, 11_624, 7, 1, 4, 63]),
                ],
            ),
            (
                1_344,
                [
                    mission_encounter_pose([-4_012, -1_393, -10_588, 243, 147, 8, 63]),
                    mission_encounter_pose([-5_608, 861, 11_624, 7, 1, 4, 63]),
                ],
            ),
            (
                1_348,
                [
                    mission_encounter_pose([-4_012, -1_393, -10_588, 243, 149, 8, 63]),
                    mission_encounter_pose([-5_612, 1_393, 11_864, 13, 2, 4, 63]),
                ],
            ),
            (
                1_352,
                [
                    mission_encounter_pose([-3_900, -1_969, -10_788, 237, 149, 8, 63]),
                    mission_encounter_pose([-5_624, 1_969, 12_096, 19, 3, 4, 63]),
                ],
            ),
            (
                1_356,
                [
                    mission_encounter_pose([-3_788, -2_585, -10_968, 232, 151, 8, 63]),
                    mission_encounter_pose([-5_640, 2_585, 12_312, 24, 4, 4, 63]),
                ],
            ),
            (
                1_360,
                [
                    mission_encounter_pose([-3_676, -3_225, -11_132, 228, 153, 8, 63]),
                    mission_encounter_pose([-5_660, 3_225, 12_512, 28, 5, 4, 63]),
                ],
            ),
            (
                1_364,
                [
                    mission_encounter_pose([-3_560, -3_881, -11_280, 224, 155, 8, 63]),
                    mission_encounter_pose([-5_684, 3_881, 12_696, 32, 6, 4, 63]),
                ],
            ),
            (
                1_368,
                [
                    mission_encounter_pose([-3_452, -4_545, -11_408, 221, 157, 8, 63]),
                    mission_encounter_pose([-5_708, 4_545, 12_864, 35, 7, 4, 63]),
                ],
            ),
            (
                1_372,
                [
                    mission_encounter_pose([-3_348, -5_209, -11_520, 218, 159, 8, 63]),
                    mission_encounter_pose([-5_736, 5_209, 13_020, 38, 8, 4, 63]),
                ],
            ),
            (
                1_376,
                [
                    mission_encounter_pose([-3_248, -5_865, -11_616, 216, 161, 8, 63]),
                    mission_encounter_pose([-5_764, 5_865, 13_160, 40, 9, 4, 63]),
                ],
            ),
            (
                1_380,
                [
                    mission_encounter_pose([-3_148, -6_501, -11_700, 214, 163, 8, 63]),
                    mission_encounter_pose([-5_792, 6_069, 13_288, 40, 10, 4, 63]),
                ],
            ),
            (
                1_384,
                [
                    mission_encounter_pose([-3_052, -6_815, -11_776, 213, 165, 8, 63]),
                    mission_encounter_pose([-5_792, 6_501, 13_288, 42, 10, 4, 63]),
                ],
            ),
            (
                1_388,
                [
                    mission_encounter_pose([-3_052, -7_121, -11_776, 213, 165, 8, 63]),
                    mission_encounter_pose([-5_824, 7_121, 13_408, 43, 11, 4, 63]),
                ],
            ),
            (
                1_392,
                [
                    mission_encounter_pose([-2_956, -7_713, -11_840, 212, 167, 8, 63]),
                    mission_encounter_pose([-5_856, 7_713, 13_520, 44, 12, 4, 63]),
                ],
            ),
            (
                1_396,
                [
                    mission_encounter_pose([-2_864, -8_273, -11_896, 211, 169, 8, 63]),
                    mission_encounter_pose([-5_888, 8_273, 13_628, 45, 13, 4, 63]),
                ],
            ),
            (
                1_400,
                [
                    mission_encounter_pose([-2_768, -8_793, -11_948, 212, 171, 8, 63]),
                    mission_encounter_pose([-5_920, 8_793, 13_728, 44, 14, 4, 63]),
                ],
            ),
            (
                1_404,
                [
                    mission_encounter_pose([-2_668, -9_273, -11_996, 213, 173, 8, 63]),
                    mission_encounter_pose([-5_960, 9_273, 13_832, 43, 15, 4, 63]),
                ],
            ),
            (
                1_408,
                [
                    mission_encounter_pose([-2_560, -9_705, -12_044, 214, 175, 8, 63]),
                    mission_encounter_pose([-6_004, 9_705, 13_940, 42, 16, 4, 63]),
                ],
            ),
            (
                1_412,
                [
                    mission_encounter_pose([-2_444, -10_085, -12_088, 215, 177, 8, 63]),
                    mission_encounter_pose([-6_052, 10_085, 14_052, 41, 17, 4, 63]),
                ],
            ),
            (
                1_416,
                [
                    mission_encounter_pose([-2_324, -10_413, -12_124, 217, 179, 8, 63]),
                    mission_encounter_pose([-6_052, 10_085, 14_052, 41, 17, 4, 63]),
                ],
            ),
            (
                1_420,
                [
                    mission_encounter_pose([-2_324, -10_413, -12_124, 217, 179, 8, 63]),
                    mission_encounter_pose([-6_104, 10_413, 14_168, 39, 18, 4, 63]),
                ],
            ),
            (
                1_424,
                [
                    mission_encounter_pose([-2_188, -10_685, -12_160, 220, 181, 8, 63]),
                    mission_encounter_pose([-6_164, 10_685, 14_292, 36, 19, 4, 63]),
                ],
            ),
            (
                1_428,
                [
                    mission_encounter_pose([-2_040, -10_901, -12_192, 223, 183, 8, 63]),
                    mission_encounter_pose([-6_236, 10_901, 14_428, 33, 20, 4, 63]),
                ],
            ),
            (
                1_432,
                [
                    mission_encounter_pose([-1_876, -11_053, -12_216, 226, 185, 8, 63]),
                    mission_encounter_pose([-6_316, 11_053, 14_572, 30, 21, 4, 63]),
                ],
            ),
            (
                1_436,
                [
                    mission_encounter_pose([-1_696, -11_145, -12_236, 230, 187, 8, 63]),
                    mission_encounter_pose([-6_404, 11_145, 14_724, 26, 22, 4, 63]),
                ],
            ),
            (
                1_440,
                [
                    mission_encounter_pose([-1_500, -11_169, -12_248, 234, 189, 8, 63]),
                    mission_encounter_pose([-6_504, 11_169, 14_888, 22, 23, 4, 63]),
                ],
            ),
            (
                1_444,
                [
                    mission_encounter_pose([-1_292, -11_125, -12_248, 238, 191, 8, 63]),
                    mission_encounter_pose([-6_616, 11_125, 15_060, 18, 24, 4, 63]),
                ],
            ),
            (
                1_448,
                [
                    mission_encounter_pose([-1_072, -11_013, -12_248, 242, 193, 8, 63]),
                    mission_encounter_pose([-6_740, 11_013, 15_236, 14, 25, 4, 63]),
                ],
            ),
            (
                1_452,
                [
                    mission_encounter_pose([-844, -10_833, -12_236, 247, 195, 8, 63]),
                    mission_encounter_pose([-6_872, 10_833, 15_420, 9, 26, 4, 63]),
                ],
            ),
            (
                1_456,
                [
                    mission_encounter_pose([-608, -10_585, -12_212, 252, 197, 8, 63]),
                    mission_encounter_pose([-7_016, 10_585, 15_608, 4, 27, 4, 63]),
                ],
            ),
            (
                1_460,
                [
                    mission_encounter_pose([-368, -10_265, -12_176, 1, 199, 8, 63]),
                    mission_encounter_pose([-7_016, 10_585, 15_608, 4, 28, 4, 63]),
                ],
            ),
            (
                1_464,
                [
                    mission_encounter_pose([-368, -10_265, -12_176, 1, 201, 8, 63]),
                    mission_encounter_pose([-7_168, 10_265, 15_796, 255, 28, 4, 63]),
                ],
            ),
            (
                1_468,
                [
                    mission_encounter_pose([-132, -9_885, -12_128, 6, 201, 8, 63]),
                    mission_encounter_pose([-7_324, 9_885, 15_980, 250, 29, 4, 63]),
                ],
            ),
            (
                1_472,
                [
                    mission_encounter_pose([100, -9_445, -12_068, 11, 203, 8, 63]),
                    mission_encounter_pose([-7_484, 9_445, 16_156, 245, 30, 4, 63]),
                ],
            ),
            (
                1_476,
                [
                    mission_encounter_pose([324, -8_949, -11_996, 16, 205, 8, 63]),
                    mission_encounter_pose([-7_644, 8_949, 16_320, 240, 31, 4, 63]),
                ],
            ),
            (
                1_480,
                [
                    mission_encounter_pose([536, -8_401, -11_916, 20, 207, 8, 63]),
                    mission_encounter_pose([-7_800, 8_401, 16_476, 236, 32, 4, 63]),
                ],
            ),
            (
                1_484,
                [
                    mission_encounter_pose([732, -7_809, -11_832, 24, 209, 8, 63]),
                    mission_encounter_pose([-7_952, 7_809, 16_620, 232, 33, 4, 63]),
                ],
            ),
            (
                1_488,
                [
                    mission_encounter_pose([912, -7_181, -11_744, 28, 211, 8, 63]),
                    mission_encounter_pose([-8_100, 7_181, 16_752, 228, 34, 4, 63]),
                ],
            ),
            (
                1_492,
                [
                    mission_encounter_pose([1_076, -6_525, -11_656, 32, 213, 8, 63]),
                    mission_encounter_pose([-8_240, 6_525, 16_872, 224, 35, 4, 63]),
                ],
            ),
            (
                1_496,
                [
                    mission_encounter_pose([1_220, -5_849, -11_568, 35, 215, 8, 63]),
                    mission_encounter_pose([-8_372, 5_849, 16_980, 221, 36, 4, 63]),
                ],
            ),
            (
                1_500,
                [
                    mission_encounter_pose([1_348, -5_157, -11_480, 38, 217, 8, 63]),
                    mission_encounter_pose([-8_496, 5_535, 17_076, 218, 37, 4, 63]),
                ],
            ),
            (
                1_504,
                [
                    mission_encounter_pose([1_460, -4_457, -11_392, 41, 219, 8, 63]),
                    mission_encounter_pose([-8_496, 5_157, 17_076, 218, 37, 4, 63]),
                ],
            ),
            (
                1_508,
                [
                    mission_encounter_pose([1_556, -4_249, -11_312, 41, 221, 8, 63]),
                    mission_encounter_pose([-8_612, 4_457, 17_160, 215, 38, 4, 63]),
                ],
            ),
            (
                1_512,
                [
                    mission_encounter_pose([1_556, -3_757, -11_312, 43, 221, 8, 63]),
                    mission_encounter_pose([-8_716, 3_757, 17_232, 213, 39, 4, 63]),
                ],
            ),
            (
                1_516,
                [
                    mission_encounter_pose([1_640, -3_065, -11_232, 45, 223, 8, 63]),
                    mission_encounter_pose([-8_812, 3_065, 17_296, 211, 40, 4, 63]),
                ],
            ),
            (
                1_520,
                [
                    mission_encounter_pose([1_712, -2_389, -11_156, 46, 225, 8, 63]),
                    mission_encounter_pose([-8_904, 2_389, 17_352, 210, 41, 4, 63]),
                ],
            ),
            (
                1_524,
                [
                    mission_encounter_pose([1_776, -1_733, -11_080, 47, 227, 8, 63]),
                    mission_encounter_pose([-8_992, 1_733, 17_404, 209, 42, 4, 63]),
                ],
            ),
            (
                1_528,
                [
                    mission_encounter_pose([1_836, -1_097, -11_004, 48, 229, 8, 63]),
                    mission_encounter_pose([-9_076, 1_097, 17_448, 208, 43, 4, 63]),
                ],
            ),
            (
                1_532,
                [
                    mission_encounter_pose([1_888, -493, -10_932, 49, 231, 8, 63]),
                    mission_encounter_pose([-9_156, 493, 17_488, 207, 44, 4, 63]),
                ],
            ),
            (
                1_536,
                [
                    mission_encounter_pose([1_932, 79, -10_860, 48, 233, 8, 63]),
                    mission_encounter_pose([-9_232, -79, 17_524, 208, 45, 4, 63]),
                ],
            ),
            (
                1_540,
                [
                    mission_encounter_pose([1_976, 607, -10_780, 47, 235, 8, 63]),
                    mission_encounter_pose([-9_316, -607, 17_560, 209, 46, 4, 63]),
                ],
            ),
            (
                1_544,
                [
                    mission_encounter_pose([2_020, 1_095, -10_696, 46, 237, 8, 63]),
                    mission_encounter_pose([-9_404, -1_095, 17_596, 210, 47, 4, 63]),
                ],
            ),
            (
                1_548,
                [
                    mission_encounter_pose([2_060, 1_535, -10_600, 45, 239, 8, 63]),
                    mission_encounter_pose([-9_500, -1_535, 17_632, 211, 48, 4, 63]),
                ],
            ),
            (
                1_552,
                [
                    mission_encounter_pose([2_096, 1_923, -10_500, 43, 241, 8, 63]),
                    mission_encounter_pose([-9_600, -1_923, 17_668, 213, 49, 4, 63]),
                ],
            ),
            (
                1_556,
                [
                    mission_encounter_pose([2_132, 2_259, -10_388, 41, 243, 8, 63]),
                    mission_encounter_pose([-9_712, -2_139, 17_704, 213, 50, 4, 63]),
                ],
            ),
            (
                1_560,
                [
                    mission_encounter_pose([2_164, 2_467, -10_264, 38, 245, 8, 63]),
                    mission_encounter_pose([-9_712, -2_259, 17_704, 215, 50, 4, 63]),
                ],
            ),
            (
                1_564,
                [
                    mission_encounter_pose([2_164, 2_539, -10_264, 38, 245, 8, 63]),
                    mission_encounter_pose([-9_832, -2_539, 17_740, 218, 51, 4, 63]),
                ],
            ),
            (
                1_568,
                [
                    mission_encounter_pose([2_192, 2_763, -10_124, 35, 247, 8, 63]),
                    mission_encounter_pose([-9_968, -2_763, 17_776, 221, 52, 4, 63]),
                ],
            ),
            (
                1_572,
                [
                    mission_encounter_pose([2_216, 2_927, -9_968, 31, 249, 8, 63]),
                    mission_encounter_pose([-10_120, -2_927, 17_816, 225, 53, 4, 63]),
                ],
            ),
            (
                1_576,
                [
                    mission_encounter_pose([2_232, 3_023, -9_792, 27, 251, 8, 63]),
                    mission_encounter_pose([-10_288, -3_023, 17_852, 229, 54, 4, 63]),
                ],
            ),
            (
                1_580,
                [
                    mission_encounter_pose([2_244, 3_055, -9_600, 23, 253, 8, 63]),
                    mission_encounter_pose([-10_472, -3_055, 17_892, 233, 55, 4, 63]),
                ],
            ),
            (
                1_584,
                [
                    mission_encounter_pose([2_244, 3_015, -9_396, 19, 255, 8, 63]),
                    mission_encounter_pose([-10_672, -3_015, 17_928, 237, 56, 4, 63]),
                ],
            ),
            (
                1_588,
                [
                    mission_encounter_pose([2_244, 2_911, -9_180, 14, 1, 8, 63]),
                    mission_encounter_pose([-10_884, -2_911, 17_960, 242, 57, 4, 63]),
                ],
            ),
        ];

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        assert_eq!(
            fighter_continuation::END_RETAIL_FRAME,
            MISSION_ENCOUNTER_CERTIFIED_END_RETAIL_FRAME,
            "the certified opening fighter window must remain fully native"
        );
        let mut all_matched = true;
        for (retail_frame, expected_poses) in
            CHECKPOINTS.into_iter().take_while(|(retail_frame, _)| {
                *retail_frame
                    <= u32::from(
                        fighter_continuation::START_RETAIL_FRAME
                            - FIGHTER_COOPERATIVE_SCHEDULE_STEP,
                    )
            })
        {
            while game.state().mode_frame
                < retail_frame / u32::from(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            {
                game.tick(Button::B as u16).unwrap();
            }
            if retail_frame == 584 {
                assert_eq!(game.state.random.bytes(), FIGHTER_FIRST_SHOT_RANDOM_STATE);
            } else if retail_frame == 728 {
                assert_eq!(
                    game.state.random.bytes(),
                    FIGHTER_SECOND_FIRE_APPROACH_RANDOM_STATE
                );
            } else if retail_frame == 732 {
                assert_eq!(
                    game.state.random.bytes(),
                    FIGHTER_SECOND_FIRE_CHECK_RANDOM_STATE
                );
            } else if retail_frame == 876 {
                assert_eq!(
                    game.state.random.bytes(),
                    FIGHTER_THIRD_FIRE_CHECK_RANDOM_STATE
                );
            } else if retail_frame == 1_336 {
                assert_eq!(game.state.random.bytes(), FIGHTER_WAVE_HANDOFF_RANDOM_STATE);
            } else if retail_frame == 1_480 {
                assert_eq!(
                    game.state.random.bytes(),
                    FIGHTER_LATE_FIRE_CHECK_RANDOM_STATE
                );
            } else if retail_frame == 1_588 {
                assert_eq!(game.state.random.bytes(), FIGHTER_CAPTURE_END_RANDOM_STATE);
            }
            for (actor, expected) in [
                MissionEncounterActor::UpperFighter,
                MissionEncounterActor::LowerFighter,
            ]
            .into_iter()
            .zip(expected_poses)
            {
                let id = game.mission_entry_flyby[actor.index()].unwrap();
                let object = game.state().objects.get(id).unwrap();
                let actual = mission_encounter_pose([
                    object.base.position.x,
                    object.base.position.y,
                    object.base.position.z,
                    i16::from(object.base.pitch.units()),
                    i16::from(object.base.yaw.units()),
                    i16::from(object.base.roll.units()),
                    i16::from(object.base.speed),
                ]);
                if actual != expected {
                    all_matched = false;
                    eprintln!(
                        "typed fighter {actor:?} diverged at retail frame {retail_frame}:\nactual:   {actual:?}\nexpected: {expected:?}\nactivity: {:?}",
                        object.extension.activity,
                    );
                }
            }
        }
        for expected_frame in opening_continuation::ENCOUNTER_KEYFRAMES
            .iter()
            .filter(|frame| {
                frame.retail_frame >= fighter_continuation::START_RETAIL_FRAME
                    && frame.retail_frame <= fighter_continuation::END_RETAIL_FRAME
            })
        {
            let retail_frame = u32::from(expected_frame.retail_frame);
            while game.state().mode_frame
                < retail_frame / u32::from(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            {
                game.tick(Button::B as u16).unwrap();
            }
            if retail_frame == 1_872 {
                assert_eq!(game.state.random.bytes(), SHARED_RANDOM_PARTIAL_STATE);
            } else if retail_frame == 1_876 {
                assert_eq!(game.state.random.bytes(), SHARED_RANDOM_COMPLETION_STATE);
            } else if retail_frame == 2_448 {
                assert_eq!(
                    game.state.random.bytes(),
                    CERTIFIED_FIGHTER_END_RANDOM_STATE
                );
            }
            for actor in [
                MissionEncounterActor::UpperFighter,
                MissionEncounterActor::LowerFighter,
            ] {
                let id = game.mission_entry_flyby[actor.index()].unwrap();
                let object = game.state().objects.get(id).unwrap();
                let actual = mission_encounter_pose([
                    object.base.position.x,
                    object.base.position.y,
                    object.base.position.z,
                    i16::from(object.base.pitch.units()),
                    i16::from(object.base.yaw.units()),
                    i16::from(object.base.roll.units()),
                    i16::from(object.base.speed),
                ]);
                let expected = expected_frame.poses[actor.index()];
                if actual != expected {
                    all_matched = false;
                    eprintln!(
                        "typed fighter {actor:?} diverged at retail frame {retail_frame}:\nactual:   {actual:?}\nexpected: {expected:?}\nactivity: {:?}",
                        object.extension.activity,
                    );
                }
            }
        }
        assert!(
            all_matched,
            "typed fighter dynamics diverged from the oracle"
        );
    }

    #[test]
    fn typed_capital_flight_matches_every_certified_oracle_frame() {
        assert_eq!(
            capital_continuation::END_RETAIL_FRAME,
            MISSION_ENCOUNTER_CERTIFIED_END_RETAIL_FRAME,
            "the certified opening capital-craft window must remain fully native"
        );

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        for expected_frame in opening_continuation::ENCOUNTER_KEYFRAMES
            .iter()
            .filter(|frame| {
                frame.retail_frame >= CAPITAL_FLIGHT_HANDOFF_RETAIL_FRAME
                    && frame.retail_frame <= capital_continuation::END_RETAIL_FRAME
            })
        {
            let retail_frame = u32::from(expected_frame.retail_frame);
            while game.state().mode_frame
                < retail_frame / u32::from(RETAIL_PRESENTATION_FRAMES_PER_TICK)
            {
                game.tick(Button::B as u16).unwrap();
            }

            for actor in [
                MissionEncounterActor::FirstCapital,
                MissionEncounterActor::SecondCapital,
            ] {
                let id = game.mission_entry_flyby[actor.index()].unwrap();
                let object = game.state().objects.get(id).unwrap();
                assert!(
                    matches!(object.extension.activity, ObjectActivity::CapitalFlight(_)),
                    "capital craft {actor:?} lacked typed flight state at retail frame {retail_frame}"
                );
                let actual = mission_encounter_pose([
                    object.base.position.x,
                    object.base.position.y,
                    object.base.position.z,
                    i16::from(object.base.pitch.units()),
                    i16::from(object.base.yaw.units()),
                    i16::from(object.base.roll.units()),
                    i16::from(object.base.speed),
                ]);
                assert_eq!(
                    actual,
                    expected_frame.poses[actor.index()],
                    "typed capital craft {actor:?} diverged at retail frame {retail_frame}"
                );
            }
        }
    }

    #[test]
    fn post_nine_hundred_opening_path_matches_the_extended_oracle() {
        const ORACLE_RETAIL_FRAME: u32 = 1_140;
        const ORACLE_KEYFRAME_INDEX: usize = 60;
        const ORACLE_PLAYER: MissionPlayerKeyframe =
            mission_player_keyframe(1_140, -4_577, -2_881, -2_603, 0, 227, 2, 30);
        const ORACLE_CAMERA: MissionCameraKeyframe =
            mission_camera_keyframe(1_140, -4_577, -2_905, -2_603, 0, 0, 0);
        const ORACLE_ENCOUNTER_POSES: [MissionEncounterPose; 4] = [
            mission_encounter_pose([-16_212, 419, -1_156, 231, 177, 252, 60]),
            mission_encounter_pose([-7_824, -94, -11_028, 240, 227, 4, 60]),
            mission_encounter_pose([-1_964, 11_697, -1_264, 27, 128, 248, 63]),
            mission_encounter_pose([-8_928, -11_743, 2_640, 225, 242, 252, 63]),
        ];

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        let oracle_tick = ORACLE_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK;
        while game.state().mode_frame < oracle_tick {
            game.tick(Button::B as u16).unwrap();
        }

        let expected_player = opening_continuation::PLAYER_KEYFRAMES[ORACLE_KEYFRAME_INDEX];
        assert_eq!(expected_player, ORACLE_PLAYER);
        let player = game
            .state()
            .mission
            .primary_player
            .and_then(|id| game.state().objects.get(id))
            .expect("opening sortie retains its player");
        assert_eq!(player.base.position, expected_player.position);
        assert_eq!(player.base.roll.units(), expected_player.roll);
        assert_eq!(player.base.speed, expected_player.speed);

        let expected_camera = opening_continuation::CAMERA_KEYFRAMES[ORACLE_KEYFRAME_INDEX];
        assert_eq!(expected_camera, ORACLE_CAMERA);
        assert_eq!(game.camera().position, expected_camera.position);
        assert_eq!(game.camera().rotation.pitch.units(), expected_camera.pitch);
        assert_eq!(game.camera().rotation.yaw.units(), expected_camera.yaw);
        assert!(!game.state().mission.departed_certified_neutral_path);

        let expected_encounter = opening_continuation::ENCOUNTER_KEYFRAMES[ORACLE_KEYFRAME_INDEX];
        for (index, (id, certified_pose)) in game
            .mission_entry_flyby
            .iter()
            .flatten()
            .copied()
            .zip(expected_encounter.poses)
            .enumerate()
        {
            assert_eq!(certified_pose, ORACLE_ENCOUNTER_POSES[index]);
            let expected = ORACLE_ENCOUNTER_POSES[index];
            let object = game.state().objects.get(id).unwrap();
            assert_eq!(object.base.pitch.units(), expected.pitch);
            assert_eq!(object.base.yaw.units(), expected.yaw);
            assert_eq!(object.base.roll.units(), expected.roll);
            assert_eq!(object.base.speed, expected.speed);
            assert_eq!(object.base.position, expected.position);
        }
    }

    #[test]
    fn encounter_enemy_lasers_follow_the_retail_typed_trajectories() {
        const FIRST_SHOT_FRAME: u32 = 588;
        const FIRST_SHOT_END_FRAME: u32 = 732;
        const CAPITAL_SHOT_FRAME: u32 = 876;
        const SECOND_CAPITAL_SHOT_FRAME: u32 = 880;
        const BASE_TRAJECTORY_END_FRAME: u32 = 900;

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        while game.state().mode_frame < FIRST_SHOT_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK {
            game.tick(Button::B as u16).unwrap();
        }

        let first = game
            .mission_projectiles
            .iter()
            .find(|projectile| {
                projectile.trajectory == MissionProjectileTrajectory::UpperFighterOpeningShotOne
            })
            .expect("first encounter fighter launched its laser");
        let first_object = game.state().objects.get(first.object).unwrap();
        assert_eq!(first_object.base.shape, ShapeId::ENEMY_LASER);
        assert_eq!(first_object.base.weapon, WeaponKind::EnemyLaser);
        assert_eq!(
            first_object.base.collision_class,
            CollisionClass::EnemyWeapon
        );
        assert_eq!(first_object.base.attack_power, ENEMY_LASER_ATTACK_POWER);
        assert_eq!(
            first_object.base.position,
            Vector3 {
                x: -361,
                y: -8_658,
                z: -2_750
            }
        );
        assert_eq!(first_object.base.pitch, Angle::from_units(37));
        assert_eq!(first_object.base.yaw, Angle::from_units(79));
        assert_eq!(first_object.base.speed, 63);
        assert_eq!(
            first_object.base.linked_object,
            game.mission_entry_flyby[MissionEncounterActor::UpperFighter.index()]
        );

        while game.state().mode_frame <= FIRST_SHOT_END_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(Button::B as u16).unwrap();
        }
        assert!(game.mission_projectiles.iter().all(|projectile| {
            projectile.trajectory != MissionProjectileTrajectory::UpperFighterOpeningShotOne
        }));

        while game.state().mode_frame < CAPITAL_SHOT_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK {
            game.tick(Button::B as u16).unwrap();
        }
        assert_eq!(game.mission_projectiles.len(), 1);
        assert_eq!(
            game.mission_projectiles[0].trajectory,
            MissionProjectileTrajectory::UpperFighterOpeningShotTwo
        );

        while game.state().mode_frame
            < SECOND_CAPITAL_SHOT_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(Button::B as u16).unwrap();
        }
        assert_eq!(game.mission_projectiles.len(), 2);
        for projectile in &game.mission_projectiles {
            let object = game.state().objects.get(projectile.object).unwrap();
            assert_eq!(object.base.behavior, Behavior::MissionScriptedProjectile);
            assert_eq!(object.base.velocity, Vector3::default());
        }

        while game.state().mode_frame
            <= BASE_TRAJECTORY_END_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(Button::B as u16).unwrap();
        }
        assert_eq!(game.mission_projectiles.len(), 2);
        assert!(game.mission_projectiles.iter().any(|projectile| {
            projectile.trajectory == MissionProjectileTrajectory::UpperFighterOpeningShotTwo
        }));
        assert!(game.mission_projectiles.iter().any(|projectile| {
            projectile.trajectory == MissionProjectileTrajectory::LowerFighterOpeningShot
        }));
    }

    #[test]
    fn later_enemy_lasers_match_the_extended_oracle() {
        const ORACLE_RETAIL_FRAME: u32 = 1140;
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        while game.state().mode_frame < ORACLE_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK {
            game.tick(Button::B as u16).unwrap();
        }

        let expected = [
            (
                MissionProjectileTrajectory::SecondCapitalOpeningShotOne,
                *opening_continuation::SECOND_CAPITAL_OPENING_SHOT_ONE_KEYFRAMES
                    .iter()
                    .find(|keyframe| keyframe.retail_frame == ORACLE_RETAIL_FRAME as u16)
                    .unwrap(),
            ),
            (
                MissionProjectileTrajectory::UpperFighterOpeningShotThree,
                opening_continuation::UPPER_FIGHTER_OPENING_SHOT_THREE_KEYFRAMES
                    .iter()
                    .find(|keyframe| keyframe.retail_frame == ORACLE_RETAIL_FRAME as u16)
                    .copied()
                    .unwrap(),
            ),
            (
                MissionProjectileTrajectory::SecondCapitalOpeningShotTwo,
                opening_continuation::SECOND_CAPITAL_OPENING_SHOT_TWO_KEYFRAMES
                    .iter()
                    .find(|keyframe| keyframe.retail_frame == ORACLE_RETAIL_FRAME as u16)
                    .copied()
                    .unwrap(),
            ),
        ];
        assert_eq!(game.mission_projectiles.len(), expected.len());

        for (trajectory, keyframe) in expected {
            let projectile = game
                .mission_projectiles
                .iter()
                .find(|projectile| projectile.trajectory == trajectory)
                .expect("retail projectile remains active at the oracle checkpoint");
            let object = game.state().objects.get(projectile.object).unwrap();
            assert_eq!(object.base.position, keyframe.pose.position);
            assert_eq!(object.base.pitch.units(), keyframe.pose.pitch);
            assert_eq!(object.base.yaw.units(), keyframe.pose.yaw);
            assert_eq!(object.base.roll.units(), keyframe.pose.roll);
            assert_eq!(object.base.speed, keyframe.pose.speed);
            assert_eq!(
                object.base.linked_object,
                game.mission_entry_flyby[trajectory.firing_actor().index()]
            );
        }
    }

    #[test]
    fn late_mission_enemy_laser_overlap_matches_the_oracle() {
        const ORACLE_RETAIL_FRAME: u32 = 5_232;
        const EXPECTED: [(MissionProjectileTrajectory, MissionEncounterPose); 3] = [
            (
                MissionProjectileTrajectory::SecondCapitalMissionShotSeventeen,
                mission_encounter_pose([14_213, -2_078, 23_779, 9, 12, 0, 63]),
            ),
            (
                MissionProjectileTrajectory::SecondCapitalMissionShotEighteen,
                mission_encounter_pose([15_532, -2_891, 20_531, 2, 15, 0, 63]),
            ),
            (
                MissionProjectileTrajectory::SecondCapitalMissionShotNineteen,
                mission_encounter_pose([21_682, -246, 12_349, 241, 26, 0, 63]),
            ),
        ];

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        while game.state().mode_frame < ORACLE_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK {
            game.tick(0).unwrap();
        }

        assert_eq!(game.mission_projectiles.len(), EXPECTED.len());
        let capital = game.mission_entry_flyby[MissionEncounterActor::SecondCapital.index()];
        for (trajectory, expected) in EXPECTED {
            let projectile = game
                .mission_projectiles
                .iter()
                .find(|projectile| projectile.trajectory == trajectory)
                .expect("late mission laser is active at the oracle checkpoint");
            let object = game.state().objects.get(projectile.object).unwrap();
            assert_eq!(object.base.position, expected.position);
            assert_eq!(object.base.pitch.units(), expected.pitch);
            assert_eq!(object.base.yaw.units(), expected.yaw);
            assert_eq!(object.base.roll.units(), expected.roll);
            assert_eq!(object.base.speed, expected.speed);
            assert_eq!(object.base.linked_object, capital);
        }
    }

    #[test]
    fn late_opening_path_and_final_capital_shot_match_the_oracle() {
        const ORACLE_RETAIL_FRAME: u32 = 2_448;
        const OPENING_KEYFRAME_INDEX: usize = 387;

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        while game.state().mode_frame < ORACLE_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK {
            game.tick(Button::B as u16).unwrap();
        }

        let expected_player = opening_continuation::PLAYER_KEYFRAMES[OPENING_KEYFRAME_INDEX];
        let player = game
            .state()
            .mission
            .primary_player
            .and_then(|id| game.state().objects.get(id))
            .expect("the certified late opening retains its player");
        assert_eq!(player.base.position, expected_player.position);
        assert_eq!(player.base.roll.units(), expected_player.roll);
        assert_eq!(
            game.camera().position,
            opening_continuation::CAMERA_KEYFRAMES[OPENING_KEYFRAME_INDEX].position
        );

        let expected_encounter = opening_continuation::ENCOUNTER_KEYFRAMES[OPENING_KEYFRAME_INDEX];
        for (id, expected) in game
            .mission_entry_flyby
            .iter()
            .flatten()
            .copied()
            .zip(expected_encounter.poses)
        {
            let object = game.state().objects.get(id).unwrap();
            assert_eq!(object.base.position, expected.position);
            assert_eq!(object.base.pitch.units(), expected.pitch);
            assert_eq!(object.base.yaw.units(), expected.yaw);
            assert_eq!(object.base.roll.units(), expected.roll);
        }

        assert_eq!(game.mission_projectiles.len(), 1);
        let projectile = game.mission_projectiles[0];
        assert_eq!(
            projectile.trajectory,
            MissionProjectileTrajectory::SecondCapitalOpeningShotFive
        );
        let object = game.state().objects.get(projectile.object).unwrap();
        let expected = opening_continuation::SECOND_CAPITAL_OPENING_SHOT_FIVE_KEYFRAMES[0];
        assert_eq!(object.base.position, expected.pose.position);
        assert_eq!(
            object.base.linked_object,
            game.mission_entry_flyby[MissionEncounterActor::SecondCapital.index()]
        );
        assert!(!game.state().mission.departed_certified_neutral_path);
    }

    #[test]
    fn neutral_first_sortie_matches_independent_late_mission_checkpoints() {
        const MID_MISSION_RETAIL_FRAME: u32 = 5_000;
        const CAPITAL_INACTIVE_RETAIL_FRAME: u32 = 5_124;
        const CAPITAL_RESUMED_RETAIL_FRAME: u32 = 5_128;
        const CAPITAL_LAST_RETAIL_FRAME: u32 = 7_872;
        const CAPITAL_DEPARTURE_RETAIL_FRAME: u32 = 7_876;
        const MID_MISSION_PLAYER: MissionPlayerKeyframe =
            mission_player_keyframe(5_000, 14_179, -2_881, 19_279, 0, 227, 252, 30);
        const MID_MISSION_CAMERA: MissionCameraKeyframe =
            mission_camera_keyframe(5_000, 14_179, -2_899, 19_279, 0, 0, 0);
        const MID_MISSION_CAPITAL: MissionEncounterPose =
            mission_encounter_pose([13_064, -7_184, 5_944, 0, 197, 13, 60]);
        const RESUMED_CAPITAL: MissionEncounterPose =
            mission_encounter_pose([18_884, -6_298, 11_424, 41, 214, 252, 60]);
        const LAST_CAPITAL: MissionEncounterPose =
            mission_encounter_pose([3_976, -578, -26_956, 0, 33, 245, 60]);

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        while game.state().mode_frame
            < MID_MISSION_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }

        let player = game
            .state()
            .mission
            .primary_player
            .and_then(|id| game.state().objects.get(id))
            .expect("the neutral sortie retains its player");
        assert_eq!(player.base.position, MID_MISSION_PLAYER.position);
        assert_eq!(player.base.pitch.units(), MID_MISSION_PLAYER.pitch);
        assert_eq!(player.base.yaw.units(), MID_MISSION_PLAYER.yaw);
        assert_eq!(player.base.roll.units(), MID_MISSION_PLAYER.roll);
        assert_eq!(player.base.speed, MID_MISSION_PLAYER.speed);
        assert_eq!(game.camera().position, MID_MISSION_CAMERA.position);
        assert_eq!(
            game.camera().rotation.pitch.units(),
            MID_MISSION_CAMERA.pitch
        );
        assert_eq!(game.camera().rotation.yaw.units(), MID_MISSION_CAMERA.yaw);
        assert_eq!(game.camera().rotation.roll.units(), MID_MISSION_CAMERA.roll);
        assert_eq!(
            game.mission_entry_flyby
                .iter()
                .filter(|actor| actor.is_some())
                .count(),
            1
        );
        let capital_id = game.mission_entry_flyby[MissionEncounterActor::SecondCapital.index()]
            .expect("the second capital craft remains in the mission");
        let capital = game.state().objects.get(capital_id).unwrap();
        assert_eq!(capital.base.position, MID_MISSION_CAPITAL.position);
        assert_eq!(capital.base.pitch.units(), MID_MISSION_CAPITAL.pitch);
        assert_eq!(capital.base.yaw.units(), MID_MISSION_CAPITAL.yaw);
        assert_eq!(capital.base.roll.units(), MID_MISSION_CAPITAL.roll);

        while game.state().mode_frame
            < CAPITAL_INACTIVE_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        let capital = game.state().objects.get(capital_id).unwrap();
        assert!(!capital.base.flags.active);
        assert!(!capital.base.flags.visible);
        assert!(capital.base.flags.collision_disabled);

        while game.state().mode_frame
            < CAPITAL_RESUMED_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        let capital = game.state().objects.get(capital_id).unwrap();
        assert!(capital.base.flags.active);
        assert!(capital.base.flags.visible);
        assert!(!capital.base.flags.collision_disabled);
        assert_eq!(capital.base.position, RESUMED_CAPITAL.position);
        assert_eq!(capital.base.pitch.units(), RESUMED_CAPITAL.pitch);
        assert_eq!(capital.base.yaw.units(), RESUMED_CAPITAL.yaw);
        assert_eq!(capital.base.roll.units(), RESUMED_CAPITAL.roll);

        while game.state().mode_frame
            < CAPITAL_LAST_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        let capital = game.state().objects.get(capital_id).unwrap();
        assert_eq!(capital.base.position, LAST_CAPITAL.position);
        assert_eq!(capital.base.pitch.units(), LAST_CAPITAL.pitch);
        assert_eq!(capital.base.yaw.units(), LAST_CAPITAL.yaw);
        assert_eq!(capital.base.roll.units(), LAST_CAPITAL.roll);

        while game.state().mode_frame
            < CAPITAL_DEPARTURE_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        assert!(game.mission_entry_flyby[MissionEncounterActor::SecondCapital.index()].is_none());
        assert!(game.state().objects.get(capital_id).is_none());
        assert!(!game.state().mission.departed_certified_neutral_path);
    }

    #[test]
    fn mission_timer_matches_the_retail_fractional_schedule() {
        const CHECKPOINTS: [(u32, u16); 7] = [
            (328, 0),
            (900, 5),
            (1_588, 11),
            (5_000, 51),
            (5_588, 58),
            (7_588, 85),
            (7_844, 88),
        ];

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        for (retail_frame, expected_tenths) in CHECKPOINTS {
            while game.state().mode_frame < retail_frame / RETAIL_PRESENTATION_FRAMES_PER_TICK {
                game.tick(0).unwrap();
            }
            assert_eq!(game.state().mission.elapsed_time_tenths, expected_tenths);
        }

        while game.state().mode_frame
            < (OPENING_SORTIE_STRATEGIC_MAP_RETURN_RETAIL_FRAME as u32
                - RETAIL_PRESENTATION_FRAMES_PER_TICK)
                / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        assert_eq!(game.state().mission.elapsed_time_tenths, 88);
    }

    #[test]
    fn opening_sortie_returns_to_the_strategic_map_on_the_retail_schedule() {
        const RETURN_TRIGGER_TICK: u32 =
            OPENING_SORTIE_RETURN_TRIGGER_RETAIL_FRAME as u32 / RETAIL_PRESENTATION_FRAMES_PER_TICK;
        const STRATEGIC_MAP_RETURN_TICK: u32 = OPENING_SORTIE_STRATEGIC_MAP_RETURN_RETAIL_FRAME
            as u32
            / RETAIL_PRESENTATION_FRAMES_PER_TICK;

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        while game.state().mode_frame + 1 < RETURN_TRIGGER_TICK {
            game.tick(0).unwrap();
        }
        assert_eq!(game.mode(), GameMode::Mission);
        assert_eq!(game.state().mission.phase, MissionPhase::Active);

        game.tick(Button::B as u16).unwrap();
        assert_eq!(game.state().mode_frame, RETURN_TRIGGER_TICK);
        assert_eq!(
            game.state().mission.phase,
            MissionPhase::ReturningToStrategicMap
        );
        assert!(game
            .state()
            .objects
            .active_objects()
            .all(|(_, object)| object.base.behavior != Behavior::Projectile));

        while game.state().mode_frame + 1 < STRATEGIC_MAP_RETURN_TICK {
            game.tick(0).unwrap();
        }
        assert_eq!(game.mode(), GameMode::Mission);
        game.tick(0).unwrap();

        assert_eq!(game.mode(), GameMode::StrategicMap);
        assert_eq!(game.state().mode_frame, 0);
        assert_eq!(
            game.state().strategic_map.phase,
            StrategicMapPhase::Planning
        );
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::Reengagement
        );
        assert_eq!(
            game.state().strategic_map.actors,
            OPENING_ASSAULT_MAP_ACTORS
        );
        assert!(!game.state().mission.active);
        assert_eq!(game.state().mission.mission, None);
        assert!(game.mission_entry_flyby.iter().all(Option::is_none));
        assert!(game.mission_projectiles.is_empty());
        assert_eq!(game.state().objects.active_objects().count(), 2);
        for player in [
            game.state().mission.primary_player,
            game.state().mission.wingmate,
        ]
        .into_iter()
        .flatten()
        {
            let object = game.state().objects.get(player).unwrap();
            assert!(!object.base.flags.visible);
            assert!(object.base.flags.collision_disabled);
        }
        let primary = game
            .state()
            .objects
            .get(game.state().mission.primary_player.unwrap())
            .unwrap();
        let wingmate = game
            .state()
            .objects
            .get(game.state().mission.wingmate.unwrap())
            .unwrap();
        assert_eq!(primary.base.hit_points, FIRST_RETURN_PRIMARY_SHIELD);
        assert_eq!(wingmate.base.hit_points, FIRST_RETURN_WINGMATE_SHIELD);
    }

    #[test]
    fn first_reengagement_reuses_the_campaign_craft_and_matches_the_oracle_path() {
        const CHECKPOINTS: [u16; 6] = [0, 320, 588, 688, 1_648, 6_316];

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        let primary_id = game.state().mission.primary_player.unwrap();
        let wingmate_id = game.state().mission.wingmate.unwrap();
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::Reengagement
        );

        press(&mut game, Button::Up);
        press(&mut game, Button::B);
        assert_eq!(
            game.state().strategic_map.phase,
            StrategicMapPhase::Traveling
        );
        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }

        assert_eq!(game.state().mission.visit, MissionVisit::Reengagement);
        assert_eq!(game.state().mission.primary_player, Some(primary_id));
        assert_eq!(game.state().mission.wingmate, Some(wingmate_id));
        assert_eq!(game.state().objects.active_objects().count(), 6);
        assert!(game.mission_entry_flyby.iter().all(Option::is_some));

        for retail_frame in CHECKPOINTS {
            while game.state().mode_frame
                < u32::from(retail_frame) / RETAIL_PRESENTATION_FRAMES_PER_TICK
            {
                game.tick(0).unwrap();
            }
            let index = usize::from(retail_frame / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
            let expected_player = second_sortie::PLAYER_KEYFRAMES[index];
            let expected_wingmate = second_sortie::WINGMATE_KEYFRAMES[index];
            let expected_camera = second_sortie::CAMERA_KEYFRAMES[index];
            let player = game.state().objects.get(primary_id).unwrap();
            let wingmate = game.state().objects.get(wingmate_id).unwrap();
            assert_eq!(player.base.position, expected_player.position);
            assert_eq!(player.base.yaw.units(), expected_player.yaw);
            assert_eq!(player.base.speed, expected_player.speed);
            assert_eq!(wingmate.base.position, expected_wingmate.position);
            assert_eq!(wingmate.base.yaw.units(), expected_wingmate.yaw);
            assert_eq!(wingmate.base.shape, ShapeId::EMPTY);
            assert!(!wingmate.base.flags.visible);
            assert_eq!(
                player.base.flags.visible,
                (MISSION_ACTIVE_RETAIL_FRAMES as u16..MISSION_PLAYER_CRAFT_HIDE_RETAIL_FRAME)
                    .contains(&retail_frame)
            );
            assert_eq!(game.state().camera.position, expected_camera.position);
            assert_eq!(
                game.state().camera.rotation.yaw.units(),
                expected_camera.yaw
            );

            let expected_target_count = match retail_frame {
                0 => 0,
                320 => 4,
                588 => 3,
                688 => 3,
                1_648 => 2,
                6_316 => 0,
                _ => unreachable!(),
            };
            let active_target_count = game
                .mission_entry_flyby
                .iter()
                .flatten()
                .filter_map(|id| game.state().objects.get(*id))
                .filter(|object| object.base.flags.active)
                .count();
            assert_eq!(active_target_count, expected_target_count);
            if retail_frame == 320 {
                let tracks: [&[MissionActorKeyframe]; MISSION_ENCOUNTER_ACTOR_COUNT] = [
                    &second_sortie::FIRST_CAPITAL_KEYFRAMES,
                    &second_sortie::SECOND_CAPITAL_KEYFRAMES,
                    &second_sortie::UPPER_FIGHTER_KEYFRAMES,
                    &second_sortie::LOWER_FIGHTER_KEYFRAMES,
                ];
                for (target, keyframes) in game.mission_entry_flyby.iter().zip(tracks) {
                    let object = game.state().objects.get(target.unwrap()).unwrap();
                    let expected = keyframes
                        .iter()
                        .find(|keyframe| keyframe.retail_frame == retail_frame)
                        .unwrap();
                    let MissionActorPresentation::Present(pose) = expected.presentation else {
                        panic!("re-engagement target is not present at retail frame 320");
                    };
                    assert_eq!(object.base.position, pose.position);
                    assert_eq!(object.base.pitch.units(), pose.pitch);
                    assert_eq!(object.base.yaw.units(), pose.yaw);
                    assert_eq!(object.base.roll.units(), pose.roll);
                    assert_eq!(object.base.speed, pose.speed);
                    assert_eq!(object.base.hit_points, MISSION_ENCOUNTER_HEALTH);
                    assert_eq!(object.base.attack_power, MISSION_ENCOUNTER_ATTACK_POWER);
                    assert_eq!(object.base.collision_class, CollisionClass::Enemy);
                }
            }
            if retail_frame == 588 {
                assert_eq!(game.reengagement_projectiles.len(), 1);
                let active_projectile = game.reengagement_projectiles[0];
                assert_eq!(active_projectile.track_index, 0);
                let projectile = game.state().objects.get(active_projectile.object).unwrap();
                let expected = second_sortie::ENEMY_LASER_KEYFRAME_TRACKS[0][0];
                assert_eq!(expected.retail_frame, retail_frame);
                assert_eq!(projectile.base.position, expected.pose.position);
                assert_eq!(projectile.base.pitch.units(), expected.pose.pitch);
                assert_eq!(projectile.base.yaw.units(), expected.pose.yaw);
                assert_eq!(projectile.base.roll.units(), expected.pose.roll);
                assert_eq!(projectile.base.speed, expected.pose.speed);
                assert_eq!(projectile.base.hit_points, SF2_HOSTILE_LASER_HEALTH);
                assert_eq!(projectile.base.attack_power, SF2_HOSTILE_LASER_ATTACK_POWER);
                assert_eq!(projectile.base.collision_class, CollisionClass::EnemyWeapon);
            }
        }

        game.tick(0).unwrap();
        assert_eq!(game.mode(), GameMode::StrategicMap);
        assert_eq!(
            game.state().strategic_map.phase,
            StrategicMapPhase::Planning
        );
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::MissileInterception
        );
        assert_eq!(
            game.state().strategic_map.actors,
            ESCALATED_ASSAULT_MAP_ACTORS
        );
        assert_eq!(
            game.state().campaign.elapsed_frames,
            SECOND_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND
        );
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            SECOND_RETURN_CORNERIA_DAMAGE_PERCENT
        );
        assert_eq!(
            game.state().mission.score,
            u32::from(MISSION_ENCOUNTER_HEALTH)
        );
        assert_eq!(game.state().mission.objects_destroyed, 1);
        assert_eq!(
            game.state().strategic_map.player_map_position,
            INITIAL_PLAYER_MAP_POSITION
        );
        assert_eq!(
            game.state()
                .objects
                .get(primary_id)
                .unwrap()
                .base
                .hit_points,
            SECOND_RETURN_PRIMARY_SHIELD
        );
        assert_eq!(
            game.state()
                .objects
                .get(wingmate_id)
                .unwrap()
                .base
                .hit_points,
            SECOND_RETURN_WINGMATE_SHIELD
        );
    }

    #[test]
    fn typed_second_sortie_capital_flight_matches_every_oracle_boundary() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }
        press(&mut game, Button::Up);
        press(&mut game, Button::B);
        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }
        assert_eq!(game.state().mission.visit, MissionVisit::Reengagement);

        for (index, first) in second_sortie::FIRST_CAPITAL_KEYFRAMES.iter().enumerate() {
            let retail_frame = first.retail_frame;
            while game.state().mode_frame
                < u32::from(retail_frame) / RETAIL_PRESENTATION_FRAMES_PER_TICK
            {
                game.tick(0).unwrap();
            }
            assert_eq!(
                index,
                usize::from(
                    retail_frame.saturating_sub(second_sortie_capital::INITIAL_RETAIL_FRAME)
                        / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16
                )
            );
            assert_mission_actor_presentation(
                &game,
                MissionEncounterActor::FirstCapital,
                first.presentation,
                retail_frame,
            );

            if retail_frame <= second_sortie_capital::SECOND_DEPARTURE_RETAIL_FRAME {
                let second_index = usize::from(
                    retail_frame.saturating_sub(second_sortie_capital::INITIAL_RETAIL_FRAME)
                        / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
                );
                let second = second_sortie::SECOND_CAPITAL_KEYFRAMES[second_index];
                assert_eq!(second.retail_frame, retail_frame);
                assert_mission_actor_presentation(
                    &game,
                    MissionEncounterActor::SecondCapital,
                    second.presentation,
                    retail_frame,
                );
            }
        }
    }

    #[test]
    fn typed_second_sortie_fighter_flight_matches_every_oracle_boundary() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }
        press(&mut game, Button::Up);
        press(&mut game, Button::B);
        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }
        assert_eq!(game.state().mission.visit, MissionVisit::Reengagement);

        for (index, upper) in second_sortie::UPPER_FIGHTER_KEYFRAMES.iter().enumerate() {
            let retail_frame = upper.retail_frame;
            while game.state().mode_frame
                < u32::from(retail_frame) / RETAIL_PRESENTATION_FRAMES_PER_TICK
            {
                game.tick(0).unwrap();
            }
            assert_eq!(
                index,
                usize::from(
                    retail_frame.saturating_sub(second_sortie_fighters::INITIAL_RETAIL_FRAME)
                        / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16
                )
            );
            assert_mission_actor_presentation(
                &game,
                MissionEncounterActor::UpperFighter,
                upper.presentation,
                retail_frame,
            );

            if retail_frame <= second_sortie_fighters::LOWER_DEPARTURE_RETAIL_FRAME {
                let lower_index = usize::from(
                    retail_frame.saturating_sub(second_sortie_fighters::INITIAL_RETAIL_FRAME)
                        / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
                );
                let lower = second_sortie::LOWER_FIGHTER_KEYFRAMES[lower_index];
                assert_eq!(lower.retail_frame, retail_frame);
                assert_mission_actor_presentation(
                    &game,
                    MissionEncounterActor::LowerFighter,
                    lower.presentation,
                    retail_frame,
                );
            }
        }
    }

    #[test]
    fn typed_second_sortie_projectiles_match_every_oracle_boundary() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }
        press(&mut game, Button::Up);
        press(&mut game, Button::B);
        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }
        assert_eq!(game.state().mission.visit, MissionVisit::Reengagement);

        let last_retail_frame = second_sortie::ENEMY_LASER_KEYFRAME_TRACKS
            .iter()
            .filter_map(|track| track.last())
            .map(|keyframe| keyframe.retail_frame)
            .max()
            .expect("projectile oracle contains retained poses");
        for retail_frame in
            (0..=last_retail_frame).step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
        {
            while game.state().mode_frame
                < u32::from(retail_frame) / RETAIL_PRESENTATION_FRAMES_PER_TICK
            {
                game.tick(0).unwrap();
            }

            let expected_active = second_sortie::ENEMY_LASER_KEYFRAME_TRACKS
                .iter()
                .filter(|track| {
                    track
                        .first()
                        .zip(track.last())
                        .is_some_and(|(first, last)| {
                            (first.retail_frame..=last.retail_frame).contains(&retail_frame)
                        })
                })
                .count();
            assert_eq!(
                game.reengagement_projectiles.len(),
                expected_active,
                "active projectile count at frame {retail_frame}"
            );

            for (track_index, keyframes) in second_sortie::ENEMY_LASER_KEYFRAME_TRACKS
                .iter()
                .copied()
                .enumerate()
            {
                let Some(first) = keyframes.first() else {
                    continue;
                };
                let Some(last) = keyframes.last() else {
                    continue;
                };
                if !(first.retail_frame..=last.retail_frame).contains(&retail_frame) {
                    continue;
                }
                let keyframe_index = usize::from(
                    (retail_frame - first.retail_frame)
                        / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
                );
                let expected = keyframes[keyframe_index];
                assert_eq!(expected.retail_frame, retail_frame);
                let active = game
                    .reengagement_projectiles
                    .iter()
                    .find(|projectile| projectile.track_index == track_index)
                    .unwrap_or_else(|| {
                        panic!("projectile track {track_index} absent at frame {retail_frame}")
                    });
                let projectile = game.state().objects.get(active.object).unwrap();
                assert_eq!(
                    projectile.base.position, expected.pose.position,
                    "projectile track {track_index} at frame {retail_frame}"
                );
                assert_eq!(projectile.base.pitch.units(), expected.pose.pitch);
                assert_eq!(projectile.base.yaw.units(), expected.pose.yaw);
                assert_eq!(projectile.base.roll.units(), expected.pose.roll);
                assert_eq!(projectile.base.speed, expected.pose.speed);
                assert_eq!(projectile.base.behavior, Behavior::Projectile);

                let mut expected_flight = HostileProjectileFlightState {
                    phase: HostileProjectileFlightPhase::Homing,
                    motion_steps_elapsed: 0,
                    movement_phase: HostileProjectileMovementPhase::Ready,
                };
                let first_action_frame = first
                    .retail_frame
                    .saturating_add(RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
                for action_frame in (first_action_frame..=retail_frame)
                    .step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
                {
                    for action in second_sortie_projectiles::actions(track_index, action_frame) {
                        let phase = match action {
                            HostileProjectileAction::AdvanceHoming => {
                                Some(HostileProjectileFlightPhase::Homing)
                            }
                            HostileProjectileAction::AdvanceAimCorrection => {
                                Some(HostileProjectileFlightPhase::AimCorrection)
                            }
                            HostileProjectileAction::AdvanceCruise => {
                                Some(HostileProjectileFlightPhase::Cruise)
                            }
                            _ => None,
                        };
                        if let Some(phase) = phase {
                            expected_flight.phase = phase;
                            expected_flight.motion_steps_elapsed += 1;
                        }
                    }
                }
                assert_eq!(
                    projectile.extension.activity,
                    ObjectActivity::HostileProjectileFlight(expected_flight),
                    "projectile track {track_index} state at frame {retail_frame}"
                );
            }
        }
    }

    #[test]
    fn typed_fighter_intercept_flight_matches_every_oracle_boundary() {
        const RETAINED_FIGHTER_POSE_COUNT: usize = 1_503;

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.campaign.route_step = CampaignRouteStep::FighterIntercept;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.begin_fighter_intercept_sortie().unwrap();

        let tracks: [(FighterInterceptActor, &[MissionActorKeyframe]);
            FIGHTER_INTERCEPT_TARGET_COUNT] = [
            (
                FighterInterceptActor::Lead,
                &fighter_intercept::LEAD_FIGHTER_KEYFRAMES,
            ),
            (
                FighterInterceptActor::Flank,
                &fighter_intercept::FLANK_FIGHTER_KEYFRAMES,
            ),
            (
                FighterInterceptActor::Rear,
                &fighter_intercept::REAR_FIGHTER_KEYFRAMES,
            ),
        ];
        let last_retail_frame = tracks
            .iter()
            .filter_map(|(_, keyframes)| keyframes.last())
            .map(|keyframe| keyframe.retail_frame)
            .max()
            .expect("fighter oracle contains retained poses");
        let mut retained_poses = 0;

        for retail_frame in
            (0..=last_retail_frame).step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
        {
            while game.state().mode_frame
                < u32::from(retail_frame) / RETAIL_PRESENTATION_FRAMES_PER_TICK
            {
                game.tick(0).unwrap();
            }

            for &(actor, keyframes) in &tracks {
                let keyframe_index =
                    usize::from(retail_frame / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
                let Some(expected) = keyframes.get(keyframe_index) else {
                    continue;
                };
                assert_eq!(expected.retail_frame, retail_frame);
                let actor_id = match actor {
                    FighterInterceptActor::Lead => game.fighter_intercept_actors.lead,
                    FighterInterceptActor::Flank => game.fighter_intercept_actors.flank,
                    FighterInterceptActor::Rear => game.fighter_intercept_actors.rear,
                };
                match expected.presentation {
                    MissionActorPresentation::Present(pose) => {
                        retained_poses += 1;
                        let object = game
                            .state()
                            .objects
                            .get(actor_id.unwrap_or_else(|| {
                                panic!("fighter departed before retail frame {retail_frame}")
                            }))
                            .unwrap();
                        assert_eq!(object.base.position, pose.position, "frame {retail_frame}");
                        assert_eq!(
                            object.base.pitch.units(),
                            pose.pitch,
                            "frame {retail_frame}"
                        );
                        assert_eq!(object.base.yaw.units(), pose.yaw, "frame {retail_frame}");
                        assert_eq!(object.base.roll.units(), pose.roll, "frame {retail_frame}");
                        assert_eq!(object.base.speed, pose.speed, "frame {retail_frame}");
                        assert!(object.base.flags.active, "frame {retail_frame}");
                        assert!(object.base.flags.visible, "frame {retail_frame}");
                        assert!(
                            !object.base.flags.collision_disabled,
                            "frame {retail_frame}"
                        );
                        assert!(matches!(
                            object.extension.activity,
                            ObjectActivity::FighterInterceptFlight(_)
                        ));
                    }
                    MissionActorPresentation::Inactive => {
                        let object = game.state().objects.get(actor_id.unwrap()).unwrap();
                        assert!(!object.base.flags.active, "frame {retail_frame}");
                        assert!(!object.base.flags.visible, "frame {retail_frame}");
                        assert!(object.base.flags.collision_disabled, "frame {retail_frame}");
                        assert!(matches!(
                            object.extension.activity,
                            ObjectActivity::FighterInterceptFlight(_)
                        ));
                    }
                    MissionActorPresentation::Departed => {
                        assert!(
                            actor_id.is_none(),
                            "fighter remains allocated at retail frame {retail_frame}"
                        );
                    }
                }
            }
        }

        assert_eq!(retained_poses, RETAINED_FIGHTER_POSE_COUNT);
    }

    #[test]
    fn typed_fighter_intercept_projectiles_match_every_oracle_boundary() {
        let mut game = Game::new();
        let last_retail_frame = fighter_intercept::ENEMY_LASER_KEYFRAME_TRACKS
            .iter()
            .filter_map(|track| track.last())
            .map(|keyframe| keyframe.retail_frame)
            .max()
            .expect("projectile oracle contains retained poses");

        for retail_frame in
            (0..=last_retail_frame).step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
        {
            let player_index =
                usize::from(retail_frame / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
            let previous_player_index = usize::from(
                retail_frame.saturating_sub(RETAIL_PRESENTATION_FRAMES_PER_TICK as u16)
                    / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
            );
            let player_position = fighter_intercept::PLAYER_KEYFRAMES[player_index].position;
            let previous_player_position =
                fighter_intercept::PLAYER_KEYFRAMES[previous_player_index].position;
            game.update_fighter_intercept_projectiles(
                retail_frame,
                player_position,
                previous_player_position,
            )
            .unwrap();

            let expected_active = fighter_intercept::ENEMY_LASER_KEYFRAME_TRACKS
                .iter()
                .filter(|track| {
                    track
                        .first()
                        .zip(track.last())
                        .is_some_and(|(first, last)| {
                            (first.retail_frame..=last.retail_frame).contains(&retail_frame)
                        })
                })
                .count();
            assert_eq!(
                game.fighter_intercept_projectiles.len(),
                expected_active,
                "active projectile count at frame {retail_frame}"
            );

            for (track_index, keyframes) in fighter_intercept::ENEMY_LASER_KEYFRAME_TRACKS
                .iter()
                .copied()
                .enumerate()
            {
                let Some(first) = keyframes.first() else {
                    continue;
                };
                let Some(last) = keyframes.last() else {
                    continue;
                };
                if !(first.retail_frame..=last.retail_frame).contains(&retail_frame) {
                    continue;
                }
                let keyframe_index = usize::from(
                    (retail_frame - first.retail_frame)
                        / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
                );
                let expected = keyframes[keyframe_index];
                assert_eq!(expected.retail_frame, retail_frame);
                let active = game
                    .fighter_intercept_projectiles
                    .iter()
                    .find(|projectile| projectile.track_index == track_index)
                    .unwrap_or_else(|| {
                        panic!("projectile track {track_index} absent at frame {retail_frame}")
                    });
                let projectile = game.state().objects.get(active.object).unwrap();
                assert_eq!(
                    projectile.base.position, expected.pose.position,
                    "projectile track {track_index} at frame {retail_frame}"
                );
                assert_eq!(projectile.base.pitch.units(), expected.pose.pitch);
                assert_eq!(projectile.base.yaw.units(), expected.pose.yaw);
                assert_eq!(projectile.base.roll.units(), expected.pose.roll);
                assert_eq!(projectile.base.speed, expected.pose.speed);
                assert_eq!(projectile.base.behavior, Behavior::Projectile);

                let mut expected_flight = HostileProjectileFlightState {
                    phase: HostileProjectileFlightPhase::Homing,
                    motion_steps_elapsed: 0,
                    movement_phase: HostileProjectileMovementPhase::Ready,
                };
                let first_action_frame = first
                    .retail_frame
                    .saturating_add(RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
                for action_frame in (first_action_frame..=retail_frame)
                    .step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
                {
                    for action in fighter_intercept_projectiles::actions(track_index, action_frame)
                    {
                        let phase = match action {
                            HostileProjectileAction::AdvanceHoming => {
                                Some(HostileProjectileFlightPhase::Homing)
                            }
                            HostileProjectileAction::AdvanceAimCorrection => {
                                Some(HostileProjectileFlightPhase::AimCorrection)
                            }
                            HostileProjectileAction::AdvanceCruise => {
                                Some(HostileProjectileFlightPhase::Cruise)
                            }
                            _ => None,
                        };
                        if let Some(phase) = phase {
                            expected_flight.phase = phase;
                            expected_flight.motion_steps_elapsed += 1;
                        }
                    }
                }
                assert_eq!(
                    projectile.extension.activity,
                    ObjectActivity::HostileProjectileFlight(expected_flight),
                    "projectile track {track_index} state at frame {retail_frame}"
                );
            }
        }

        let cleanup_frame = last_retail_frame + RETAIL_PRESENTATION_FRAMES_PER_TICK as u16;
        let player_index = usize::from(cleanup_frame / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
        game.update_fighter_intercept_projectiles(
            cleanup_frame,
            fighter_intercept::PLAYER_KEYFRAMES[player_index].position,
            fighter_intercept::PLAYER_KEYFRAMES[player_index - 1].position,
        )
        .unwrap();
        assert!(game.fighter_intercept_projectiles.is_empty());
    }

    #[test]
    fn typed_pigma_projectiles_match_every_oracle_boundary() {
        const RETAINED_PROJECTILE_POSE_COUNT: usize = 120;

        let mut game = Game::new();
        let last_retail_frame = pigma_duel::ENEMY_LASER_KEYFRAME_TRACKS
            .iter()
            .filter_map(|track| track.last())
            .map(|keyframe| keyframe.retail_frame)
            .max()
            .expect("Pigma projectile oracle contains retained poses");
        let mut retained_poses = 0;

        for retail_frame in
            (0..=last_retail_frame).step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
        {
            let player_index =
                usize::from(retail_frame / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
            let previous_player_index = usize::from(
                retail_frame.saturating_sub(RETAIL_PRESENTATION_FRAMES_PER_TICK as u16)
                    / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
            );
            let player_position = pigma_duel::PLAYER_KEYFRAMES[player_index].position;
            let previous_player_position =
                pigma_duel::PLAYER_KEYFRAMES[previous_player_index].position;
            game.update_pigma_projectiles(retail_frame, player_position, previous_player_position)
                .unwrap();

            let expected_active = pigma_duel::ENEMY_LASER_KEYFRAME_TRACKS
                .iter()
                .filter(|track| {
                    track
                        .first()
                        .zip(track.last())
                        .is_some_and(|(first, last)| {
                            (first.retail_frame..=last.retail_frame).contains(&retail_frame)
                        })
                })
                .count();
            assert_eq!(
                game.pigma_projectiles.len(),
                expected_active,
                "active Pigma projectile count at frame {retail_frame}"
            );

            for (track_index, keyframes) in pigma_duel::ENEMY_LASER_KEYFRAME_TRACKS
                .iter()
                .copied()
                .enumerate()
            {
                let Some(first) = keyframes.first() else {
                    continue;
                };
                let Some(last) = keyframes.last() else {
                    continue;
                };
                if !(first.retail_frame..=last.retail_frame).contains(&retail_frame) {
                    continue;
                }
                let keyframe_index = usize::from(
                    (retail_frame - first.retail_frame)
                        / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
                );
                let expected = keyframes[keyframe_index];
                assert_eq!(expected.retail_frame, retail_frame);
                retained_poses += 1;

                let active = game
                    .pigma_projectiles
                    .iter()
                    .find(|projectile| projectile.track_index == track_index)
                    .unwrap_or_else(|| {
                        panic!(
                            "Pigma projectile track {track_index} absent at frame {retail_frame}"
                        )
                    });
                let projectile = game.state().objects.get(active.object).unwrap();
                assert_eq!(
                    projectile.base.position, expected.pose.position,
                    "Pigma projectile track {track_index} at frame {retail_frame}"
                );
                assert_eq!(projectile.base.pitch.units(), expected.pose.pitch);
                assert_eq!(projectile.base.yaw.units(), expected.pose.yaw);
                assert_eq!(projectile.base.roll.units(), expected.pose.roll);
                assert_eq!(projectile.base.speed, expected.pose.speed);
                assert_eq!(projectile.base.behavior, Behavior::Projectile);

                let descriptor = pigma_duel_projectiles::descriptor(track_index).unwrap();
                let mut expected_object = Object::new(
                    ObjectKind::Projectile,
                    ShapeId::ENEMY_LASER,
                    Behavior::Projectile,
                );
                expected_object.base.position = descriptor.initial_pose.position;
                expected_object.base.pitch = Angle::from_units(descriptor.initial_pose.pitch);
                expected_object.base.yaw = Angle::from_units(descriptor.initial_pose.yaw);
                expected_object.base.roll = Angle::from_units(descriptor.initial_pose.roll);
                expected_object.base.speed = descriptor.initial_pose.speed;
                let mut expected_flight = HostileProjectileFlightState {
                    phase: HostileProjectileFlightPhase::Homing,
                    motion_steps_elapsed: 0,
                    movement_phase: HostileProjectileMovementPhase::Ready,
                };
                let first_action_frame = first
                    .retail_frame
                    .saturating_add(RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
                for action_frame in (first_action_frame..=retail_frame)
                    .step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
                {
                    let action_player_index =
                        usize::from(action_frame / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
                    let action_previous_player_index = usize::from(
                        action_frame.saturating_sub(RETAIL_PRESENTATION_FRAMES_PER_TICK as u16)
                            / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
                    );
                    let action_player_position =
                        pigma_duel::PLAYER_KEYFRAMES[action_player_index].position;
                    let action_previous_player_position =
                        pigma_duel::PLAYER_KEYFRAMES[action_previous_player_index].position;
                    for &action in pigma_duel_projectiles::actions(track_index, action_frame) {
                        apply_hostile_projectile_action(
                            &mut expected_object,
                            &mut expected_flight,
                            action,
                            action_player_position,
                            action_previous_player_position,
                        );
                    }
                }
                assert_eq!(projectile.base.velocity, expected_object.base.velocity);
                assert_eq!(
                    projectile.extension.activity,
                    ObjectActivity::HostileProjectileFlight(expected_flight),
                    "Pigma projectile track {track_index} state at frame {retail_frame}"
                );
            }
        }

        assert_eq!(retained_poses, RETAINED_PROJECTILE_POSE_COUNT);
        let cleanup_frame = last_retail_frame + RETAIL_PRESENTATION_FRAMES_PER_TICK as u16;
        let player_index = usize::from(cleanup_frame / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
        game.update_pigma_projectiles(
            cleanup_frame,
            pigma_duel::PLAYER_KEYFRAMES[player_index].position,
            pigma_duel::PLAYER_KEYFRAMES[player_index - 1].position,
        )
        .unwrap();
        assert!(game.pigma_projectiles.is_empty());
    }

    #[test]
    fn typed_pigma_rival_flight_matches_every_oracle_boundary() {
        const RETAINED_RIVAL_POSE_COUNT: usize = 298;

        let mut game = Game::new();
        game.spawn_pigma_rival().unwrap();
        let rival_id = game.pigma_rival.unwrap();
        let mut retained_poses = 0;

        for retail_frame in (0..=pigma_duel_rival::DEPARTURE_RETAIL_FRAME)
            .step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
        {
            let player_index =
                usize::from(retail_frame / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
            let previous_player_index = usize::from(
                retail_frame.saturating_sub(RETAIL_PRESENTATION_FRAMES_PER_TICK as u16)
                    / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
            );
            let player_position = pigma_duel::PLAYER_KEYFRAMES[player_index].position;
            let previous_player_position =
                pigma_duel::PLAYER_KEYFRAMES[previous_player_index].position;
            game.update_pigma_rival(retail_frame, player_position, previous_player_position);

            let expected = pigma_duel::RIVAL_KEYFRAMES
                .iter()
                .find(|keyframe| keyframe.retail_frame == retail_frame);
            let Some(expected) = expected else {
                let object = game.state().objects.get(rival_id).unwrap();
                assert!(!object.base.flags.active, "frame {retail_frame}");
                assert!(!object.base.flags.visible, "frame {retail_frame}");
                assert!(object.base.flags.collision_disabled, "frame {retail_frame}");
                continue;
            };
            match expected.presentation {
                MissionActorPresentation::Present(pose) => {
                    retained_poses += 1;
                    let object = game.state().objects.get(rival_id).unwrap_or_else(|| {
                        panic!("Pigma departed before retail frame {retail_frame}")
                    });
                    assert_eq!(object.base.position, pose.position, "frame {retail_frame}");
                    assert_eq!(
                        object.base.pitch.units(),
                        pose.pitch,
                        "frame {retail_frame}"
                    );
                    assert_eq!(object.base.yaw.units(), pose.yaw, "frame {retail_frame}");
                    assert_eq!(object.base.roll.units(), pose.roll, "frame {retail_frame}");
                    assert_eq!(object.base.speed, pose.speed, "frame {retail_frame}");
                    assert!(object.base.flags.active, "frame {retail_frame}");
                    assert!(object.base.flags.visible, "frame {retail_frame}");
                    assert!(
                        !object.base.flags.collision_disabled,
                        "frame {retail_frame}"
                    );
                    assert!(matches!(
                        object.extension.activity,
                        ObjectActivity::PigmaRivalFlight(_)
                    ));
                }
                MissionActorPresentation::Departed => {
                    assert!(game.pigma_rival.is_none(), "frame {retail_frame}");
                }
                MissionActorPresentation::Inactive => {
                    panic!("Pigma oracle unexpectedly becomes inactive at frame {retail_frame}")
                }
            }
        }

        assert_eq!(retained_poses, RETAINED_RIVAL_POSE_COUNT);
        assert_eq!(game.state().mission.score, PIGMA_SCORE_AWARD);
        assert_eq!(game.state().mission.objects_destroyed, 1);
    }

    #[test]
    fn typed_leon_rival_flight_matches_every_oracle_boundary() {
        const RETAINED_RIVAL_POSE_COUNT: usize = 153;

        let mut game = Game::new();
        game.spawn_leon_rival().unwrap();
        let rival_id = game.leon_rival.unwrap();
        let mut retained_poses = 0;

        for retail_frame in (0..=leon_duel_rival::DEPARTURE_RETAIL_FRAME)
            .step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
        {
            game.update_leon_rival(retail_frame);

            let expected = leon_duel::RIVAL_KEYFRAMES
                .iter()
                .find(|keyframe| keyframe.retail_frame == retail_frame);
            let Some(expected) = expected else {
                let object = game.state().objects.get(rival_id).unwrap();
                assert!(!object.base.flags.active, "frame {retail_frame}");
                assert!(!object.base.flags.visible, "frame {retail_frame}");
                assert!(object.base.flags.collision_disabled, "frame {retail_frame}");
                continue;
            };
            match expected.presentation {
                MissionActorPresentation::Present(pose) => {
                    retained_poses += 1;
                    let object = game.state().objects.get(rival_id).unwrap_or_else(|| {
                        panic!("Leon departed before retail frame {retail_frame}")
                    });
                    assert_eq!(object.base.position, pose.position, "frame {retail_frame}");
                    assert_eq!(
                        object.base.pitch.units(),
                        pose.pitch,
                        "frame {retail_frame}"
                    );
                    assert_eq!(object.base.yaw.units(), pose.yaw, "frame {retail_frame}");
                    assert_eq!(object.base.roll.units(), pose.roll, "frame {retail_frame}");
                    assert_eq!(object.base.speed, pose.speed, "frame {retail_frame}");
                    assert!(object.base.flags.active, "frame {retail_frame}");
                    assert!(object.base.flags.visible, "frame {retail_frame}");
                    assert!(
                        !object.base.flags.collision_disabled,
                        "frame {retail_frame}"
                    );
                    assert!(matches!(
                        object.extension.activity,
                        ObjectActivity::LeonRivalFlight(_)
                    ));
                }
                MissionActorPresentation::Departed => {
                    assert!(game.leon_rival.is_none(), "frame {retail_frame}");
                }
                MissionActorPresentation::Inactive => {
                    panic!("Leon oracle unexpectedly becomes inactive at frame {retail_frame}")
                }
            }
        }

        assert_eq!(retained_poses, RETAINED_RIVAL_POSE_COUNT);
        assert_eq!(game.state().mission.score, LEON_SCORE_AWARD);
        assert_eq!(game.state().mission.objects_destroyed, 1);
    }

    fn assert_typed_final_rival_flight_matches_every_oracle_boundary(
        visit: MissionVisit,
        shape: ShapeId,
        plan: final_rivals_flight::FinalRivalFlightPlan,
        player_keyframes: &[MissionPlayerKeyframe],
        rival_keyframes: &[MissionActorKeyframe],
        retained_rival_pose_count: usize,
    ) {
        let mut game = Game::new();
        game.state.mission.visit = visit;
        game.spawn_final_rival(shape).unwrap();
        let rival_id = game.final_rival.unwrap();
        let mut retained_poses = 0;

        for retail_frame in (0..=plan.departure_retail_frame)
            .step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
        {
            let player_index =
                usize::from(retail_frame / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16);
            let previous_player_index = usize::from(
                retail_frame.saturating_sub(RETAIL_PRESENTATION_FRAMES_PER_TICK as u16)
                    / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
            );
            let player_position = player_keyframes
                .get(player_index)
                .or_else(|| player_keyframes.last())
                .expect("final rival oracle has player poses")
                .position;
            let previous_player_position = player_keyframes
                .get(previous_player_index)
                .or_else(|| player_keyframes.last())
                .expect("final rival oracle has player poses")
                .position;
            game.update_final_rival_actor(
                retail_frame,
                player_position,
                previous_player_position,
            );

            let expected = rival_keyframes
                .iter()
                .find(|keyframe| keyframe.retail_frame == retail_frame);
            let Some(expected) = expected else {
                let object = game.state().objects.get(rival_id).unwrap();
                assert!(!object.base.flags.active, "frame {retail_frame}");
                assert!(!object.base.flags.visible, "frame {retail_frame}");
                assert!(object.base.flags.collision_disabled, "frame {retail_frame}");
                continue;
            };
            match expected.presentation {
                MissionActorPresentation::Present(pose) => {
                    retained_poses += 1;
                    let object = game.state().objects.get(rival_id).unwrap_or_else(|| {
                        panic!("final rival departed before retail frame {retail_frame}")
                    });
                    assert_eq!(object.base.position, pose.position, "frame {retail_frame}");
                    assert_eq!(
                        object.base.pitch.units(),
                        pose.pitch,
                        "frame {retail_frame}"
                    );
                    assert_eq!(object.base.yaw.units(), pose.yaw, "frame {retail_frame}");
                    assert_eq!(object.base.roll.units(), pose.roll, "frame {retail_frame}");
                    assert_eq!(object.base.speed, pose.speed, "frame {retail_frame}");
                    assert!(object.base.flags.active, "frame {retail_frame}");
                    assert!(object.base.flags.visible, "frame {retail_frame}");
                    assert!(
                        !object.base.flags.collision_disabled,
                        "frame {retail_frame}"
                    );
                    assert!(matches!(
                        object.extension.activity,
                        ObjectActivity::FinalRivalFlight(_)
                    ));
                }
                MissionActorPresentation::Departed => {
                    assert!(game.final_rival.is_none(), "frame {retail_frame}");
                }
                MissionActorPresentation::Inactive => {
                    let object = game.state().objects.get(rival_id).unwrap();
                    assert!(!object.base.flags.active, "frame {retail_frame}");
                    assert!(!object.base.flags.visible, "frame {retail_frame}");
                    assert!(object.base.flags.collision_disabled, "frame {retail_frame}");
                }
            }
        }

        assert_eq!(retained_poses, retained_rival_pose_count);
    }

    #[test]
    fn typed_final_pursuer_flight_matches_every_oracle_boundary() {
        const RETAINED_RIVAL_POSE_COUNT: usize = 209;

        assert_typed_final_rival_flight_matches_every_oracle_boundary(
            MissionVisit::FinalPursuer,
            ShapeId::FINAL_PURSUER_CRAFT,
            final_rivals_flight::FINAL_PURSUER,
            &final_pursuer::PLAYER_KEYFRAMES,
            &final_pursuer::RIVAL_KEYFRAMES,
            RETAINED_RIVAL_POSE_COUNT,
        );
    }

    #[test]
    fn typed_wolf_blockade_flight_matches_every_oracle_boundary() {
        const RETAINED_RIVAL_POSE_COUNT: usize = 224;

        assert_typed_final_rival_flight_matches_every_oracle_boundary(
            MissionVisit::WolfBlockade,
            ShapeId::WOLF_BLOCKADE_CRAFT,
            final_rivals_flight::WOLF_BLOCKADE,
            &wolf_blockade::PLAYER_KEYFRAMES,
            &wolf_blockade::RIVAL_KEYFRAMES,
            RETAINED_RIVAL_POSE_COUNT,
        );
    }

    #[test]
    fn missile_interception_native_targets_match_every_certified_pose() {
        const CERTIFIED_POSE_COUNT: usize = 1_817;
        let tracks: [&[MissionActorKeyframe]; INTERCEPTION_MISSILE_COUNT] = [
            &missile_interception::LEAD_MISSILE_KEYFRAMES,
            &missile_interception::UPPER_MISSILE_KEYFRAMES,
            &missile_interception::LOWER_MISSILE_KEYFRAMES,
        ];

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.campaign.route_step = CampaignRouteStep::MissileInterception;
        game.state.strategic_map.player_map_position = MISSILE_INTERCEPTION_DESTINATION;
        game.begin_missile_interception_sortie().unwrap();

        let mut compared = 0;
        for retail_frame in (missile_interception_targets::START_RETAIL_FRAME
            ..=missile_interception_targets::END_RETAIL_FRAME)
            .step_by(RETAIL_PRESENTATION_FRAMES_PER_TICK as usize)
        {
            game.update_interception_missiles(retail_frame);
            for (index, keyframes) in tracks.into_iter().enumerate() {
                if retail_frame >= missile_interception_targets::DEPARTURE_RETAIL_FRAMES[index] {
                    assert!(
                        game.interception_missiles[index].is_none(),
                        "missile {index} remained after frame {retail_frame}"
                    );
                    continue;
                }
                let expected = keyframes
                    .iter()
                    .find(|keyframe| keyframe.retail_frame == retail_frame)
                    .unwrap_or_else(|| {
                        panic!("missing certified missile {index} pose at frame {retail_frame}")
                    });
                let MissionActorPresentation::Present(expected_pose) = expected.presentation else {
                    panic!("missile {index} departed early at frame {retail_frame}");
                };
                let id = game.interception_missiles[index]
                    .unwrap_or_else(|| panic!("missile {index} missing at frame {retail_frame}"));
                let object = game.state.objects.get(id).unwrap();
                assert_eq!(
                    object.base.position, expected_pose.position,
                    "missile {index} position at frame {retail_frame}"
                );
                assert_eq!(
                    object.base.pitch.units(),
                    expected_pose.pitch,
                    "missile {index} pitch at frame {retail_frame}"
                );
                assert_eq!(
                    object.base.yaw.units(),
                    expected_pose.yaw,
                    "missile {index} yaw at frame {retail_frame}"
                );
                assert_eq!(
                    object.base.roll.units(),
                    expected_pose.roll,
                    "missile {index} roll at frame {retail_frame}"
                );
                assert_eq!(
                    object.base.speed, expected_pose.speed,
                    "missile {index} speed at frame {retail_frame}"
                );
                assert!(object.base.flags.active, "frame {retail_frame}");
                assert!(object.base.flags.visible, "frame {retail_frame}");
                assert!(object.base.flags.collision_disabled, "frame {retail_frame}");
                assert!(matches!(
                    object.extension.activity,
                    ObjectActivity::InterceptionMissileFlight(_)
                ));
                compared += 1;
            }
        }
        assert_eq!(compared, CERTIFIED_POSE_COUNT);
    }

    #[test]
    fn missile_interception_has_three_certified_targets_and_the_retail_return() {
        const ACTIVE_CHECKPOINT_RETAIL_FRAME: u16 = 432;
        const MISSILE_COUNT_CHECKPOINTS: [(u16, usize); 7] = [
            (64, 3),
            (2_416, 3),
            (2_420, 2),
            (2_468, 2),
            (2_472, 1),
            (2_564, 1),
            (2_568, 0),
        ];

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.campaign.route_step = CampaignRouteStep::MissileInterception;
        game.state.campaign.corneria_damage_percent = SECOND_RETURN_CORNERIA_DAMAGE_PERCENT;
        game.state.mission.score = 100;
        game.state.strategic_map.player_map_position = MISSILE_INTERCEPTION_DESTINATION;
        game.begin_missile_interception_sortie().unwrap();

        assert_eq!(
            game.state().mission.visit,
            MissionVisit::MissileInterception
        );
        assert_eq!(game.mission(), Some(MissionId::MISSILE_INTERCEPTION));
        assert_eq!(game.interception_missiles.iter().flatten().count(), 3);

        while game.state().mode_frame
            < u32::from(ACTIVE_CHECKPOINT_RETAIL_FRAME) / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        let expected_player = missile_interception::PLAYER_KEYFRAMES[usize::from(
            ACTIVE_CHECKPOINT_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
        )];
        let expected_camera = missile_interception::CAMERA_KEYFRAMES[usize::from(
            ACTIVE_CHECKPOINT_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
        )];
        let player = game
            .state()
            .objects
            .get(game.state().mission.primary_player.unwrap())
            .unwrap();
        assert_eq!(player.base.position, expected_player.position);
        assert_eq!(player.base.shape, ShapeId::FOX_FALCO_FLIGHT_CRAFT);
        assert_eq!(game.state().camera.position, expected_camera.position);
        assert_eq!(game.state().mission.elapsed_time_tenths, 1);

        for (retail_frame, expected_count) in MISSILE_COUNT_CHECKPOINTS {
            while game.state().mode_frame
                < u32::from(retail_frame) / RETAIL_PRESENTATION_FRAMES_PER_TICK
            {
                game.tick(0).unwrap();
            }
            assert_eq!(
                game.interception_missiles.iter().flatten().count(),
                expected_count,
                "missile count at retail frame {retail_frame}"
            );
        }

        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::FighterIntercept
        );
        assert_eq!(
            game.state().campaign.objectives.missiles,
            StrategicThreatCount::NONE
        );
        assert_eq!(
            game.state().campaign.elapsed_frames,
            MISSILE_INTERCEPTION_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND
        );
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT
        );
        assert_eq!(game.state().mission.score, 100);
        assert_eq!(
            game.state().strategic_map.actors[6].map(|actor| actor.kind),
            Some(StrategicMapActorKind::AttackingFighter)
        );
        assert!(game.interception_missiles.iter().all(Option::is_none));
    }

    #[test]
    fn fighter_intercept_matches_the_certified_three_target_sortie_and_return() {
        const POSE_CHECKPOINT_RETAIL_FRAME: u16 = 320;
        const FIRST_PROJECTILE_RETAIL_FRAME: u16 = 1_016;
        const TARGET_CHECKPOINTS: [(u16, usize, u32); 7] = [
            (320, 3, 0),
            (1_116, 2, 100),
            (1_368, 1, 100),
            (1_372, 2, 100),
            (1_784, 1, 200),
            (3_112, 1, 200),
            (3_116, 0, 300),
        ];

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.campaign.route_step = CampaignRouteStep::FighterIntercept;
        game.state.campaign.corneria_damage_percent =
            MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT;
        game.state.mission.score = 100;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.begin_fighter_intercept_sortie().unwrap();

        assert_eq!(game.state().mission.visit, MissionVisit::FighterIntercept);
        assert_eq!(game.mission(), Some(MissionId::FIGHTER_INTERCEPT));
        assert_eq!(game.state().mission.score, 0);
        assert_eq!(game.state().mission.objects_destroyed, 0);
        assert_eq!(
            [
                game.fighter_intercept_actors.lead,
                game.fighter_intercept_actors.flank,
                game.fighter_intercept_actors.rear,
            ]
            .into_iter()
            .flatten()
            .count(),
            FIGHTER_INTERCEPT_TARGET_COUNT
        );

        while game.state().mode_frame
            < u32::from(POSE_CHECKPOINT_RETAIL_FRAME) / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        let expected_player = fighter_intercept::PLAYER_KEYFRAMES[usize::from(
            POSE_CHECKPOINT_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
        )];
        let expected_camera = fighter_intercept::CAMERA_KEYFRAMES[usize::from(
            POSE_CHECKPOINT_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16,
        )];
        let player = game
            .state()
            .objects
            .get(game.state().mission.primary_player.unwrap())
            .unwrap();
        assert_eq!(player.base.position, expected_player.position);
        assert_eq!(player.base.yaw.units(), expected_player.yaw);
        assert_eq!(player.base.roll.units(), expected_player.roll);
        assert_eq!(player.base.shape, ShapeId::FOX_FALCO_FLIGHT_CRAFT);
        assert_eq!(game.camera().position, expected_camera.position);
        for (actor, keyframes) in [
            (
                game.fighter_intercept_actors.lead,
                &fighter_intercept::LEAD_FIGHTER_KEYFRAMES[..],
            ),
            (
                game.fighter_intercept_actors.flank,
                &fighter_intercept::FLANK_FIGHTER_KEYFRAMES[..],
            ),
            (
                game.fighter_intercept_actors.rear,
                &fighter_intercept::REAR_FIGHTER_KEYFRAMES[..],
            ),
        ] {
            let object = game.state().objects.get(actor.unwrap()).unwrap();
            let expected = keyframes
                .iter()
                .find(|keyframe| keyframe.retail_frame == POSE_CHECKPOINT_RETAIL_FRAME)
                .unwrap();
            let MissionActorPresentation::Present(expected) = expected.presentation else {
                panic!("fighter is absent at the certified pose checkpoint");
            };
            assert_eq!(object.base.position, expected.position);
            assert_eq!(object.base.pitch.units(), expected.pitch);
            assert_eq!(object.base.yaw.units(), expected.yaw);
            assert_eq!(object.base.roll.units(), expected.roll);
            assert_eq!(object.base.shape, ShapeId::INTERCEPT_FIGHTER);
        }

        while game.state().mode_frame
            < u32::from(FIRST_PROJECTILE_RETAIL_FRAME) / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        assert_eq!(game.fighter_intercept_projectiles.len(), 1);
        let projectile = game.fighter_intercept_projectiles[0];
        let expected_projectile = fighter_intercept::ENEMY_LASER_KEYFRAME_TRACKS[0][0];
        assert_eq!(
            expected_projectile.retail_frame,
            FIRST_PROJECTILE_RETAIL_FRAME
        );
        assert_eq!(
            game.state()
                .objects
                .get(projectile.object)
                .unwrap()
                .base
                .position,
            expected_projectile.pose.position
        );

        for (retail_frame, expected_targets, expected_score) in TARGET_CHECKPOINTS {
            while game.state().mode_frame
                < u32::from(retail_frame) / RETAIL_PRESENTATION_FRAMES_PER_TICK
            {
                game.tick(0).unwrap();
            }
            let active_targets = [
                game.fighter_intercept_actors.lead,
                game.fighter_intercept_actors.flank,
                game.fighter_intercept_actors.rear,
            ]
            .into_iter()
            .flatten()
            .filter_map(|id| game.state().objects.get(id))
            .filter(|object| object.base.flags.active)
            .count();
            assert_eq!(
                active_targets, expected_targets,
                "active fighter count at retail frame {retail_frame}"
            );
            assert_eq!(
                game.state().mission.score,
                expected_score,
                "score at retail frame {retail_frame}"
            );
        }

        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::PigmaDuel
        );
        assert_eq!(
            game.state().campaign.elapsed_frames,
            FIGHTER_INTERCEPT_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND
        );
        assert_eq!(game.state().mission.score, 300);
        assert_eq!(game.state().mission.objects_destroyed, 3);
        assert_eq!(
            game.state().strategic_map.actors,
            POST_FIGHTER_INTERCEPT_MAP_ACTORS
        );
        assert_eq!(
            game.state().strategic_map.player_map_position,
            INITIAL_PLAYER_MAP_POSITION
        );
        assert!(game.fighter_intercept_projectiles.is_empty());
        assert!(game.fighter_intercept_actors.lead.is_none());
        assert!(game.fighter_intercept_actors.flank.is_none());
        assert!(game.fighter_intercept_actors.rear.is_none());
    }

    #[test]
    fn pigma_duel_matches_the_certified_rival_sortie_and_return() {
        const PLAYER_REVEAL_RETAIL_FRAME: u16 = 376;
        const FIRST_PROJECTILE_RETAIL_FRAME: u16 = 772;
        const RIVAL_DEPARTURE_RETAIL_FRAME: u16 = 1_228;

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.campaign.route_step = CampaignRouteStep::PigmaDuel;
        game.state.campaign.corneria_damage_percent =
            MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT;
        game.state.mission.score = 300;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.begin_pigma_duel_sortie().unwrap();

        assert_eq!(game.state().mission.visit, MissionVisit::PigmaDuel);
        assert_eq!(game.mission(), Some(MissionId::PIGMA_DUEL));
        assert_eq!(game.state().mission.score, 300);
        assert_eq!(game.state().mission.objects_destroyed, 0);
        let rival_id = game.pigma_rival.expect("Pigma is allocated for the duel");
        let rival = game.state().objects.get(rival_id).unwrap();
        assert_eq!(rival.base.shape, ShapeId::PIGMA_CRAFT);
        assert_eq!(rival.base.hit_points, PIGMA_HEALTH);
        assert_eq!(rival.base.attack_power, PIGMA_ATTACK_POWER);
        assert_eq!(rival.base.collision_class, CollisionClass::Enemy);

        while game.state().mode_frame
            < u32::from(PLAYER_REVEAL_RETAIL_FRAME) / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        let expected_player = pigma_duel::PLAYER_KEYFRAMES
            [usize::from(PLAYER_REVEAL_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16)];
        let expected_camera = pigma_duel::CAMERA_KEYFRAMES
            [usize::from(PLAYER_REVEAL_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK as u16)];
        let player = game
            .state()
            .objects
            .get(game.state().mission.primary_player.unwrap())
            .unwrap();
        assert_eq!(player.base.position, expected_player.position);
        assert_eq!(player.base.shape, ShapeId::FOX_FALCO_FLIGHT_CRAFT);
        assert!(player.base.flags.visible);
        assert_eq!(game.camera().position, expected_camera.position);

        let rival = game.state().objects.get(rival_id).unwrap();
        let expected_rival = pigma_duel::RIVAL_KEYFRAMES
            .iter()
            .find(|keyframe| keyframe.retail_frame == PLAYER_REVEAL_RETAIL_FRAME)
            .unwrap();
        let MissionActorPresentation::Present(expected_rival) = expected_rival.presentation else {
            panic!("Pigma is absent at the certified reveal checkpoint");
        };
        assert_eq!(rival.base.position, expected_rival.position);
        assert_eq!(rival.base.pitch.units(), expected_rival.pitch);
        assert_eq!(rival.base.yaw.units(), expected_rival.yaw);
        assert_eq!(rival.base.roll.units(), expected_rival.roll);
        assert!(rival.base.flags.active);

        while game.state().mode_frame
            < u32::from(FIRST_PROJECTILE_RETAIL_FRAME) / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        assert_eq!(game.pigma_projectiles.len(), 1);
        let projectile = game.pigma_projectiles[0];
        let expected_projectile = pigma_duel::ENEMY_LASER_KEYFRAME_TRACKS[0][0];
        assert_eq!(
            expected_projectile.retail_frame,
            FIRST_PROJECTILE_RETAIL_FRAME
        );
        assert_eq!(
            game.state()
                .objects
                .get(projectile.object)
                .unwrap()
                .base
                .position,
            expected_projectile.pose.position
        );

        while game.state().mode_frame
            < u32::from(RIVAL_DEPARTURE_RETAIL_FRAME) / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }
        assert!(game.pigma_rival.is_none());
        assert_eq!(game.state().mission.score, 1_300);
        assert_eq!(game.state().mission.objects_destroyed, 1);

        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::EladardBase
        );
        assert_eq!(
            game.state().campaign.elapsed_frames,
            PIGMA_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND
        );
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT
        );
        assert_eq!(game.state().mission.item_count, 2);
        assert_eq!(game.state().mission.score, 1_300);
        assert_eq!(
            game.state().strategic_map.player_map_position,
            INITIAL_PLAYER_MAP_POSITION
        );
        assert_eq!(game.state().strategic_map.actors, POST_PIGMA_MAP_ACTORS);
        assert!(game.pigma_projectiles.is_empty());
    }

    #[test]
    fn pigma_native_destruction_preserves_the_boss_score_award() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.campaign.route_step = CampaignRouteStep::PigmaDuel;
        game.state.mission.score = 300;
        game.begin_pigma_duel_sortie().unwrap();
        while game.state().mode_frame
            < u32::from(PIGMA_PLAYER_REVEAL_RETAIL_FRAME) / RETAIL_PRESENTATION_FRAMES_PER_TICK
        {
            game.tick(0).unwrap();
        }

        let rival_id = game.pigma_rival.unwrap();
        let rival = game.state.objects.get_mut(rival_id).unwrap();
        rival.base.flags.collision_disabled = true;
        rival.base.explosion_timer = ENEMY_DESTRUCTION_TICKS;
        for _ in 0..ENEMY_DESTRUCTION_TICKS {
            game.tick(0).unwrap();
        }

        assert!(game.pigma_rival.is_none());
        assert!(game.state().objects.get(rival_id).is_none());
        assert_eq!(game.state().mission.score, 1_300);
        assert_eq!(game.state().mission.objects_destroyed, 1);
    }

    #[test]
    fn eladard_route_uses_typed_objectives_and_matches_the_sixth_return() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.mode = GameMode::StrategicMap;
        game.state.campaign.route_step = CampaignRouteStep::EladardBase;
        game.state.campaign.corneria_damage_percent =
            MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT;
        game.state.mission.score = 1_300;
        game.state.mission.item_count = 2;
        game.state.strategic_map.phase = StrategicMapPhase::Planning;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.state.strategic_map.destination = ELADARD_BASE_DESTINATION;

        game.tick(Button::B as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.travel_total_ticks,
            ELADARD_STRATEGIC_TRAVEL_TICKS
        );
        assert_eq!(
            game.state().strategic_map.phase,
            StrategicMapPhase::Traveling
        );
        for _ in 0..ELADARD_STRATEGIC_TRAVEL_TICKS / 2 {
            game.tick(0).unwrap();
        }
        assert!(
            game.state().campaign.corneria_damage_percent
                > MISSILE_INTERCEPTION_RETURN_CORNERIA_DAMAGE_PERCENT
        );
        assert!(
            game.state().campaign.corneria_damage_percent < ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT
        );
        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }

        assert_eq!(game.state().mission.visit, MissionVisit::EladardBase);
        assert_eq!(game.mission(), Some(MissionId::ELADARD_BASE));
        assert_eq!(
            game.state().strategic_map.player_map_position,
            ELADARD_BASE_DESTINATION
        );
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT
        );
        assert_eq!(
            game.state().mission.eladard,
            EladardMissionState {
                phase: EladardPhase::SurfaceApproach,
                phase_started_retail_frame: 0,
                surface_barriers_remaining: ELADARD_SURFACE_BARRIER_COUNT,
                platform_switch_pressed: false,
                wall_spider_hit_points: ELADARD_WALL_SPIDER_HEALTH,
                generator_active: false,
                generator_hit_points: ELADARD_GENERATOR_HEALTH,
            }
        );

        let advance_to = |game: &mut Game, retail_frame: u16| {
            let target_tick = u32::from(retail_frame).div_ceil(RETAIL_PRESENTATION_FRAMES_PER_TICK);
            while game.state().mode_frame < target_tick {
                game.tick(0).unwrap();
            }
        };

        advance_to(&mut game, ELADARD_SURFACE_BARRIERS_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.eladard.phase,
            EladardPhase::SurfaceBarriers
        );
        assert_eq!(game.state().mission.eladard.surface_barriers_remaining, 0);

        advance_to(&mut game, ELADARD_BASE_ENTRANCE_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.eladard.phase,
            EladardPhase::BaseEntrance
        );
        advance_to(&mut game, ELADARD_WALKER_TRANSFORMATION_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.eladard.phase,
            EladardPhase::WalkerTransformation
        );
        assert_eq!(
            game.state().mission.player_craft_form,
            PlayerCraftForm::Transforming(PlayerCraftTransformation {
                direction: PlayerCraftTransformationDirection::ToWalker,
                elapsed_retail_frames: PLAYER_TRANSFORMATION_RETAIL_FRAMES_PER_TICK,
            })
        );
        advance_to(&mut game, ELADARD_WALKER_READY_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.player_craft_form,
            PlayerCraftForm::Walker
        );

        advance_to(&mut game, ELADARD_PLATFORM_SWITCH_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.eladard.phase,
            EladardPhase::PlatformSwitch
        );
        assert!(!game.state().mission.eladard.platform_switch_pressed);
        advance_to(&mut game, ELADARD_WALL_SPIDER_RETAIL_FRAME);
        assert_eq!(game.state().mission.eladard.phase, EladardPhase::WallSpider);
        assert!(game.state().mission.eladard.platform_switch_pressed);
        assert_eq!(
            game.state().mission.eladard.wall_spider_hit_points,
            ELADARD_WALL_SPIDER_HEALTH
        );

        advance_to(&mut game, ELADARD_GENERATOR_RETAIL_FRAME);
        assert_eq!(game.state().mission.eladard.phase, EladardPhase::Generator);
        assert_eq!(game.state().mission.eladard.wall_spider_hit_points, 0);
        assert!(game.state().mission.eladard.generator_active);
        assert_eq!(
            game.state().mission.eladard.generator_hit_points,
            ELADARD_GENERATOR_HEALTH
        );
        advance_to(&mut game, ELADARD_GENERATOR_DESTROYED_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.eladard.phase,
            EladardPhase::BaseDestruction
        );
        assert!(!game.state().mission.eladard.generator_active);
        assert_eq!(game.state().mission.eladard.generator_hit_points, 0);

        advance_to(&mut game, ELADARD_RETURN_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.eladard.phase,
            EladardPhase::ReturnFlight
        );
        assert_eq!(
            game.state().mission.phase,
            MissionPhase::ReturningToStrategicMap
        );
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::FirstBattleCarrier
        );
        assert_eq!(
            game.state().campaign.objectives.eladard,
            PlanetObjectiveStatus::Rescued
        );
        assert_eq!(
            game.state().campaign.elapsed_frames,
            ELADARD_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND
        );
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT
        );
        assert_eq!(game.state().mission.item_count, ELADARD_RETURN_ITEM_COUNT);
        assert_eq!(game.state().mission.score, ELADARD_RETURN_SCORE);
        assert_eq!(game.state().strategic_map.actors, POST_ELADARD_MAP_ACTORS);
        assert_eq!(
            game.state().strategic_map.recommended_destination,
            POST_ELADARD_RECOMMENDED_DESTINATION
        );
        for player in [
            game.state().mission.primary_player,
            game.state().mission.wingmate,
        ]
        .into_iter()
        .flatten()
        {
            assert_eq!(
                game.state().objects.get(player).unwrap().base.hit_points,
                ELADARD_RETURN_SHIELD
            );
        }
    }

    #[test]
    fn battle_carrier_uses_typed_panels_and_matches_the_seventh_return() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.mode = GameMode::StrategicMap;
        game.state.campaign.route_step = CampaignRouteStep::FirstBattleCarrier;
        game.state.campaign.elapsed_frames =
            ELADARD_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
        game.state.campaign.corneria_damage_percent = ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT;
        game.state.mission.item_count = ELADARD_RETURN_ITEM_COUNT;
        game.state.mission.score = ELADARD_RETURN_SCORE;
        game.state.strategic_map.phase = StrategicMapPhase::Planning;
        game.state.strategic_map.actors = POST_ELADARD_MAP_ACTORS;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.state.strategic_map.destination = POST_ELADARD_RECOMMENDED_DESTINATION;

        game.tick(Button::B as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.travel_total_ticks,
            CARRIER_STRATEGIC_TRAVEL_TICKS
        );
        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }

        assert_eq!(game.state().mission.visit, MissionVisit::FirstBattleCarrier);
        assert_eq!(game.mission(), Some(MissionId::BATTLE_CARRIER));
        assert_eq!(
            game.state().strategic_map.player_map_position,
            POST_ELADARD_RECOMMENDED_DESTINATION
        );
        assert_eq!(
            game.state().mission.carrier_assault,
            CarrierAssaultState {
                phase: CarrierAssaultPhase::ExteriorApproach,
                phase_started_retail_frame: 0,
                corridor_progress: 0,
                reactor_room_open: false,
                port_panel: CarrierReactorPanel {
                    integrity: CARRIER_PANEL_INITIAL_INTEGRITY,
                    active: true,
                },
                starboard_panel: CarrierReactorPanel {
                    integrity: CARRIER_PANEL_INITIAL_INTEGRITY,
                    active: true,
                },
            }
        );

        let advance_to = |game: &mut Game, retail_frame: u16| {
            let target_tick = u32::from(retail_frame).div_ceil(RETAIL_PRESENTATION_FRAMES_PER_TICK);
            while game.state().mode_frame < target_tick {
                game.tick(0).unwrap();
            }
        };

        advance_to(&mut game, CARRIER_EXTERIOR_END_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.carrier_assault.phase,
            CarrierAssaultPhase::InteriorCorridor
        );
        advance_to(&mut game, CARRIER_REACTOR_APPROACH_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.carrier_assault.phase,
            CarrierAssaultPhase::ReactorApproach
        );
        advance_to(&mut game, CARRIER_REACTOR_OPEN_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.carrier_assault.phase,
            CarrierAssaultPhase::ReactorCombat
        );
        assert_eq!(
            game.state().mission.player_craft_form,
            PlayerCraftForm::Walker
        );
        assert!(game.state().mission.carrier_assault.reactor_room_open);

        advance_to(&mut game, CARRIER_STARBOARD_DESTROYED_RETAIL_FRAME);
        assert_eq!(
            game.state()
                .mission
                .carrier_assault
                .starboard_panel
                .integrity,
            CARRIER_PANEL_DESTROYED_INTEGRITY
        );
        assert!(!game.state().mission.carrier_assault.starboard_panel.active);
        let starboard = game.carrier_panels[CARRIER_STARBOARD_PANEL_INDEX].unwrap();
        assert_eq!(
            game.state().objects.get(starboard).unwrap().base.shape,
            ShapeId::CARRIER_REACTOR_PANEL_DESTROYED
        );

        advance_to(&mut game, CARRIER_PORT_DESTROYED_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.carrier_assault.port_panel.integrity,
            CARRIER_PANEL_DESTROYED_INTEGRITY
        );
        assert!(!game.state().mission.carrier_assault.port_panel.active);
        let port = game.carrier_panels[CARRIER_PORT_PANEL_INDEX].unwrap();
        assert_eq!(
            game.state().objects.get(port).unwrap().base.shape,
            ShapeId::CARRIER_REACTOR_PANEL_DESTROYED
        );

        advance_to(&mut game, CARRIER_CORE_DESTROYED_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.carrier_assault.phase,
            CarrierAssaultPhase::CoreDestruction
        );
        advance_to(&mut game, CARRIER_RETURN_FLIGHT_RETAIL_FRAME);
        assert_eq!(
            game.state().mission.carrier_assault.phase,
            CarrierAssaultPhase::ReturnFlight
        );
        assert_eq!(
            game.state().mission.phase,
            MissionPhase::ReturningToStrategicMap
        );
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::LeonDuel
        );
        assert_eq!(
            game.state().campaign.objectives.first_carrier,
            CarrierObjectiveStatus::Destroyed
        );
        assert_eq!(
            game.state().campaign.elapsed_frames,
            CARRIER_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND
        );
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT
        );
        assert_eq!(game.state().mission.item_count, CARRIER_RETURN_ITEM_COUNT);
        assert_eq!(game.state().mission.score, CARRIER_RETURN_SCORE);
        assert_eq!(
            game.state().mission.objects_destroyed,
            CARRIER_REACTOR_PANEL_COUNT
        );
        assert_eq!(game.state().strategic_map.actors, POST_CARRIER_MAP_ACTORS);
        assert_eq!(
            game.state().strategic_map.recommended_destination,
            LEON_DUEL_DESTINATION
        );
        for (craft, expected_shield) in [
            (
                game.state().mission.primary_player,
                CARRIER_RETURN_PRIMARY_SHIELD,
            ),
            (
                game.state().mission.wingmate,
                CARRIER_RETURN_WINGMATE_SHIELD,
            ),
        ] {
            let craft = craft.expect("campaign craft remains allocated");
            assert_eq!(
                game.state().objects.get(craft).unwrap().base.hit_points,
                expected_shield
            );
        }
        assert!(game.carrier_scenery.is_empty());
        assert_eq!(game.carrier_panels, [None; 2]);
    }

    #[test]
    fn leon_duel_uses_typed_rival_state_and_matches_the_eighth_return() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.mode = GameMode::StrategicMap;
        game.state.campaign.route_step = CampaignRouteStep::LeonDuel;
        game.state.campaign.elapsed_frames =
            CARRIER_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
        game.state.campaign.corneria_damage_percent = ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT;
        game.state.mission.item_count = CARRIER_RETURN_ITEM_COUNT;
        game.state.mission.score = CARRIER_RETURN_SCORE;
        game.set_campaign_craft_shields(
            CARRIER_RETURN_PRIMARY_SHIELD,
            CARRIER_RETURN_WINGMATE_SHIELD,
        );
        game.state.strategic_map.phase = StrategicMapPhase::Planning;
        game.state.strategic_map.actors = POST_CARRIER_MAP_ACTORS;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.state.strategic_map.destination = LEON_DUEL_DESTINATION;

        game.tick(Button::B as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.travel_total_ticks,
            LEON_STRATEGIC_TRAVEL_TICKS
        );
        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }

        assert_eq!(game.state().mission.visit, MissionVisit::LeonDuel);
        assert_eq!(game.mission(), Some(MissionId::LEON_DUEL));
        assert_eq!(
            game.state().strategic_map.player_map_position,
            LEON_DUEL_DESTINATION
        );
        let rival_id = game.leon_rival.expect("Leon remains allocated");
        let rival = game.state().objects.get(rival_id).unwrap();
        assert_eq!(rival.base.shape, ShapeId::LEON_CRAFT);
        assert_eq!(rival.base.hit_points, LEON_HEALTH);
        assert_eq!(rival.base.attack_power, LEON_ATTACK_POWER);
        assert!(!rival.base.flags.visible);

        let reveal_tick = u32::from(LEON_PLAYER_REVEAL_RETAIL_FRAME)
            .div_ceil(RETAIL_PRESENTATION_FRAMES_PER_TICK);
        while game.state().mode_frame < reveal_tick {
            game.tick(0).unwrap();
        }
        let primary = game.state().mission.primary_player.unwrap();
        let primary = game.state().objects.get(primary).unwrap();
        assert_eq!(primary.base.shape, ShapeId::FOX_FALCO_FLIGHT_CRAFT);
        assert!(primary.base.flags.visible);
        let rival = game.state().objects.get(rival_id).unwrap();
        assert_eq!(
            rival.base.position,
            Vector3 {
                x: 10_139,
                y: 0,
                z: 8_138
            }
        );
        assert!(rival.base.flags.visible);
        assert_eq!(game.leon_projectiles.len(), 1);

        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::MirageDragon
        );
        assert_eq!(
            game.state().campaign.elapsed_frames,
            LEON_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND
        );
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT
        );
        assert_eq!(game.state().mission.item_count, LEON_RETURN_ITEM_COUNT);
        assert_eq!(game.state().mission.score, LEON_RETURN_SCORE);
        assert_eq!(game.state().mission.objects_destroyed, 1);
        assert_eq!(game.state().strategic_map.actors, POST_LEON_MAP_ACTORS);
        assert_eq!(
            game.state().strategic_map.recommended_destination,
            MIRAGE_DRAGON_DESTINATION
        );
        for craft in [
            game.state().mission.primary_player,
            game.state().mission.wingmate,
        ]
        .into_iter()
        .flatten()
        {
            assert_eq!(
                game.state().objects.get(craft).unwrap().base.hit_points,
                LEON_RETURN_SHIELD
            );
        }
        assert!(game.leon_rival.is_none());
        assert!(game.leon_projectiles.is_empty());
    }

    #[test]
    fn post_leon_corneria_pressure_matches_the_verified_loss_timeline() {
        const LAST_UNDAMAGED_RETAIL_FRAME: u16 = 44;
        const DAMAGE_AFTER_FIRST_STEP: u8 = ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT + 1;
        const DAMAGE_AFTER_SECOND_STEP: u8 = ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT + 2;
        const DAMAGE_AFTER_THIRD_STEP: u8 = ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT + 3;
        const DAMAGE_AFTER_FIFTH_STEP: u8 = ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT + 5;
        const DAMAGE_AFTER_TENTH_STEP: u8 = ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT + 10;
        const THIRD_DAMAGE_RETAIL_FRAME: u16 = POST_LEON_DAMAGE_RETAIL_FRAMES[2];
        const FIFTH_DAMAGE_RETAIL_FRAME: u16 = POST_LEON_DAMAGE_RETAIL_FRAMES[4];
        const TENTH_DAMAGE_RETAIL_FRAME: u16 = POST_LEON_DAMAGE_RETAIL_FRAMES[9];
        const FINAL_DAMAGE_RETAIL_FRAME: u16 = POST_LEON_DAMAGE_RETAIL_FRAMES[10];
        const LAST_PRE_RESULTS_RETAIL_FRAME: u16 =
            POST_LEON_RESULTS_RETAIL_FRAME - RETAIL_PRESENTATION_FRAMES_PER_TICK as u16;

        fn advance_defense_to_retail_frame(game: &mut Game, retail_frame: u16) {
            let target_tick = u32::from(retail_frame).div_ceil(RETAIL_PRESENTATION_FRAMES_PER_TICK);
            while u32::from(game.state().campaign.corneria_defense.elapsed_retail_frames)
                / RETAIL_PRESENTATION_FRAMES_PER_TICK
                < target_tick
            {
                game.tick(0).unwrap();
            }
        }

        let mut game = Game::new();
        game.state.mode = GameMode::Mission;
        game.state.campaign.route_step = CampaignRouteStep::LeonDuel;
        game.state.campaign.corneria_damage_percent = ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT;
        game.finish_sortie();

        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::MirageDragon
        );
        assert_eq!(
            game.state().campaign.corneria_defense,
            CorneriaDefenseState::post_leon()
        );
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT
        );

        advance_defense_to_retail_frame(&mut game, LAST_UNDAMAGED_RETAIL_FRAME);
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            ELADARD_RETURN_CORNERIA_DAMAGE_PERCENT
        );
        advance_defense_to_retail_frame(&mut game, POST_LEON_DAMAGE_RETAIL_FRAMES[0]);
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            DAMAGE_AFTER_FIRST_STEP
        );
        advance_defense_to_retail_frame(&mut game, POST_LEON_DAMAGE_RETAIL_FRAMES[1]);
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            DAMAGE_AFTER_SECOND_STEP
        );
        advance_defense_to_retail_frame(&mut game, THIRD_DAMAGE_RETAIL_FRAME);
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            DAMAGE_AFTER_THIRD_STEP
        );
        advance_defense_to_retail_frame(&mut game, FIFTH_DAMAGE_RETAIL_FRAME);
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            DAMAGE_AFTER_FIFTH_STEP
        );
        advance_defense_to_retail_frame(&mut game, TENTH_DAMAGE_RETAIL_FRAME);
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            DAMAGE_AFTER_TENTH_STEP
        );

        advance_defense_to_retail_frame(&mut game, POST_LEON_PLANET_CANNON_WARNING_RETAIL_FRAME);
        assert_eq!(
            game.state().campaign.corneria_defense.phase,
            CorneriaDefensePhase::PlanetCannonWarning
        );
        advance_defense_to_retail_frame(&mut game, POST_LEON_PLANET_CANNON_CINEMATIC_RETAIL_FRAME);
        assert_eq!(
            game.state().campaign.corneria_defense.phase,
            CorneriaDefensePhase::PlanetCannonCinematic
        );
        advance_defense_to_retail_frame(&mut game, POST_LEON_CARRIER_APPROACH_RETAIL_FRAME);
        assert_eq!(
            game.state().campaign.corneria_defense.phase,
            CorneriaDefensePhase::CarrierApproach
        );
        advance_defense_to_retail_frame(&mut game, FINAL_DAMAGE_RETAIL_FRAME);
        assert_eq!(
            game.state().campaign.corneria_damage_percent,
            CORNERIA_DESTROYED_DAMAGE_PERCENT
        );
        assert_eq!(game.mode(), GameMode::StrategicMap);
        advance_defense_to_retail_frame(&mut game, POST_LEON_CARRIER_WARNING_RETAIL_FRAME);
        assert_eq!(
            game.state().campaign.corneria_defense.phase,
            CorneriaDefensePhase::CarrierWarning
        );
        advance_defense_to_retail_frame(&mut game, POST_LEON_CARRIER_CINEMATIC_RETAIL_FRAME);
        assert_eq!(
            game.state().campaign.corneria_defense.phase,
            CorneriaDefensePhase::CarrierCinematic
        );
        advance_defense_to_retail_frame(&mut game, LAST_PRE_RESULTS_RETAIL_FRAME);
        assert_eq!(game.mode(), GameMode::StrategicMap);
        game.tick(0).unwrap();
        assert_eq!(game.mode(), GameMode::Results);
        assert_eq!(
            game.state().campaign.corneria_defense.phase,
            CorneriaDefensePhase::Complete
        );
    }

    #[test]
    fn mirage_dragon_uses_typed_boss_state_and_matches_the_ninth_return() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.mode = GameMode::StrategicMap;
        game.state.campaign.route_step = CampaignRouteStep::MirageDragon;
        game.state.campaign.elapsed_frames =
            LEON_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
        game.state.campaign.corneria_damage_percent = 0;
        game.state.campaign.corneria_defense = CorneriaDefenseState::default();
        game.state.mission.item_count = LEON_RETURN_ITEM_COUNT;
        game.state.mission.score = LEON_RETURN_SCORE;
        game.state.strategic_map.phase = StrategicMapPhase::Planning;
        game.state.strategic_map.actors = POST_LEON_MAP_ACTORS;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.state.strategic_map.destination = MIRAGE_DRAGON_DESTINATION;

        game.tick(Button::B as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.travel_total_ticks,
            MIRAGE_DRAGON_STRATEGIC_TRAVEL_TICKS
        );
        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }

        assert_eq!(game.state().mission.visit, MissionVisit::MirageDragon);
        assert_eq!(game.mission(), Some(MissionId::MIRAGE_DRAGON));
        assert_eq!(
            game.state().strategic_map.player_map_position,
            MIRAGE_DRAGON_DESTINATION
        );
        let boss_id = game.mirage_dragon.expect("Mirage Dragon remains allocated");
        let boss = game.state().objects.get(boss_id).unwrap();
        assert_eq!(boss.base.shape, ShapeId::MIRAGE_DRAGON_HEAD);
        assert_eq!(boss.base.hit_points, MIRAGE_DRAGON_HEALTH);
        assert_eq!(boss.base.attack_power, MIRAGE_DRAGON_ATTACK_POWER);
        assert!(!boss.base.flags.visible);
        for segment in game.mirage_dragon_body {
            let segment = game
                .state()
                .objects
                .get(segment.expect("all eight body segments remain allocated"))
                .unwrap();
            assert_eq!(segment.base.shape, ShapeId::MIRAGE_DRAGON_BODY);
            assert_eq!(segment.base.hit_points, MIRAGE_DRAGON_SEGMENT_HEALTH);
            assert_eq!(
                segment.base.attack_power,
                MIRAGE_DRAGON_SEGMENT_ATTACK_POWER
            );
            assert!(!segment.base.flags.visible);
        }
        let tail_id = game
            .mirage_dragon_tail
            .expect("Mirage Dragon tail remains allocated");
        assert_eq!(
            game.state().objects.get(tail_id).unwrap().base.shape,
            ShapeId::MIRAGE_DRAGON_TAIL
        );

        let reveal_tick = u32::from(MIRAGE_DRAGON_PLAYER_REVEAL_RETAIL_FRAME)
            .div_ceil(RETAIL_PRESENTATION_FRAMES_PER_TICK);
        while game.state().mode_frame < reveal_tick {
            game.tick(0).unwrap();
        }
        let primary = game.state().mission.primary_player.unwrap();
        let primary = game.state().objects.get(primary).unwrap();
        assert_eq!(primary.base.shape, ShapeId::FOX_FALCO_FLIGHT_CRAFT);
        assert!(primary.base.flags.visible);
        let expected_boss = mirage_dragon::RIVAL_KEYFRAMES
            .iter()
            .find(|keyframe| keyframe.retail_frame == MIRAGE_DRAGON_PLAYER_REVEAL_RETAIL_FRAME)
            .expect("the certified boss path includes the reveal frame");
        let MissionActorPresentation::Present(expected_boss) = expected_boss.presentation else {
            panic!("Mirage Dragon is present at the reveal frame");
        };
        let boss = game.state().objects.get(boss_id).unwrap();
        assert_eq!(boss.base.position, expected_boss.position);
        assert!(boss.base.flags.visible);
        for (segment, keyframes) in game
            .mirage_dragon_body
            .iter()
            .zip(mirage_dragon_segments::BODY_SEGMENT_KEYFRAME_TRACKS)
        {
            let expected = keyframes
                .iter()
                .find(|keyframe| keyframe.retail_frame == MIRAGE_DRAGON_PLAYER_REVEAL_RETAIL_FRAME)
                .expect("every articulated body track includes the reveal frame");
            let MissionActorPresentation::Present(expected) = expected.presentation else {
                panic!("every articulated body segment is present at the reveal frame");
            };
            let segment = game
                .state()
                .objects
                .get(segment.expect("body segment remains allocated"))
                .unwrap();
            assert_eq!(segment.base.position, expected.position);
            assert!(segment.base.flags.visible);
        }
        let expected_tail = mirage_dragon_segments::TAIL_KEYFRAMES
            .iter()
            .find(|keyframe| keyframe.retail_frame == MIRAGE_DRAGON_PLAYER_REVEAL_RETAIL_FRAME)
            .expect("the articulated tail track includes the reveal frame");
        let MissionActorPresentation::Present(expected_tail) = expected_tail.presentation else {
            panic!("the articulated tail is present at the reveal frame");
        };
        let tail = game.state().objects.get(tail_id).unwrap();
        assert_eq!(tail.base.position, expected_tail.position);
        assert!(tail.base.flags.visible);

        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::StrategicPressure
        );
        assert_eq!(
            game.state().campaign.elapsed_frames,
            MIRAGE_DRAGON_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND
        );
        assert_eq!(
            game.state().mission.item_count,
            MIRAGE_DRAGON_RETURN_ITEM_COUNT
        );
        assert_eq!(game.state().mission.score, MIRAGE_DRAGON_RETURN_SCORE);
        assert_eq!(game.state().mission.objects_destroyed, 1);
        for craft in [
            game.state().mission.primary_player,
            game.state().mission.wingmate,
        ]
        .into_iter()
        .flatten()
        {
            assert_eq!(
                game.state().objects.get(craft).unwrap().base.hit_points,
                MIRAGE_DRAGON_RETURN_SHIELD
            );
        }
        assert!(game.mirage_dragon.is_none());
        assert!(game.mirage_dragon_body.iter().all(Option::is_none));
        assert!(game.mirage_dragon_tail.is_none());
    }

    #[test]
    fn recurring_attackers_return_without_creating_a_tenth_sortie() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.mode = GameMode::StrategicMap;
        game.state.campaign.route_step = CampaignRouteStep::StrategicPressure;
        game.state.campaign.elapsed_frames =
            MIRAGE_DRAGON_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
        game.state.campaign.corneria_defense = CorneriaDefenseState::default();
        game.state.mission.score = MIRAGE_DRAGON_RETURN_SCORE;
        game.state.mission.objects_destroyed = 1;
        game.state.strategic_map.phase = StrategicMapPhase::Planning;
        game.state.strategic_map.actors = POST_MIRAGE_MAP_ACTORS;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.state.strategic_map.destination = INITIAL_PLAYER_MAP_POSITION;

        game.tick(Button::Left as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::RecurringAttackers)
        );
        assert_eq!(
            game.state().strategic_map.destination,
            RECURRING_ATTACKERS_DESTINATION
        );
        game.tick(0).unwrap();
        game.tick(Button::B as u16).unwrap();

        assert_eq!(game.state().mission.visit, MissionVisit::RecurringAttackers);
        assert_eq!(game.mission(), Some(MissionId::FIGHTER_INTERCEPT));
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::StrategicPressure
        );
        let fighter_ids = [
            game.pressure_fighter_actors.vanguard,
            game.pressure_fighter_actors.high_guard,
            game.pressure_fighter_actors.flanker,
            game.pressure_fighter_actors.pursuer,
        ];
        for (fighter_kind, fighter) in PressureFighter::ALL.into_iter().zip(fighter_ids) {
            let fighter = game
                .state()
                .objects
                .get(fighter.expect("all four pressure fighters are typed fields"))
                .unwrap();
            assert_eq!(fighter.base.shape, fighter_kind.shape());
            assert_eq!(fighter.base.hit_points, PRESSURE_FIGHTER_HEALTH);
            assert_eq!(fighter.base.attack_power, PRESSURE_FIGHTER_ATTACK_POWER);
        }

        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert_eq!(game.mode(), GameMode::StrategicMap);
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::StrategicPressure
        );
        assert_eq!(game.state().mission.score, MIRAGE_DRAGON_RETURN_SCORE);
        assert_eq!(game.state().mission.objects_destroyed, 1);
        assert_eq!(
            game.state().campaign.elapsed_frames,
            (MIRAGE_DRAGON_RETURN_DISPLAY_SECONDS + RECURRING_ATTACKERS_ELAPSED_DISPLAY_SECONDS)
                * CAMPAIGN_TICKS_PER_DISPLAY_SECOND
        );
        assert_eq!(game.state().strategic_map.actors, POST_MIRAGE_MAP_ACTORS);
        assert_eq!(
            game.state().strategic_map.player_map_position,
            INITIAL_PLAYER_MAP_POSITION
        );
        assert_eq!(game.state().strategic_map.selected_encounter, None);
        assert!(game
            .pressure_fighter_actors
            .slots_mut()
            .into_iter()
            .all(|slot| slot.is_none()));
        assert!(game.pressure_fighter_projectiles.is_empty());
    }

    #[test]
    fn recurring_leon_pressure_preserves_score_and_completion_count() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.mode = GameMode::StrategicMap;
        game.state.campaign.route_step = CampaignRouteStep::StrategicPressure;
        game.state.campaign.elapsed_frames =
            MIRAGE_DRAGON_RETURN_DISPLAY_SECONDS * CAMPAIGN_TICKS_PER_DISPLAY_SECOND;
        game.state.campaign.corneria_defense = CorneriaDefenseState::default();
        game.state.mission.item_count = MIRAGE_DRAGON_RETURN_ITEM_COUNT;
        game.state.mission.score = MIRAGE_DRAGON_RETURN_SCORE;
        game.state.mission.objects_destroyed = 1;
        game.state.strategic_map.phase = StrategicMapPhase::Planning;
        game.state.strategic_map.actors = POST_MIRAGE_MAP_ACTORS;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.state.strategic_map.destination = INITIAL_PLAYER_MAP_POSITION;

        game.tick(Button::Right as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::LeonPressure)
        );
        assert_eq!(
            game.state().strategic_map.destination,
            LEON_PRESSURE_DESTINATION
        );
        game.tick(0).unwrap();
        game.tick(Button::B as u16).unwrap();

        assert_eq!(game.state().mission.visit, MissionVisit::LeonPressure);
        assert_eq!(game.mission(), Some(MissionId::LEON_DUEL));
        let rival_id = game.leon_rival.expect("Leon pressure rival is allocated");
        let rival = game.state().objects.get(rival_id).unwrap();
        assert_eq!(rival.base.shape, ShapeId::LEON_CRAFT);
        assert_eq!(rival.base.hit_points, LEON_HEALTH);
        assert_eq!(rival.base.attack_power, LEON_ATTACK_POWER);

        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert_eq!(game.mode(), GameMode::StrategicMap);
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::StrategicPressure
        );
        assert_eq!(game.state().mission.score, MIRAGE_DRAGON_RETURN_SCORE);
        assert_eq!(game.state().mission.objects_destroyed, 1);
        assert_eq!(
            game.state().mission.item_count,
            MIRAGE_DRAGON_RETURN_ITEM_COUNT
        );
        assert_eq!(
            game.state().campaign.elapsed_frames,
            (MIRAGE_DRAGON_RETURN_DISPLAY_SECONDS + LEON_PRESSURE_ELAPSED_DISPLAY_SECONDS)
                * CAMPAIGN_TICKS_PER_DISPLAY_SECOND
        );
        assert_eq!(game.state().strategic_map.actors, POST_MIRAGE_MAP_ACTORS);
        assert!(game.leon_rival.is_none());
        assert!(game.leon_pressure_projectiles.is_empty());
    }

    #[test]
    fn certified_campaign_route_reaches_the_end_screen_without_state_injection() {
        const MAX_MISSION_TICKS: usize = 5_000;
        const MAX_STRATEGIC_TRAVEL_TICKS: usize = 1_000;

        fn complete_current_mission(game: &mut Game) {
            for _ in 0..MAX_MISSION_TICKS {
                if game.mode() != GameMode::Mission {
                    return;
                }
                game.tick(0).unwrap();
            }
            panic!("mission exceeded the certified completion budget");
        }

        fn launch_recommended_visit(game: &mut Game) {
            assert_eq!(game.mode(), GameMode::StrategicMap);
            game.state.strategic_map.destination = game.state.strategic_map.recommended_destination;
            game.tick(Button::B as u16).unwrap();
            for _ in 0..MAX_STRATEGIC_TRAVEL_TICKS {
                if game.mode() != GameMode::StrategicMap {
                    break;
                }
                game.tick(0).unwrap();
            }
            assert_eq!(game.mode(), GameMode::Mission);
            complete_current_mission(game);
        }

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        complete_current_mission(&mut game);
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::Reengagement
        );

        while game.state().campaign.route_step != CampaignRouteStep::StrategicPressure {
            launch_recommended_visit(&mut game);
        }
        assert_eq!(game.mode(), GameMode::StrategicMap);
        assert_eq!(
            game.state().campaign.objectives.eladard,
            PlanetObjectiveStatus::Rescued
        );
        assert_eq!(
            game.state().campaign.objectives.first_carrier,
            CarrierObjectiveStatus::Destroyed
        );
        assert_eq!(
            game.state().campaign.objectives.live_attackers,
            StrategicThreatCount::new(1)
        );

        press(&mut game, Button::Down);
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::TitaniaBase)
        );
        game.tick(Button::B as u16).unwrap();
        complete_current_mission(&mut game);
        assert_eq!(
            game.state().campaign.objectives.titania,
            PlanetObjectiveStatus::Rescued
        );

        press(&mut game, Button::Down);
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::SecondBattleCarrier)
        );
        game.tick(Button::B as u16).unwrap();
        complete_current_mission(&mut game);
        assert!(game.state().campaign.objectives.major_objectives_complete());

        press(&mut game, Button::Up);
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::FinalPursuer)
        );
        game.tick(Button::B as u16).unwrap();
        complete_current_mission(&mut game);
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::WolfBlockade
        );

        game.tick(0).unwrap();
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::WolfBlockade)
        );
        game.tick(Button::B as u16).unwrap();
        complete_current_mission(&mut game);
        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::AstropolisAssault
        );

        game.tick(0).unwrap();
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::AstropolisAssault)
        );
        game.tick(Button::B as u16).unwrap();
        while game.state().mission.astropolis.phase != AstropolisPhase::InteriorCorridor {
            game.tick(0).unwrap();
        }

        let retail_frame = (game.state().mode_frame * RETAIL_PRESENTATION_FRAMES_PER_TICK)
            .min(u32::from(u16::MAX)) as u16;
        let mission = &mut game.state.mission.astropolis;
        mission.enter_security_room(retail_frame);
        assert!(mission
            .damage_security_turret(astropolis_assault::SECURITY_TURRET_DURABILITY, retail_frame,));
        assert!(mission.choose_branch(super::super::state::AstropolisBranch::Left));
        assert!(mission.enter_core_chamber(retail_frame));
        for index in 0..astropolis_assault::CORE_SPIKE_COUNT {
            mission.damage_core_spike(
                index,
                astropolis_assault::CORE_SPIKE_DURABILITY,
                retail_frame,
            );
        }
        assert!(
            mission.damage_exposed_cube(astropolis_assault::EXPOSED_CUBE_DURABILITY, retail_frame,)
        );
        mission.damage_eye(
            super::super::state::AstropolisEye::Left,
            astropolis_assault::MASK_EYE_DURABILITY,
            retail_frame,
        );
        assert!(mission.damage_eye(
            super::super::state::AstropolisEye::Right,
            astropolis_assault::MASK_EYE_DURABILITY,
            retail_frame,
        ));
        assert!(mission.damage_final_core(astropolis_assault::FINAL_CORE_DURABILITY, retail_frame,));

        complete_current_mission(&mut game);
        assert_eq!(game.mode(), GameMode::Ending);
        for _ in 0..MAX_MISSION_TICKS {
            if game.state().ending.phase == EndingPhase::EndScreen {
                break;
            }
            game.tick(0).unwrap();
        }
        assert_eq!(game.state().ending.phase, EndingPhase::EndScreen);
        assert_eq!(
            game.state().campaign.objectives.astropolis,
            AstropolisStatus::Assaulted
        );
    }

    #[test]
    fn late_major_objectives_are_distinct_and_unlock_the_final_route() {
        assert_eq!(BATTLE_CARRIER_REQUIRED_VISITS, 2);
        assert_eq!(MissionId::TITANIA_BASE.catalog_index(), 1);
        assert_eq!(MissionId::ELADARD_BASE.catalog_index(), 3);
        assert_eq!(MissionId::BATTLE_CARRIER.catalog_index(), 8);

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.mode = GameMode::StrategicMap;
        game.state.campaign.route_step = CampaignRouteStep::StrategicPressure;
        game.state.campaign.corneria_defense = CorneriaDefenseState::default();
        game.state.campaign.objectives.eladard = PlanetObjectiveStatus::Rescued;
        game.state.campaign.objectives.first_carrier = CarrierObjectiveStatus::Destroyed;
        game.state.campaign.objectives.missiles = StrategicThreatCount::NONE;
        game.state.campaign.objectives.live_attackers = StrategicThreatCount::new(1);
        game.state.mission.item_count = MIRAGE_DRAGON_RETURN_ITEM_COUNT;
        game.state.mission.score = MIRAGE_DRAGON_RETURN_SCORE;
        game.state.strategic_map.phase = StrategicMapPhase::Planning;
        game.state.strategic_map.actors = POST_MIRAGE_MAP_ACTORS;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.state.strategic_map.destination = INITIAL_PLAYER_MAP_POSITION;

        game.tick(Button::Down as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::TitaniaBase)
        );
        assert_eq!(
            game.state().strategic_map.destination,
            TITANIA_BASE_DESTINATION
        );
        game.tick(0).unwrap();
        game.tick(Button::B as u16).unwrap();

        assert_eq!(game.state().mission.visit, MissionVisit::TitaniaBase);
        assert_eq!(game.mission(), Some(MissionId::TITANIA_BASE));
        assert_eq!(
            game.state().mission.titania,
            TitaniaMissionState {
                phase: TitaniaPhase::SurfaceApproach,
                phase_started_retail_frame: 0,
                surface_switches: [TitaniaSurfaceSwitchStatus::Active;
                    TITANIA_SURFACE_SWITCH_COUNT],
                reactor: TitaniaReactorStatus::Shielded,
            }
        );

        let advance_to = |game: &mut Game, retail_frame: u16| {
            let target_tick = u32::from(retail_frame).div_ceil(RETAIL_PRESENTATION_FRAMES_PER_TICK);
            while game.state().mode_frame < target_tick {
                game.tick(0).unwrap();
            }
        };
        advance_to(&mut game, TITANIA_BASE_ENTRY_RETAIL_FRAME);
        assert_eq!(game.state().mission.titania.phase, TitaniaPhase::BaseEntry);
        assert_eq!(
            game.state().mission.titania.surface_switches,
            [TitaniaSurfaceSwitchStatus::Disabled; TITANIA_SURFACE_SWITCH_COUNT]
        );
        advance_to(&mut game, TITANIA_REACTOR_RETAIL_FRAME);
        assert_eq!(game.state().mission.titania.phase, TitaniaPhase::Reactor);
        assert_eq!(
            game.state().mission.titania.reactor,
            TitaniaReactorStatus::Exposed
        );
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert_eq!(
            game.state().campaign.objectives.titania,
            PlanetObjectiveStatus::Rescued
        );
        assert_eq!(
            game.state().campaign.objectives.second_carrier,
            CarrierObjectiveStatus::Operational
        );
        assert_eq!(
            game.state().campaign.objectives.first_carrier,
            CarrierObjectiveStatus::Destroyed
        );
        assert_eq!(
            game.state().strategic_map.recommended_destination,
            SECOND_BATTLE_CARRIER_DESTINATION
        );

        game.tick(Button::Down as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::SecondBattleCarrier)
        );
        game.tick(0).unwrap();
        game.tick(Button::B as u16).unwrap();
        assert_eq!(
            game.state().mission.visit,
            MissionVisit::SecondBattleCarrier
        );
        assert_eq!(game.mission(), Some(MissionId::BATTLE_CARRIER));
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert!(game.state().campaign.objectives.major_objectives_complete());
        assert_eq!(
            game.state().campaign.objectives.first_carrier,
            CarrierObjectiveStatus::Destroyed
        );
        assert_eq!(
            game.state().campaign.objectives.second_carrier,
            CarrierObjectiveStatus::Destroyed
        );
        assert_eq!(
            game.state().campaign.objectives.wolf_blockade,
            WolfBlockadeStatus::Unavailable
        );
        assert_eq!(
            game.state().strategic_map.recommended_destination,
            FINAL_PURSUER_DESTINATION
        );

        game.tick(Button::Up as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::FinalPursuer)
        );
        game.tick(0).unwrap();
        game.tick(Button::B as u16).unwrap();
        assert_eq!(game.state().mission.visit, MissionVisit::FinalPursuer);
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::WolfBlockade
        );
        assert_eq!(
            game.state().campaign.objectives.live_attackers,
            StrategicThreatCount::NONE
        );
        assert_eq!(
            game.state().campaign.objectives.wolf_blockade,
            WolfBlockadeStatus::Active
        );
    }

    #[test]
    fn final_pursuer_blockade_and_astropolis_are_distinct_typed_visits() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.mode = GameMode::StrategicMap;
        game.state.campaign.route_step = CampaignRouteStep::StrategicPressure;
        game.state.campaign.corneria_defense = CorneriaDefenseState::default();
        game.state.campaign.objectives = super::super::state::CampaignObjectives {
            eladard: PlanetObjectiveStatus::Rescued,
            titania: PlanetObjectiveStatus::Rescued,
            first_carrier: CarrierObjectiveStatus::Destroyed,
            second_carrier: CarrierObjectiveStatus::Destroyed,
            missiles: StrategicThreatCount::NONE,
            live_attackers: StrategicThreatCount::new(1),
            ..super::super::state::CampaignObjectives::default()
        };
        game.state.strategic_map.phase = StrategicMapPhase::Planning;
        game.state.strategic_map.actors = POST_MIRAGE_MAP_ACTORS;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.state.strategic_map.destination = INITIAL_PLAYER_MAP_POSITION;

        game.tick(Button::Up as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::FinalPursuer)
        );
        assert_eq!(
            game.state().strategic_map.destination,
            FINAL_PURSUER_DESTINATION
        );
        game.tick(0).unwrap();
        game.tick(Button::B as u16).unwrap();

        assert_eq!(game.state().mission.visit, MissionVisit::FinalPursuer);
        assert_eq!(game.mission(), Some(MissionId::FINAL_PURSUER));
        assert_eq!(
            game.state()
                .objects
                .get(game.final_rival.expect("final pursuer is allocated"))
                .unwrap()
                .base
                .shape,
            ShapeId::FINAL_PURSUER_CRAFT
        );
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::WolfBlockade
        );
        assert_eq!(
            game.state().campaign.objectives.live_attackers,
            StrategicThreatCount::NONE
        );
        assert_eq!(
            game.state().campaign.objectives.wolf_blockade,
            WolfBlockadeStatus::Active
        );
        assert_eq!(
            game.state().campaign.objectives.astropolis,
            AstropolisStatus::BlockedByWolf
        );

        game.tick(0).unwrap();
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::WolfBlockade)
        );
        game.tick(Button::B as u16).unwrap();
        assert_eq!(game.state().mission.visit, MissionVisit::WolfBlockade);
        assert_eq!(game.mission(), Some(MissionId::WOLF_BLOCKADE));
        assert_eq!(
            game.state()
                .objects
                .get(game.final_rival.expect("Wolf blockade is allocated"))
                .unwrap()
                .base
                .shape,
            ShapeId::WOLF_BLOCKADE_CRAFT
        );
        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }

        assert_eq!(
            game.state().campaign.route_step,
            CampaignRouteStep::AstropolisAssault
        );
        assert_eq!(
            game.state().campaign.objectives.wolf_blockade,
            WolfBlockadeStatus::Defeated
        );
        assert_eq!(
            game.state().campaign.objectives.astropolis,
            AstropolisStatus::Vulnerable
        );

        game.tick(0).unwrap();
        assert_eq!(
            game.state().strategic_map.selected_encounter,
            Some(StrategicEncounter::AstropolisAssault)
        );
        game.tick(Button::B as u16).unwrap();
        assert_eq!(game.state().mission.visit, MissionVisit::AstropolisAssault);
        assert_eq!(game.mission(), Some(MissionId::ASTROPOLIS));

        while game.state().mode_frame * RETAIL_PRESENTATION_FRAMES_PER_TICK
            < u32::from(ASTROPOLIS_INTERIOR_CORRIDOR_RETAIL_FRAME)
        {
            game.tick(0).unwrap();
        }
        assert_eq!(
            game.state().mission.astropolis.phase,
            AstropolisPhase::InteriorCorridor
        );
        assert_eq!(game.state().mission.phase, MissionPhase::Active);
        assert_eq!(
            game.state().mission.astropolis.phase_started_retail_frame,
            ASTROPOLIS_INTERIOR_CORRIDOR_RETAIL_FRAME
        );
    }

    #[test]
    fn astropolis_core_destruction_hands_off_to_the_typed_ending() {
        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.mode = GameMode::Mission;
        game.state.mode_frame = 0;
        game.state.mission.visit = MissionVisit::AstropolisAssault;
        game.state.mission.mission = Some(MissionId::ASTROPOLIS);
        game.state.mission.phase = MissionPhase::Active;
        game.state.mission.astropolis.phase = AstropolisPhase::CoreDestruction;
        game.state.mission.astropolis.phase_started_retail_frame = 0;

        while game.mode() == GameMode::Mission {
            game.tick(0).unwrap();
        }
        assert_eq!(game.mode(), GameMode::Ending);
        assert_eq!(
            game.state().campaign.objectives.astropolis,
            AstropolisStatus::Assaulted
        );
        assert_eq!(
            game.state().mission.astropolis.phase,
            AstropolisPhase::Escape
        );
        assert_eq!(game.state().ending.phase, EndingPhase::EscapeFlash);

        while game.state().ending.retail_frame
            < astropolis_assault::ENDING_CREDITS_SAMPLE_RETAIL_FRAME
        {
            game.tick(0).unwrap();
        }
        assert_eq!(game.state().ending.phase, EndingPhase::Credits);

        while game.state().ending.retail_frame
            < astropolis_assault::ENDING_END_SCREEN_SAMPLE_RETAIL_FRAME
        {
            game.tick(0).unwrap();
        }
        assert_eq!(game.state().ending.phase, EndingPhase::EndScreen);
    }

    #[test]
    fn missile_interception_travel_uses_typed_map_positions_and_damage() {
        const HALF_TRAVEL_TICKS: u16 = 234;

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        game.state.mode = GameMode::StrategicMap;
        game.state.campaign.route_step = CampaignRouteStep::MissileInterception;
        game.state.campaign.corneria_damage_percent = SECOND_RETURN_CORNERIA_DAMAGE_PERCENT;
        game.state.strategic_map.phase = StrategicMapPhase::Planning;
        game.state.strategic_map.player_map_position = INITIAL_PLAYER_MAP_POSITION;
        game.state.strategic_map.destination = MISSILE_INTERCEPTION_DESTINATION;

        game.tick(Button::B as u16).unwrap();
        assert_eq!(
            game.state().strategic_map.travel_total_ticks,
            MISSILE_INTERCEPTION_STRATEGIC_TRAVEL_TICKS
        );
        for _ in 0..HALF_TRAVEL_TICKS {
            game.tick(0).unwrap();
        }
        assert_eq!(
            game.state().strategic_map.player_map_position,
            MapPoint { x: 105, y: 120 }
        );
        assert_eq!(game.state().campaign.corneria_damage_percent, 41);

        while game.mode() == GameMode::StrategicMap {
            game.tick(0).unwrap();
        }
        assert_eq!(
            game.state().strategic_map.player_map_position,
            MISSILE_INTERCEPTION_DESTINATION
        );
        assert_eq!(game.mission(), Some(MissionId::MISSILE_INTERCEPTION));
    }

    #[test]
    fn opening_projectile_slots_are_split_into_real_lifetimes() {
        const QUIET_RETAIL_FRAME: u32 = 1600;

        let mut game = Game::new();
        game.begin_opening_sortie().unwrap();
        while game.state().mode_frame < QUIET_RETAIL_FRAME / RETAIL_PRESENTATION_FRAMES_PER_TICK {
            game.tick(Button::B as u16).unwrap();
        }

        assert!(game.mission_projectiles.is_empty());
        assert!(game
            .state()
            .objects
            .active_objects()
            .all(|(_, object)| { object.base.behavior != Behavior::MissionScriptedProjectile }));
    }

    #[test]
    fn player_weapon_damage_precedes_the_retail_fighter_death_timeline() {
        let mut game = Game::new();
        game.state.mode = GameMode::Mission;
        game.state.mission.phase = MissionPhase::Active;
        game.spawn_mission_entry_flyby().unwrap();
        game.activate_mission_encounter_targets();

        let fighter_id = game.mission_entry_flyby[MissionEncounterActor::UpperFighter.index()]
            .expect("opening fighter is allocated");
        let collision_position = Vector3 {
            x: -4_212,
            y: -3_757,
            z: -912,
        };
        {
            let fighter = game.state.objects.get_mut(fighter_id).unwrap();
            fighter.base.position = collision_position;
            fighter.base.velocity = Vector3::default();
            assert_eq!(fighter.base.hit_points, MISSION_ENCOUNTER_HEALTH);
            assert_eq!(fighter.base.attack_power, MISSION_ENCOUNTER_ATTACK_POWER);
        }

        let mut laser = Object::new(
            ObjectKind::Projectile,
            ShapeId::PLAYER_RAPID_LASER_LAUNCH,
            Behavior::Projectile,
        );
        laser.base.position = collision_position;
        laser.base.hit_points = PLAYER_PROJECTILE_DURABILITY;
        laser.base.attack_power = PLAYER_RAPID_LASER_ATTACK_POWER;
        laser.base.weapon = WeaponKind::Laser;
        laser.base.collision_class = CollisionClass::PlayerWeapon;
        let laser_id = game.state.objects.allocate(laser).unwrap();

        game.resolve_mission_collisions();
        let fighter = game.state.objects.get(fighter_id).unwrap();
        assert!(fighter.base.flags.collided);
        assert!(!fighter.base.flags.collision_disabled);
        assert_eq!(
            fighter.base.hit_points,
            MISSION_ENCOUNTER_HEALTH - PLAYER_RAPID_LASER_ATTACK_POWER
        );
        assert_eq!(fighter.base.explosion_timer, 0);
        let laser = game.state.objects.get(laser_id).unwrap();
        assert!(laser.base.flags.collided);
        assert!(!laser.base.flags.visible);
        assert!(laser.base.flags.remove_after_tick);

        game.update_objects();
        let fighter = game.state.objects.get(fighter_id).unwrap();
        assert!(!fighter.base.flags.collided);

        game.state
            .objects
            .get_mut(fighter_id)
            .unwrap()
            .base
            .hit_points = PLAYER_CHARGED_LASER_ATTACK_POWER;
        let mut charged_laser = Object::new(
            ObjectKind::Projectile,
            ShapeId::PLAYER_CHARGED_LASER_ACTIVE,
            Behavior::Projectile,
        );
        charged_laser.base.position = collision_position;
        charged_laser.base.hit_points = PLAYER_PROJECTILE_DURABILITY;
        charged_laser.base.attack_power = PLAYER_CHARGED_LASER_ATTACK_POWER;
        charged_laser.base.weapon = WeaponKind::ChargedLaser;
        charged_laser.base.collision_class = CollisionClass::PlayerWeapon;
        game.state.objects.allocate(charged_laser).unwrap();

        game.resolve_mission_collisions();
        let fighter = game.state.objects.get(fighter_id).unwrap();
        assert!(fighter.base.flags.collided);
        assert!(fighter.base.flags.collision_disabled);
        assert_eq!(fighter.base.explosion_timer, ENEMY_DESTRUCTION_TICKS);

        game.update_objects();
        for _ in 0..2 {
            game.update_objects();
        }
        let fighter = game.state.objects.get(fighter_id).unwrap();
        assert_eq!(fighter.base.hit_points, PLAYER_CHARGED_LASER_ATTACK_POWER);
        assert_eq!(
            game.state.mission.score,
            u32::from(PLAYER_CHARGED_LASER_ATTACK_POWER)
        );

        for _ in 0..3 {
            game.update_objects();
        }
        let fighter = game.state.objects.get(fighter_id).unwrap();
        assert_eq!(fighter.base.hit_points, 0);
        assert!(fighter.base.flags.exploding);

        for _ in 0..3 {
            game.update_objects();
        }
        assert!(game.state.objects.get(fighter_id).is_none());
        assert_eq!(game.state.mission.objects_destroyed, 1);
        assert_eq!(
            game.mission_entry_flyby[MissionEncounterActor::UpperFighter.index()],
            None
        );
    }

    #[test]
    fn player_pitch_control_matches_the_retail_response_curve() {
        const FIRST_DOWN_SAMPLES: [i16; 6] = [-1_280, -2_400, -3_380, -4_237, -4_987, -5_643];

        let mut accumulator = 0;
        for expected in FIRST_DOWN_SAMPLES {
            accumulator = chase_proportional(
                accumulator,
                -PLAYER_PITCH_TARGET,
                PLAYER_CONTROL_RESPONSE_SHIFT,
            );
            assert_eq!(accumulator, expected);
        }
        assert_eq!(visible_pitch_from_lean(29), Angle::from_units(219));
        assert_eq!(visible_pitch_from_lean(-30), Angle::from_units(37));
        assert_eq!(visible_pitch_from_lean(15), Angle::from_units(237));
    }

    #[test]
    fn title_sound_mode_is_typed_and_record_screen_returns_to_title() {
        use super::super::state::AudioOutput;

        let mut game = Game::new();
        press(&mut game, Button::Start);
        press(&mut game, Button::Up);
        assert_eq!(game.state().title.menu_item, TitleMenuItem::SoundMode);
        press(&mut game, Button::Right);
        assert_eq!(game.state().title.audio_output, AudioOutput::Mono);
        press(&mut game, Button::Down);
        press(&mut game, Button::Down);
        assert_eq!(game.state().title.menu_item, TitleMenuItem::Records);
        press(&mut game, Button::B);
        assert_eq!(game.mode(), GameMode::Records);
        press(&mut game, Button::Y);
        assert_eq!(game.mode(), GameMode::Title);
    }

    #[test]
    fn intro_flyby_is_a_typed_object_and_builds_a_native_render_item() {
        let mut game = Game::new();
        let ticks = BOOT_INTRO_TICKS + ARGONAUT_LOGO_TICKS + NINTENDO_LOGO_TICKS;
        for _ in 0..ticks {
            game.tick(0).unwrap();
        }
        assert_eq!(game.mode(), GameMode::Intro(IntroPhase::Formation));
        let expected_scene_objects = TITLE_CRAFT_POSES.len() + TITLE_EFFECT_POSES.len();
        assert_eq!(game.state().objects.len(), expected_scene_objects);
        assert_eq!(game.render_objects().len(), expected_scene_objects);
        assert_eq!(
            game.render_objects()
                .iter()
                .filter(|object| object.shape == ShapeId::TITLE_CRAFT)
                .count(),
            TITLE_CRAFT_POSES.len()
        );
        assert!(game
            .render_objects()
            .iter()
            .any(|object| object.shape == ShapeId::TITLE_FORMATION_EFFECT));
        assert_eq!(game.camera().position.z, TITLE_CAMERA_START_Z);
    }

    #[test]
    fn formation_motion_matches_the_retail_global_frame_checkpoint() {
        const UPDATES_TO_CHECKPOINT: u32 = 45;
        const EXPECTED_POSITIONS: [Vector3; 3] = [
            Vector3 {
                x: -392,
                y: 16,
                z: 128,
            },
            Vector3 {
                x: 254,
                y: -130,
                z: -248,
            },
            Vector3 {
                x: 58,
                y: 116,
                z: 536,
            },
        ];
        const EXPECTED_CAMERA_DEPTH: i16 = 1_120;

        let mut game = Game::new();
        let intro_ticks = BOOT_INTRO_TICKS + ARGONAUT_LOGO_TICKS + NINTENDO_LOGO_TICKS;
        for _ in 0..intro_ticks + UPDATES_TO_CHECKPOINT {
            game.tick(0).unwrap();
        }

        let actual: Vec<_> = game
            .state()
            .objects
            .active_objects()
            .filter(|(_, object)| object.base.shape == ShapeId::TITLE_CRAFT)
            .map(|(_, object)| object.base.position)
            .collect();
        assert_eq!(
            actual,
            EXPECTED_POSITIONS.into_iter().rev().collect::<Vec<_>>()
        );
        assert_eq!(game.camera().position.z, EXPECTED_CAMERA_DEPTH);
    }

    #[test]
    fn strategic_map_cardinal_placement_matches_the_certified_result() {
        const APPROACH_DISTANCE: u16 = 18_000;
        const TARGET_Z: i16 = 1_000;
        const FORMATION_YAW: u8 = 48;
        const EXPECTED_PLAYER_Z: i16 = -17_859;

        let mut game = Game::new();
        let player = game
            .state
            .objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::FOX_FALCO_FLIGHT_CRAFT,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let target = game
            .state
            .objects
            .allocate(Object::new(
                ObjectKind::Scenery,
                ShapeId::FOX_FALCO_FLIGHT_CRAFT,
                Behavior::Effect,
            ))
            .unwrap();
        game.state.strategic_map.primary_player = Some(player);
        game.state.strategic_map.selected_target = Some(target);
        game.state.strategic_map.target_position.z = TARGET_Z;
        game.state.strategic_map.formation_yaw =
            super::super::object::Angle::from_units(FORMATION_YAW);

        game.position_strategic_map_player(APPROACH_DISTANCE);

        let player = game.state.objects.get(player).unwrap();
        let target = game.state.objects.get(target).unwrap();
        assert_eq!(
            player.base.position,
            Vector3 {
                x: 0,
                y: 0,
                z: EXPECTED_PLAYER_Z
            }
        );
        assert_eq!(
            target.base.position,
            Vector3 {
                x: 0,
                y: 0,
                z: TARGET_Z
            }
        );
        assert_eq!(player.base.yaw.units(), FORMATION_YAW);
        assert_eq!(game.state.strategic_map.marker_phase, 1);
    }
}
