//! Read-only entry into the first complete retail Corneria timing interval.

use sf_oracle::{
    sf1_input::corneria_front_end_input, RetailMachine, RETAIL_DOSTRATS, RETAIL_GAMEFRAME,
};

const WORK_RAM: u32 = 0x7E_0000;
const VIDEO_FRAMES_PER_FRONT_END_TICK: u32 = 3;
const FRONT_END_VIDEO_FRAME_BUDGET: u64 = 3_600;
const RETAIL_CORNERIA_GAME_START: u32 = 0x03_C437;
pub const FRAME_COUNTER_RESET_COMPLETE: u32 = 0x02_D963;
pub const FRAME_RATE_SAMPLE_COMPLETE: u32 = 0x02_DA7E;

pub fn enter_first_corneria_interval(retail: &mut RetailMachine) -> Result<(), String> {
    // Follow source entries, not a recorded boot duration. Inputs are tied to
    // display frames, as in mesen_corneria_timing_oracle.lua. No game state is
    // injected and no native-game observation controls the original.
    let mut entered_corneria = false;
    while retail.video_frame() < FRONT_END_VIDEO_FRAME_BUDGET {
        let tick = retail.video_frame() as u32 / VIDEO_FRAMES_PER_FRONT_END_TICK;
        let boundary = if entered_corneria {
            FRAME_COUNTER_RESET_COMPLETE
        } else {
            RETAIL_CORNERIA_GAME_START
        };
        let reached = retail.tick_until_cpu_execution(
            corneria_front_end_input(tick),
            boundary,
            VIDEO_FRAMES_PER_FRONT_END_TICK,
        )?;
        if reached {
            if entered_corneria && retail.peek16(WORK_RAM | RETAIL_GAMEFRAME) == 0 {
                return Ok(());
            }
            entered_corneria = true;
        }
    }
    Err(format!(
        "retail did not reach Corneria's first counter reset: video={}, pc={:06X}, scene={}, entered={entered_corneria}",
        retail.video_frame(), retail.pc(), retail.peek16(WORK_RAM | RETAIL_GAMEFRAME),
    ))
}

/// Reach the first actual strategy visit without advancing it or copying any
/// reference state into the port. Setup duration is a separate front-end gate.
pub fn enter_first_corneria_update(retail: &mut RetailMachine) -> Result<(), String> {
    enter_first_corneria_interval(retail)?;
    if !retail.tick_until_cpu_execution(0, RETAIL_DOSTRATS, 12)? {
        return Err("retail did not reach Corneria's first strategy visit".into());
    }
    let scene = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
    if scene != 0 {
        return Err(format!(
            "first Corneria strategy visit has unexpected scene {scene}"
        ));
    }
    Ok(())
}

/// Advance the typed native front end under the same controller schedule,
/// independently of the original's setup duration. Return before scene one.
pub fn enter_native_corneria_update(native: &mut sf_game::shell::Shell) -> Result<u32, String> {
    use sf_game::shell::{GameState, GameplayEntryPhase};
    let tick_budget = FRONT_END_VIDEO_FRAME_BUDGET as u32 / VIDEO_FRAMES_PER_FRONT_END_TICK;
    for tick in 0..tick_budget {
        native.tick(corneria_front_end_input(tick));
        if native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel
        {
            if native.game.vars.gameframe != 0 {
                return Err(format!(
                    "native Corneria entry has unexpected scene {}",
                    native.game.vars.gameframe,
                ));
            }
            return Ok(tick + 1);
        }
    }
    Err("native front end did not reach Corneria's first strategy visit".into())
}
