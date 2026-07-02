//! Background request scheduler state.
//!
//! C oracle: `src/game/bgs.c` (BGS.ASM -> C conversion), ported line for
//! line. Operates on the GameVars fields `currentbg` / `bgflags` /
//! `bg_dmalist` / `bgtransspeed` (C `g_currentbg` etc., game_vars.c).

use crate::vars::{GameVars, BGF_BG};

/// C `Bgs_Init()` (src/game/bgs.c:7).
pub fn init(vars: &mut GameVars) {
    vars.bgflags = 0;
    vars.bg_dmalist = 0;
    vars.currentbg = 0;
    vars.bgtransspeed = 0;
}

/// C `Bgs_Update()` (src/game/bgs.c:14).
///
/// Minimal modechange_l behavior: process slow background transition steps
/// on masked game frames. Without SNES bglists/DMA this collapses to a
/// single completion event.
pub fn update(vars: &mut GameVars) {
    if vars.bg_dmalist != 0 {
        // C: ((uint16)g_gameframe & g_bgtransspeed) == 0 (bgs.c:19).
        if (vars.gameframe & vars.bgtransspeed) == 0 {
            vars.bg_dmalist = 0;
            vars.bgflags |= BGF_BG;
        }
    }

    // In the full game this bit triggers the background DMA script runner
    // (bgs.c:26-28).
    if vars.bgflags & BGF_BG != 0 {
        vars.bgflags &= !BGF_BG;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_transition_completes_on_masked_frame() {
        // Mirrors bgs.c: with bgtransspeed 3, completion only fires on
        // gameframe & 3 == 0.
        let mut vars = GameVars::init();
        vars.bg_dmalist = 5;
        vars.bgtransspeed = 3;
        vars.gameframe = 1;
        update(&mut vars);
        assert_eq!(vars.bg_dmalist, 5);
        assert_eq!(vars.bgflags, 0);

        vars.gameframe = 4;
        update(&mut vars);
        assert_eq!(vars.bg_dmalist, 0);
        // BGF_BG is set then consumed within the same update (bgs.c:26-28).
        assert_eq!(vars.bgflags, 0);
    }
}
