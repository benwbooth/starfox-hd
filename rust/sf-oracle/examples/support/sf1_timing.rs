//! Read-only entry into the first complete retail Corneria timing interval.

use sf_oracle::{sf1_input::corneria_front_end_input, RetailMachine, RETAIL_GAMEFRAME};

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
