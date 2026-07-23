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
//! - `maketotalscore`  MAIN.ASM:639-777    -> [`calc_average_score`] (+ end-seq UI)
//! - `maketotalscore2` MAIN.ASM:810+       -> same average math, different layout
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
/// Per-step count sound while the stage graph advances by three points
/// (ROM `trigse $12`, MAIN.ASM:1187-1194).
pub const SE_TALLY_COUNT: u8 = 0x12;
/// Stage-score commit sound after the graph's 20-step settle
/// (ROM `trigse $11`, MAIN.ASM:1204-1217).
pub const SE_TALLY_COMMIT: u8 = 0x11;

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

const PERCENT_SCALE: i16 = 100;
const TEAMMATE_BONUS_PERCENT: u16 = 5;
const MAX_STAGE_PERCENT: u16 = 100;

/// Exact hit-ratio calculation used by the original game.
///
/// The source value is an unsigned byte, but the original multiplication
/// interprets that byte as signed before multiplying by 100.  The resulting
/// 16-bit bit pattern is then divided as an unsigned value.  Counts below 128
/// therefore behave like the ordinary `dead * 100 / total` formula, while the
/// otherwise-unreachable high half of the byte domain preserves the retail
/// overflow behavior for oracle parity.
pub fn calc_hit_percentage(specials_dead: u8, total_specials: u8) -> u16 {
    if total_specials == 0 {
        return 0;
    }

    let signed_product = i16::from(specials_dead as i8) * PERCENT_SCALE;
    (signed_product as u16) / u16::from(total_specials)
}

/// ROM `calcstageperc` (MAIN.ASM:1031-1071): a stage's percentage, 0..=100.
///
/// Base = 5% per living teammate (Peppy `bunny`, Slippy `frog`, Falco `cock`;
/// MAIN.ASM:1037-1049 `adc #5`). If the stage has any specials, add the hit
/// ratio `floor(specials_dead*100 / specialobjtotal)` (the MARIO `mcalcperc`
/// 32/16 divide, MTXTPRT.MC:355-366). The sum is capped at 100
/// (MAIN.ASM:1067-1070).
pub fn calc_stage_perc(specials_dead: u8, total_specials: u8, teammates_alive: u8) -> u8 {
    // MAIN.ASM:1037-1049: +5 for each of the three teammates still alive.
    let teammate_bonus = u16::from(teammates_alive) * TEAMMATE_BONUS_PERCENT;
    // MAIN.ASM:1057-1065: hit ratio only when specialobjtotal != 0.
    let hit = calc_hit_percentage(specials_dead, total_specials);
    (hit + teammate_bonus).min(MAX_STAGE_PERCENT) as u8
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

/// Stages that actually contributed to the total (ROM `specptr - maintempalc`
/// after `calctotalscore` tallies the 101 sentinel into `maintempalc`).
pub fn count_played_stages(stage_scores: &[u8]) -> u16 {
    stage_scores.iter().filter(|&&s| s != STAGE_SKIPPED).count() as u16
}

/// ROM `maketotalscore` / `maketotalscore2` average (MAIN.ASM:649-658 /
/// 813-822): `floor(total / played)` via MARIO `mkrisdivu3115`. Zero when no
/// stages were played (avoids the ROM's divide-by-zero edge). The end-seq
/// digit/`makeendobj` layout is HD UI, not ported here.
pub fn calc_average_score(stage_scores: &[u8]) -> u16 {
    let played = count_played_stages(stage_scores);
    if played == 0 {
        return 0;
    }
    calc_total_score(stage_scores) / played
}

/// ROM digit peel used by `maketotalscore` for the TOTAL SCORE row
/// (MAIN.ASM:686-724): hundreds / tens / ones of a 0..=999 percentage sum
/// display value (cla2 after subtracting 100s).
pub fn score_digits(mut value: u16) -> (u16, u16, u16) {
    let mut hundreds = 0u16;
    while value >= 100 {
        value -= 100;
        hundreds += 1;
    }
    let mut tens = 0u16;
    while value >= 10 {
        value -= 10;
        tens += 1;
    }
    (hundreds, tens, value)
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
    fn hit_percentage_preserves_signed_multiply_bit_pattern() {
        assert_eq!(calc_hit_percentage(127, 1), 12_700);
        assert_eq!(calc_hit_percentage(128, 1), 52_736);
        assert_eq!(calc_hit_percentage(255, 1), 65_436);
        assert_eq!(calc_hit_percentage(128, 255), 206);
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
    fn average_score_divides_played_stages() {
        // maketotalscore: mkrisdivu3115(tpa, specptr-maintempalc).
        assert_eq!(calc_average_score(&[]), 0);
        assert_eq!(calc_average_score(&[80, 100, 60]), 80); // 240/3
        assert_eq!(calc_average_score(&[80, STAGE_SKIPPED, 60]), 70); // 140/2
        assert_eq!(calc_average_score(&[STAGE_SKIPPED]), 0);
        assert_eq!(count_played_stages(&[80, STAGE_SKIPPED, 60]), 2);
    }

    #[test]
    fn score_digits_peel_matches_maketotalscore() {
        assert_eq!(score_digits(0), (0, 0, 0));
        assert_eq!(score_digits(7), (0, 0, 7));
        assert_eq!(score_digits(42), (0, 4, 2));
        assert_eq!(score_digits(240), (2, 4, 0));
        assert_eq!(score_digits(100), (1, 0, 0));
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
