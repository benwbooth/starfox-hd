//! End-of-level score: hit-percentage tally + bonus-credit system.
//!
//! Star Fox has NO per-enemy point score. `s_score` is a no-op macro
//! (STRATLIB.INC:1098-1101). The "score" is a *hit percentage*: the fraction
//! of each stage's special-flagged objects that the player destroyed, plus a
//! small living-teammate bonus, accumulated across the stages you actually
//! play. Crossing fixed total thresholds (`bonertab`) awards continue credits.
//!
//! ASM sources (reference/ultrastarfox/SF/ASM/MAIN.ASM):
//! - `calcstageperc`   MAIN.ASM:1031-1071  -> [`calc_stage_perc`]
//! - `calctotalscore`  MAIN.ASM:780-800    -> [`calc_total_score`]
//! - `checkbonus`      MAIN.ASM:1367-1383  -> [`crossed_bonus_threshold`]
//! - `bonertab`        MAIN.ASM:1383       -> [`BONERTAB`]
//! The per-kill numerator is `specials_dead`, incremented by
//! `s_test_special` (STRATMAC.INC:225-234) from `explode_Icont`
//! (EXPSTRAT.ASM:703) when a `special` OR `Cspecial` object dies, and reset
//! per stage by the map `initlevel` macro (`setvar specials_dead,0`,
//! MAPMACS.INC:876). The denominator is the stage's `specialobjtotal`
//! (map-build count of special/Cspecial spawns).

/// WRAM slot holding the running per-stage special-kill count (ROM
/// `specials_dead`, GILESALC.INC:103; enemy_a::wm::SPECIALS_DEAD in sf-strat).
/// Reset to 0 each stage by the map `initlevel` macro, incremented by the
/// sf-strat explode strat.
pub const SPECIALS_DEAD: u16 = 0x1F0B;

/// Bonus SFX played on the tally screen when a fresh `bonertab` threshold is
/// crossed (ROM `trigse $1a`, MAIN.ASM:1149).
pub const SE_BONUS: u8 = 0x1A;

/// ROM `bonertab` (MAIN.ASM:1383): the credit thresholds, descending. Each
/// threshold your running total newly reaches awards one continue credit.
pub const BONERTAB: [u16; 12] = [
    2100, 1900, 1700, 1500, 1300, 1100, 900, 700, 500, 300, 100, 0,
];

/// Sentinel stored in the per-stage score buffer for a stage that was skipped
/// (route not taken). ROM `specbuf` stores 101 for such stages (MAIN.ASM:317);
/// `calctotalscore` counts it as 0 in the sum but tallies it separately as a
/// "not played" marker (MAIN.ASM:792-796). Real stage percentages are capped
/// at 100, so 101 is an unambiguous sentinel.
pub const STAGE_SKIPPED: u8 = 101;

/// ROM `calcstageperc` (MAIN.ASM:1031-1071): a stage's percentage, 0..=100.
///
/// Base = 5% per living teammate (Peppy `bunny`, Slippy `frog`, Falco `cock`;
/// MAIN.ASM:1037-1049 `adc #5`). If the stage has any specials, add the hit
/// ratio `floor(specials_dead*100 / specialobjtotal)` (the MARIO `mcalcperc`
/// 32/16 divide, MTXTPRT.MC:355-366). The sum is capped at 100
/// (MAIN.ASM:1067-1070).
pub fn calc_stage_perc(specials_dead: u8, total_specials: u8, teammates_alive: u8) -> u8 {
    // MAIN.ASM:1037-1049: +5 for each of the three teammates still alive.
    let teammate_bonus = teammates_alive as u16 * 5;
    // MAIN.ASM:1057-1065: hit ratio only when specialobjtotal != 0.
    let hit = if total_specials == 0 {
        0
    } else {
        specials_dead as u16 * 100 / total_specials as u16
    };
    (hit + teammate_bonus).min(100) as u8
}

/// Count of the three teammates still alive, from their HP bytes
/// (ROM `bunny`/`frog`/`cock`, each nonzero => alive, +5% in calcstageperc).
pub fn teammates_alive(bunny_hp: u8, frog_hp: u8, falcon_hp: u8) -> u8 {
    (bunny_hp != 0) as u8 + (frog_hp != 0) as u8 + (falcon_hp != 0) as u8
}

/// ROM `calctotalscore` (MAIN.ASM:780-800): sum of every recorded stage
/// percentage, treating the [`STAGE_SKIPPED`] (101) sentinel as 0.
pub fn calc_total_score(stage_scores: &[u8]) -> u16 {
    stage_scores
        .iter()
        .map(|&s| if s == STAGE_SKIPPED { 0 } else { s as u16 })
        .sum()
}

/// ROM `checkbonus` (MAIN.ASM:1367-1383): true when the *new* running total
/// reaches a `bonertab` threshold that the *old* total had not yet reached.
///
/// ROM walks `bonertab` (descending) for the first entry `<= new_total`
/// (`jbe .hmm`), then awards a credit iff that entry `> old_total`
/// (`cmp clbm / jg .set`). Because thresholds are 100 apart from 100..2100
/// (plus a terminal 0), each 100-point band the total newly enters yields one
/// credit; totals in 0..99 find threshold 0, which never exceeds a prior
/// total, so award nothing.
pub fn crossed_bonus_threshold(new_total: u16, old_total: u16) -> bool {
    let threshold = BONERTAB
        .iter()
        .copied()
        .find(|&b| b <= new_total)
        .unwrap_or(0);
    threshold > old_total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_perc_hit_ratio_floor_and_teammate_bonus() {
        // No teammates: pure floor(dead*100/total).
        assert_eq!(calc_stage_perc(0, 10, 0), 0);
        assert_eq!(calc_stage_perc(5, 10, 0), 50);
        assert_eq!(calc_stage_perc(10, 10, 0), 100);
        // Floor: 1/3 -> 33.
        assert_eq!(calc_stage_perc(1, 3, 0), 33);
        // Teammate bonus: +5 each.
        assert_eq!(calc_stage_perc(5, 10, 3), 65); // 50 + 15
        assert_eq!(calc_stage_perc(0, 10, 2), 10); // 0 + 10
    }

    #[test]
    fn stage_perc_caps_at_100() {
        // 100% + full teammate bonus is clamped to 100 (MAIN.ASM:1067-1070).
        assert_eq!(calc_stage_perc(10, 10, 3), 100);
        assert_eq!(calc_stage_perc(9, 10, 3), 100); // 90 + 15 -> 100
    }

    #[test]
    fn stage_perc_no_specials_is_teammate_bonus_only() {
        // specialobjtotal == 0 -> hit ratio skipped, only the team bonus.
        assert_eq!(calc_stage_perc(0, 0, 0), 0);
        assert_eq!(calc_stage_perc(0, 0, 3), 15);
    }

    #[test]
    fn teammates_alive_counts_nonzero_hp() {
        assert_eq!(teammates_alive(0, 0, 0), 0);
        assert_eq!(teammates_alive(3, 0, 0), 1);
        assert_eq!(teammates_alive(3, 1, 2), 3);
    }

    #[test]
    fn total_score_sums_and_skips_sentinel() {
        assert_eq!(calc_total_score(&[]), 0);
        assert_eq!(calc_total_score(&[80, 100, 60]), 240);
        // 101 sentinel contributes 0.
        assert_eq!(calc_total_score(&[80, STAGE_SKIPPED, 60]), 140);
    }

    #[test]
    fn checkbonus_crosses_first_threshold() {
        // 90 -> 110 crosses the 100 threshold (first credit).
        assert!(crossed_bonus_threshold(110, 90));
        // Staying within a band awards nothing.
        assert!(!crossed_bonus_threshold(150, 110));
        // 0..99 never awards (threshold 0 <= any prior total).
        assert!(!crossed_bonus_threshold(99, 0));
        assert!(!crossed_bonus_threshold(50, 0));
    }

    #[test]
    fn checkbonus_crosses_higher_thresholds() {
        // 250 -> 350 crosses 300.
        assert!(crossed_bonus_threshold(350, 250));
        // 290 -> 620 crosses 300 AND 500 in one jump; checkbonus fires once
        // (single credit per tally), the largest band reached (500 > 290).
        assert!(crossed_bonus_threshold(620, 290));
        // Top of the table.
        assert!(crossed_bonus_threshold(2100, 1999));
        assert!(!crossed_bonus_threshold(2100, 2100));
    }
}
