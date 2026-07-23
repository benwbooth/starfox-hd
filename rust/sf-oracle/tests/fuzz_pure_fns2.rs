//! TIER-1 pure-function differential fuzz sweep — BATCH 2 (GSU-side leaves +
//! score/scale math). Companion to `fuzz_pure_fns.rs`; same doctrine: execute a
//! pure/leaf ROM routine from the built ROM (through `sf-oracle`'s 65816 core OR
//! its GSU/"MARIO" core), fuzz across broad + boundary input grids, and diff
//! bit-exact against the Rust port to prove it (or surface a latent bug, as the
//! mulslog / achase sweeps did).
//!
//! Functions in THIS file (not covered elsewhere):
//!   - msqrt16 / msqrt32   (GSU integer sqrt — distance math)   MMATHS.MC:48/109
//!   - mcalcperc           (GSU 32/16 hit-% divide)             MTXTPRT.MC:355
//!   - calcstageperc       (65816 wrapper: +5%/teammate, clamp) MAIN.ASM:1031
//!   - framescalevecs      (65816 vel*framerate/4 via mulslog)  PSTRATS.ASM:3529
//!   - addvecs0_l          (raw shared pos+=vec entry)          STRATROU.ASM:493
//!   - arctan16 / anglexy_l (GSU atan2 -> 8-bit aim angle)      MMATHS.MC:618
//!
//! GSU calling convention used here: routines are `mcall` leaves ending in
//! `jmp r11`. We seed r11 with the address of a STOP opcode (found by scanning
//! the program bank) so the leaf halts the core on return. mcallXXX wrappers
//! (mcalcperc, mcallarctan16) already end in STOP, so they run directly.

use sf_oracle::gsu::Gsu;
use sf_oracle::{call, call_near, load_built_rom, load_symbols, Entry, SnesBus};

// ---------------------------------------------------------------------------
// GSU harness helpers
// ---------------------------------------------------------------------------

/// Symbol ($01bbpppp) -> (program bank, 16-bit pc). GSU fetch maps
/// ((pbr&7F)<<15)|(pc&7FFF); for bank 1 the file offset equals pc.
fn sym_to_gsu(sym: u32) -> (u8, u16) {
    ((sym >> 16) as u8, (sym & 0xFFFF) as u16)
}

/// First STOP (0x00) opcode address in program bank `pbr` — used as the return
/// landing for `jmp r11` leaves.
fn find_stop(rom: &[u8], pbr: u8) -> u16 {
    let base = (pbr as usize & 0x7F) << 15;
    for pc in 0x8000u16..=0xFFFF {
        let off = base | (pc as usize & 0x7FFF);
        if rom.get(off % rom.len().max(1)).copied() == Some(0) {
            return pc;
        }
    }
    0
}

// ===========================================================================
// 1. msqrt16  — GSU $018058 (MMATHS.MC:48).  in: rsqr=r3 ; out: rsqrt=r6.
//    Classic bit-by-bit unsigned integer sqrt. Rust port: NONE dedicated — the
//    distance helpers (enemy_a.rs / camera.rs) use libm f32 `sqrtf` instead, so
//    there is no integer routine to diff. We (a) PROVE the ROM routine + GSU
//    core compute exact floor-sqrt over the full u16 domain, and (b) confirm the
//    float path the port actually uses (`(x as f32).sqrt() as u16`) would agree
//    with it bit-for-bit — i.e. swapping in msqrt16 changes nothing.
// ===========================================================================
const R_SQR16: usize = 3;
const R_SQRT: usize = 6;

#[test]
fn msqrt16_is_exact_floor_sqrt() {
    let syms = load_symbols();
    let (Some(&sym), Some(rom)) = (syms.get("MSQRT16"), load_built_rom()) else {
        eprintln!("skip: no MSQRT16 / ROM");
        return;
    };
    let (pbr, pc) = sym_to_gsu(sym);
    let stop = find_stop(&rom, pbr);
    let mut g = Gsu::new(rom); // reuse across the exhaustive sweep (no ROM reclone)
    let mut diff_floor = 0usize;
    let mut diff_float = 0usize;
    let mut first: Vec<String> = Vec::new();
    // EXHAUSTIVE over the whole 16-bit input domain.
    for x in 0u16..=0xFFFF {
        g.r[R_SQR16] = x;
        g.r[11] = stop;
        g.run(pbr, pc);
        let rom_r = g.r[R_SQRT];
        let floor = (x as f64).sqrt().floor() as u16; // exact integer floor-sqrt
        let float_port = (x as f32).sqrt() as u16; // what the Rust distance path does
        if rom_r != floor {
            diff_floor += 1;
            if first.len() < 8 {
                first.push(format!(
                    "DIVERGE floor msqrt16({x})=ROM {rom_r} floor {floor}"
                ));
            }
        }
        if rom_r != float_port {
            diff_float += 1;
            if first.len() < 8 {
                first.push(format!(
                    "DIVERGE float msqrt16({x})=ROM {rom_r} f32 {float_port}"
                ));
            }
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!("PROBE msqrt16: 65536 inputs; vs floor-sqrt diffs={diff_floor}, vs f32-port diffs={diff_float}");
    assert_eq!(diff_floor, 0, "msqrt16 must be exact integer floor-sqrt");
    assert_eq!(
        diff_float, 0,
        "the f32 distance path must agree with msqrt16 bit-for-bit"
    );
}

// ===========================================================================
// 2. msqrt32  — GSU $018086 (MMATHS.MC:109).  in: rsqr=r5(lo)/rsqrhi=r4(hi) ;
//    out: rsqrt=r6.  Same floor-sqrt, 32-bit domain (rsqrhi<=$7FFF per the ROM
//    self-test `mtestsqrt32`). No dedicated Rust port. PROVE exact floor-sqrt
//    over a broad grid incl. perfect squares, ±1 neighbours, and boundaries.
// ===========================================================================
const R_SQR32_LO: usize = 5;
const R_SQR32_HI: usize = 4;

#[test]
fn msqrt32_is_exact_floor_sqrt() {
    let syms = load_symbols();
    let (Some(&sym), Some(rom)) = (syms.get("MSQRT32"), load_built_rom()) else {
        eprintln!("skip: no MSQRT32 / ROM");
        return;
    };
    let (pbr, pc) = sym_to_gsu(sym);
    let stop = find_stop(&rom, pbr);
    let mut g = Gsu::new(rom);
    // Build a strong 32-bit grid: every perfect square s^2 (and s^2±1) for
    // s in 0..=46340 stepped, plus explicit boundaries. Domain capped at
    // $7FFFFFFF (rsqrhi<=$7FFF).
    let mut inputs: Vec<u32> = Vec::new();
    let mut s = 0u32;
    while s <= 46340 {
        let sq = s * s;
        inputs.push(sq.wrapping_sub(1));
        inputs.push(sq);
        inputs.push(sq.wrapping_add(1));
        s += if s < 512 { 1 } else { 137 };
    }
    inputs.extend([
        0,
        1,
        2,
        3,
        0xFFFF,
        0x1_0000,
        0x1_0001,
        0x00FF_FFFF,
        0x7FFF_0000,
        0x7FFF_FFFF,
        0x4000_0000,
        1_000_000,
        123_456_789,
    ]);
    inputs.retain(|&v| v <= 0x7FFF_FFFF);
    inputs.sort_unstable();
    inputs.dedup();
    // msqrt32 is exact floor-sqrt over the low domain; above 2^28 it reads ONE
    // low at the exact-below-a-square boundary (x == s^2-1, s >= 16384). That
    // regime never affects any Rust port (there is none — distances use f32
    // sqrt) and the error is bounded to -1. Split-bucket to prove the useful
    // domain exact and pin the boundary quirk.
    const LOW: u32 = 1 << 28; // 2^28: below this msqrt32 is exact
    let mut diffs_low = 0usize; // must be 0
    let mut diffs_hi = 0usize; // characterized boundary quirk
    let mut worst_hi_err = 0i32;
    let mut first: Vec<String> = Vec::new();
    for &x in &inputs {
        g.r[R_SQR32_LO] = x as u16;
        g.r[R_SQR32_HI] = (x >> 16) as u16;
        g.r[11] = stop;
        g.run(pbr, pc);
        let rom_r = g.r[R_SQRT];
        let floor = (x as f64).sqrt().floor() as u16;
        if rom_r != floor {
            let err = rom_r as i32 - floor as i32;
            if x < LOW {
                diffs_low += 1;
            } else {
                diffs_hi += 1;
                worst_hi_err = worst_hi_err.max(err.abs());
            }
            if first.len() < 8 {
                first.push(format!(
                    "DIVERGE msqrt32({x})=ROM {rom_r} floor {floor} err={err}"
                ));
            }
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!(
        "PROBE msqrt32: {} inputs; exact below 2^28 (diffs_low={diffs_low}); \
         boundary quirk >=2^28 at x=s^2-1 (diffs_hi={diffs_hi}, |err|<={worst_hi_err})",
        inputs.len()
    );
    let _ = (diffs_hi, worst_hi_err); // characterized, not asserted (see ignored reproducer)
    assert_eq!(
        diffs_low, 0,
        "msqrt32 must be exact floor-sqrt for x < 2^28 (the realistic domain)"
    );
}

/// A faithful, self-contained 16-bit model of the ROM's `msqrt32` (MMATHS.MC
/// msqrt32). `wide=false` masks every register to 16 bits exactly like the GSU;
/// `wide=true` lets the remainder registers (`rt`,`rt2`,`rsqrt`) grow unbounded.
/// The ONLY difference is register width — the opcode sequence is identical.
fn msqrt32_model(inp: u32, wide: bool) -> u16 {
    let m = |v: u64| if wide { v } else { v & 0xFFFF };
    let mut rsqr = (inp & 0xFFFF) as u64; // r5: input low  (always a 16-bit shift reg)
    let mut rsqrhi = ((inp >> 16) & 0xFFFF) as u64; // r4: input high
    let mut rsqrt: u64 = 0; // r6: running root
    let mut rt: u64 = 0; // r8: running remainder
    for _ in 0..16 {
        // Two `add rsqr ; rol rsqrhi ; rol rt` steps shift 2 input bits up into
        // the remainder `rt`. On real 16-bit hardware the top bit rolled out of
        // `rt` is DROPPED (no 17th-bit register) — that is the whole defect.
        for _ in 0..2 {
            let s = rsqr + rsqr;
            let c = (s > 0xFFFF) as u64;
            rsqr = s & 0xFFFF;
            let v = (rsqrhi << 1) | c;
            let c2 = ((rsqrhi & 0x8000) != 0) as u64;
            rsqrhi = v & 0xFFFF;
            rt = m((rt << 1) | c2); // narrow: overflow bit lost; wide: retained
        }
        rsqrt = m(rsqrt << 1); // root <<= 1
        let rt2 = m(rsqrt << 1); // test value 2*root (can overflow 16 bits!)
        if rt > rt2 {
            // remainder >= 2*root+1
            rt = m(rt - rt2 - 1);
            rsqrt = m(rsqrt + 1);
        }
    }
    rsqrt as u16
}

/// RESOLVED (was: "suspected GSU-core carry bug"). The prior batch flagged
/// `msqrt32` as reading low by up to ~3 LSB for x >= 2^28 and hypothesised a
/// carry-propagation bug in `gsu.rs`. That is a MISDIAGNOSIS: the emulator is
/// faithful, and the divergence is an INHERENT limitation of the ROM routine.
///
/// `msqrt32` keeps its running remainder in a SINGLE 16-bit register (`rt`=r8;
/// MMATHS.MC msqrt32) whose true value reaches ~9e7 for 32-bit inputs and so
/// overflows 16 bits once x >= 2^28 — real GSU hardware (16-bit regs, no 17th
/// remainder bit) drops the same top bit and returns the same low-by-1..3
/// result. `msqrt16` never overflows (root <= 255, remainder <= ~1020), which is
/// why it is exact over its whole domain.
///
/// This test proves it two ways, with NO change to gsu.rs (a "fix" there would
/// make the core UNfaithful to hardware):
///   (a) a faithful 16-bit model reproduces the emulator BIT-EXACT across the
///       high domain — so gsu.rs is a correct 16-bit GSU; and
///   (b) the SAME algorithm with WIDE (un-truncated) remainder registers yields
///       exact floor-sqrt — so register WIDTH, not carry handling, is the cause.
/// No Rust port is affected: integer sqrt is unused (distances use f32 `sqrtf`).
#[test]
fn msqrt32_high_domain_is_faithful_16bit_overflow() {
    let syms = load_symbols();
    let (Some(&sym), Some(rom)) = (syms.get("MSQRT32"), load_built_rom()) else {
        eprintln!("skip: no MSQRT32 / ROM");
        return;
    };
    let (pbr, pc) = sym_to_gsu(sym);
    let stop = find_stop(&rom, pbr);
    let mut g = Gsu::new(rom);
    // A grid concentrated on the failing regime: x == s^2-1 for s across the
    // whole high domain (>= 2^28) plus the canonical 16404^2-1 witness.
    let mut inputs: Vec<u32> = Vec::new();
    let mut s = 16384u32;
    while s <= 46340 {
        let sq = s * s;
        inputs.push(sq.wrapping_sub(1));
        inputs.push(sq);
        inputs.push(sq.wrapping_add(1));
        s += 7;
    }
    inputs.push(16404 * 16404 - 1); // the canonical witness (core=16402, floor=16403)
    inputs.retain(|&v| v <= 0x7FFF_FFFF);

    let mut emu_vs_model = 0usize; // emulator vs faithful 16-bit model (must be 0)
    let mut wide_vs_floor = 0usize; // wide model vs true floor-sqrt (must be 0)
    let mut overflow_confirmed = false;
    let mut first: Vec<String> = Vec::new();
    for &x in &inputs {
        g.r[R_SQR32_LO] = x as u16;
        g.r[R_SQR32_HI] = (x >> 16) as u16;
        g.r[11] = stop;
        g.run(pbr, pc);
        let emu = g.r[R_SQRT];
        let narrow = msqrt32_model(x, false);
        let wide = msqrt32_model(x, true);
        let floor = (x as f64).sqrt().floor() as u16;
        if emu != narrow {
            emu_vs_model += 1;
            if first.len() < 8 {
                first.push(format!(
                    "EMU!=MODEL msqrt32({x}) emu={emu} model16={narrow}"
                ));
            }
        }
        if wide != floor {
            wide_vs_floor += 1;
            if first.len() < 8 {
                first.push(format!(
                    "WIDE!=FLOOR msqrt32({x}) wide={wide} floor={floor}"
                ));
            }
        }
        if emu != floor {
            overflow_confirmed = true; // the register-width divergence is present
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!(
        "PROBE msqrt32 high-domain root-cause: {} inputs; emu==16bit-model diffs={emu_vs_model}; \
         wide-model==floor diffs={wide_vs_floor}; overflow-divergence present={overflow_confirmed}",
        inputs.len()
    );
    assert_eq!(
        emu_vs_model, 0,
        "gsu.rs must be a faithful 16-bit GSU (matches the 16-bit msqrt32 model)"
    );
    assert_eq!(
        wide_vs_floor, 0,
        "the algorithm is exact with wide registers — width, not carry, is the cause"
    );
    assert!(
        overflow_confirmed,
        "the 16-bit-overflow divergence should be observable in the high domain"
    );
}

// ===========================================================================
// 3. mcalcperc  — GSU $01B6B2 (MTXTPRT.MC:355).  in: m_x1=dead, m_y1=total ;
//    The multiply treats the source byte as signed, preserves the resulting
//    16-bit bit pattern, and the divide treats that pattern as unsigned.
//    Rust equivalent: `sf_game::score::calc_hit_percentage`. Fuzz dead × total
//    over the full byte domain the caller masks them to, total>=1 (the caller
//    guards total==0 upstream, so the divide never sees it).
// ===========================================================================
const M_X1: usize = 0x62;
const M_Y1: usize = 0x2C;

fn gsu_mcalcperc(g: &mut Gsu, pbr: u8, pc: u16, dead: u16, total: u16) -> u16 {
    g.ram[M_X1] = dead as u8;
    g.ram[M_X1 + 1] = (dead >> 8) as u8;
    g.ram[M_Y1] = total as u8;
    g.ram[M_Y1 + 1] = (total >> 8) as u8;
    g.run(pbr, pc);
    g.ram[M_X1] as u16 | ((g.ram[M_X1 + 1] as u16) << 8)
}

#[test]
fn mcalcperc_matches_score_hit_ratio() {
    use sf_game::score::calc_hit_percentage;
    let syms = load_symbols();
    let (Some(&sym), Some(rom)) = (syms.get("MCALCPERC"), load_built_rom()) else {
        eprintln!("skip: no MCALCPERC / ROM");
        return;
    };
    let (pbr, pc) = sym_to_gsu(sym);
    let mut g = Gsu::new(rom);
    let mut diffs = 0usize;
    let mut checked = 0usize;
    let mut first: Vec<String> = Vec::new();
    for total in 1u16..=255 {
        for dead in 0u16..=255 {
            let rom_r = gsu_mcalcperc(&mut g, pbr, pc, dead, total);
            let rust = calc_hit_percentage(dead as u8, total as u8);
            checked += 1;
            if rom_r != rust {
                diffs += 1;
                if first.len() < 8 {
                    first.push(format!(
                        "DIVERGE mcalcperc({dead}/{total})=ROM {rom_r} RUST {rust}"
                    ));
                }
            }
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!("PROBE mcalcperc: checked {checked} dead×total pairs; diffs={diffs}");
    assert_eq!(diffs, 0, "mcalcperc diverges from the native calculation");
}

// ===========================================================================
// 4. calcstageperc  — 65816 $02E7AD (MAIN.ASM:1031).  Full stage %:
//    ratio = mcalcperc(dead,total) if total!=0 else 0 ; +5 per living teammate
//    (bunny/frog/cock nonzero) ; clamped to 100. The divide runs on the GSU
//    (call_mario), which the 65816 core can't execute — so we compose the REAL
//    ROM divide (mcalcperc via GSU, proven above) with the trivial 65816
//    +5/clamp arithmetic to form a ROM-faithful oracle, and diff the WHOLE
//    Rust `calc_stage_perc` against it. (The +5 `adc`/`cmp #100` clamp is a
//    verbatim transcription of MAIN.ASM:1037-1070.)
// ===========================================================================
#[test]
fn calc_stage_perc_matches_rom_divide_plus_wrapper() {
    use sf_game::score::calc_stage_perc;
    let syms = load_symbols();
    let (Some(&sym), Some(rom)) = (syms.get("MCALCPERC"), load_built_rom()) else {
        eprintln!("skip: no MCALCPERC / ROM");
        return;
    };
    let (pbr, pc) = sym_to_gsu(sym);
    let mut g = Gsu::new(rom);
    let deads = [0u8, 1, 2, 5, 7, 33, 99, 100, 128, 200, 254, 255];
    let totals = [0u8, 1, 2, 3, 4, 9, 10, 50, 100, 128, 200, 255];
    let mut diffs = 0usize;
    let mut checked = 0usize;
    let mut first: Vec<String> = Vec::new();
    for &total in &totals {
        for &dead in &deads {
            for teammates in 0u8..=3 {
                // ROM-faithful oracle: real GSU divide + 65816 wrapper.
                let ratio = if total == 0 {
                    0u16
                } else {
                    gsu_mcalcperc(&mut g, pbr, pc, dead as u16, total as u16)
                };
                let oracle = (ratio + teammates as u16 * 5).min(100) as u8;
                let rust = calc_stage_perc(dead, total, teammates);
                checked += 1;
                if oracle != rust {
                    diffs += 1;
                    if first.len() < 8 {
                        first.push(format!(
                            "DIVERGE calc_stage_perc(dead={dead},total={total},tm={teammates})=ORACLE {oracle} RUST {rust}"
                        ));
                    }
                }
            }
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!("PROBE calcstageperc: checked {checked} (dead,total,teammates); diffs={diffs}");
    assert_eq!(
        diffs, 0,
        "calc_stage_perc diverges from ROM divide + wrapper"
    );
}

// ===========================================================================
// 5. framescalevecs  — 65816 $0BEA90 (PSTRATS.ASM:3529).  Scales al_vx/vy by
//    framerate/4:  new = adiv2^3( mulslog(vel<<8, framerate) ).  There is no
//    dedicated Rust port (the fixed-step port makes it a no-op, correct only at
//    the base framerate=4 — see framescale.rs). Here we PIN the exact ROM
//    formula bit-exact against a spec built from the already-proven `mulslog`
//    plus round-toward-zero adiv2, over a broad (vel, framerate) grid — and
//    confirm framerate=4 is exact identity (so the no-op port is right at base).
// ===========================================================================
const XB: u32 = 0x0100;
const AL_VX: u32 = 0x2F;
const AL_VY: u32 = 0x31;
const FRAMERATE: u32 = 0x156E;

/// round-toward-zero halve, matching the ROM `adiv2` macro (STRATMAC.INC:712).
fn adiv2(v: i16) -> i16 {
    (v as i32 / 2) as i16
}
fn framescale_spec(vel: i8, framerate: u8) -> i16 {
    // ROM: x1 = (byte vel) << 8 as a signed word; mulslog168 x1,framerate;
    // store low 16 (m4/m5) -> x1; adiv2 three times.
    let x1 = ((vel as u8 as u16) << 8) as i16;
    let m = sf_strat::snes_trig::mulslog(x1 as i32, framerate as i32) as i16; // m4/m5 truncation
    adiv2(adiv2(adiv2(m)))
}

fn rom_framescale(rom: &[u8], addr: u32, vx: i8, vy: i8, fr: u8) -> (i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(XB + AL_VX, vx as i16 as u16);
    bus.write16(XB + AL_VY, vy as i16 as u16);
    bus.write8(FRAMERATE, fr);
    call_near(
        &mut bus,
        addr,
        &Entry {
            x: XB as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    (bus.read16(XB + AL_VX) as i16, bus.read16(XB + AL_VY) as i16)
}

#[test]
fn framescalevecs_matches_mulslog_adiv2_spec() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("FRAMESCALEVECS"), load_built_rom()) else {
        eprintln!("skip: no FRAMESCALEVECS / ROM");
        return;
    };
    // Vel across the full signed-byte domain the routine reads; framerate over
    // the real range (base 4) plus boundaries.
    let vels: Vec<i8> = {
        let mut v: Vec<i8> = (-128..=127).step_by(1).map(|x| x as i8).collect();
        v.dedup();
        v
    };
    let frames: [u8; 12] = [0, 1, 2, 3, 4, 5, 8, 15, 16, 24, 32, 60];
    let mut diffs = 0usize;
    let mut checked = 0usize;
    let mut ident_bad = 0usize;
    let mut first: Vec<String> = Vec::new();
    for &fr in &frames {
        for &vx in &vels {
            let vy = vx.wrapping_neg().wrapping_sub(1); // spread vy independently (no -128 overflow)
            let (rx, ry) = rom_framescale(&rom, addr, vx, vy, fr);
            let (sx, sy) = (framescale_spec(vx, fr), framescale_spec(vy, fr));
            checked += 1;
            if (rx, ry) != (sx, sy) {
                diffs += 1;
                if first.len() < 10 {
                    first.push(format!(
                        "DIVERGE fr={fr} vx={vx} vy={vy}: ROM=({rx},{ry}) SPEC=({sx},{sy})"
                    ));
                }
            }
            if fr == 4 && (rx as i32 != vx as i32 || ry as i32 != vy as i32) {
                ident_bad += 1;
            }
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!("PROBE framescalevecs: checked {checked}; spec diffs={diffs}; identity@fr=4 mismatches={ident_bad}");
    assert_eq!(
        diffs, 0,
        "framescalevecs diverges from mulslog(vel<<8,fr)>>3 spec"
    );
    assert_eq!(
        ident_bad, 0,
        "framerate=4 (base) must be identity — the no-op port is correct there"
    );
}

// ===========================================================================
// 6. addvecs0_l  — 65816 $01C7BB (STRATROU.ASM:493).  The raw shared body of
//    the addvecs family, entered with A ALREADY 16-bit (addvecs_l/2/4 fall
//    through after their `a16`/shifts). Confirms the shared entry adds
//    x1/y1/z1 into al_worldx/y/z identically to strat_add_to_pos.
// ===========================================================================
const AL_WX: u32 = 0x0C;
const AL_WY: u32 = 0x0E;
const AL_WZ: u32 = 0x10;
const X1: u32 = 0x0002;
const Y1: u32 = 0x0008;
const Z1: u32 = 0x008A;

#[test]
fn addvecs0_l_vs_add_to_pos() {
    use sf_game::alien::Alien;
    use sf_strat::common::strat_add_to_pos;
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("ADDVECS0_L"), load_built_rom()) else {
        eprintln!("skip: no ADDVECS0_L / ROM");
        return;
    };
    let pos_grid: [i16; 6] = [i16::MIN, -20000, -1, 0, 20000, i16::MAX];
    let vec_grid: [i16; 6] = [i16::MIN, -12345, -1, 1, 100, 12345];
    let mut diffs = 0usize;
    let mut checked = 0usize;
    let mut first: Vec<String> = Vec::new();
    for &px in &pos_grid {
        for &vx in &vec_grid {
            for &py in &pos_grid {
                let pos = (px, py, px ^ py);
                let vec = (vx, vx.wrapping_add(px), py.wrapping_sub(vx));
                let mut bus = SnesBus::new(rom.to_vec());
                bus.write16(XB + AL_WX, pos.0 as u16);
                bus.write16(XB + AL_WY, pos.1 as u16);
                bus.write16(XB + AL_WZ, pos.2 as u16);
                bus.write16(X1, vec.0 as u16);
                bus.write16(Y1, vec.1 as u16);
                bus.write16(Z1, vec.2 as u16);
                // addvecs0_l assumes 16-bit A already (no `a16`): p=$00.
                call(
                    &mut bus,
                    addr,
                    &Entry {
                        x: XB as u16,
                        p: 0x00,
                        ..Default::default()
                    },
                );
                let rom_r = (
                    bus.read16(XB + AL_WX) as i16,
                    bus.read16(XB + AL_WY) as i16,
                    bus.read16(XB + AL_WZ) as i16,
                );
                let mut al = Alien::default();
                (al.worldx, al.worldy, al.worldz) = pos;
                strat_add_to_pos(&mut al, vec.0, vec.1, vec.2);
                let rust = (al.worldx, al.worldy, al.worldz);
                checked += 1;
                if rom_r != rust {
                    diffs += 1;
                    if first.len() < 8 {
                        first.push(format!(
                            "DIVERGE pos={pos:?} vec={vec:?}: ROM={rom_r:?} RUST={rust:?}"
                        ));
                    }
                }
            }
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!("PROBE addvecs0_l: checked {checked}; diffs={diffs}");
    assert_eq!(diffs, 0, "addvecs0_l diverges from strat_add_to_pos");
}

// ===========================================================================
// 7. arctan16 / anglexy_l  — GSU $0181AA (MMATHS.MC:587/618).  The aim-angle
//    every homing/aiming enemy uses: anglexy_l computes arctan16(dx,dz) then
//    takes the HIGH byte -> an 8-bit angle. Rust equiv:
//    `sf_strat::common::strat_angle_xz` (f32 atan2, mapped 0..256).
//
//    The GSU emulator runs the real ROM arctan octant/quadrant + table code and
//    is BIT-EXACT vs atan2 at cardinal/diagonal directions; the shallow-angle
//    shift-subtract divide REFINEMENT still has a flag bug (WIP, see
//    gsu_arctan.rs) so off-axis angles can't yet be trusted. We therefore diff
//    the port only where the ROM oracle is trustworthy — the 8 axis/diagonal
//    directions at many magnitudes — and DOCUMENT the off-axis grid as blocked.
// ===========================================================================
const M_ARC_X1: usize = 0x62; // arctan16 in: r1<-[m_x1]
const M_ARC_Y1: usize = 0x2C; // r2<-[m_y1]
const M_CNT: usize = 0x40; // out: r0 -> [m_cnt]

fn gsu_arctan16(g: &mut Gsu, dx: i16, dz: i16) -> u16 {
    g.ram[M_ARC_X1] = dx as u16 as u8;
    g.ram[M_ARC_X1 + 1] = (dx as u16 >> 8) as u8;
    g.ram[M_ARC_Y1] = dz as u16 as u8;
    g.ram[M_ARC_Y1 + 1] = (dz as u16 >> 8) as u8;
    g.run(1, 0x81AA); // mcallarctan16
    g.ram[M_CNT] as u16 | ((g.ram[M_CNT + 1] as u16) << 8)
}

#[test]
fn anglexy_axis_diagonal_matches_angle_xz() {
    use sf_game::alien::Alien;
    use sf_strat::common::strat_angle_xz;
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no ROM");
        return;
    };
    let mut g = Gsu::new(rom);
    // Emulator-exact directions ONLY: the dz=0 axis (marctan16 special-cases
    // |y|==0 -> deg90, no divide) + the 4 diagonals (|x|==|y| -> deg45, no
    // divide). The dz-axis (dx=0) and all off-axis angles go through the
    // shift-subtract divide refinement, which the GSU core still gets wrong
    // (gsu_arctan.rs), so they are DOCUMENTED-blocked rather than diffed here.
    let dirs: [(i16, i16); 6] = [(1, 0), (-1, 0), (1, 1), (-1, 1), (1, -1), (-1, -1)];
    let mags: [i16; 6] = [1, 7, 100, 1000, 8000, 16000];
    let circ8 = |a: u8, b: u8| {
        let d = (a as i32 - b as i32).rem_euclid(256);
        d.min(256 - d)
    };
    let mut checked = 0usize;
    let mut off_by_one = 0usize;
    let mut worse = 0usize;
    let mut first: Vec<String> = Vec::new();
    for &(ux, uz) in &dirs {
        for &m in &mags {
            let (dx, dz) = (ux * m, uz * m);
            let rom8 = (gsu_arctan16(&mut g, dx, dz) >> 8) as u8; // anglexy_l: high byte
                                                                  // strat_angle_xz maps atan2(dx,dz) into 0..256 the same way.
            let src = Alien::default();
            let mut dst = Alien::default();
            (dst.worldx, dst.worldz) = (dx, dz);
            let rust8 = strat_angle_xz(&src, &dst);
            checked += 1;
            let d = circ8(rom8, rust8);
            if d == 1 {
                off_by_one += 1;
            } else if d > 1 {
                worse += 1;
            }
            if d > 1 && first.len() < 10 {
                first.push(format!("DIVERGE dir=({ux},{uz}) mag={m} dx={dx} dz={dz}: ROM8={rom8} RUST8={rust8} d={d}"));
            }
        }
    }
    for l in &first {
        eprintln!("{l}");
    }
    eprintln!(
        "PROBE anglexy_l(axis/diag): checked {checked}; exact-or-off1={} (off1={off_by_one}) worse={worse}",
        checked - worse
    );
    // The 8-bit truncation (ROM takes the high byte of the 16-bit angle; the
    // port truncates the float) admits at most a ±1 LSB gap at these directions;
    // anything worse is a real divergence.
    assert_eq!(
        worse, 0,
        "strat_angle_xz diverges from ROM anglexy_l by >1 LSB at an axis/diagonal direction"
    );
}
