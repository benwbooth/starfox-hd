//! Star Fox HD — Rust port entry point.
//!
//! Ports (C oracle): `src/main.c` (SDL init, fixed 20 Hz tick accumulator
//! with render interpolation), `src/game/boot.c` (game state machine),
//! `src/config.c` (starfox.ini).
//!
//! Until the game core lands, this binary only exercises the headless tick
//! plumbing so `sf-difftest` has something to grow around.

fn main() {
    println!(
        "starfox-hd-rs scaffold (tick = {} ms); game core not ported yet",
        sf_core::GAME_TICK_MS
    );
}
