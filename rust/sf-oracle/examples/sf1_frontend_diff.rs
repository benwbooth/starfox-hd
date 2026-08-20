//! Compare retail and native SF1 front-end phase timing under one physical
//! input trace. A divergence is expected until the native timing is corrected.

use sf_core::{pad, sf1_controls::BriefingPhase, sf1_planets::PlanetSequencePhase};
use sf_difftest::{first_divergence, SemanticFrame};
use sf_game::shell::{GameState, Shell};
use sf_oracle::{
    RetailMachine, RETAIL_BRIEFING_CHOICE, RETAIL_CURRENTBG, RETAIL_CURRENT_PLANET,
    RETAIL_PLANET_INTERRUPT, RETAIL_PLANET_SHIP_FLASH, RETAIL_PLANET_STAGE, RETAIL_PSHIPFLAGS,
    RETAIL_WHICH_ROUTE,
};
use std::path::Path;
use std::process::ExitCode;

const DEFAULT_TICKS: u32 = 540;
const VIDEO_FRAMES_PER_TICK: u32 = 3;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrontEndPhase {
    AttractIntro,
    Title,
    BriefingControl,
    BriefingDestination,
    PlanetMapSetup,
    RouteSelection,
    RouteConfirmed,
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
            Self::RouteConfirmed => "route-confirmed",
        }
    }
}

fn scripted_input(tick: u32) -> u16 {
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
    0
}

fn retail_phase(machine: &RetailMachine, route_selection_seen: &mut bool) -> Option<FrontEndPhase> {
    match machine.peek16(WORK_RAM | RETAIL_CURRENTBG) {
        RETAIL_ATTRACT_BACKGROUND => Some(FrontEndPhase::AttractIntro),
        RETAIL_TITLE_BACKGROUND => Some(FrontEndPhase::Title),
        RETAIL_BRIEFING_BACKGROUND => {
            let game_selected = machine.peek8(RETAIL_BRIEFING_CHOICE) != 0;
            let planet_interrupt = machine.peek8(WORK_RAM | RETAIL_PLANET_INTERRUPT) != 0;
            let control_disabled =
                machine.peek8(WORK_RAM | RETAIL_PSHIPFLAGS) & BRIEFING_CONTROL_DISABLED_MASK != 0;
            if game_selected && !planet_interrupt {
                let route_ready = machine.peek8(WORK_RAM | RETAIL_WHICH_ROUTE) == INITIAL_ROUTE
                    && machine.peek8(WORK_RAM | RETAIL_PLANET_STAGE) == ROUTE_PREVIEW_STAGE
                    && machine.peek8(WORK_RAM | RETAIL_CURRENT_PLANET) as i8
                        == HIDDEN_CURRENT_PLANET;
                let route_confirmed = machine.peek8(WORK_RAM | RETAIL_PLANET_SHIP_FLASH) != 0
                    && machine.peek8(WORK_RAM | RETAIL_WHICH_ROUTE) == INITIAL_ROUTE
                    && machine.peek8(WORK_RAM | RETAIL_PLANET_STAGE) == 0
                    && machine.peek8(WORK_RAM | RETAIL_CURRENT_PLANET) == 0;
                if route_ready {
                    *route_selection_seen = true;
                }
                Some(if route_confirmed {
                    FrontEndPhase::RouteConfirmed
                } else if *route_selection_seen {
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

fn native_phase(shell: &Shell) -> Option<FrontEndPhase> {
    match shell.state() {
        GameState::AttractIntro => Some(FrontEndPhase::AttractIntro),
        GameState::Title => Some(FrontEndPhase::Title),
        GameState::Briefing => match shell.frame().briefing_phase {
            BriefingPhase::ControlType => Some(FrontEndPhase::BriefingControl),
            BriefingPhase::Destination => Some(FrontEndPhase::BriefingDestination),
        },
        GameState::PlanetSelect => match shell.frame().planet_presentation.phase {
            PlanetSequencePhase::InitialSetup => Some(FrontEndPhase::PlanetMapSetup),
            PlanetSequencePhase::RouteSelection => Some(FrontEndPhase::RouteSelection),
            _ => Some(FrontEndPhase::RouteConfirmed),
        },
        _ => None,
    }
}

fn record_transition(
    trace: &mut Vec<SemanticFrame>,
    previous: &mut Option<FrontEndPhase>,
    origin_tick: &mut Option<u32>,
    tick: u32,
    phase: Option<FrontEndPhase>,
) {
    let Some(phase) = phase else { return };
    if *previous == Some(phase) {
        return;
    }
    let origin = *origin_tick.get_or_insert(tick);
    trace.push(
        SemanticFrame::new(
            trace.len() as u64,
            u64::from(tick.saturating_sub(origin)),
            0,
        )
        .with_field("phase", phase.name()),
    );
    *previous = Some(phase);
}

fn configured_shell() -> Shell {
    let mut shell = Shell::new();
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(|game, map| {
        let _ = sf_strat::player::strat_spawn_player_for_map(game, map);
    }));
    shell
}

fn print_trace(name: &str, trace: &[SemanticFrame]) {
    for frame in trace {
        let phase = frame.fields.get("phase").expect("phase field");
        println!(
            "{name}: transition {} at relative tick {} ({phase:?})",
            frame.sequence, frame.source_frame
        );
    }
}

fn main() -> ExitCode {
    let tick_limit = std::env::args()
        .nth(1)
        .map(|value| value.parse::<u32>().expect("tick limit must be decimal"))
        .unwrap_or(DEFAULT_TICKS);
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace has a repository parent");
    let rom_path = repository.join("Star Fox (USA) (Rev 2).sfc");
    let rom = std::fs::read(&rom_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", rom_path.display()));
    let mut retail = RetailMachine::new(rom);
    let mut native = configured_shell();
    let mut retail_trace = Vec::new();
    let mut native_trace = Vec::new();
    let mut previous_retail = None;
    let mut previous_native = None;
    let mut retail_origin = None;
    let mut native_origin = None;
    let mut retail_route_selection_seen = false;

    for tick in 0..tick_limit {
        let input = scripted_input(tick);
        retail
            .tick_video_frames(input, VIDEO_FRAMES_PER_TICK)
            .unwrap_or_else(|error| panic!("retail machine failed: {error}"));
        native.tick(input);
        record_transition(
            &mut retail_trace,
            &mut previous_retail,
            &mut retail_origin,
            tick,
            retail_phase(&retail, &mut retail_route_selection_seen),
        );
        record_transition(
            &mut native_trace,
            &mut previous_native,
            &mut native_origin,
            tick,
            native_phase(&native),
        );
    }

    print_trace("retail", &retail_trace);
    print_trace("native", &native_trace);
    match first_divergence(&retail_trace, &native_trace) {
        Ok(None) => {
            println!("OK: SF1 front-end semantic phase timing matches retail");
            ExitCode::SUCCESS
        }
        Ok(Some(divergence)) => {
            println!("FIRST FRONT-END DIVERGENCE: {divergence}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("trace error: {error}");
            ExitCode::from(2)
        }
    }
}
