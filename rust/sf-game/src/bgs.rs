//! Background request scheduler state.
//!
//! C oracle: `src/game/bgs.c` (BGS.ASM -> C conversion), ported line for
//! line. Operates on the GameVars fields `currentbg` / `bgflags` /
//! `bg_dmalist` / `bgtransspeed` (C `g_currentbg` etc., game_vars.c).

use crate::vars::{GameVars, BGF_BG, BGF_INFO, BGF_RESTART};
use sf_core::scene::{
    BackgroundHorizontalMode, PaletteFadeTarget, BG2_HORIZONTAL_OFFSET_ROWS,
    BG2_VERTICAL_OFFSET_COLUMNS, PALETTE_FADE_COUNTER_START,
};

/// The rotating-ground transform is mirrored around the 112-line horizon.
const HORIZONTAL_OFFSET_HALF_ROWS: usize = BG2_HORIZONTAL_OFFSET_ROWS / 2;
/// Fixed-point reduction used by the authored ground-roll gradient.
const HORIZONTAL_ROLL_REDUCTION: u32 = 7;
/// Fractional portion retained by the authored 8.8 recurrence.
const HORIZONTAL_FRACTION_BITS: u32 = 8;
/// Player lateral position contributes one background pixel per eight world
/// units.
const HORIZONTAL_WORLD_REDUCTION: u32 = 3;
/// Complete yaw contributes one background pixel per 32 turn fractions.
const HORIZONTAL_YAW_REDUCTION: u32 = 5;

/// `seapal - seapal + 30`, saved by `fadetoseado` as `lastpalfade`.
const SEA_FADE_LAST_COLOR_OFFSET: u16 = 30;
/// `groundpal - seapal + 30`; the two source rows are 32 bytes apart.
const GROUND_FADE_LAST_COLOR_OFFSET: u16 = 62;

/// Low-coordinate portion of `SGDATA.ASM` `bg2tab1` through `bg2tab6`.
/// The source words also set the Mode-2 vertical-offset enable bit; the Rust
/// renderer carries that state in the surrounding `Option` instead.
const BG2_VERTICAL_OFFSET_TABLES: [[i16; BG2_VERTICAL_OFFSET_COLUMNS]; 6] = [
    [16; BG2_VERTICAL_OFFSET_COLUMNS],
    [
        20, 19, 19, 19, 18, 18, 18, 18, 18, 17, 17, 17, 17, 16, 16, 16, 16, 16, 15, 15, 15, 15, 14,
        14, 14, 14, 14, 13, 13, 13, 12, 12,
    ],
    [
        23, 22, 21, 21, 20, 20, 20, 19, 19, 18, 18, 18, 17, 17, 16, 16, 16, 15, 15, 14, 14, 14, 13,
        13, 12, 12, 12, 11, 11, 10, 9, 9,
    ],
    [
        25, 24, 24, 23, 23, 22, 21, 21, 20, 20, 19, 18, 18, 17, 17, 16, 15, 15, 14, 14, 13, 12, 12,
        11, 11, 10, 9, 9, 8, 8, 7, 7,
    ],
    [
        28, 27, 26, 25, 24, 24, 23, 22, 21, 21, 20, 19, 18, 18, 17, 16, 15, 14, 14, 13, 12, 11, 11,
        10, 9, 8, 7, 7, 6, 5, 4, 4,
    ],
    [
        32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10,
        9, 8, 7, 6, 5, 4, 3, 2, 1,
    ],
];

/// Typed presentation form of `calcbg2voffsets`. `player_roll` is the
/// source's complete signed 16-bit turn fraction (`plrotz`), not a camera or
/// processor field. Negative banking reverses the authored column order.
pub fn background_vertical_offsets(
    player_roll: i16,
    depth_rotation: bool,
) -> [i16; BG2_VERTICAL_OFFSET_COLUMNS] {
    let effective_roll = if depth_rotation { player_roll } else { 0 };
    let magnitude = if effective_roll < 0 {
        effective_roll.wrapping_neg() as u16
    } else {
        effective_roll as u16
    };
    // The Rev-2 cartridge selects successive authored tables for high-byte
    // roll buckets 0 through 5, then saturates at table 6. Deterministic WRAM
    // captures cover bucket 1 -> table 2 and bucket 3 -> table 4. This differs
    // from the duplicated pointer entries in the surviving development tree,
    // so retail behavior is authoritative for the port.
    let table_index = usize::from((magnitude >> 8) & 7).min(BG2_VERTICAL_OFFSET_TABLES.len() - 1);
    let mut offsets = BG2_VERTICAL_OFFSET_TABLES[table_index];
    if effective_roll < 0 {
        offsets.reverse();
    }
    offsets
}

/// Typed presentation form of the rotating-ground horizontal transform.
///
/// The source builds the top and bottom halves outward from the horizon with
/// a signed 8.8 roll gradient. The returned values are complete background
/// offsets, including player position, camera yaw, turn compensation, and the
/// background's authored base scroll.
pub fn background_horizontal_offsets(
    player_roll: i16,
    player_world_x: i16,
    view_yaw: i16,
    player_turn_rotation: i16,
    background_scroll_x: i16,
) -> [i16; BG2_HORIZONTAL_OFFSET_ROWS] {
    let base = player_world_x
        .wrapping_shr(HORIZONTAL_WORLD_REDUCTION)
        .wrapping_add(
            view_yaw
                .wrapping_sub(player_turn_rotation)
                .wrapping_shr(HORIZONTAL_YAW_REDUCTION),
        )
        .wrapping_add(background_scroll_x);
    let gradient = (!player_roll).wrapping_shr(HORIZONTAL_ROLL_REDUCTION);
    let whole_step = gradient.wrapping_shr(HORIZONTAL_FRACTION_BITS);
    let fractional_step = (gradient as u16) << HORIZONTAL_FRACTION_BITS;
    let mut fractional_accumulator = 0u16;
    let mut whole_accumulator = 0i16;
    let mut offsets = [0; BG2_HORIZONTAL_OFFSET_ROWS];

    for distance in 0..HORIZONTAL_OFFSET_HALF_ROWS {
        let (fraction, carry) = fractional_accumulator.overflowing_add(fractional_step);
        fractional_accumulator = fraction;
        whole_accumulator = whole_accumulator
            .wrapping_add(whole_step)
            .wrapping_add(i16::from(carry));
        offsets[HORIZONTAL_OFFSET_HALF_ROWS - 1 - distance] = base.wrapping_add(whole_accumulator);
        offsets[HORIZONTAL_OFFSET_HALF_ROWS + distance] = base.wrapping_add(!whole_accumulator);
    }

    offsets
}

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

/// Apply one complete typed BGS `info` declaration. This is shared by the
/// ordinary request lane and the level-loader path for builders that omit the
/// common `initlevel` bytes while retaining their flat background identity.
pub fn apply_background_info(vars: &mut GameVars, info: sf_map::catalog::BackgroundInfo) {
    vars.point_field_mode = info.point_field;
    vars.dotsflag = info.point_field.source_flag();
    if info.vertical_offsets {
        vars.vofs_on_please();
    } else {
        vars.vofs_off_please();
    }
    vars.background_horizontal_mode = info.horizontal_mode;
    vars.dohofs = u8::from(info.horizontal_mode != BackgroundHorizontalMode::Disabled);
    vars.shared.do_depth_rotation = u8::from(info.depth_rotation);
    vars.preserve_player_strategy = false;
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
    vars.bg2_vertical_offsets = background_vertical_offsets(player_rotz, dozrot);
    Bg2VofsResult {
        needs_dma: true,
        table_key: key,
    }
}

/// Transfer-bound background preparation. `calcbgscroll_l` clears the
/// source's cached table pointer immediately before `calcbg2voffsets`, so an
/// enabled Mode-2 table is materialized from the pre-strategy player roll on
/// every game-loop transfer. Keeping the 32 columns in [`GameVars`] preserves
/// that observable one-update presentation phase without machine memory.
pub fn prepare_transfer(vars: &mut GameVars, player_world_x: i16) -> Bg2VofsResult {
    vars.shared.last_rotation = u16::MAX;
    let vertical = calc_bg2_voffsets(vars, vars.strategy.player_rotation[2]);
    if vars.background_horizontal_mode == BackgroundHorizontalMode::Rotate {
        vars.bg2_horizontal_offsets = background_horizontal_offsets(
            vars.strategy.player_rotation[2],
            player_world_x,
            vars.strategy.view_yaw,
            vars.strategy.player_turn_rotation,
            vars.shared.background_scroll_x,
        );
    }
    vertical
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
            apply_background_info(vars, info);
        }
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
        assert_eq!(
            vars.background_horizontal_mode,
            BackgroundHorizontalMode::Disabled
        );
        assert_eq!(vars.shared.do_depth_rotation, 0);

        vars.currentbg = background_id::TRAINING;
        vars.bgflags = BGF_INFO;
        update(&mut vars);
        assert_eq!(vars.point_field_mode, PointFieldMode::GroundGrid);
        assert_eq!(vars.dotsflag, 1);
        assert_eq!((vars.dovofs, vars.dohofs), (1, 1));
        assert_eq!(
            vars.background_horizontal_mode,
            BackgroundHorizontalMode::Rotate
        );
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
        vars.background_horizontal_mode = BackgroundHorizontalMode::Rotate;
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
        assert_eq!(vars.bg2_vertical_offsets, BG2_VERTICAL_OFFSET_TABLES[5]);
    }

    #[test]
    fn transfer_materializes_columns_from_the_pre_strategy_roll() {
        let mut vars = GameVars::init();
        vars.dovofs = 1;
        vars.shared.do_depth_rotation = 1;
        vars.strategy.player_rotation[2] = 0x0100;

        let player_world_x = vars.player_posx;
        let result = prepare_transfer(&mut vars, player_world_x);

        assert!(result.needs_dma);
        assert_eq!(vars.bg2_vertical_offsets, BG2_VERTICAL_OFFSET_TABLES[1]);
    }

    #[test]
    fn source_vertical_offset_columns_follow_roll_direction() {
        assert_eq!(
            background_vertical_offsets(0, true),
            [16; BG2_VERTICAL_OFFSET_COLUMNS]
        );
        assert_eq!(
            background_vertical_offsets(0x0100, true),
            BG2_VERTICAL_OFFSET_TABLES[1]
        );
        let mut reversed = BG2_VERTICAL_OFFSET_TABLES[1];
        reversed.reverse();
        assert_eq!(background_vertical_offsets(-0x0100, true), reversed);
        assert_eq!(
            background_vertical_offsets(0x0600, false),
            [16; BG2_VERTICAL_OFFSET_COLUMNS]
        );
        assert_eq!(
            background_vertical_offsets(0x0600, true),
            BG2_VERTICAL_OFFSET_TABLES[5]
        );
        for (bucket, expected) in BG2_VERTICAL_OFFSET_TABLES.iter().enumerate() {
            assert_eq!(
                background_vertical_offsets((bucket as i16) << 8, true),
                *expected,
                "source vertical-offset table {}",
                bucket + 1,
            );
        }
    }

    #[test]
    fn horizontal_offsets_are_mirrored_around_the_source_horizon() {
        const BASE_SCROLL: i16 = 20;
        const PLAYER_WORLD_X: i16 = 64;
        const VIEW_YAW: i16 = 320;
        const PLAYER_TURN: i16 = 64;
        const EXPECTED_BASE: i16 = 36;

        let offsets =
            background_horizontal_offsets(0, PLAYER_WORLD_X, VIEW_YAW, PLAYER_TURN, BASE_SCROLL);

        assert_eq!(
            offsets[..HORIZONTAL_OFFSET_HALF_ROWS],
            [EXPECTED_BASE - 1; 112]
        );
        assert_eq!(offsets[HORIZONTAL_OFFSET_HALF_ROWS..], [EXPECTED_BASE; 112]);
    }

    #[test]
    fn transfer_materializes_horizontal_rows_from_typed_scene_state() {
        let mut vars = GameVars::init();
        vars.dohofs = 1;
        vars.background_horizontal_mode = BackgroundHorizontalMode::Rotate;
        vars.strategy.player_rotation[2] = 768;
        vars.player_posx = -40;
        vars.strategy.view_yaw = 256;
        vars.strategy.player_turn_rotation = 32;
        vars.shared.background_scroll_x = 7;

        let player_world_x = vars.player_posx;
        prepare_transfer(&mut vars, player_world_x);

        assert_eq!(
            vars.bg2_horizontal_offsets,
            background_horizontal_offsets(768, -40, 256, 32, 7)
        );
    }
}
