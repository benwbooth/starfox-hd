//! End-to-end proof that a real retail routine and the flat native port can be
//! compared through the shared, storage-independent semantic trace format.

use sf_core::{pad, sf1_controls::BriefingPhase, sf1_planets::PlanetSequencePhase};
use sf_difftest::{
    compare_scenario, first_divergence, semantic_frame_sha256, CaptureChannel, EvidenceProducer,
    NonStrictEvidence, ScenarioClock, ScenarioEvidence, ScenarioInputRun, ScenarioManifest,
    SemanticEvent, SemanticFrame, SemanticObject, EVIDENCE_SCHEMA_VERSION, SCENARIO_SCHEMA_VERSION,
};
use sf_game::alien::{ExplosionSize, ObjectVisualKind, ASF2_COLLDISABLE, ASF3_NOHITAFFECT};
use sf_game::camera::{VIEWTYPE_FPOS, VIEWTYPE_TOOBJ};
use sf_game::shell::{GameState, GameplayEntryPhase, Shell};
use sf_oracle::{
    call, load_retail_rom, snapshot_objects, Entry, RetailMachine, SnesBus, AL_HP, AL_PTR, AL_ROTX,
    AL_ROTY, AL_ROTZ, AL_SBYTE1, AL_SBYTE3, AL_SFLAGS2, AL_SFLAGS3, AL_SWORD2, AL_VEL, AL_VX,
    AL_VY, AL_VZ, RETAIL_BRIEFING_CHOICE, RETAIL_CURRENTBG, RETAIL_CURRENT_PLANET, RETAIL_DOSTRATS,
    RETAIL_DOSTRATS_COMPLETE, RETAIL_GAMEFRAME, RETAIL_LASTPLAYZ, RETAIL_LASTZCHANGE,
    RETAIL_MAPCNT, RETAIL_PEPPER_CHARACTERS, RETAIL_PLANET_BRIEFING_PREP_ENTRY,
    RETAIL_PLANET_CENTER_ENTRY, RETAIL_PLANET_DISMISS_ENTRY, RETAIL_PLANET_EXIT_FADE_ENTRY,
    RETAIL_PLANET_GAME_START_ENTRY, RETAIL_PLANET_INTERRUPT, RETAIL_PLANET_ISOLATION_ENTRY,
    RETAIL_PLANET_MAP_FADE_ENTRY, RETAIL_PLANET_MESSAGE_ENTRY, RETAIL_PLANET_NAME_ENTRY,
    RETAIL_PLANET_SHIP_FLASH, RETAIL_PLANET_SHIP_FLASH_ENTRY, RETAIL_PLANET_STAGE,
    RETAIL_PLANET_ZOOM_ENTRY, RETAIL_PLAYPT, RETAIL_POOL, RETAIL_PSHIPFLAGS, RETAIL_PSHIPFLAGS2,
    RETAIL_PSHIPFLAGS3, RETAIL_PSTRATFLAGS, RETAIL_PVIEWVELZ, RETAIL_RAND, RETAIL_SHAPES,
    RETAIL_STRAIGHT_STRAT, RETAIL_VIEW_POSITION_X, RETAIL_VIEW_POSITION_Y, RETAIL_VIEW_POSITION_Z,
    RETAIL_WHICH_ROUTE,
};
use sf_strat::common::StratRam;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const FRAME_COUNT: u64 = 30;
const INITIAL_POSITION_X: i16 = 1_000;
const INITIAL_POSITION_Y: i16 = 500;
const INITIAL_POSITION_Z: i16 = 8_000;
const VELOCITY_X: i16 = 300;
const VELOCITY_Y: i16 = -120;
const VELOCITY_Z: i16 = -50;
const VIEW_FORWARD_VELOCITY: i16 = -200;
const NO_INPUT: u32 = 0;
const PRIMARY_ENEMY: &str = "primary-enemy";
const RETAIL_ROM_SHA256: &str = "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548";
const FRONT_END_SCENARIO_ID: &str = "sf1-front-end-corneria-opening";
const ATTACK_CARRIER_SCENARIO_ID: &str = "sf1-front-end-corneria-attack-carrier";
const ATTACK_CARRIER_TRACE_ENV: &str = "SF1_CORNERIA_ATTACK_CARRIER_TRACE";
/// Exclusive strict boundary for the currently certified Corneria scenario.
const CORNERIA_SCENARIO_TICKS: u32 = 1_877;
const ATTACK_CARRIER_SCENARIO_TICKS: u32 = 3_075;
const CERTIFIED_CORNERIA_LEVEL_FRAME: u16 = 983;
const VIDEO_FRAMES_PER_NATIVE_TICK: u32 = 3;
const COMPLETED_FRAME_ALIGNMENT_TICK: u32 = PLANET_DISMISS_END_TICK;
/// The retail front end produces two video-only ticks while handing control to
/// the first level. A standalone replay must preserve those logical pauses.
const NATIVE_REPLAY_LEVEL_UPDATE_PAUSE_TICKS: [u32; 2] = [895, 898];
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const CORNERIA_AUDIO_UPLOAD_TICK: u32 = 1_080;
const MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD: u32 = 240;
const WORK_RAM: u32 = 0x7E_0000;
const RETAIL_EFFECTIVE_VIEW_YAW: u32 = 0x1635;
const RETAIL_PLAYER_VIEW_X: u32 = 0x14F6;
const RETAIL_PLAYER_VIEW_Y: u32 = 0x14F8;
const RETAIL_PLAYER_VIEW_Z: u32 = 0x14FA;
const RETAIL_PLAYER_FLY_MODE: u32 = 0x14DA;
const RETAIL_PLAYER_DEPTH_TILT: u32 = 0x1507;
const RETAIL_VIEW_FLOAT_X: u32 = 0x14E6;
const RETAIL_VIEW_FLOAT_Y: u32 = 0x14E8;
const RETAIL_VIEW_KIND: u32 = 0x15CA;
const RETAIL_VIEW_PITCH: u32 = 0x18C5;
const RETAIL_VIEW_SHAKE_X: u32 = 0x1595;
const RETAIL_VIEW_YAW: u32 = 0x18C7;
const RETAIL_VIEW_DISTANCE: u32 = 0x18CB;
const RETAIL_OBJECT_LIFETIME_OFFSET: u32 = 0x0A;
const RETAIL_OBJECT_DELAY_OFFSET: u32 = 0x22;
const RETAIL_OBJECT_HIT_FLAGS_OFFSET: u32 = 0x35;
const RETAIL_SHAPE_COORDINATE_SHIFT_OFFSET: u32 = 7;
const RETAIL_SHAPE_VISUAL_EXTENT_OFFSET: u32 = 16;
const RETAIL_ATTRACT_BACKGROUND: u16 = 243;
const RETAIL_TITLE_BACKGROUND: u16 = 249;
const RETAIL_BRIEFING_BACKGROUND: u16 = 255;
const BRIEFING_CONTROL_DISABLED_MASK: u8 = 0x60;
const INITIAL_ROUTE: u8 = 1;
const ROUTE_PREVIEW_STAGE: u8 = 10;
const HIDDEN_CURRENT_PLANET: i8 = -2;
const FRONT_END_CONFIRM_CADENCE_TICKS: u32 = 60;
const FRONT_END_CONFIRM_HOLD_TICKS: u32 = 2;
const FRONT_END_LAST_CONFIRM_TICK: u32 = 360;
const GAME_DESTINATION_SELECT_TICK: u32 = 380;
const GAME_DESTINATION_CONFIRM_TICK: u32 = 420;
const ROUTE_SELECTION_CONFIRM_TICK: u32 = 500;
const ROUTE_SELECTION_CONFIRM_HOLD_TICKS: u32 = 12;
const PLANET_DISMISS_START_TICK: u32 = 840;
const PLANET_DISMISS_END_TICK: u32 = 900;
const PLANET_DISMISS_CADENCE_TICKS: u32 = 2;
const FRONT_END_TRANSITIONS: usize = 18;
const PEPPER_CURSOR_CHECKPOINTS: [(u32, u8); 5] =
    [(654, 0), (656, 1), (657, 2), (761, 64), (839, 103)];
/// Full semantic-frame anchors certified by the paired retail run. The native
/// replay test reaches these in well under a second, providing a fast first
/// gate while the direct cartridge trace remains the final authority.
const CORNERIA_SEMANTIC_CHECKPOINTS: [(u32, &str); 8] = [
    (
        892,
        "e4c928c8285ed9bbbb312b089f689b7a48f4d555a52b6e52d8dc82a270df581e",
    ),
    (
        1_080,
        "8f40d5f1a873dbea20b6122df41a299dbe5d071c98917233c929e431a758628a",
    ),
    (
        1_200,
        "b5aea1deb228974183adbd75c3b600fe4eeac73442a83b64bb7cd6a4c7fba8fa",
    ),
    (
        1_500,
        "b585eff5eedf1183b156e6df21a01a7cd57a0a96b54ba8309c7d86fa07f90941",
    ),
    (
        1_700,
        "eefede6ebe9700ad79f568bc1d446d8f27ae39abfd5bee8680206f8a06daf378",
    ),
    (
        1_800,
        "f3e2c04597003256a3f0d923c8c76f09ccfaf32d361fa43394caa90aa3a46160",
    ),
    (
        1_841,
        "ca6f58d3f811b83bf9e18d12824292cbce9f76f84d64e178b0fde79ecb0e13f2",
    ),
    (
        1_876,
        "d0cc108da73cc3a112a85e59d16119fded0aceb52f566c593d8d70df84e0f29b",
    ),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Position(i16, i16, i16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StartupSnapshot {
    background: u16,
    game_frame: u16,
    player: Position,
    body: Position,
    left_wing: Position,
    right_wing: Position,
    follower: Position,
    camera: Option<Position>,
    active_objects: usize,
}

const STARTUP_CHECKPOINTS: [(u32, StartupSnapshot); 5] = [
    (
        859,
        StartupSnapshot {
            background: 0,
            game_frame: 141,
            player: Position(0, 0, 63),
            body: Position(0, 0, 0),
            left_wing: Position(0, 0, 0),
            right_wing: Position(0, 0, 0),
            follower: Position(0, 0, 0),
            camera: None,
            active_objects: 5,
        },
    ),
    (
        864,
        StartupSnapshot {
            background: 0,
            game_frame: 142,
            player: Position(0, 0, 126),
            body: Position(0, 0, 126),
            left_wing: Position(-32, 12, 126),
            right_wing: Position(32, 12, 126),
            follower: Position(0, 0, 63),
            camera: None,
            active_objects: 5,
        },
    ),
    (
        890,
        StartupSnapshot {
            background: 0,
            game_frame: 142,
            player: Position(0, 0, 126),
            body: Position(0, 0, 126),
            left_wing: Position(-32, 12, 126),
            right_wing: Position(32, 12, 126),
            follower: Position(0, 0, 63),
            camera: None,
            active_objects: 5,
        },
    ),
    (
        891,
        StartupSnapshot {
            background: 0,
            game_frame: 0,
            player: Position(0, -28, 191),
            body: Position(0, -28, 191),
            left_wing: Position(-32, -16, 191),
            right_wing: Position(32, -16, 191),
            follower: Position(0, 0, 126),
            camera: Some(Position(-1175, -1961, 3560)),
            active_objects: 6,
        },
    ),
    (
        892,
        StartupSnapshot {
            background: 0,
            game_frame: 1,
            player: Position(0, -26, 256),
            body: Position(0, -26, 256),
            left_wing: Position(-32, -13, 256),
            right_wing: Position(32, -15, 256),
            follower: Position(0, -28, 191),
            camera: Some(Position(-1151, -1923, 3498)),
            active_objects: 6,
        },
    ),
];
const FIRST_LEVEL_STATE_COMPARISON_TICK: u32 = 892;
const STARTUP_ROLE_SLOTS: u16 = 6;
const PLAYER_BODY_SLOT: usize = 1;
const RETAIL_DIRECT_SHAPE_OP_0: u16 = 0xBB48;
const RETAIL_DIRECT_SHAPE_OP_1: u16 = 0xBB64;
const RETAIL_DIRECT_SHAPE_OP_2: u16 = 0xBB80;
const RETAIL_DIRECT_SHAPE_BOOST: u16 = 0xB219;
const RETAIL_DIRECT_SHAPE_MYSHIP_4: u16 = 0xD304;
const RETAIL_DIRECT_SHAPE_MYBASE_0: u16 = 0xDD84;
const RETAIL_DIRECT_SHAPE_ENEMY_LASER: u16 = 0xB34D;
const RETAIL_DIRECT_SHAPE_PLAYER_LASER: u16 = 0xB369;
const RETAIL_DIRECT_SHAPE_LARGE_LASER_FLASH: u16 = 0xB075;
const RETAIL_DIRECT_SHAPE_SPARK_EXPLOSION: u16 = 0xB289;
const RETAIL_DIRECT_SHAPE_LASER_DEATH_FLASH: u16 = 0xB2A5;
const RETAIL_DIRECT_SHAPE_LINE_SPARK: u16 = 0xB2C1;
const RETAIL_DIRECT_SHAPE_MEDIUM_EXPLOSION_SPRITE: u16 = 0xB11D;
const RETAIL_DIRECT_SHAPE_MEDIUM_EXPLOSION_POLYGONS: u16 = 0xBE04;
const RETAIL_DIRECT_SHAPE_SMALL_EXPLOSION_SPRITE: u16 = 0xB101;
const RETAIL_DIRECT_SHAPE_SMALL_EXPLOSION_POLYGONS: u16 = 0xB587;
const RETAIL_DIRECT_SHAPE_BOUNCYBALL: u16 = 0xAEED;
const RETAIL_DIRECT_SHAPE_TOWER_CHILD: u16 = 0xBD78;
const RETAIL_DIRECT_SHAPE_OVERSIZED_EXPLOSION_ENVELOPE: u16 = 0xACBD;
const RETAIL_DIRECT_SHAPE_LARGE_EXPLOSION_ENVELOPE: u16 = 0xACD9;
const RETAIL_DIRECT_SHAPE_MEDIUM_EXPLOSION_ENVELOPE: u16 = 0xACF5;
const RETAIL_DIRECT_SHAPE_SMALL_EXPLOSION_ENVELOPE: u16 = 0xAD11;
const RETAIL_DIRECT_SHAPE_SMOKE: u16 = 0xADD5;
const RETAIL_DIRECT_SHAPE_ROBOT_0: u16 = 0xBB9C;
const RETAIL_DIRECT_SHAPE_PILLAR3_NS: u16 = 0xB882;
const NATIVE_SHAPE_ENEMY_LASER: u16 = 478;
const NATIVE_SHAPE_PLAYER_LASER: u16 = 511;
const NATIVE_SHAPE_LARGE_LASER_FLASH: u16 = 479;
const NATIVE_SHAPE_SPARK_EXPLOSION: u16 = 367;
const NATIVE_SHAPE_LASER_DEATH_FLASH: u16 = 342;
const NATIVE_SHAPE_LINE_SPARK: u16 = 380;
const NATIVE_SHAPE_MEDIUM_EXPLOSION_SPRITE: u16 = 462;
const NATIVE_SHAPE_MEDIUM_EXPLOSION_POLYGONS: u16 = 466;
const NATIVE_SHAPE_SMALL_EXPLOSION_SPRITE: u16 = 461;
const NATIVE_SHAPE_SMALL_EXPLOSION_POLYGONS: u16 = 465;
const NATIVE_SHAPE_BOUNCYBALL: u16 = 405;
const NATIVE_SHAPE_TOWER_CHILD: u16 = 447;
const NATIVE_SHAPE_SMOKE: u16 = 357;
const NATIVE_SHAPE_BOMBER: u16 = 48;
const NATIVE_SHAPE_ZACO_A: u16 = 217;
const NATIVE_SHAPE_ZACO_6: u16 = 52;
const NATIVE_SHAPE_KAMIKAZE: u16 = 9;
const AUTHORED_KAMIKAZE_COUNT: usize = 2;
const NATIVE_SHAPE_ROBOT_0: u16 = 420;
const NATIVE_SHAPE_PILLAR3_NS: u16 = 452;
const RETAIL_PLAYER_PRESENTATION_BYTES: u32 = 0x1551;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LevelObjectSnapshot {
    slot: u16,
    shape: Option<u16>,
    position: Position,
    explosion_size: Option<ExplosionSize>,
    durability: u8,
    hit_flags: u8,
    collision_disabled: bool,
    damage_immune: bool,
    departure_lifetime: Option<u8>,
    departure_delay: Option<u8>,
    path_wait: Option<u8>,
    fighter_motion: Option<FighterMotion>,
    authored_motion: Option<AuthoredMotion>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AuthoredMotion {
    rotation: [u8; 3],
    speed: u8,
    velocity: Position,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FighterMotion {
    rotation: [u8; 3],
    speed: u8,
    velocity: Position,
    lateral_offset: i16,
    vertical_offset: i16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LevelSnapshot {
    background: u16,
    game_frame: u16,
    game_flags: u8,
    player_ship_flags: [u8; 3],
    player_strategy_flags: u8,
    player_fly_mode: u8,
    player_object: u16,
    map_countdown: u16,
    view_kind: u8,
    player_view_position: Position,
    view_float: [i16; 2],
    view_shake: [i8; 3],
    view_position: Position,
    view_pitch: i16,
    view_yaw: i16,
    effective_view_yaw: i16,
    view_distance: i16,
    forward_velocity: i16,
    previous_player_depth: i16,
    last_depth_change: i16,
    player_hit_timer: u8,
    player_hit_flags: u8,
    player_body_durability: u8,
    player_presentation_bytes: [u8; 3],
    active_order: Vec<u16>,
    free_order: Vec<u16>,
    objects: Vec<LevelObjectSnapshot>,
}

#[derive(Default)]
struct ObjectIdentityTracker {
    generations: BTreeMap<u16, u32>,
    active: BTreeSet<u16>,
}

impl ObjectIdentityTracker {
    fn record_level(
        &mut self,
        mut frame: SemanticFrame,
        snapshot: &LevelSnapshot,
        random_state: [u8; 4],
    ) -> SemanticFrame {
        frame = frame
            .with_field("level.background", snapshot.background)
            .with_field("level.frame", snapshot.game_frame)
            .with_field("level.flags", snapshot.game_flags)
            .with_field("player.ship_flags", snapshot.player_ship_flags[0])
            .with_field("player.ship_flags_2", snapshot.player_ship_flags[1])
            .with_field("player.ship_flags_3", snapshot.player_ship_flags[2])
            .with_field("player.strategy_flags", snapshot.player_strategy_flags)
            .with_field("player.fly_mode", snapshot.player_fly_mode)
            .with_field("player.object", snapshot.player_object)
            .with_field("map.countdown", snapshot.map_countdown)
            .with_field("view.kind", snapshot.view_kind)
            .with_field("view.player_position.x", snapshot.player_view_position.0)
            .with_field("view.player_position.y", snapshot.player_view_position.1)
            .with_field("view.player_position.z", snapshot.player_view_position.2)
            .with_field("view.float.x", snapshot.view_float[0])
            .with_field("view.float.y", snapshot.view_float[1])
            .with_field("view.shake.x", snapshot.view_shake[0])
            .with_field("view.shake.y", snapshot.view_shake[1])
            .with_field("view.shake.z", snapshot.view_shake[2])
            .with_field("view.distance", snapshot.view_distance)
            .with_field("view.forward_velocity", snapshot.forward_velocity)
            .with_field("player.previous_depth", snapshot.previous_player_depth)
            .with_field("player.last_depth_change", snapshot.last_depth_change)
            .with_field("player.hit_timer", snapshot.player_hit_timer)
            .with_field("player.hit_flags", snapshot.player_hit_flags)
            .with_field("player.body_durability", snapshot.player_body_durability)
            .with_field(
                "opening.rotation_phase",
                snapshot.player_presentation_bytes[0],
            )
            .with_field(
                "opening.vertical_phase",
                snapshot.player_presentation_bytes[1],
            )
            .with_field("opening.boost_delay", snapshot.player_presentation_bytes[2])
            .with_field("random.byte_0", random_state[0])
            .with_field("random.byte_1", random_state[1])
            .with_field("random.byte_2", random_state[2])
            .with_field("random.byte_3", random_state[3])
            .with_field(
                "object_pool.active_order",
                format!("{:?}", snapshot.active_order),
            )
            .with_field(
                "object_pool.free_order",
                format!("{:?}", snapshot.free_order),
            );

        // Fixed/look-at phases expose their camera object as the authored
        // logical state; its slot and position are compared below. The final
        // transform is produced later in the retail presentation pipeline.
        // Normal flight derives the transform in the same logical update as
        // the native shell, so compare its position and angles directly.
        if snapshot.view_kind & (VIEWTYPE_FPOS | VIEWTYPE_TOOBJ) == 0 {
            frame = frame
                .with_field("view.position.x", snapshot.view_position.0)
                .with_field("view.position.y", snapshot.view_position.1)
                .with_field("view.position.z", snapshot.view_position.2)
                .with_field("view.pitch", snapshot.view_pitch)
                .with_field("view.yaw", snapshot.view_yaw)
                .with_field("view.effective_yaw", snapshot.effective_view_yaw);
        }

        let current: BTreeSet<_> = snapshot.objects.iter().map(|object| object.slot).collect();
        for slot in self.active.difference(&current) {
            let generation = self
                .generations
                .get(slot)
                .copied()
                .expect("active object generation");
            frame.events.push(
                SemanticEvent::new("object-death")
                    .with_field("identity", format!("slot-{slot}-birth-{generation}")),
            );
        }
        for slot in current.difference(&self.active) {
            let generation = self.generations.entry(*slot).or_default();
            *generation += 1;
            frame.events.push(
                SemanticEvent::new("object-birth")
                    .with_field("identity", format!("slot-{slot}-birth-{generation}")),
            );
        }
        self.active = current;

        for object in &snapshot.objects {
            let generation = self.generations[&object.slot];
            let kind = match object.slot {
                0 => "player",
                1 => "player-body",
                2 => "player-left-wing",
                3 => "player-right-wing",
                4 => "player-follower",
                5 => "opening-camera",
                _ => "level-object",
            };
            let mut semantic =
                SemanticObject::new(format!("slot-{}-birth-{generation}", object.slot), kind)
                    .with_field("slot", object.slot)
                    .with_field("position.x", object.position.0)
                    .with_field("position.y", object.position.1)
                    .with_field("position.z", object.position.2);
            if let Some(shape) = object.shape {
                semantic = semantic.with_field("shape", shape);
            }
            if let Some(size) = object.explosion_size {
                semantic = semantic.with_field("visual.explosion_size", explosion_size_name(size));
            }
            semantic = semantic
                .with_field("collision.durability", object.durability)
                .with_field("collision.hit_flags", object.hit_flags)
                .with_field("collision.disabled", object.collision_disabled)
                .with_field("collision.damage_immune", object.damage_immune);
            if let Some(lifetime) = object.departure_lifetime {
                semantic = semantic.with_field("departure.lifetime", lifetime);
            }
            if let Some(delay) = object.departure_delay {
                semantic = semantic.with_field("departure.delay", delay);
            }
            if let Some(wait) = object.path_wait {
                semantic = semantic.with_field("path.wait", wait);
            }
            if let Some(motion) = object.fighter_motion {
                semantic = semantic
                    .with_field("fighter.rotation.x", motion.rotation[0])
                    .with_field("fighter.rotation.y", motion.rotation[1])
                    .with_field("fighter.rotation.z", motion.rotation[2])
                    .with_field("fighter.speed", motion.speed)
                    .with_field("fighter.velocity.x", motion.velocity.0)
                    .with_field("fighter.velocity.y", motion.velocity.1)
                    .with_field("fighter.velocity.z", motion.velocity.2)
                    .with_field("fighter.lateral_offset", motion.lateral_offset)
                    .with_field("fighter.vertical_offset", motion.vertical_offset);
            }
            if let Some(motion) = object.authored_motion {
                semantic = semantic
                    .with_field("motion.rotation.x", motion.rotation[0])
                    .with_field("motion.rotation.y", motion.rotation[1])
                    .with_field("motion.rotation.z", motion.rotation[2])
                    .with_field("motion.speed", motion.speed)
                    .with_field("motion.velocity.x", motion.velocity.0)
                    .with_field("motion.velocity.y", motion.velocity.1)
                    .with_field("motion.velocity.z", motion.velocity.2);
            }
            frame.objects.push(semantic);
        }
        frame
    }
}

fn explosion_size_name(size: ExplosionSize) -> &'static str {
    match size {
        ExplosionSize::Small => "small",
        ExplosionSize::Medium => "medium",
        ExplosionSize::Large => "large",
        ExplosionSize::Oversized => "oversized",
    }
}
const RETAIL_SOURCE_EDGE_COVERAGE: [(u32, &str); 13] = [
    (RETAIL_PLANET_SHIP_FLASH_ENTRY, "source:planet.ship-flash"),
    (RETAIL_PLANET_MAP_FADE_ENTRY, "source:planet.fade-map"),
    (RETAIL_PLANET_ISOLATION_ENTRY, "source:planet.isolate"),
    (RETAIL_PLANET_CENTER_ENTRY, "source:planet.center"),
    (
        RETAIL_PLANET_BRIEFING_PREP_ENTRY,
        "source:planet.prepare-briefing",
    ),
    (RETAIL_PLANET_ZOOM_ENTRY, "source:planet.zoom"),
    (RETAIL_PLANET_NAME_ENTRY, "source:planet.reveal-name"),
    (RETAIL_PLANET_MESSAGE_ENTRY, "source:planet.message"),
    (RETAIL_PLANET_DISMISS_ENTRY, "source:planet.dismiss"),
    (RETAIL_PLANET_EXIT_FADE_ENTRY, "source:planet.exit-fade"),
    (RETAIL_PLANET_GAME_START_ENTRY, "source:planet.start-game"),
    (RETAIL_DOSTRATS, "source:gameplay.strategies-begin"),
    (
        RETAIL_DOSTRATS_COMPLETE,
        "source:gameplay.strategies-complete",
    ),
];
const RETAIL_PLANET_PHASE_ENTRY_OPCODES: [(u32, u8); 11] = [
    (RETAIL_PLANET_SHIP_FLASH_ENTRY, 0xA9),
    (RETAIL_PLANET_MAP_FADE_ENTRY, 0xA2),
    (RETAIL_PLANET_ISOLATION_ENTRY, 0x20),
    (RETAIL_PLANET_CENTER_ENTRY, 0xA2),
    (RETAIL_PLANET_BRIEFING_PREP_ENTRY, 0xE2),
    (RETAIL_PLANET_ZOOM_ENTRY, 0xA2),
    (RETAIL_PLANET_NAME_ENTRY, 0x20),
    (RETAIL_PLANET_MESSAGE_ENTRY, 0x20),
    (RETAIL_PLANET_DISMISS_ENTRY, 0x68),
    (RETAIL_PLANET_EXIT_FADE_ENTRY, 0x78),
    (RETAIL_PLANET_GAME_START_ENTRY, 0x20),
];

fn trace_frame(
    sequence: u64,
    position: (i16, i16, i16),
    velocity: (i16, i16, i16),
) -> SemanticFrame {
    SemanticFrame::new(sequence, sequence, NO_INPUT)
        .with_field("view.forward_velocity", VIEW_FORWARD_VELOCITY)
        .with_object(
            SemanticObject::new(PRIMARY_ENEMY, "fighter")
                .with_field("position.x", position.0)
                .with_field("position.y", position.1)
                .with_field("position.z", position.2)
                .with_field("velocity.x", velocity.0)
                .with_field("velocity.y", velocity.1)
                .with_field("velocity.z", velocity.2),
        )
}

#[test]
fn retail_straight_motion_matches_native_semantic_trace() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("retail semantic trace skipped: Star Fox retail ROM not found");
        return;
    };

    let mut retail = SnesBus::new(rom);
    let object_block = RETAIL_POOL.base;
    retail.wram_write16(RETAIL_PVIEWVELZ, VIEW_FORWARD_VELOCITY as u16);
    retail.wram_write16(
        object_block + RETAIL_POOL.al_worldx,
        INITIAL_POSITION_X as u16,
    );
    retail.wram_write16(
        object_block + RETAIL_POOL.al_worldy,
        INITIAL_POSITION_Y as u16,
    );
    retail.wram_write16(
        object_block + RETAIL_POOL.al_worldz,
        INITIAL_POSITION_Z as u16,
    );
    retail.wram_write16(object_block + AL_VX, VELOCITY_X as u16);
    retail.wram_write16(object_block + AL_VY, VELOCITY_Y as u16);
    retail.wram_write16(object_block + AL_VZ, VELOCITY_Z as u16);

    let mut native = sf_game::alien::Alien {
        worldx: INITIAL_POSITION_X,
        worldy: INITIAL_POSITION_Y,
        worldz: INITIAL_POSITION_Z,
        vx: VELOCITY_X,
        vy: VELOCITY_Y,
        vz: VELOCITY_Z,
        ..Default::default()
    };

    let mut retail_trace = vec![trace_frame(
        0,
        (INITIAL_POSITION_X, INITIAL_POSITION_Y, INITIAL_POSITION_Z),
        (VELOCITY_X, VELOCITY_Y, VELOCITY_Z),
    )];
    let mut native_trace = retail_trace.clone();

    for sequence in 1..=FRAME_COUNT {
        call(
            &mut retail,
            RETAIL_STRAIGHT_STRAT,
            &Entry {
                x: object_block as u16,
                ..Default::default()
            },
        );
        let retail_object = snapshot_objects(&retail, &RETAIL_POOL)[0];
        retail_trace.push(trace_frame(
            sequence,
            (
                retail_object.worldx,
                retail_object.worldy,
                retail_object.worldz,
            ),
            (VELOCITY_X, VELOCITY_Y, VELOCITY_Z),
        ));

        sf_strat::common::strat_apply_velocity(&mut native);
        native.worldz = native.worldz.wrapping_add(VIEW_FORWARD_VELOCITY);
        native_trace.push(trace_frame(
            sequence,
            (native.worldx, native.worldy, native.worldz),
            (native.vx, native.vy, native.vz),
        ));
    }

    if let Some(divergence) =
        first_divergence(&retail_trace, &native_trace).expect("semantic traces must be valid")
    {
        panic!("retail straight-motion trace diverged: {divergence}");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrontEndPhase {
    AttractIntro,
    Title,
    BriefingControl,
    BriefingDestination,
    PlanetMapSetup,
    RouteSelection,
    ShipFlash,
    FadingMap,
    IsolatingPlanet,
    CenteringPlanet,
    PreparingBriefing,
    ZoomingPlanet,
    RevealingPlanetName,
    Briefing,
    DismissingBriefing,
    FadingOut,
    LevelInitialization,
    CorneriaOpening,
}

impl FrontEndPhase {
    fn name(self) -> &'static str {
        match self {
            Self::AttractIntro => "attract-intro",
            Self::Title => "title",
            Self::BriefingControl => "briefing-control",
            Self::BriefingDestination => "briefing-destination",
            Self::PlanetMapSetup => "planet-map-setup",
            Self::RouteSelection => "route-selection",
            Self::ShipFlash => "ship-flash",
            Self::FadingMap => "fading-map",
            Self::IsolatingPlanet => "isolating-planet",
            Self::CenteringPlanet => "centering-planet",
            Self::PreparingBriefing => "preparing-briefing",
            Self::ZoomingPlanet => "zooming-planet",
            Self::RevealingPlanetName => "revealing-planet-name",
            Self::Briefing => "briefing",
            Self::DismissingBriefing => "dismissing-briefing",
            Self::FadingOut => "fading-out",
            Self::LevelInitialization => "level-initialization",
            Self::CorneriaOpening => "corneria-opening",
        }
    }
}

const REQUIRED_FRONT_END_PHASES: [FrontEndPhase; 18] = [
    FrontEndPhase::AttractIntro,
    FrontEndPhase::Title,
    FrontEndPhase::BriefingControl,
    FrontEndPhase::BriefingDestination,
    FrontEndPhase::PlanetMapSetup,
    FrontEndPhase::RouteSelection,
    FrontEndPhase::ShipFlash,
    FrontEndPhase::FadingMap,
    FrontEndPhase::IsolatingPlanet,
    FrontEndPhase::CenteringPlanet,
    FrontEndPhase::PreparingBriefing,
    FrontEndPhase::ZoomingPlanet,
    FrontEndPhase::RevealingPlanetName,
    FrontEndPhase::Briefing,
    FrontEndPhase::DismissingBriefing,
    FrontEndPhase::FadingOut,
    FrontEndPhase::LevelInitialization,
    FrontEndPhase::CorneriaOpening,
];
const RETAIL_COVERAGE_PRODUCER: &str = "retail";
const NATIVE_COVERAGE_PRODUCER: &str = "native";
const COVERAGE_CORNERIA_LEVEL_STATE: &str = "corneria-level-state";
const COVERAGE_CORNERIA_KAMIKAZE_WAVE: &str = "corneria-kamikaze-wave";
const COVERAGE_PLAYER_BODY_DAMAGE: &str = "player-body-damage";
const COVERAGE_OBJECT_BIRTH: &str = "event:object-birth";
const COVERAGE_OBJECT_DEATH: &str = "event:object-death";
const CORNERIA_SCENARIO_COVERAGE: [&str; 5] = [
    COVERAGE_CORNERIA_LEVEL_STATE,
    COVERAGE_CORNERIA_KAMIKAZE_WAVE,
    COVERAGE_PLAYER_BODY_DAMAGE,
    COVERAGE_OBJECT_BIRTH,
    COVERAGE_OBJECT_DEATH,
];

fn coverage_point(producer: &str, point: &str) -> String {
    format!("{producer}:{point}")
}

fn phase_coverage_point(producer: &str, phase: FrontEndPhase) -> String {
    coverage_point(producer, &format!("phase:{}", phase.name()))
}

fn required_phase_coverage(producer: &str) -> BTreeSet<String> {
    REQUIRED_FRONT_END_PHASES
        .into_iter()
        .map(|phase| phase_coverage_point(producer, phase))
        .collect()
}

fn required_corneria_coverage(producer: &str) -> BTreeSet<String> {
    CORNERIA_SCENARIO_COVERAGE
        .into_iter()
        .map(|point| coverage_point(producer, point))
        .collect()
}

fn record_phase_coverage(
    coverage: &mut BTreeSet<String>,
    producer: &str,
    phase: Option<FrontEndPhase>,
) {
    if let Some(phase) = phase {
        coverage.insert(phase_coverage_point(producer, phase));
    }
}

fn record_retail_source_edge_coverage(coverage: &mut BTreeSet<String>, execution_entries: &[u32]) {
    for entry in execution_entries {
        if let Some((_, point)) = RETAIL_SOURCE_EDGE_COVERAGE
            .iter()
            .find(|(address, _)| address == entry)
        {
            coverage.insert(coverage_point(RETAIL_COVERAGE_PRODUCER, point));
        }
    }
}

fn record_event_coverage(coverage: &mut BTreeSet<String>, producer: &str, frame: &SemanticFrame) {
    for event in &frame.events {
        coverage.insert(coverage_point(producer, &format!("event:{}", event.kind)));
    }
}

#[derive(Default)]
struct RetailPhaseTracker {
    route_selection_seen: bool,
    planet_phase: Option<FrontEndPhase>,
    gameplay_update_entries: u8,
}

fn front_end_input(tick: u32) -> u16 {
    if (GAME_DESTINATION_SELECT_TICK..GAME_DESTINATION_SELECT_TICK + FRONT_END_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::DOWN;
    }
    if (GAME_DESTINATION_CONFIRM_TICK..GAME_DESTINATION_CONFIRM_TICK + FRONT_END_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::START;
    }
    if tick <= FRONT_END_LAST_CONFIRM_TICK
        && tick % FRONT_END_CONFIRM_CADENCE_TICKS < FRONT_END_CONFIRM_HOLD_TICKS
    {
        return pad::START;
    }
    if (ROUTE_SELECTION_CONFIRM_TICK
        ..ROUTE_SELECTION_CONFIRM_TICK + ROUTE_SELECTION_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::START;
    }
    if (PLANET_DISMISS_START_TICK..PLANET_DISMISS_END_TICK).contains(&tick) {
        return if (tick - PLANET_DISMISS_START_TICK) % PLANET_DISMISS_CADENCE_TICKS == 0 {
            pad::B
        } else {
            0
        };
    }
    if attack_carrier_trace_enabled() && tick >= FIRST_LEVEL_STATE_COMPARISON_TICK {
        let paused_updates = NATIVE_REPLAY_LEVEL_UPDATE_PAUSE_TICKS
            .iter()
            .filter(|pause| **pause < tick)
            .count() as u32;
        let level_frame = tick
            .saturating_sub(FIRST_LEVEL_STATE_COMPARISON_TICK)
            .saturating_sub(paused_updates);
        return sf_oracle::sf1_input::corneria_attack_carrier_input(
            u16::try_from(level_frame).expect("Corneria route frame must fit"),
        );
    }
    0
}

fn front_end_input_runs() -> Vec<ScenarioInputRun> {
    let mut runs = Vec::<ScenarioInputRun>::new();
    for tick in 0..corneria_scenario_ticks() {
        let input = u32::from(front_end_input(tick));
        if let Some(run) = runs.last_mut().filter(|run| run.input == input) {
            run.frames += 1;
        } else {
            runs.push(ScenarioInputRun { frames: 1, input });
        }
    }
    runs
}

fn front_end_manifest() -> ScenarioManifest {
    let mut required_retail_coverage = required_phase_coverage(RETAIL_COVERAGE_PRODUCER);
    required_retail_coverage.extend(required_corneria_coverage(RETAIL_COVERAGE_PRODUCER));
    required_retail_coverage.extend(
        RETAIL_SOURCE_EDGE_COVERAGE
            .iter()
            .map(|(_, point)| coverage_point(RETAIL_COVERAGE_PRODUCER, point)),
    );
    let mut required_native_coverage = required_phase_coverage(NATIVE_COVERAGE_PRODUCER);
    required_native_coverage.extend(required_corneria_coverage(NATIVE_COVERAGE_PRODUCER));

    ScenarioManifest {
        schema_version: SCENARIO_SCHEMA_VERSION,
        id: corneria_scenario_id().to_owned(),
        description: if attack_carrier_trace_enabled() {
            "Retail boot through the controller-only Corneria Attack Carrier approach".to_owned()
        } else {
            "Retail boot through Corneria frame 983 and natural player damage".to_owned()
        },
        retail_rom_sha256: RETAIL_ROM_SHA256.to_owned(),
        clock: ScenarioClock::logical_update(),
        input_runs: front_end_input_runs(),
        required_channels: [
            CaptureChannel::SemanticState,
            CaptureChannel::ObjectLifecycle,
            CaptureChannel::Coverage,
        ]
        .into_iter()
        .collect(),
        required_retail_coverage,
        required_native_coverage,
    }
}

fn attack_carrier_trace_enabled() -> bool {
    std::env::var_os(ATTACK_CARRIER_TRACE_ENV).is_some()
}

fn corneria_scenario_ticks() -> u32 {
    if attack_carrier_trace_enabled() {
        ATTACK_CARRIER_SCENARIO_TICKS
    } else {
        CORNERIA_SCENARIO_TICKS
    }
}

fn corneria_scenario_id() -> &'static str {
    if attack_carrier_trace_enabled() {
        ATTACK_CARRIER_SCENARIO_ID
    } else {
        FRONT_END_SCENARIO_ID
    }
}

fn scenario_frame(tick: u32, input: u16, phase: Option<FrontEndPhase>) -> SemanticFrame {
    SemanticFrame::new(u64::from(tick), u64::from(tick), u32::from(input))
        .with_field("phase", phase.map_or("unobserved", FrontEndPhase::name))
}

fn retail_front_end_phase(
    retail: &RetailMachine,
    tracker: &mut RetailPhaseTracker,
    execution_entries: &[u32],
) -> Option<FrontEndPhase> {
    for entry in execution_entries {
        if *entry == RETAIL_DOSTRATS {
            if tracker.planet_phase == Some(FrontEndPhase::LevelInitialization) {
                tracker.gameplay_update_entries = tracker.gameplay_update_entries.saturating_add(1);
                if tracker.gameplay_update_entries >= 2 {
                    tracker.planet_phase = Some(FrontEndPhase::CorneriaOpening);
                }
            }
            continue;
        }
        tracker.planet_phase = Some(match *entry {
            RETAIL_PLANET_SHIP_FLASH_ENTRY => FrontEndPhase::ShipFlash,
            RETAIL_PLANET_MAP_FADE_ENTRY => FrontEndPhase::FadingMap,
            RETAIL_PLANET_ISOLATION_ENTRY => FrontEndPhase::IsolatingPlanet,
            RETAIL_PLANET_CENTER_ENTRY => FrontEndPhase::CenteringPlanet,
            RETAIL_PLANET_BRIEFING_PREP_ENTRY => FrontEndPhase::PreparingBriefing,
            RETAIL_PLANET_ZOOM_ENTRY => FrontEndPhase::ZoomingPlanet,
            RETAIL_PLANET_NAME_ENTRY => FrontEndPhase::RevealingPlanetName,
            RETAIL_PLANET_MESSAGE_ENTRY => FrontEndPhase::Briefing,
            RETAIL_PLANET_DISMISS_ENTRY => FrontEndPhase::DismissingBriefing,
            RETAIL_PLANET_EXIT_FADE_ENTRY => FrontEndPhase::FadingOut,
            RETAIL_PLANET_GAME_START_ENTRY => {
                tracker.gameplay_update_entries = 0;
                FrontEndPhase::LevelInitialization
            }
            _ => continue,
        });
    }
    if let Some(phase) = tracker.planet_phase {
        return Some(phase);
    }

    match retail.peek16(WORK_RAM | RETAIL_CURRENTBG) {
        RETAIL_ATTRACT_BACKGROUND => Some(FrontEndPhase::AttractIntro),
        RETAIL_TITLE_BACKGROUND => Some(FrontEndPhase::Title),
        RETAIL_BRIEFING_BACKGROUND => {
            let game_selected = retail.peek8(RETAIL_BRIEFING_CHOICE) != 0;
            let planet_interrupt = retail.peek8(WORK_RAM | RETAIL_PLANET_INTERRUPT) != 0;
            let control_disabled =
                retail.peek8(WORK_RAM | RETAIL_PSHIPFLAGS) & BRIEFING_CONTROL_DISABLED_MASK != 0;
            if game_selected && !planet_interrupt {
                let route_ready = retail.peek8(WORK_RAM | RETAIL_WHICH_ROUTE) == INITIAL_ROUTE
                    && retail.peek8(WORK_RAM | RETAIL_PLANET_STAGE) == ROUTE_PREVIEW_STAGE
                    && retail.peek8(WORK_RAM | RETAIL_CURRENT_PLANET) as i8
                        == HIDDEN_CURRENT_PLANET;
                let route_confirmed = retail.peek8(WORK_RAM | RETAIL_PLANET_SHIP_FLASH) != 0
                    && retail.peek8(WORK_RAM | RETAIL_WHICH_ROUTE) == INITIAL_ROUTE
                    && retail.peek8(WORK_RAM | RETAIL_PLANET_STAGE) == 0
                    && retail.peek8(WORK_RAM | RETAIL_CURRENT_PLANET) == 0;
                if route_ready {
                    tracker.route_selection_seen = true;
                }
                Some(if route_confirmed {
                    tracker.planet_phase = Some(FrontEndPhase::ShipFlash);
                    FrontEndPhase::ShipFlash
                } else if tracker.route_selection_seen {
                    FrontEndPhase::RouteSelection
                } else {
                    FrontEndPhase::PlanetMapSetup
                })
            } else if planet_interrupt && control_disabled {
                Some(FrontEndPhase::BriefingDestination)
            } else {
                Some(FrontEndPhase::BriefingControl)
            }
        }
        _ => None,
    }
}

fn native_front_end_phase(native: &Shell) -> Option<FrontEndPhase> {
    match native.state() {
        GameState::AttractIntro => Some(FrontEndPhase::AttractIntro),
        GameState::Title => Some(FrontEndPhase::Title),
        GameState::Briefing => match native.frame().briefing_phase {
            BriefingPhase::ControlType => Some(FrontEndPhase::BriefingControl),
            BriefingPhase::Destination => Some(FrontEndPhase::BriefingDestination),
        },
        GameState::PlanetSelect => match native.frame().planet_presentation.phase {
            PlanetSequencePhase::InitialSetup => Some(FrontEndPhase::PlanetMapSetup),
            PlanetSequencePhase::RouteSelection => Some(FrontEndPhase::RouteSelection),
            PlanetSequencePhase::ShipFlash => Some(FrontEndPhase::ShipFlash),
            PlanetSequencePhase::FadingMap => Some(FrontEndPhase::FadingMap),
            PlanetSequencePhase::IsolatingPlanet => Some(FrontEndPhase::IsolatingPlanet),
            PlanetSequencePhase::CenteringPlanet => Some(FrontEndPhase::CenteringPlanet),
            PlanetSequencePhase::PreparingBriefing => Some(FrontEndPhase::PreparingBriefing),
            PlanetSequencePhase::ZoomingPlanet => Some(FrontEndPhase::ZoomingPlanet),
            PlanetSequencePhase::RevealingPlanetName => Some(FrontEndPhase::RevealingPlanetName),
            PlanetSequencePhase::Briefing => Some(FrontEndPhase::Briefing),
            PlanetSequencePhase::DismissingBriefing => Some(FrontEndPhase::DismissingBriefing),
            PlanetSequencePhase::FadingOut => Some(FrontEndPhase::FadingOut),
            PlanetSequencePhase::Traveling | PlanetSequencePhase::AwaitingConfirmation => None,
        },
        GameState::Playing => Some(match native.frame().gameplay_entry_phase {
            GameplayEntryPhase::LevelInitialization => FrontEndPhase::LevelInitialization,
            GameplayEntryPhase::ActiveLevel => FrontEndPhase::CorneriaOpening,
            GameplayEntryPhase::Inactive => return None,
        }),
        _ => None,
    }
}

fn record_front_end_transition(
    trace: &mut Vec<SemanticFrame>,
    previous: &mut Option<FrontEndPhase>,
    origin: &mut Option<u32>,
    tick: u32,
    phase: Option<FrontEndPhase>,
) {
    let Some(phase) = phase else { return };
    if *previous == Some(phase) {
        return;
    }
    let origin_tick = *origin.get_or_insert(tick);
    trace.push(
        SemanticFrame::new(
            trace.len() as u64,
            u64::from(tick.saturating_sub(origin_tick)),
            0,
        )
        .with_field("phase", phase.name()),
    );
    *previous = Some(phase);
}

fn object_position(object: sf_oracle::ObjState) -> Position {
    Position(object.worldx, object.worldy, object.worldz)
}

fn retail_startup_snapshot(retail: &RetailMachine) -> StartupSnapshot {
    let objects = retail.object_snapshot();
    let active = retail.active_object_slots();
    let camera = active.contains(&5).then(|| object_position(objects[5]));
    StartupSnapshot {
        background: sf_oracle::retail_background_catalog_id(
            retail.peek16(WORK_RAM | RETAIL_CURRENTBG),
        )
        .expect("retail background offset must identify a catalog record"),
        game_frame: retail.peek16(WORK_RAM | RETAIL_GAMEFRAME),
        player: object_position(objects[0]),
        body: object_position(objects[1]),
        left_wing: object_position(objects[2]),
        right_wing: object_position(objects[3]),
        follower: object_position(objects[4]),
        camera,
        active_objects: active.len(),
    }
}

fn native_position(native: &Shell, slot: u16) -> Position {
    let object = native.game.objs.aliens[slot as usize];
    Position(object.worldx, object.worldy, object.worldz)
}

fn native_startup_snapshot(native: &Shell) -> StartupSnapshot {
    let boxes = native.game.coldet.pcbox;
    let player = boxes.player.expect("startup player");
    let body = boxes.body.expect("startup body proxy");
    let left_wing = boxes.lwing.expect("startup left-wing proxy");
    let right_wing = boxes.rwing.expect("startup right-wing proxy");
    let follower = u16::try_from(native.game.vars.dummyobj).expect("startup follower");
    let role_slots = [player, body, left_wing, right_wing, follower];
    let active = native.game.objs.active_indices();
    let extra: Vec<_> = active
        .iter()
        .copied()
        .filter(|slot| !role_slots.contains(slot))
        .collect();
    let camera = match extra.as_slice() {
        [] => None,
        [slot] => Some(native_position(native, *slot)),
        _ => panic!("unexpected startup objects outside semantic roles: {extra:?}"),
    };

    StartupSnapshot {
        background: native.game.vars.currentbg,
        game_frame: native.game.vars.gameframe,
        player: native_position(native, player),
        body: native_position(native, body),
        left_wing: native_position(native, left_wing),
        right_wing: native_position(native, right_wing),
        follower: native_position(native, follower),
        camera,
        active_objects: active.len(),
    }
}

fn retail_object_list(retail: &RetailMachine, head_address: u32) -> Vec<u16> {
    let mut slots = Vec::new();
    let mut object = retail.peek16(WORK_RAM | head_address) as u32;
    while object != 0 {
        assert!(
            object >= RETAIL_POOL.base,
            "retail object-list pointer precedes pool: {object:#06X}"
        );
        let offset = object - RETAIL_POOL.base;
        assert_eq!(
            offset % RETAIL_POOL.stride,
            0,
            "retail object-list pointer is not aligned: {object:#06X}"
        );
        let slot = offset / RETAIL_POOL.stride;
        assert!(
            slot < RETAIL_POOL.count,
            "retail object-list pointer exceeds pool: {object:#06X}"
        );
        slots.push(slot as u16);
        assert!(
            slots.len() <= RETAIL_POOL.count as usize,
            "retail object list contains a cycle"
        );
        object = retail.peek16(WORK_RAM | object + RETAIL_POOL.al_next) as u32;
    }
    slots
}

fn retail_object_slot(object: u16) -> u16 {
    let object = u32::from(object);
    assert!(
        object >= RETAIL_POOL.base,
        "retail object pointer precedes pool: {object:#06X}"
    );
    let offset = object - RETAIL_POOL.base;
    assert_eq!(
        offset % RETAIL_POOL.stride,
        0,
        "retail object pointer is not aligned: {object:#06X}"
    );
    let slot = offset / RETAIL_POOL.stride;
    assert!(
        slot < RETAIL_POOL.count,
        "retail object pointer exceeds pool: {object:#06X}"
    );
    slot as u16
}

fn native_free_order(native: &Shell) -> Vec<u16> {
    let mut slots = Vec::new();
    let mut current = native.game.objs.free_head;
    while let Some(slot) = current {
        slots.push(slot);
        assert!(
            slots.len() <= sf_game::alien::NUMBER_AL,
            "native free list contains a cycle"
        );
        current = native.game.objs.aliens[slot as usize].next;
    }
    slots
}

fn retail_level_snapshot(retail: &RetailMachine) -> LevelSnapshot {
    const SOURCE_SHAPE_CATALOG_ENTRIES: u16 = 512;

    let flat_visual = |source_word| {
        let explosion_size = match source_word {
            RETAIL_DIRECT_SHAPE_SMALL_EXPLOSION_ENVELOPE => Some(ExplosionSize::Small),
            RETAIL_DIRECT_SHAPE_MEDIUM_EXPLOSION_ENVELOPE => Some(ExplosionSize::Medium),
            RETAIL_DIRECT_SHAPE_LARGE_EXPLOSION_ENVELOPE => Some(ExplosionSize::Large),
            RETAIL_DIRECT_SHAPE_OVERSIZED_EXPLOSION_ENVELOPE => Some(ExplosionSize::Oversized),
            _ => None,
        };
        if let Some(size) = explosion_size {
            return (0, Some(size));
        }
        let direct_shape = match source_word {
            RETAIL_DIRECT_SHAPE_OP_0 => Some(sf_map::consts::sh::OP_0),
            RETAIL_DIRECT_SHAPE_OP_1 => Some(sf_map::consts::sh::OP_1),
            RETAIL_DIRECT_SHAPE_OP_2 => Some(sf_map::consts::sh::OP_2),
            RETAIL_DIRECT_SHAPE_BOOST => Some(sf_map::consts::sh::BOOST_SHAPE),
            RETAIL_DIRECT_SHAPE_MYSHIP_4 => Some(sf_core::shape::SF1_SHAPE_INTRO_ARWING),
            RETAIL_DIRECT_SHAPE_MYBASE_0 => Some(sf_map::consts::sh::MYBASE_0),
            RETAIL_DIRECT_SHAPE_ENEMY_LASER => Some(NATIVE_SHAPE_ENEMY_LASER),
            RETAIL_DIRECT_SHAPE_PLAYER_LASER => Some(NATIVE_SHAPE_PLAYER_LASER),
            RETAIL_DIRECT_SHAPE_LARGE_LASER_FLASH => Some(NATIVE_SHAPE_LARGE_LASER_FLASH),
            RETAIL_DIRECT_SHAPE_SPARK_EXPLOSION => Some(NATIVE_SHAPE_SPARK_EXPLOSION),
            RETAIL_DIRECT_SHAPE_LASER_DEATH_FLASH => Some(NATIVE_SHAPE_LASER_DEATH_FLASH),
            RETAIL_DIRECT_SHAPE_LINE_SPARK => Some(NATIVE_SHAPE_LINE_SPARK),
            RETAIL_DIRECT_SHAPE_MEDIUM_EXPLOSION_SPRITE => {
                Some(NATIVE_SHAPE_MEDIUM_EXPLOSION_SPRITE)
            }
            RETAIL_DIRECT_SHAPE_MEDIUM_EXPLOSION_POLYGONS => {
                Some(NATIVE_SHAPE_MEDIUM_EXPLOSION_POLYGONS)
            }
            RETAIL_DIRECT_SHAPE_SMALL_EXPLOSION_SPRITE => Some(NATIVE_SHAPE_SMALL_EXPLOSION_SPRITE),
            RETAIL_DIRECT_SHAPE_SMALL_EXPLOSION_POLYGONS => {
                Some(NATIVE_SHAPE_SMALL_EXPLOSION_POLYGONS)
            }
            RETAIL_DIRECT_SHAPE_BOUNCYBALL => Some(NATIVE_SHAPE_BOUNCYBALL),
            RETAIL_DIRECT_SHAPE_TOWER_CHILD => Some(NATIVE_SHAPE_TOWER_CHILD),
            RETAIL_DIRECT_SHAPE_SMOKE => Some(NATIVE_SHAPE_SMOKE),
            RETAIL_DIRECT_SHAPE_ROBOT_0 => Some(NATIVE_SHAPE_ROBOT_0),
            RETAIL_DIRECT_SHAPE_PILLAR3_NS => Some(NATIVE_SHAPE_PILLAR3_NS),
            _ => None,
        };
        if let Some(shape) = direct_shape {
            return (sf_core::shape::resolve_shape_word(shape), None);
        }
        let shape = (0..SOURCE_SHAPE_CATALOG_ENTRIES)
            .find(|catalog_id| {
                retail.peek16(RETAIL_SHAPES + u32::from(*catalog_id) * 2) == source_word
            })
            .map(sf_core::shape::resolve_shape_word)
            .unwrap_or_else(|| sf_core::shape::resolve_shape_word(source_word));
        (shape, None)
    };
    let objects = retail.object_snapshot();
    let active_order = retail_object_list(retail, RETAIL_POOL.active_head);
    let free_order = retail_object_list(retail, RETAIL_POOL.freelist_head);
    let mut active = active_order.clone();
    active.sort_unstable();
    LevelSnapshot {
        background: sf_oracle::retail_background_catalog_id(
            retail.peek16(WORK_RAM | RETAIL_CURRENTBG),
        )
        .expect("retail background offset must identify a catalog record"),
        game_frame: retail.peek16(WORK_RAM | RETAIL_GAMEFRAME),
        game_flags: retail.peek8(WORK_RAM | sf_oracle::RETAIL_GAMEFLAGS),
        player_ship_flags: [
            retail.peek8(WORK_RAM | RETAIL_PSHIPFLAGS),
            retail.peek8(WORK_RAM | RETAIL_PSHIPFLAGS2),
            retail.peek8(WORK_RAM | RETAIL_PSHIPFLAGS3),
        ],
        player_strategy_flags: retail.peek8(WORK_RAM | RETAIL_PSTRATFLAGS),
        player_fly_mode: retail.peek8(WORK_RAM | RETAIL_PLAYER_FLY_MODE),
        player_object: retail_object_slot(retail.peek16(WORK_RAM | RETAIL_PLAYPT)),
        map_countdown: retail.peek16(WORK_RAM | RETAIL_MAPCNT),
        view_kind: retail.peek8(WORK_RAM | RETAIL_VIEW_KIND),
        player_view_position: Position(
            retail.peek16(WORK_RAM | RETAIL_PLAYER_VIEW_X) as i16,
            retail.peek16(WORK_RAM | RETAIL_PLAYER_VIEW_Y) as i16,
            retail.peek16(WORK_RAM | RETAIL_PLAYER_VIEW_Z) as i16,
        ),
        view_float: [
            retail.peek16(WORK_RAM | RETAIL_VIEW_FLOAT_X) as i16,
            retail.peek16(WORK_RAM | RETAIL_VIEW_FLOAT_Y) as i16,
        ],
        view_shake: [
            retail.peek8(WORK_RAM | RETAIL_VIEW_SHAKE_X) as i8,
            retail.peek8(WORK_RAM | RETAIL_VIEW_SHAKE_X + 1) as i8,
            retail.peek8(WORK_RAM | RETAIL_VIEW_SHAKE_X + 2) as i8,
        ],
        view_position: Position(
            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_X) as i16,
            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_Y) as i16,
            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_Z) as i16,
        ),
        view_pitch: retail.peek16(WORK_RAM | RETAIL_VIEW_PITCH) as i16,
        view_yaw: retail.peek16(WORK_RAM | RETAIL_VIEW_YAW) as i16,
        effective_view_yaw: retail.peek16(WORK_RAM | RETAIL_EFFECTIVE_VIEW_YAW) as i16,
        view_distance: retail.peek16(WORK_RAM | RETAIL_VIEW_DISTANCE) as i16,
        forward_velocity: retail.peek16(WORK_RAM | RETAIL_PVIEWVELZ) as i16,
        previous_player_depth: retail.peek16(WORK_RAM | RETAIL_LASTPLAYZ) as i16,
        last_depth_change: retail.peek16(WORK_RAM | RETAIL_LASTZCHANGE) as i16,
        player_hit_timer: retail.peek8(WORK_RAM | RETAIL_POOL.base + AL_SBYTE1),
        player_hit_flags: retail
            .peek8(WORK_RAM | RETAIL_POOL.base + RETAIL_OBJECT_HIT_FLAGS_OFFSET),
        player_body_durability: retail.peek8(
            WORK_RAM
                | (RETAIL_POOL.base
                    + u32::try_from(PLAYER_BODY_SLOT).expect("player body slot")
                        * RETAIL_POOL.stride
                    + AL_HP),
        ),
        player_presentation_bytes: [
            retail.peek8(WORK_RAM | RETAIL_PLAYER_PRESENTATION_BYTES),
            retail.peek8(WORK_RAM | RETAIL_PLAYER_PRESENTATION_BYTES + 1),
            retail.peek8(WORK_RAM | RETAIL_PLAYER_PRESENTATION_BYTES + 2),
        ],
        active_order,
        free_order,
        objects: active
            .into_iter()
            .map(|slot| {
                let object = objects[slot as usize];
                let (flat_object_shape, explosion_size) = flat_visual(object.shape);
                let shape = (slot >= STARTUP_ROLE_SLOTS).then_some(flat_object_shape);
                let departure =
                    explosion_size.is_none() && shape == Some(sf_map::consts::sh::MYSHIP_4);
                let path_driven = shape == Some(sf_map::consts::sh::FRIENDSHIP_4);
                let fighter = shape == Some(sf_map::consts::sh::ZACO_5);
                let authored_motion = matches!(
                    flat_object_shape,
                    NATIVE_SHAPE_BOMBER
                        | NATIVE_SHAPE_BOUNCYBALL
                        | NATIVE_SHAPE_ENEMY_LASER
                        | NATIVE_SHAPE_KAMIKAZE
                        | NATIVE_SHAPE_ZACO_A
                        | NATIVE_SHAPE_ZACO_6
                );
                let object_base = RETAIL_POOL.base + u32::from(slot) * RETAIL_POOL.stride;
                LevelObjectSnapshot {
                    slot,
                    shape,
                    position: object_position(object),
                    explosion_size,
                    durability: retail.peek8(WORK_RAM | object_base + AL_HP),
                    hit_flags: retail
                        .peek8(WORK_RAM | object_base + RETAIL_OBJECT_HIT_FLAGS_OFFSET),
                    collision_disabled: retail.peek8(WORK_RAM | object_base + AL_SFLAGS2)
                        & ASF2_COLLDISABLE
                        != 0,
                    damage_immune: retail.peek8(WORK_RAM | object_base + AL_SFLAGS3)
                        & ASF3_NOHITAFFECT
                        != 0,
                    departure_lifetime: departure.then(|| {
                        retail.peek8(WORK_RAM | object_base + RETAIL_OBJECT_LIFETIME_OFFSET)
                    }),
                    departure_delay: departure
                        .then(|| retail.peek8(WORK_RAM | object_base + RETAIL_OBJECT_DELAY_OFFSET)),
                    path_wait: path_driven
                        .then(|| retail.peek8(WORK_RAM | object_base + AL_SBYTE3)),
                    fighter_motion: fighter.then(|| FighterMotion {
                        rotation: [
                            retail.peek8(WORK_RAM | object_base + AL_ROTX),
                            retail.peek8(WORK_RAM | object_base + AL_ROTY),
                            retail.peek8(WORK_RAM | object_base + AL_ROTZ),
                        ],
                        speed: retail.peek8(WORK_RAM | object_base + AL_VEL),
                        velocity: Position(
                            retail.peek16(WORK_RAM | object_base + AL_VX) as i16,
                            retail.peek16(WORK_RAM | object_base + AL_VY) as i16,
                            retail.peek16(WORK_RAM | object_base + AL_VZ) as i16,
                        ),
                        lateral_offset: retail.peek16(WORK_RAM | object_base + AL_PTR) as i16,
                        vertical_offset: retail.peek16(WORK_RAM | object_base + AL_SWORD2) as i16,
                    }),
                    authored_motion: authored_motion.then(|| AuthoredMotion {
                        rotation: [
                            retail.peek8(WORK_RAM | object_base + AL_ROTX),
                            retail.peek8(WORK_RAM | object_base + AL_ROTY),
                            retail.peek8(WORK_RAM | object_base + AL_ROTZ),
                        ],
                        speed: retail.peek8(WORK_RAM | object_base + AL_VEL),
                        velocity: Position(
                            retail.peek16(WORK_RAM | object_base + AL_VX) as i16,
                            retail.peek16(WORK_RAM | object_base + AL_VY) as i16,
                            retail.peek16(WORK_RAM | object_base + AL_VZ) as i16,
                        ),
                    }),
                }
            })
            .collect(),
    }
}

fn native_level_snapshot(native: &Shell) -> LevelSnapshot {
    let camera = native.frame().camera;
    let active_order = native.game.objs.active_indices();
    let free_order = native_free_order(native);
    let mut active = active_order.clone();
    active.sort_unstable();
    LevelSnapshot {
        background: native.game.vars.currentbg,
        game_frame: native.game.vars.gameframe,
        game_flags: native.game.vars.gameflags,
        player_ship_flags: [
            native.game.vars.pshipflags,
            native.game.vars.pshipflags2,
            native.game.vars.pshipflags3,
        ],
        player_strategy_flags: native.game.vars.pstratflags,
        player_fly_mode: native.game.vars.playerflymode,
        player_object: native.game.vars.player_object as u16,
        map_countdown: native.game.vars.mapcnt,
        view_kind: native.game.vars.strategy.view_kind,
        player_view_position: Position(
            native.game.vars.strategy.player_view_position[0],
            native.game.vars.strategy.player_view_position[1],
            native.game.vars.strategy.player_view_position[2],
        ),
        view_float: [
            native.game.vars.strategy.view_float_x,
            native.game.vars.strategy.view_float_y,
        ],
        view_shake: [
            native.game.vars.strategy.view_shake[0] as i8,
            native.game.vars.strategy.view_shake[1] as i8,
            native.game.vars.strategy.view_shake[2] as i8,
        ],
        view_position: Position(
            (camera.x >> 16) as i16,
            (camera.y >> 16) as i16,
            (camera.z >> 16) as i16,
        ),
        view_pitch: native.game.vars.strategy.view_pitch,
        view_yaw: native.game.vars.strategy.view_yaw,
        effective_view_yaw: camera.rotation[1] as i16,
        view_distance: native.game.vars.strategy.view_distance,
        forward_velocity: native.game.vars.pviewvelz,
        previous_player_depth: native.game.world.lastplayz,
        last_depth_change: native.game.world.lastzchange,
        player_hit_timer: native.game.objs.aliens[0].sbyte1,
        player_hit_flags: native.game.objs.aliens[0].hitflags,
        player_body_durability: native.game.objs.aliens[PLAYER_BODY_SLOT].hp,
        player_presentation_bytes: native.game.vars.strategy.player_bytes,
        active_order,
        free_order,
        objects: active
            .into_iter()
            .map(|slot| {
                let object = native.game.objs.aliens[slot as usize];
                let explosion_size = match object.visual_kind {
                    ObjectVisualKind::ExplosionEnvelope(size) => Some(size),
                    ObjectVisualKind::Mesh | ObjectVisualKind::ScaledSprite => None,
                };
                let departure = explosion_size.is_none()
                    && slot >= STARTUP_ROLE_SLOTS
                    && object.shape == sf_map::consts::sh::MYSHIP_4;
                let path_driven =
                    slot >= STARTUP_ROLE_SLOTS && object.shape == sf_map::consts::sh::FRIENDSHIP_4;
                let fighter =
                    slot >= STARTUP_ROLE_SLOTS && object.shape == sf_map::consts::sh::ZACO_5;
                let authored_motion = matches!(
                    object.shape,
                    NATIVE_SHAPE_BOMBER
                        | NATIVE_SHAPE_BOUNCYBALL
                        | NATIVE_SHAPE_ENEMY_LASER
                        | NATIVE_SHAPE_KAMIKAZE
                        | NATIVE_SHAPE_ZACO_A
                        | NATIVE_SHAPE_ZACO_6
                );
                LevelObjectSnapshot {
                    slot,
                    shape: (slot >= STARTUP_ROLE_SLOTS).then_some(object.shape),
                    position: Position(object.worldx, object.worldy, object.worldz),
                    explosion_size,
                    durability: object.hp,
                    hit_flags: object.hitflags,
                    collision_disabled: object.sflags2 & ASF2_COLLDISABLE != 0,
                    damage_immune: object.sflags3 & ASF3_NOHITAFFECT != 0,
                    departure_lifetime: departure.then_some(object.count),
                    departure_delay: departure.then_some(object.sbyte1),
                    path_wait: path_driven.then_some(object.sbyte3),
                    fighter_motion: fighter.then_some(FighterMotion {
                        rotation: [object.rotx, object.roty, object.rotz],
                        speed: object.vel,
                        velocity: Position(object.vx, object.vy, object.vz),
                        lateral_offset: object.ptr as i16,
                        vertical_offset: object.sword2,
                    }),
                    authored_motion: authored_motion.then_some(AuthoredMotion {
                        rotation: [object.rotx, object.roty, object.rotz],
                        speed: object.vel,
                        velocity: Position(object.vx, object.vy, object.vz),
                    }),
                }
            })
            .collect(),
    }
}

fn native_scenario_frame(
    native: &Shell,
    identities: &mut ObjectIdentityTracker,
    tick: u32,
    input: u16,
    phase: Option<FrontEndPhase>,
) -> SemanticFrame {
    let frame = scenario_frame(tick, input, phase);
    if tick < FIRST_LEVEL_STATE_COMPARISON_TICK {
        return frame;
    }
    identities.record_level(frame, &native_level_snapshot(native), native.game.vars.rng)
}

fn assert_semantic_checkpoint(producer: &str, tick: u32, frame: &SemanticFrame, expected: &str) {
    let actual = semantic_frame_sha256(frame).expect("semantic checkpoint fingerprint");
    assert_eq!(
        actual, expected,
        "{producer} semantic checkpoint changed at tick {tick}"
    );
}

fn configured_native_shell() -> Shell {
    let mut native = Shell::new();
    native.set_register_strats(Box::new(sf_strat::table::register_all));
    native.set_spawn_player(Box::new(sf_strat::player::strat_spawn_player));
    native.set_advance_startup_player(Box::new(
        sf_strat::player::advance_player_during_level_initialization,
    ));
    native.set_initialize_player(Box::new(sf_strat::player::initialize_player_for_map));
    native.set_prepare_presentation_player(Box::new(sf_strat::player::prepare_presentation_player));
    native.set_prepare_restart_player(Box::new(
        sf_strat::player::prepare_checkpoint_restart_player,
    ));
    native.set_shape_extents(sf_render::shapes::sf1_shape_half_extents());
    native
}

#[test]
fn native_corneria_startup_retains_certified_checkpoints() {
    let mut native = configured_native_shell();
    let final_tick = STARTUP_CHECKPOINTS.last().expect("startup checkpoints").0;
    for tick in 0..=final_tick {
        native.tick(front_end_input(tick));
        if let Some((_, expected)) = STARTUP_CHECKPOINTS
            .iter()
            .find(|(checkpoint, _)| *checkpoint == tick)
        {
            assert_eq!(
                native_startup_snapshot(&native),
                *expected,
                "native startup tick {tick}"
            );
        }
    }
}

#[test]
fn native_corneria_retains_certified_semantic_checkpoints() {
    let mut native = configured_native_shell();
    let mut identities = ObjectIdentityTracker::default();
    let final_tick = CORNERIA_SEMANTIC_CHECKPOINTS
        .last()
        .expect("semantic checkpoints")
        .0;

    for tick in 0..=final_tick {
        let input = front_end_input(tick);
        if !NATIVE_REPLAY_LEVEL_UPDATE_PAUSE_TICKS.contains(&tick) {
            native.tick(input);
        }
        let frame = native_scenario_frame(
            &native,
            &mut identities,
            tick,
            input,
            native_front_end_phase(&native),
        );
        if let Some((_, expected)) = CORNERIA_SEMANTIC_CHECKPOINTS
            .iter()
            .find(|(checkpoint, _)| *checkpoint == tick)
        {
            assert_semantic_checkpoint("native replay", tick, &frame, expected);
        }
    }
}

#[test]
fn native_corneria_initializes_both_authored_kamikazes() {
    // The certified front-end and launch presentation now reaches this map
    // pair at tick 1686; keep a small deterministic margin around that birth.
    const KAMIKAZE_PROBE_TICKS: u32 = 1_700;

    let mut native = configured_native_shell();
    for tick in 0..KAMIKAZE_PROBE_TICKS {
        native.tick(front_end_input(tick));
        let kamikazes: Vec<_> = native
            .game
            .objs
            .active_indices()
            .into_iter()
            .filter(|&slot| native.game.objs.aliens[slot as usize].shape == NATIVE_SHAPE_KAMIKAZE)
            .collect();
        if kamikazes.len() == 2 {
            for slot in kamikazes {
                assert_eq!(
                    native.game.objs.aliens[slot as usize].rotz,
                    sf_strat::enemy_a::DEG90,
                    "Corneria kamikaze slot {slot} skipped its source initializer at tick {tick}"
                );
            }
            return;
        }
    }
    panic!("Corneria did not spawn both authored kamikazes");
}

#[test]
fn retail_direct_explosion_headers_match_small_native_shapes() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("retail explosion-header check skipped: Star Fox retail ROM not found");
        return;
    };
    let retail = RetailMachine::new(rom);

    for (retail_shape, expected_size, expected_extent) in [
        (
            RETAIL_DIRECT_SHAPE_SMALL_EXPLOSION_ENVELOPE,
            ExplosionSize::Small,
            50,
        ),
        (
            RETAIL_DIRECT_SHAPE_MEDIUM_EXPLOSION_ENVELOPE,
            ExplosionSize::Medium,
            90,
        ),
        (
            RETAIL_DIRECT_SHAPE_LARGE_EXPLOSION_ENVELOPE,
            ExplosionSize::Large,
            200,
        ),
        (
            RETAIL_DIRECT_SHAPE_OVERSIZED_EXPLOSION_ENVELOPE,
            ExplosionSize::Oversized,
            1_000,
        ),
    ] {
        let retail_shape = u32::from(retail_shape);
        assert_eq!(
            retail.peek8(retail_shape + RETAIL_SHAPE_COORDINATE_SHIFT_OFFSET),
            0,
            "retail {:?} envelope coordinate shift changed at {retail_shape:#06X}",
            expected_size
        );
        assert_eq!(
            retail.peek16(retail_shape + RETAIL_SHAPE_VISUAL_EXTENT_OFFSET),
            expected_extent,
            "retail {:?} envelope visual extent changed at {retail_shape:#06X}",
            expected_size
        );
    }

    for (retail_shape, native_shape) in [
        (
            RETAIL_DIRECT_SHAPE_SMALL_EXPLOSION_SPRITE,
            NATIVE_SHAPE_SMALL_EXPLOSION_SPRITE,
        ),
        (
            RETAIL_DIRECT_SHAPE_SMALL_EXPLOSION_POLYGONS,
            NATIVE_SHAPE_SMALL_EXPLOSION_POLYGONS,
        ),
    ] {
        let native_metrics = sf_core::sf1_shape_metrics::sf1_shape_metrics(native_shape)
            .expect("native explosion shape must have generated ShapeHdr metrics");
        let retail_shape = u32::from(retail_shape);
        assert_eq!(
            retail.peek8(retail_shape + RETAIL_SHAPE_COORDINATE_SHIFT_OFFSET),
            native_metrics.coordinate_shift,
            "retail explosion coordinate shift changed at {retail_shape:#06X}"
        );
        assert_eq!(
            retail.peek16(retail_shape + RETAIL_SHAPE_VISUAL_EXTENT_OFFSET),
            native_metrics.visual_extent,
            "retail explosion visual extent changed at {retail_shape:#06X}"
        );
    }
}

#[test]
fn retail_front_end_and_corneria_opening_match_native_semantic_state() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("retail front-end trace skipped: Star Fox retail ROM not found");
        return;
    };
    let retail_rom_sha256 = format!("{:x}", Sha256::digest(&rom));
    assert_eq!(
        retail_rom_sha256, RETAIL_ROM_SHA256,
        "retail scenario requires the pinned Star Fox USA Rev 2 ROM"
    );
    let manifest = front_end_manifest();

    let mut retail = RetailMachine::new(rom);
    for (entry, opcode) in RETAIL_PLANET_PHASE_ENTRY_OPCODES {
        assert_eq!(
            retail.peek8(entry),
            opcode,
            "retail planet phase entry moved at {entry:#08X}"
        );
    }
    let retail_source_entries: Vec<_> = RETAIL_SOURCE_EDGE_COVERAGE
        .iter()
        .map(|(entry, _)| *entry)
        .collect();
    retail.watch_cpu_execution(&retail_source_entries);

    let mut native = configured_native_shell();
    let mut retail_trace = Vec::new();
    let mut native_trace = Vec::new();
    let mut retail_scenario_frames = Vec::new();
    let mut native_scenario_frames = Vec::new();
    let mut retail_identities = ObjectIdentityTracker::default();
    let mut native_identities = ObjectIdentityTracker::default();
    let mut previous_retail = None;
    let mut previous_native = None;
    let mut retail_origin = None;
    let mut native_origin = None;
    let mut retail_phase_tracker = RetailPhaseTracker::default();
    let mut previous_retail_level_frame = None;
    let mut retail_level_boundary_aligned = false;
    let mut retail_coverage = BTreeSet::new();
    let mut native_coverage = BTreeSet::new();

    for tick in 0..corneria_scenario_ticks() {
        let input = front_end_input(tick);
        let retail_entry_motion_refreshes = retail.peek8(WORK_RAM | sf_oracle::RETAIL_FRAMERATE);
        let native_level_active = native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let align_completed_level_frame =
            native_level_active && tick >= COMPLETED_FRAME_ALIGNMENT_TICK;
        if align_completed_level_frame {
            if !retail_level_boundary_aligned {
                assert!(
                    retail
                        .tick_until_cpu_execution(
                            input,
                            RETAIL_DOSTRATS,
                            MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
                        )
                        .expect("retail initial level boundary"),
                    "retail did not reach the initial level boundary at tick {tick}"
                );
                retail_level_boundary_aligned = true;
            }
            let max_video_frames = if tick == CORNERIA_AUDIO_UPLOAD_TICK {
                MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD
            } else {
                MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE
            };
            assert!(
                retail
                    .tick_until_cpu_execution(input, RETAIL_DOSTRATS, max_video_frames,)
                    .expect("retail complete level boundary"),
                "retail level frame did not reach its next entry boundary at tick {tick}"
            );
        } else {
            retail
                .tick_video_frames(input, VIDEO_FRAMES_PER_NATIVE_TICK)
                .expect("retail front-end trace");
        }
        let retail_execution_entries = retail.take_cpu_execution_watch_hits();
        record_retail_source_edge_coverage(&mut retail_coverage, &retail_execution_entries);
        let retail_level_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        let retail_completed_level_update = align_completed_level_frame
            || previous_retail_level_frame
                .map(|previous| previous != retail_level_frame)
                .unwrap_or(true);
        if !native_level_active || retail_completed_level_update {
            native.tick(input);
        }
        if native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel
        {
            previous_retail_level_frame = Some(retail_level_frame);
        }

        if let Some((_, expected)) = STARTUP_CHECKPOINTS
            .iter()
            .find(|(checkpoint, _)| *checkpoint == tick)
        {
            let retail_snapshot = retail_startup_snapshot(&retail);
            let native_snapshot = native_startup_snapshot(&native);
            assert_eq!(retail_snapshot, *expected, "retail startup tick {tick}");
            assert_eq!(native_snapshot, *expected, "native startup tick {tick}");
            assert_eq!(
                native_snapshot, retail_snapshot,
                "startup parity tick {tick}"
            );
        }

        let mut retail_level_evidence = None;
        let mut retail_random_evidence = None;
        if tick >= FIRST_LEVEL_STATE_COMPARISON_TICK {
            let native_snapshot = native_level_snapshot(&native);
            let retail_snapshot = retail_level_snapshot(&retail);
            retail_coverage.insert(coverage_point(
                RETAIL_COVERAGE_PRODUCER,
                COVERAGE_CORNERIA_LEVEL_STATE,
            ));
            native_coverage.insert(coverage_point(
                NATIVE_COVERAGE_PRODUCER,
                COVERAGE_CORNERIA_LEVEL_STATE,
            ));
            let retail_kamikazes = retail_snapshot
                .objects
                .iter()
                .filter(|object| object.shape == Some(NATIVE_SHAPE_KAMIKAZE))
                .count();
            let native_kamikazes = native_snapshot
                .objects
                .iter()
                .filter(|object| object.shape == Some(NATIVE_SHAPE_KAMIKAZE))
                .count();
            if retail_kamikazes == AUTHORED_KAMIKAZE_COUNT {
                retail_coverage.insert(coverage_point(
                    RETAIL_COVERAGE_PRODUCER,
                    COVERAGE_CORNERIA_KAMIKAZE_WAVE,
                ));
            }
            if native_kamikazes == AUTHORED_KAMIKAZE_COUNT {
                native_coverage.insert(coverage_point(
                    NATIVE_COVERAGE_PRODUCER,
                    COVERAGE_CORNERIA_KAMIKAZE_WAVE,
                ));
            }
            if retail_snapshot.player_body_durability < sf_game::coldet::PCBOX_BODY_HP {
                retail_coverage.insert(coverage_point(
                    RETAIL_COVERAGE_PRODUCER,
                    COVERAGE_PLAYER_BODY_DAMAGE,
                ));
            }
            if native_snapshot.player_body_durability < sf_game::coldet::PCBOX_BODY_HP {
                native_coverage.insert(coverage_point(
                    NATIVE_COVERAGE_PRODUCER,
                    COVERAGE_PLAYER_BODY_DAMAGE,
                ));
            }
            let retail_random_state = [
                retail.peek8(WORK_RAM | RETAIL_RAND),
                retail.peek8(WORK_RAM | RETAIL_RAND + 1),
                retail.peek8(WORK_RAM | RETAIL_RAND + 2),
                retail.peek8(WORK_RAM | RETAIL_RAND + 3),
            ];
            retail_level_evidence = Some(retail_snapshot);
            retail_random_evidence = Some(retail_random_state);
        }

        let retail_phase = retail_front_end_phase(
            &retail,
            &mut retail_phase_tracker,
            &retail_execution_entries,
        );
        let native_phase = native_front_end_phase(&native);
        record_phase_coverage(&mut retail_coverage, RETAIL_COVERAGE_PRODUCER, retail_phase);
        record_phase_coverage(&mut native_coverage, NATIVE_COVERAGE_PRODUCER, native_phase);
        let retail_frame = scenario_frame(tick, input, retail_phase);
        retail_scenario_frames.push(
            match (retail_level_evidence.as_ref(), retail_random_evidence) {
                (Some(snapshot), Some(random_state)) => {
                    retail_identities.record_level(retail_frame, snapshot, random_state)
                }
                _ => retail_frame,
            },
        );
        native_scenario_frames.push(native_scenario_frame(
            &native,
            &mut native_identities,
            tick,
            input,
            native_phase,
        ));
        record_event_coverage(
            &mut retail_coverage,
            RETAIL_COVERAGE_PRODUCER,
            retail_scenario_frames
                .last()
                .expect("retail scenario frame"),
        );
        record_event_coverage(
            &mut native_coverage,
            NATIVE_COVERAGE_PRODUCER,
            native_scenario_frames
                .last()
                .expect("native scenario frame"),
        );
        if let Some(divergence) = first_divergence(
            std::slice::from_ref(
                retail_scenario_frames
                    .last()
                    .expect("retail scenario frame"),
            ),
            std::slice::from_ref(
                native_scenario_frames
                    .last()
                    .expect("native scenario frame"),
            ),
        )
        .expect("live scenario frames must be valid")
        {
            if attack_carrier_trace_enabled() {
                let retail_snapshot = retail_level_evidence
                    .as_ref()
                    .expect("retail level evidence at route divergence");
                let native_snapshot = native_level_snapshot(&native);
                let retail_player = retail_snapshot
                    .objects
                    .iter()
                    .find(|object| object.slot == retail_snapshot.player_object);
                let native_player = native_snapshot
                    .objects
                    .iter()
                    .find(|object| object.slot == native_snapshot.player_object);
                eprintln!(
                    "attack_carrier_route_divergence tick={tick} input={input} retail_frame={} native_frame={} retail_entry_motion_refreshes={retail_entry_motion_refreshes} retail_next_motion_refreshes={} native_motion_refreshes={} retail_player={retail_player:?} native_player={native_player:?} retail_pview={:?} native_pview={:?} retail_plroty={} native_plroty={} retail_plrotz={} native_plrotz={} retail_ztilt={} native_ztilt={} retail_gsu_runs={:?}",
                    retail_snapshot.game_frame,
                    native_snapshot.game_frame,
                    retail.peek8(WORK_RAM | sf_oracle::RETAIL_FRAMERATE),
                    native.game.vars.strategy.frame_rate,
                    retail_snapshot.player_view_position,
                    native_snapshot.player_view_position,
                    retail.peek16(WORK_RAM | sf_oracle::RETAIL_PLROTY) as i16,
                    native.game.vars.sv_i16(sf_strat::common::sv::PLROTY),
                    retail.peek16(WORK_RAM | sf_oracle::RETAIL_PLROTZ) as i16,
                    native.game.vars.sv_i16(sf_strat::common::sv::PLROTZ),
                    retail.peek8(WORK_RAM | RETAIL_PLAYER_DEPTH_TILT) as i8,
                    native.game.vars.sv_u8(sf_strat::common::sv::PLAYER_ZTILT) as i8,
                    retail
                        .gsu_recent_runs()
                        .into_iter()
                        .rev()
                        .take(8)
                        .collect::<Vec<_>>(),
                );
            }
            panic!("live retail/native divergence: {divergence}");
        }
        if let Some((_, expected)) = (!attack_carrier_trace_enabled())
            .then_some(&CORNERIA_SEMANTIC_CHECKPOINTS)
            .into_iter()
            .flatten()
            .find(|(checkpoint, _)| *checkpoint == tick)
        {
            assert_semantic_checkpoint(
                "native",
                tick,
                native_scenario_frames
                    .last()
                    .expect("native scenario frame"),
                expected,
            );
            assert_semantic_checkpoint(
                "retail",
                tick,
                retail_scenario_frames
                    .last()
                    .expect("retail scenario frame"),
                expected,
            );
        }
        record_front_end_transition(
            &mut retail_trace,
            &mut previous_retail,
            &mut retail_origin,
            tick,
            retail_phase,
        );
        record_front_end_transition(
            &mut native_trace,
            &mut previous_native,
            &mut native_origin,
            tick,
            native_phase,
        );

        if let Some((_, expected_cursor)) = PEPPER_CURSOR_CHECKPOINTS
            .iter()
            .find(|(checkpoint, _)| *checkpoint == tick)
        {
            assert_eq!(
                retail.peek8(RETAIL_PEPPER_CHARACTERS),
                *expected_cursor,
                "retail Pepper cursor checkpoint changed at tick {tick}"
            );
            assert_eq!(
                native.frame().planet_presentation.briefing_characters,
                *expected_cursor,
                "native Pepper cursor diverged at tick {tick}"
            );
        }
    }

    let channels: BTreeSet<_> = [
        CaptureChannel::SemanticState,
        CaptureChannel::ObjectLifecycle,
        CaptureChannel::Coverage,
    ]
    .into_iter()
    .collect();
    let retail_evidence = ScenarioEvidence {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        scenario_id: corneria_scenario_id().to_owned(),
        producer: EvidenceProducer::Retail,
        retail_rom_sha256: retail_rom_sha256.clone(),
        clock: ScenarioClock::logical_update(),
        channels: channels.clone(),
        coverage: retail_coverage,
        non_strict: NonStrictEvidence::default(),
        frames: retail_scenario_frames,
    };
    let native_evidence = ScenarioEvidence {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        scenario_id: corneria_scenario_id().to_owned(),
        producer: EvidenceProducer::Native,
        retail_rom_sha256,
        clock: ScenarioClock::logical_update(),
        channels,
        coverage: native_coverage,
        non_strict: NonStrictEvidence::default(),
        frames: native_scenario_frames,
    };
    let report = compare_scenario(&manifest, &retail_evidence, &native_evidence)
        .expect("front-end scenario evidence must be structurally valid");
    assert!(
        report.strict_pass,
        "strict front-end scenario failed: {report:#?}"
    );

    if let Some(divergence) =
        first_divergence(&retail_trace, &native_trace).expect("front-end traces must be valid")
    {
        panic!("retail front-end trace diverged: {divergence}");
    }
    assert_eq!(
        retail_trace.len(),
        FRONT_END_TRANSITIONS,
        "trace must reach the initialized retail Corneria opening"
    );
    assert!(
        previous_retail_level_frame >= Some(CERTIFIED_CORNERIA_LEVEL_FRAME),
        "trace must compare Corneria through certified level frame {CERTIFIED_CORNERIA_LEVEL_FRAME}"
    );
    if attack_carrier_trace_enabled() {
        assert!(
            native.game.objs.aliens.iter().any(|object| object.active
                && object.shape == sf_oracle::sf1_input::CORNERIA_ATTACK_CARRIER_SHAPE),
            "native controller tape did not reach the Corneria Attack Carrier"
        );
        assert!(
            retail_level_snapshot(&retail).objects.iter().any(|object| {
                object.shape == Some(sf_oracle::sf1_input::CORNERIA_ATTACK_CARRIER_SHAPE)
            }),
            "retail controller tape did not reach the Corneria Attack Carrier"
        );
    }
}
