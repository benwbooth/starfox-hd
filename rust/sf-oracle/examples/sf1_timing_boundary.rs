//! Read-only timing diagnostic for the front-end/gameplay handoff.
//!
//! Uses the same input and synchronization as the semantic scenario, but
//! reports refresh-counter writes as well as sampled state. No retail state
//! is injected into the native game, and this probe is not a parity gate.

#[path = "support/mod.rs"]
mod support;

use sf_game::shell::{GameState, GameplayEntryPhase};
use sf_oracle::{
    load_retail_rom, sf1_input::corneria_front_end_input, RetailMachine, RETAIL_DOSTRATS,
    RETAIL_FRAMERATE, RETAIL_GAMEFRAME,
};

fn main() {
    let mut retail = RetailMachine::new(load_retail_rom().expect("Star Fox retail ROM"));
    let mut native = support::configured_shell();
    let mut previous_frame = None;
    let mut boundary_aligned = false;
    for tick in 0..930 {
        let input = corneria_front_end_input(tick);
        let active = native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let aligned = active && tick >= 900;
        let retail_before = retail.peek8(0x7E_0000 | RETAIL_FRAMERATE);
        let native_before = native.game.vars.strategy.frame_rate;
        if tick >= 880 {
            retail.arm_wram_write_watch(RETAIL_FRAMERATE);
        }
        if aligned {
            if !boundary_aligned {
                assert!(retail
                    .tick_until_cpu_execution(input, RETAIL_DOSTRATS, 12)
                    .unwrap());
                boundary_aligned = true;
            }
            assert!(retail
                .tick_until_cpu_execution(input, RETAIL_DOSTRATS, 12)
                .unwrap());
        } else {
            retail.tick_video_frames(input, 3).unwrap();
        }
        let frame = retail.peek16(0x7E_0000 | RETAIL_GAMEFRAME);
        let completed = aligned
            || previous_frame
                .map(|previous| previous != frame)
                .unwrap_or(true);
        let native_advanced = !active || completed;
        if native_advanced {
            native.tick(input);
        }
        if native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel
        {
            previous_frame = Some(frame);
        }
        if tick >= 880 {
            println!(
                "tick={tick} video={} aligned={aligned} native_advanced={native_advanced} retail_scene={frame} native_scene={} retail_before={retail_before} native_before={native_before} retail_after={} native_after={} writes={:?}",
                retail.video_frame(),
                native.game.vars.gameframe,
                retail.peek8(0x7E_0000 | RETAIL_FRAMERATE),
                native.game.vars.strategy.frame_rate,
                retail.take_wram_write_watch(),
            );
        }
    }
}
