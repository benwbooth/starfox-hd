//! Background request scheduler state.
//!
//! C oracle: `src/game/bgs.c` (BGS.ASM -> C conversion), ported line for
//! line. Operates on the GameVars fields `currentbg` / `bgflags` /
//! `bg_dmalist` / `bgtransspeed` (C `g_currentbg` etc., game_vars.c).

use crate::vars::{GameVars, BGF_BG, BGF_INFO, BGF_RESTART};
use sf_core::scene::{PaletteFadeTarget, PALETTE_FADE_COUNTER_START};

/// `seapal - seapal + 30`, saved by `fadetoseado` as `lastpalfade`.
const SEA_FADE_LAST_COLOR_OFFSET: u16 = 30;
/// `groundpal - seapal + 30`; the two source rows are 32 bytes apart.
const GROUND_FADE_LAST_COLOR_OFFSET: u16 = 62;

/// Result of draining pending background-request flags (ROM `transswap` /
/// `dobgreq_l` / `setbginforeq_l` side effects without SNES DMA).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BgRequestResult {
    /// `bgflags & BGF_RESTART` was set — ROM would `jsl restart_l`.
    pub restart: bool,
    /// `bgflags & BGF_BG` was set — ROM would `jsl dobgreq_l` (walk bglists).
    pub bg_change: bool,
    /// `bgflags & BGF_INFO` was set — ROM would `jsl setbginforeq_l`.
    pub info: bool,
}

/// C `Bgs_Init()` (src/game/bgs.c:7).
pub fn init(vars: &mut GameVars) {
    vars.bgflags = 0;
    vars.bg_dmalist = 0;
    vars.currentbg = 0;
    vars.bgtransspeed = 0;
}

/// ROM `dobgreq_l` (WORLD.ASM:1295) — walk `bglists[currentbg]` DMA scripts.
/// HD has no SNES DMA; consuming `BGF_BG` is the observable effect (same as
/// the end of [`update`]).
pub fn do_bg_req(vars: &mut GameVars) {
    vars.bgflags &= !BGF_BG;
}

/// ROM `setbg_l` (WORLD.ASM:1288) — latch `currentbg` and request a BG swap.
pub fn set_bg(vars: &mut GameVars, bg_id: u16) {
    vars.currentbg = bg_id;
    vars.set_sound_environment_for_bg(bg_id);
    vars.bgflags |= BGF_BG;
}

/// ROM `setbginforeq_l` / `setbginfo_l` request path — arm `BGF_INFO`.
pub fn set_bg_info_req(vars: &mut GameVars) {
    vars.bgflags |= BGF_INFO;
}

/// ROM `setrestartfade_l` (WORLD.ASM:396) — restore the saved source-row
/// cursor and restart `palnum` at 30. The saved value is a palette byte
/// offset (30 for sea, 62 for ground), not a frame counter.
pub fn set_restart_fade(vars: &mut GameVars, restart_palfade: u16) {
    vars.palfade_target = match restart_palfade {
        0 => return,
        SEA_FADE_LAST_COLOR_OFFSET => Some(PaletteFadeTarget::Sea),
        GROUND_FADE_LAST_COLOR_OFFSET => Some(PaletteFadeTarget::Ground),
        _ => return,
    };
    vars.palfade_num = PALETTE_FADE_COUNTER_START;
}

/// Result of [`calc_bg2_voffsets`] (ROM TRANS.ASM:367).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Bg2VofsResult {
    /// ROM `dobg2` — HDMA table needs upload this frame.
    pub needs_dma: bool,
    /// Key written to `lastrot` when a rebuild happened.
    pub table_key: u16,
}

/// ROM `calcbg2voffsets` (TRANS.ASM:367): if `dovofs` clear, no-op; else
/// pick a BG2 VOFS table from `plrotz` (when `dozrot`) or 0, skip if
/// `lastrot` unchanged, otherwise mark `dobg2` and update `lastrot`.
/// HD does not materialize the 64-word HDMA table — only the gate/key.
pub fn calc_bg2_voffsets(vars: &mut GameVars, player_rotz: i16) -> Bg2VofsResult {
    if vars.dovofs == 0 {
        return Bg2VofsResult::default();
    }
    let dozrot = vars.shared.do_depth_rotation != 0;
    let rot = if dozrot { player_rotz } else { 0 };
    // ROM: swa; and #7; asl → table index 0..7 from high byte of |rotz|.
    let abs_rot = if rot < 0 { rot.wrapping_neg() } else { rot };
    let key = (((abs_rot as u16) >> 8) & 7) << 1;
    let last = vars.shared.last_rotation;
    if key == last {
        return Bg2VofsResult::default();
    }
    vars.shared.last_rotation = key;
    Bg2VofsResult {
        needs_dma: true,
        table_key: key,
    }
}

/// ROM `transswap` body after the bitmap wait (TRANS.ASM:273): service
/// restart / bg / info request bits, then clear them. Returns which bits
/// were pending so the shell can act (reload level, swap BG, etc.).
pub fn trans_swap(vars: &mut GameVars) -> BgRequestResult {
    let result = BgRequestResult {
        restart: vars.bgflags & BGF_RESTART != 0,
        bg_change: vars.bgflags & BGF_BG != 0,
        info: vars.bgflags & BGF_INFO != 0,
    };
    if result.bg_change {
        do_bg_req(vars);
    }
    vars.bgflags &= !(BGF_RESTART | BGF_BG | BGF_INFO);
    result
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
        do_bg_req(vars);
    }
    if vars.bgflags & BGF_INFO != 0 {
        if let Some(info) = sf_map::catalog::background_info(vars.currentbg) {
            vars.point_field_mode = info.point_field;
            vars.dotsflag = info.point_field.source_flag();
            if info.vertical_offsets {
                vars.vofs_on_please();
            } else {
                vars.vofs_off_please();
            }
            vars.dohofs = u8::from(info.horizontal_offsets);
            vars.shared.do_depth_rotation = u8::from(info.depth_rotation);
        }
        vars.preserve_player_strategy = false;
        vars.bgflags &= !BGF_INFO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vars::{GameVars, BGF_BG, BGF_INFO, BGF_RESTART};

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

    #[test]
    fn trans_swap_drains_request_flags() {
        let mut vars = GameVars::init();
        vars.bgflags = BGF_BG | BGF_INFO | BGF_RESTART;
        let r = trans_swap(&mut vars);
        assert!(r.restart && r.bg_change && r.info);
        assert_eq!(vars.bgflags, 0);
        // Idle — nothing pending.
        let idle_result = trans_swap(&mut vars);
        assert_eq!(idle_result, BgRequestResult::default());
    }

    #[test]
    fn do_bg_req_clears_bg_bit() {
        let mut vars = GameVars::init();
        vars.bgflags = BGF_BG | BGF_INFO;
        do_bg_req(&mut vars);
        assert_eq!(vars.bgflags, BGF_INFO);
    }

    #[test]
    fn background_info_consumes_player_strategy_preservation() {
        let mut vars = GameVars::init();
        vars.preserve_player_strategy = true;
        vars.bgflags = BGF_INFO;

        update(&mut vars);

        assert!(!vars.preserve_player_strategy);
        assert_eq!(vars.bgflags, 0);
    }

    #[test]
    fn background_info_applies_title_and_training_declarations() {
        use sf_core::point_field::PointFieldMode;
        use sf_map::catalog::background_id;

        let mut vars = GameVars::init();
        vars.currentbg = background_id::TITLE;
        vars.bgflags = BGF_INFO;
        update(&mut vars);
        assert_eq!(vars.point_field_mode, PointFieldMode::SpaceDust);
        assert_eq!(vars.dotsflag, -1);
        assert_eq!((vars.dovofs, vars.dohofs), (0, 0));
        assert_eq!(vars.shared.do_depth_rotation, 0);

        vars.currentbg = background_id::TRAINING;
        vars.bgflags = BGF_INFO;
        update(&mut vars);
        assert_eq!(vars.point_field_mode, PointFieldMode::GroundGrid);
        assert_eq!(vars.dotsflag, 1);
        assert_eq!((vars.dovofs, vars.dohofs), (1, 1));
        assert_eq!(vars.shared.do_depth_rotation, 1);
    }

    #[test]
    fn blink_background_retains_previous_info() {
        use sf_core::point_field::PointFieldMode;

        let mut vars = GameVars::init();
        vars.currentbg = 1;
        vars.point_field_mode = PointFieldMode::SpaceDust;
        vars.dotsflag = -1;
        vars.dovofs = 1;
        vars.dohofs = 1;
        vars.shared.do_depth_rotation = 1;
        vars.bgflags = BGF_INFO;
        update(&mut vars);
        assert_eq!(vars.point_field_mode, PointFieldMode::SpaceDust);
        assert_eq!(vars.dotsflag, -1);
        assert_eq!((vars.dovofs, vars.dohofs), (1, 1));
        assert_eq!(vars.shared.do_depth_rotation, 1);
    }

    #[test]
    fn calc_bg2_voffsets_gates_on_dovofs_and_lastrot() {
        let mut vars = GameVars::init();
        // dovofs off → no-op.
        assert_eq!(
            calc_bg2_voffsets(&mut vars, 0x500),
            Bg2VofsResult::default()
        );
        vars.dovofs = 1;
        vars.shared.do_depth_rotation = 0;
        vars.shared.last_rotation = u16::MAX;
        let r = calc_bg2_voffsets(&mut vars, 0x500);
        assert!(r.needs_dma);
        assert_eq!(r.table_key, 0);
        // Same key → skip.
        assert_eq!(
            calc_bg2_voffsets(&mut vars, 0x500),
            Bg2VofsResult::default()
        );
        // dozrot on: key from |rotz| high nibble.
        vars.shared.do_depth_rotation = 1;
        let rotated_result = calc_bg2_voffsets(&mut vars, 0x0500); // hi=5 → key=(5&7)<<1=10
        assert!(rotated_result.needs_dma);
        assert_eq!(rotated_result.table_key, 10);
    }
}
