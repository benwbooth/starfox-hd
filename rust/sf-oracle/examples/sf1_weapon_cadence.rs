//! Oracle-only cadence census for the Corneria weapon scenario.

mod support;

use sf_oracle::{
    load_retail_rom, RetailMachine, RETAIL_DOSTRATS, RETAIL_FRAMERATE, RETAIL_GAMEFRAME,
};

const WORK_RAM: u32 = 0x7E_0000;
const VIDEO_FRAMES_PER_FRONT_END_TICK: u32 = 3;
const FIRST_CERTIFIED_TICK: u32 = 900;
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD: u32 = 240;
const CORNERIA_AUDIO_UPLOAD_TICK: u32 = 1_080;

fn main() {
    let rom = load_retail_rom().expect("Star Fox retail ROM is required");
    let mut retail = RetailMachine::new(rom);
    for tick in 0..FIRST_CERTIFIED_TICK {
        retail
            .tick_video_frames(support::weapon_input(tick), VIDEO_FRAMES_PER_FRONT_END_TICK)
            .expect("retail front-end cadence");
    }
    assert!(
        retail
            .tick_until_cpu_execution(
                support::weapon_input(FIRST_CERTIFIED_TICK),
                RETAIL_DOSTRATS,
                MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
            )
            .expect("initial Corneria cadence boundary"),
        "retail did not reach the first Corneria cadence boundary"
    );

    let mut previous_rate = None;
    let mut run_start = FIRST_CERTIFIED_TICK;
    let mut run_start_game_frame = 0;
    let mut updates = 0u32;
    for tick in FIRST_CERTIFIED_TICK..=support::WEAPON_TRACE_END_TICK {
        let rate = retail.peek8(WORK_RAM | RETAIL_FRAMERATE);
        let game_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        if previous_rate != Some(rate) {
            if let Some(previous) = previous_rate {
                println!(
                    "ticks={run_start}..{} game_frames={run_start_game_frame}..{} frame_rate={previous}",
                    tick - 1,
                    game_frame.wrapping_sub(1),
                );
            }
            previous_rate = Some(rate);
            run_start = tick;
            run_start_game_frame = game_frame;
        }
        let max_video_frames = if tick == CORNERIA_AUDIO_UPLOAD_TICK {
            MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD
        } else {
            MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE
        };
        assert!(
            retail
                .tick_until_cpu_execution(
                    support::weapon_input(tick.saturating_add(1)),
                    RETAIL_DOSTRATS,
                    max_video_frames,
                )
                .expect("next Corneria cadence boundary"),
            "retail did not reach cadence boundary after tick {tick}"
        );
        updates += 1;
    }
    let last_game_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME).wrapping_sub(1);
    println!(
        "ticks={run_start}..{} game_frames={run_start_game_frame}..{last_game_frame} frame_rate={}",
        support::WEAPON_TRACE_END_TICK,
        previous_rate.expect("at least one cadence sample"),
    );
    println!("sf1_weapon_cadence certified_updates={updates} first_divergence=none");
}
