//! Compare retail and native SF1 front-end phase timing under one physical
//! input trace. A divergence is expected until the native timing is corrected.

use sf_core::pad;
use sf_difftest::{first_divergence, SemanticFrame};
use sf_game::shell::{GameState, Shell};
use sf_oracle::{RetailMachine, RETAIL_CURRENTBG};
use std::path::Path;
use std::process::ExitCode;

const DEFAULT_TICKS: u32 = 320;
const VIDEO_FRAMES_PER_TICK: u32 = 3;
const WORK_RAM: u32 = 0x7E_0000;
const RETAIL_ATTRACT_BACKGROUND: u16 = 243;
const RETAIL_TITLE_BACKGROUND: u16 = 249;
const RETAIL_BRIEFING_BACKGROUND: u16 = 255;
const FRONT_END_CONFIRM_CADENCE_TICKS: u32 = 60;
const FRONT_END_CONFIRM_HOLD_TICKS: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrontEndPhase {
    AttractIntro,
    Title,
    Briefing,
}

impl FrontEndPhase {
    fn name(self) -> &'static str {
        match self {
            Self::AttractIntro => "attract-intro",
            Self::Title => "title",
            Self::Briefing => "briefing",
        }
    }
}

fn scripted_input(tick: u32) -> u16 {
    if tick % FRONT_END_CONFIRM_CADENCE_TICKS < FRONT_END_CONFIRM_HOLD_TICKS {
        pad::START
    } else {
        0
    }
}

fn retail_phase(machine: &RetailMachine) -> Option<FrontEndPhase> {
    match machine.peek16(WORK_RAM | RETAIL_CURRENTBG) {
        RETAIL_ATTRACT_BACKGROUND => Some(FrontEndPhase::AttractIntro),
        RETAIL_TITLE_BACKGROUND => Some(FrontEndPhase::Title),
        RETAIL_BRIEFING_BACKGROUND => Some(FrontEndPhase::Briefing),
        _ => None,
    }
}

fn native_phase(shell: &Shell) -> Option<FrontEndPhase> {
    match shell.state() {
        GameState::AttractIntro => Some(FrontEndPhase::AttractIntro),
        GameState::Title => Some(FrontEndPhase::Title),
        GameState::Briefing => Some(FrontEndPhase::Briefing),
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
            retail_phase(&retail),
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
