//! Enemy-B strategy accuracy audit — differential evidence for the Achase
//! (proportional angle chase) rounding used throughout enemy_b.rs.
//!
//! ROM `sr8_achase_alvarN` (STRATROU.ASM:2760-2795) implements the
//! `s_Achase_alvar B,...` macro body `Achase_var2A` (STRATMAC.INC:525):
//!
//!   diff = target - current            ; sec / sbc
//!   nolessrange -(1<<N),(1<<N)         ; clamp |diff| up to at least 1<<N
//!   REPT N adiv2                       ; signed halve, rounds TOWARD ZERO
//!   current += diff
//!
//! Two properties matter:
//!   1. min-step: |step| >= 1 (via the nolessrange pre-clamp), and
//!   2. rounding: `adiv2` rounds toward zero, NOT toward -infinity.
//!
//! `sf_strat::common::strat_chase_proportional` (common.rs:296) was fixed to
//! this contract (16-bit W-size chases). But the 8-bit angle helper
//! `sf_strat::enemy_a::achase_angle` (enemy_a.rs:271) still computes the step
//! with a plain arithmetic shift `diff >> shift`, which rounds toward
//! -infinity — one unit larger in magnitude whenever `current < target`
//! (mod 256) and the diff is not a multiple of 1<<shift. Every enemy-B boss
//! rotation (boss7 phase turns, bossA cup/turret aim, bossF turn-around) uses
//! achase_angle, so all of them converge asymmetrically vs the ROM.
//!
//! This test drives the ROM routine for a sweep of (current, target) pairs at
//! rates 3 and 4 and diffs it against both Rust helpers.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::common::strat_chase_proportional;
use sf_strat::enemy_a::achase_angle;

const TPX: u32 = 0x3A; // STRATROU scratch var `tpx` (symbols.txt: TPX $3A)

/// Ground truth: one ROM 8-bit Achase step. A(8-bit)=target, tpx=current;
/// returns (new_current, reached).
fn rom_achase8(rom: &[u8], addr: u32, current: u8, target: u8) -> (u8, bool) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(TPX, current);
    // p=0x20: 8-bit A (the sr8_ variants run with LONGA=0), 16-bit X/Y.
    let exit = call(&mut bus, addr, &Entry { a: target as u16, p: 0x20, ..Default::default() });
    let new = bus.read8(TPX);
    // carry set (fin path) only when current == target on entry.
    let _ = exit;
    (new, current == target)
}

/// Ground truth: one ROM 16-bit Achase step (W size).
fn rom_achase16(rom: &[u8], addr: u32, current: i16, target: i16) -> i16 {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(TPX, current as u16);
    let exit = call(&mut bus, addr, &Entry { a: target as u16, p: 0x00, ..Default::default() });
    let _ = exit;
    bus.read16(TPX) as i16
}

#[test]
fn achase_angle_rounds_away_from_rom_for_positive_diffs() {
    let syms = load_symbols();
    let (Some(&a3), Some(&a4), Some(rom)) = (
        syms.get("SR8_ACHASE_ALVAR3"),
        syms.get("SR8_ACHASE_ALVAR4"),
        load_built_rom(),
    ) else {
        eprintln!("skip: no SR8_ACHASE_ALVAR3/4 symbol / built ROM");
        return;
    };

    // (current, target) sweep: boss7b turns roty from 180 toward 180-22=158
    // and back; bossA cups chase rotx between -90 and 0; bossFC2 chases roty
    // to 180. Mix of directions and non-multiple-of-8 diffs.
    let cases: [(u8, u8); 10] = [
        (0, 100),   // ROM step 100/8=12 (trunc); Rust >>3 gives 13
        (0, 9),     // ROM 1; Rust 2
        (100, 0),   // negative diff: both 12
        (158, 128), // boss7b roty 158 -> deg180
        (128, 158), // boss7b roty 128 -> 158 (deg180-deg22 opening)
        (0, 4),     // inside nolessrange window: both 1
        (252, 4),   // wraparound short way
        (4, 252),
        (200, 100),
        (100, 200),
    ];

    let mut mismatch3 = 0;
    for &(cur, tgt) in &cases {
        let (rom_new, _) = rom_achase8(&rom, a3, cur, tgt);
        let mut rust_cur = cur;
        achase_angle(&mut rust_cur, tgt, 3);
        println!(
            "rate3 cur={cur:>3} tgt={tgt:>3} -> ROM {rom_new:>3}  rust {rust_cur:>3}  {}",
            if rom_new == rust_cur { "" } else { "<-- MISMATCH" }
        );
        if rom_new != rust_cur {
            mismatch3 += 1;
        }
    }

    let mut mismatch4 = 0;
    for &(cur, tgt) in &cases {
        let (rom_new, _) = rom_achase8(&rom, a4, cur, tgt);
        let mut rust_cur = cur;
        achase_angle(&mut rust_cur, tgt, 4);
        println!(
            "rate4 cur={cur:>3} tgt={tgt:>3} -> ROM {rom_new:>3}  rust {rust_cur:>3}  {}",
            if rom_new == rust_cur { "" } else { "<-- MISMATCH" }
        );
        if rom_new != rust_cur {
            mismatch4 += 1;
        }
    }

    // The floor-shift bug fires on positive-diff, non-multiple-of-2^rate
    // cases; expect several mismatches at each rate.
    assert!(
        mismatch3 >= 2 && mismatch4 >= 2,
        "expected achase_angle to diverge from ROM sr8_achase (got {mismatch3}/{mismatch4}) — \
         if this now passes with 0 mismatches, achase_angle was fixed; flip this test to assert equality"
    );
}

#[test]
fn strat_chase_proportional_matches_rom_16bit_achase() {
    let syms = load_symbols();
    let (Some(&a2), Some(&a4), Some(rom)) = (
        syms.get("SR16_ACHASE_ALVAR2"),
        syms.get("SR16_ACHASE_ALVAR4"),
        load_built_rom(),
    ) else {
        eprintln!("skip: no SR16_ACHASE_ALVAR2/4 symbol / built ROM");
        return;
    };

    // bossAcup UP/DOWN srou chase world coords at rates 2/3; boss7alldead
    // chases worldx to player at rate 4; spacepilon zooms at rate 4.
    let cases: [(i16, i16); 8] = [
        (0, 1000),
        (1000, 0),
        (0, -1000),
        (-353, 941),
        (941, -353),
        (0, 7),
        (0, -7),
        (12345, 12344),
    ];

    for &(cur, tgt) in &cases {
        for (addr, rate) in [(a2, 2u32), (a4, 4u32)] {
            let rom_new = rom_achase16(&rom, addr, cur, tgt);
            let rust_new = strat_chase_proportional(cur, tgt, rate);
            println!(
                "rate{rate} cur={cur:>6} tgt={tgt:>6} -> ROM {rom_new:>6}  rust {rust_new:>6}  {}",
                if rom_new == rust_new { "" } else { "<-- MISMATCH" }
            );
            assert_eq!(
                rom_new, rust_new,
                "strat_chase_proportional(cur={cur}, tgt={tgt}, rate={rate}) diverged from ROM"
            );
        }
    }
}
