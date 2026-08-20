//! End-to-end proof that a real retail routine and the flat native port can be
//! compared through the shared, storage-independent semantic trace format.

use sf_core::pad;
use sf_difftest::{first_divergence, SemanticFrame, SemanticObject};
use sf_game::shell::{GameState, Shell};
use sf_oracle::{
    call, load_retail_rom, snapshot_objects, Entry, RetailMachine, SnesBus, AL_VX, AL_VY, AL_VZ,
    RETAIL_CURRENTBG, RETAIL_POOL, RETAIL_PVIEWVELZ, RETAIL_STRAIGHT_STRAT,
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
const FRONT_END_TICKS: u32 = 130;
const VIDEO_FRAMES_PER_NATIVE_TICK: u32 = 3;
const WORK_RAM: u32 = 0x7E_0000;
const RETAIL_ATTRACT_BACKGROUND: u16 = 243;
const RETAIL_TITLE_BACKGROUND: u16 = 249;
const FRONT_END_CONFIRM_CADENCE_TICKS: u32 = 60;
const FRONT_END_CONFIRM_HOLD_TICKS: u32 = 2;

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
}

impl FrontEndPhase {
    fn name(self) -> &'static str {
        match self {
            Self::AttractIntro => "attract-intro",
            Self::Title => "title",
        }
    }
}

fn front_end_input(tick: u32) -> u16 {
    if tick % FRONT_END_CONFIRM_CADENCE_TICKS < FRONT_END_CONFIRM_HOLD_TICKS {
        pad::START
    } else {
        0
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
fn retail_boot_and_first_attract_handoff_match_native_semantic_timing() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("retail front-end trace skipped: Star Fox retail ROM not found");
        return;
    };

    let mut retail = RetailMachine::new(rom);
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

    for tick in 0..FRONT_END_TICKS {
        let input = front_end_input(tick);
        retail
            .tick_video_frames(input, VIDEO_FRAMES_PER_NATIVE_TICK)
            .expect("retail front-end trace");
        native.tick(input);

        let retail_phase = match retail.peek16(WORK_RAM | RETAIL_CURRENTBG) {
            RETAIL_ATTRACT_BACKGROUND => Some(FrontEndPhase::AttractIntro),
            RETAIL_TITLE_BACKGROUND => Some(FrontEndPhase::Title),
            _ => None,
        };
        let native_phase = match native.state() {
            GameState::AttractIntro => Some(FrontEndPhase::AttractIntro),
            GameState::Title => Some(FrontEndPhase::Title),
            _ => None,
        };
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
    }

    if let Some(divergence) =
        first_divergence(&retail_trace, &native_trace).expect("front-end traces must be valid")
    {
        panic!("retail boot/attract trace diverged: {divergence}");
    }
    assert_eq!(retail_trace.len(), 2, "trace must reach the retail title");
}
