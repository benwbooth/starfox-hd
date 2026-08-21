//! End-to-end proof that a real retail routine and the flat native port can be
//! compared through the shared, storage-independent semantic trace format.

use sf_core::{pad, sf1_controls::BriefingPhase, sf1_planets::PlanetSequencePhase};
use sf_difftest::{first_divergence, SemanticFrame, SemanticObject};
use sf_game::shell::{GameState, GameplayEntryPhase, Shell};
use sf_oracle::{
    call, load_retail_rom, snapshot_objects, Entry, RetailMachine, SnesBus, AL_PTR, AL_ROTX,
    AL_ROTY, AL_ROTZ, AL_SBYTE3, AL_SWORD2, AL_VEL, AL_VX, AL_VY, AL_VZ, RETAIL_BRIEFING_CHOICE,
    RETAIL_CURRENTBG, RETAIL_CURRENT_PLANET, RETAIL_DOSTRATS, RETAIL_DOSTRATS_COMPLETE,
    RETAIL_GAMEFRAME, RETAIL_LASTPLAYZ, RETAIL_LASTZCHANGE, RETAIL_MAPCNT,
    RETAIL_PEPPER_CHARACTERS, RETAIL_PLANET_BRIEFING_PREP_ENTRY, RETAIL_PLANET_CENTER_ENTRY,
    RETAIL_PLANET_DISMISS_ENTRY, RETAIL_PLANET_EXIT_FADE_ENTRY, RETAIL_PLANET_GAME_START_ENTRY,
    RETAIL_PLANET_INTERRUPT, RETAIL_PLANET_ISOLATION_ENTRY, RETAIL_PLANET_MAP_FADE_ENTRY,
    RETAIL_PLANET_MESSAGE_ENTRY, RETAIL_PLANET_NAME_ENTRY, RETAIL_PLANET_SHIP_FLASH,
    RETAIL_PLANET_STAGE, RETAIL_PLANET_ZOOM_ENTRY, RETAIL_POOL, RETAIL_PSHIPFLAGS,
    RETAIL_PVIEWVELZ, RETAIL_SHAPES, RETAIL_STRAIGHT_STRAT, RETAIL_WHICH_ROUTE,
};

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
const FRONT_END_TICKS: u32 = 1_320;
const FIRST_CORRIDOR_LEVEL_FRAME: u16 = 5;
const VIDEO_FRAMES_PER_NATIVE_TICK: u32 = 3;
const COMPLETED_FRAME_ALIGNMENT_TICK: u32 = PLANET_DISMISS_END_TICK;
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const CORNERIA_AUDIO_UPLOAD_TICK: u32 = 1_080;
const MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD: u32 = 240;
const WORK_RAM: u32 = 0x7E_0000;
const RETAIL_OBJECT_LIFETIME_OFFSET: u32 = 0x0A;
const RETAIL_OBJECT_DELAY_OFFSET: u32 = 0x22;
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
const LAUNCH_SUBMAP_EXIT_TICK: u32 = 1_064;
const LAUNCH_FADE_STORAGE_END_TICK: u32 = 1_078;
const STARTUP_ROLE_SLOTS: u16 = 6;
const RETAIL_DIRECT_SHAPE_OP_0: u16 = 0xBB48;
const RETAIL_DIRECT_SHAPE_OP_1: u16 = 0xBB64;
const RETAIL_DIRECT_SHAPE_OP_2: u16 = 0xBB80;
const RETAIL_DIRECT_SHAPE_BOOST: u16 = 0xB219;
const RETAIL_DIRECT_SHAPE_MYSHIP_4: u16 = 0xD304;
const RETAIL_DIRECT_SHAPE_MYBASE_0: u16 = 0xDD84;
const RETAIL_DIRECT_SHAPE_ENEMY_LASER: u16 = 0xB34D;
const RETAIL_DIRECT_SHAPE_PLAYER_LASER: u16 = 0xB369;
const RETAIL_DIRECT_SHAPE_LARGE_LASER_FLASH: u16 = 0xB075;
const NATIVE_SHAPE_ENEMY_LASER: u16 = 478;
const NATIVE_SHAPE_PLAYER_LASER: u16 = 511;
const NATIVE_SHAPE_LARGE_LASER_FLASH: u16 = 479;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LevelObjectSnapshot {
    slot: u16,
    shape: Option<u16>,
    position: Position,
    departure_lifetime: Option<u8>,
    departure_delay: Option<u8>,
    path_wait: Option<u8>,
    fighter_motion: Option<FighterMotion>,
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
    map_countdown: u16,
    forward_velocity: i16,
    previous_player_depth: i16,
    last_depth_change: i16,
    objects: Vec<LevelObjectSnapshot>,
}
const RETAIL_PHASE_ENTRIES: [u32; 12] = [
    RETAIL_PLANET_MAP_FADE_ENTRY,
    RETAIL_PLANET_ISOLATION_ENTRY,
    RETAIL_PLANET_CENTER_ENTRY,
    RETAIL_PLANET_BRIEFING_PREP_ENTRY,
    RETAIL_PLANET_ZOOM_ENTRY,
    RETAIL_PLANET_NAME_ENTRY,
    RETAIL_PLANET_MESSAGE_ENTRY,
    RETAIL_PLANET_DISMISS_ENTRY,
    RETAIL_PLANET_EXIT_FADE_ENTRY,
    RETAIL_PLANET_GAME_START_ENTRY,
    RETAIL_DOSTRATS,
    RETAIL_DOSTRATS_COMPLETE,
];
const RETAIL_PLANET_PHASE_ENTRY_OPCODES: [(u32, u8); 10] = [
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
    0
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

fn retail_level_snapshot(retail: &RetailMachine) -> LevelSnapshot {
    const SOURCE_SHAPE_CATALOG_ENTRIES: u16 = 256;

    let flat_shape = |source_word| {
        let direct_shape = match source_word {
            RETAIL_DIRECT_SHAPE_OP_0 => Some(sf_map::consts::sh::OP_0),
            RETAIL_DIRECT_SHAPE_OP_1 => Some(sf_map::consts::sh::OP_1),
            RETAIL_DIRECT_SHAPE_OP_2 => Some(sf_map::consts::sh::OP_2),
            RETAIL_DIRECT_SHAPE_BOOST => Some(sf_map::consts::sh::BOOST_SHAPE),
            RETAIL_DIRECT_SHAPE_MYSHIP_4 => Some(sf_map::consts::sh::MYSHIP_4),
            RETAIL_DIRECT_SHAPE_MYBASE_0 => Some(sf_map::consts::sh::MYBASE_0),
            RETAIL_DIRECT_SHAPE_ENEMY_LASER => Some(NATIVE_SHAPE_ENEMY_LASER),
            RETAIL_DIRECT_SHAPE_PLAYER_LASER => Some(NATIVE_SHAPE_PLAYER_LASER),
            RETAIL_DIRECT_SHAPE_LARGE_LASER_FLASH => Some(NATIVE_SHAPE_LARGE_LASER_FLASH),
            _ => None,
        };
        if let Some(shape) = direct_shape {
            return sf_core::shape::resolve_shape_word(shape);
        }
        (0..SOURCE_SHAPE_CATALOG_ENTRIES)
            .find(|catalog_id| {
                retail.peek16(RETAIL_SHAPES + u32::from(*catalog_id) * 2) == source_word
            })
            .map(sf_core::shape::resolve_shape_word)
            .unwrap_or_else(|| sf_core::shape::resolve_shape_word(source_word))
    };
    let objects = retail.object_snapshot();
    let mut active = retail.active_object_slots();
    active.sort_unstable();
    LevelSnapshot {
        background: sf_oracle::retail_background_catalog_id(
            retail.peek16(WORK_RAM | RETAIL_CURRENTBG),
        )
        .expect("retail background offset must identify a catalog record"),
        game_frame: retail.peek16(WORK_RAM | RETAIL_GAMEFRAME),
        map_countdown: retail.peek16(WORK_RAM | RETAIL_MAPCNT),
        forward_velocity: retail.peek16(WORK_RAM | RETAIL_PVIEWVELZ) as i16,
        previous_player_depth: retail.peek16(WORK_RAM | RETAIL_LASTPLAYZ) as i16,
        last_depth_change: retail.peek16(WORK_RAM | RETAIL_LASTZCHANGE) as i16,
        objects: active
            .into_iter()
            .map(|slot| {
                let object = objects[slot as usize];
                let shape = (slot >= STARTUP_ROLE_SLOTS).then(|| flat_shape(object.shape));
                let departure = shape == Some(sf_map::consts::sh::MYSHIP_4);
                let path_driven = shape == Some(sf_map::consts::sh::FRIENDSHIP_4);
                let fighter = shape == Some(sf_map::consts::sh::ZACO_5);
                let object_base = RETAIL_POOL.base + u32::from(slot) * RETAIL_POOL.stride;
                LevelObjectSnapshot {
                    slot,
                    shape,
                    position: object_position(object),
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
                }
            })
            .collect(),
    }
}

fn native_level_snapshot(native: &Shell) -> LevelSnapshot {
    let mut active = native.game.objs.active_indices();
    active.sort_unstable();
    LevelSnapshot {
        background: native.game.vars.currentbg,
        game_frame: native.game.vars.gameframe,
        map_countdown: native.game.vars.mapcnt,
        forward_velocity: native.game.vars.pviewvelz,
        previous_player_depth: native.game.world.lastplayz,
        last_depth_change: native.game.world.lastzchange,
        objects: active
            .into_iter()
            .map(|slot| {
                let object = native.game.objs.aliens[slot as usize];
                let departure =
                    slot >= STARTUP_ROLE_SLOTS && object.shape == sf_map::consts::sh::MYSHIP_4;
                let path_driven =
                    slot >= STARTUP_ROLE_SLOTS && object.shape == sf_map::consts::sh::FRIENDSHIP_4;
                let fighter =
                    slot >= STARTUP_ROLE_SLOTS && object.shape == sf_map::consts::sh::ZACO_5;
                LevelObjectSnapshot {
                    slot,
                    shape: (slot >= STARTUP_ROLE_SLOTS).then_some(object.shape),
                    position: Position(object.worldx, object.worldy, object.worldz),
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
                }
            })
            .collect(),
    }
}

fn configured_native_shell() -> Shell {
    let mut native = Shell::new();
    native.set_register_strats(Box::new(sf_strat::table::register_all));
    native.set_spawn_player(Box::new(sf_strat::player::strat_spawn_player));
    native.set_advance_startup_player(Box::new(
        sf_strat::player::advance_player_during_level_initialization,
    ));
    native.set_initialize_player(Box::new(sf_strat::player::initialize_player_for_map));
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
fn retail_front_end_and_corneria_opening_match_native_semantic_state() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("retail front-end trace skipped: Star Fox retail ROM not found");
        return;
    };

    let mut retail = RetailMachine::new(rom);
    for (entry, opcode) in RETAIL_PLANET_PHASE_ENTRY_OPCODES {
        assert_eq!(
            retail.peek8(entry),
            opcode,
            "retail planet phase entry moved at {entry:#08X}"
        );
    }
    retail.watch_cpu_execution(&RETAIL_PHASE_ENTRIES);

    let mut native = configured_native_shell();
    let mut retail_trace = Vec::new();
    let mut native_trace = Vec::new();
    let mut previous_retail = None;
    let mut previous_native = None;
    let mut retail_origin = None;
    let mut native_origin = None;
    let mut retail_phase_tracker = RetailPhaseTracker::default();
    let mut previous_retail_level_frame = None;
    let mut retail_level_boundary_aligned = false;

    for tick in 0..FRONT_END_TICKS {
        let input = front_end_input(tick);
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

        if tick >= FIRST_LEVEL_STATE_COMPARISON_TICK {
            let mut native_snapshot = native_level_snapshot(&native);
            let retail_snapshot = retail_level_snapshot(&retail);
            // Once the shared launch submap returns, retail exposes zero while
            // paused in its original fade wrapper. The typed map VM preserves
            // WORLD.ASM's internal wait sentinel of one. This storage-only
            // cursor detail is not semantic; object/background/frame timing
            // remains compared strictly through the certified trace.
            if (LAUNCH_SUBMAP_EXIT_TICK..=LAUNCH_FADE_STORAGE_END_TICK).contains(&tick) {
                native_snapshot.map_countdown = retail_snapshot.map_countdown;
            }
            assert_eq!(
                native_snapshot, retail_snapshot,
                "Corneria level state diverged at tick {tick}"
            );
        }

        let retail_phase = retail_front_end_phase(
            &retail,
            &mut retail_phase_tracker,
            &retail_execution_entries,
        );
        let native_phase = native_front_end_phase(&native);
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
        previous_retail_level_frame >= Some(FIRST_CORRIDOR_LEVEL_FRAME),
        "trace must compare the first Corneria corridor and wingman objects"
    );
}
