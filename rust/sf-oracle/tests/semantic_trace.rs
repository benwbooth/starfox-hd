//! End-to-end proof that a real retail routine and the flat native port can be
//! compared through the shared, storage-independent semantic trace format.

use sf_core::{pad, sf1_controls::BriefingPhase, sf1_planets::PlanetSequencePhase};
use sf_difftest::{first_divergence, SemanticFrame, SemanticObject};
use sf_game::shell::{GameState, GameplayEntryPhase, Shell};
use sf_oracle::{
    call, load_retail_rom, snapshot_objects, Entry, RetailMachine, SnesBus, AL_VX, AL_VY, AL_VZ,
    RETAIL_BRIEFING_CHOICE, RETAIL_CURRENTBG, RETAIL_CURRENT_PLANET, RETAIL_DOSTRATS,
    RETAIL_PEPPER_CHARACTERS, RETAIL_PLANET_BRIEFING_PREP_ENTRY, RETAIL_PLANET_CENTER_ENTRY,
    RETAIL_PLANET_DISMISS_ENTRY, RETAIL_PLANET_EXIT_FADE_ENTRY, RETAIL_PLANET_GAME_START_ENTRY,
    RETAIL_PLANET_INTERRUPT, RETAIL_PLANET_ISOLATION_ENTRY, RETAIL_PLANET_MAP_FADE_ENTRY,
    RETAIL_PLANET_MESSAGE_ENTRY, RETAIL_PLANET_NAME_ENTRY, RETAIL_PLANET_SHIP_FLASH,
    RETAIL_PLANET_STAGE, RETAIL_PLANET_ZOOM_ENTRY, RETAIL_POOL, RETAIL_PSHIPFLAGS,
    RETAIL_PVIEWVELZ, RETAIL_STRAIGHT_STRAT, RETAIL_WHICH_ROUTE,
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
const FRONT_END_TICKS: u32 = 920;
const VIDEO_FRAMES_PER_NATIVE_TICK: u32 = 3;
const WORK_RAM: u32 = 0x7E_0000;
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
const RETAIL_PHASE_ENTRIES: [u32; 11] = [
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

#[test]
fn retail_front_end_through_corneria_initialization_matches_native_semantic_timing() {
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

    let mut native = Shell::new();
    native.set_register_strats(Box::new(sf_strat::table::register_all));
    native.set_spawn_player(Box::new(|game, map| {
        let _ = sf_strat::player::strat_spawn_player_for_map(game, map);
    }));
    let mut retail_trace = Vec::new();
    let mut native_trace = Vec::new();
    let mut previous_retail = None;
    let mut previous_native = None;
    let mut retail_origin = None;
    let mut native_origin = None;
    let mut retail_phase_tracker = RetailPhaseTracker::default();

    for tick in 0..FRONT_END_TICKS {
        let input = front_end_input(tick);
        retail
            .tick_video_frames(input, VIDEO_FRAMES_PER_NATIVE_TICK)
            .expect("retail front-end trace");
        let retail_execution_entries = retail.take_cpu_execution_watch_hits();
        native.tick(input);

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
}
