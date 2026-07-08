//! TIER-1 pure-function differential fuzz sweep.
//!
//! Extends the proven `mulslog_oracle.rs` / `audit_strats_b.rs` pattern to a
//! BATCH of pure/leaf ROM routines, fuzzed across broad/boundary input grids
//! against the actual ROM (sf-oracle) and diffed bit-exact against the Rust
//! port. The goal is to harden the shared math layer every strat depends on and
//! to catch latent bugs of the mulslog kind (signed-byte / rounding / overflow
//! regimes that only bite at boundary inputs).
//!
//! Scope of THIS file (functions not already exhaustively swept elsewhere):
//!   - sr8_achase_alvar1..7  (8-bit proportional chase, ALL rates)  vs achase_angle
//!   - sr16_achase_alvar1..7 (16-bit proportional chase, ALL rates) vs strat_chase_proportional
//!   - addvecs_l / addvecs2_l / addvecs4_l  (pos += vec<<{0,1,2})    vs strat_add_to_pos (+spec)
//!   - add_objvecs_l         (obj2.vel += obj1.vel)                  spec (no dedicated Rust fn)
//!
//! Already covered (NOT duplicated here): mulslog (mulslog_oracle.rs), gen-vecs
//! 2d/3d/side/front (audit_trig, vector_family), perc* (audit_trig), speedto
//! (audit_trig, speedto.rs), SIN/COS tables (audit_trig), xzdiffs_l/dist
//! (audit_coldet), addalvecs_l/apply_velocity (apply_vel.rs), achase rates
//! 2/3/4 small-case (audit_strats_b). This file widens the achase sweep to all
//! rates + boundaries and adds the addvecs/add_objvecs leaves.
//!
//! ROM refs: STRATROU.ASM (addvecs_l:497, add_objvecs_l:1576, sr_achase:2740),
//! STRATMAC.INC Achase_var2A:525 + nolessrange:697 + adiv2:712.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::common::{strat_add_to_pos, strat_chase_proportional};
use sf_strat::enemy_a::achase_angle;
use sf_game::alien::Alien;

// ---- DP / field addresses (data/symbols.txt) -------------------------------
const TPX: u32 = 0x003A; // STRATROU scratch `tpx` (chase current)
const X1: u32 = 0x0002;
const Y1: u32 = 0x0008;
const Z1: u32 = 0x008A;
const XB: u32 = 0x0100; // test alien slot 1 (clear of $02..$8A scratch)
const YB: u32 = 0x0200; // test alien slot 2
const AL_WORLDX: u32 = 0x000C;
const AL_WORLDY: u32 = 0x000E;
const AL_WORLDZ: u32 = 0x0010;
const AL_VX: u32 = 0x002F;
const AL_VY: u32 = 0x0031;
const AL_VZ: u32 = 0x0033;

// ===========================================================================
// 1. sr8_achase_alvarN  (STRATROU.ASM:2763-2795)   ALL rates 1..7
//    ABI: entry A (8-bit) = target, mem[tpx] = current; result -> mem[tpx].
//    Rust equiv: sf_strat::enemy_a::achase_angle (8-bit, wraps mod 256).
// ===========================================================================
fn rom_achase8(rom: &[u8], addr: u32, current: u8, target: u8) -> u8 {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(TPX, current);
    call(&mut bus, addr, &Entry { a: target as u16, p: 0x20, ..Default::default() });
    bus.read8(TPX)
}

#[test]
fn sr8_achase_all_rates_vs_achase_angle() {
    let syms = load_symbols();
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no built ROM");
        return;
    };
    // Dense diff sweep: every `current`, a boundary+stepped set of targets, so
    // every (diff mod 256) and both wrap directions are exercised at each rate.
    let targets: Vec<u8> = {
        let mut v = vec![0u8, 1, 2, 3, 4, 7, 8, 9, 15, 16, 17, 31, 32, 33];
        v.extend((0..=255u8).step_by(17));
        v.extend([120, 127, 128, 129, 200, 240, 254, 255]);
        v.sort_unstable();
        v.dedup();
        v
    };
    let mut checked = 0usize;
    let mut diffs_reachable = 0usize; // rates 1..6, |8-bit diff| != 128
    let mut diffs_antipodal = 0usize; // |8-bit diff| == 128 (the exactly-180 boundary)
    let mut diffs_rate7 = 0usize; // rate 7: 2^7=128 unrepresentable in i8 (unreachable)
    let mut first: Vec<String> = Vec::new();
    for rate in 1u32..=7 {
        let name = format!("SR8_ACHASE_ALVAR{rate}");
        let Some(&addr) = syms.get(&name) else {
            eprintln!("skip {name}: no symbol");
            continue;
        };
        for current in 0..=255u8 {
            for &target in &targets {
                let rom = rom_achase8(&rom, addr, current, target);
                let mut rust = current;
                achase_angle(&mut rust, target, rate);
                checked += 1;
                if rom != rust {
                    let d = current.wrapping_sub(target); // 8-bit signed diff (current-target)
                    if rate == 7 {
                        // 2^7 = 128 cannot be a positive i8: the ROM's nolessrange
                        // min-step `lda #128` loads 0x80 = -128, so the step sign is
                        // corrupted. No 8-bit chase in the game uses rate 7 (ROM
                        // strat tables + achase_angle callers top out at 6), so this
                        // whole rate is unreachable.
                        diffs_rate7 += 1;
                    } else if d == 128 {
                        // Antipodal / exact-180 turn: negating the diff overflows
                        // (-(-128) == -128), so the two forms pick opposite turn
                        // directions. Reachable but symmetric (finding #1).
                        diffs_antipodal += 1;
                        if first.len() < 12 {
                            first.push(format!(
                                "ANTIPODAL rate{rate} cur={current} tgt={target}: ROM={rom} RUST={rust}"
                            ));
                        }
                    } else {
                        diffs_reachable += 1;
                        if first.len() < 12 {
                            first.push(format!(
                                "rate{rate} cur={current} tgt={target} (8bit diff={}): ROM={rom} RUST={rust}",
                                d as i8
                            ));
                        }
                    }
                }
            }
        }
    }
    eprintln!("PROBE sr8_achase: checked {checked} (rate,cur,tgt) triples over rates 1..7");
    for l in &first {
        eprintln!("DIVERGE {l}");
    }
    eprintln!(
        "test result note: sr8_achase diffs reachable(r1-6,non-antipodal)={diffs_reachable} \
         antipodal(|diff|=128)={diffs_antipodal} rate7(unreachable)={diffs_rate7}"
    );
    // Reachable everyday regime (rates 1-6, any non-antipodal angle gap) is
    // bit-exact. The antipodal |diff|=128 case and the unreachable rate-7 case
    // are documented latent divergences (see the #[ignore]d
    // sr8_achase_antipodal_divergence test + AUDIT_PURE_FNS_FINDINGS.md).
    assert_eq!(
        diffs_reachable, 0,
        "achase_angle diverges from ROM sr8_achase for a reachable (rate<=6, non-antipodal) input"
    );
    // FIXED (achase_angle now uses the ROM's target-current+add form): the
    // antipodal |diff|==128 tie is bit-exact too. Only rate 7 (unreachable, the
    // ROM's own 2^7=-128 min-step quirk) remains.
    assert_eq!(diffs_antipodal, 0, "achase_angle should now match ROM at |diff|==128");
}

/// LATENT DIVERGENCE (finding #1). ROM `sr8_achase_alvarN` computes
/// `new = current + adiv2^N(target - current)`. The Rust `achase_angle`
/// (enemy_a.rs:299) computes `new = current - adiv2^N(current - target)`. These
/// are algebraically equal EXCEPT when the 8-bit signed diff == -128 (0x80): the
/// exactly-antipodal (180-degree) angle gap, where `-(-128)` overflows back to
/// -128, so the two forms pick OPPOSITE turn directions. Both land equidistant
/// from an antipodal target, so convergence still happens, but the path (and the
/// intermediate angle) mismatches the ROM. Reachable (an enemy aimed exactly
/// opposite its target). TODO: match the ROM by adding the adiv2 result to
/// `current` from `target-current` instead of subtracting `current-target`.
#[test]
// FIXED (achase_angle now matches ROM's target-current+add at the ±128 tie).
fn sr8_achase_antipodal_divergence() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("SR8_ACHASE_ALVAR1"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    // current=0, target=128 (exactly 180 deg away), rate 1.
    let rom_new = rom_achase8(&rom, addr, 0, 128);
    let mut rust = 0u8;
    achase_angle(&mut rust, 128, 1);
    eprintln!("sr8 antipodal cur=0 tgt=128 rate1: ROM={rom_new} RUST={rust}");
    assert_eq!(rom_new, rust, "ROM turns one way (192), achase_angle the other (64)");
}

// ===========================================================================
// 2. sr16_achase_alvarN  (STRATROU.ASM:2740-2760)  ALL rates 1..7
//    ABI: entry A (16-bit) = target, mem[tpx] = current (word); result -> tpx.
//    Rust equiv: strat_chase_proportional(current, target, rate).
//    Boundary focus: diff == i16::MIN (the `-diff` overflow path documented in
//    common.rs:314) and full-range magnitudes.
// ===========================================================================
fn rom_achase16(rom: &[u8], addr: u32, current: i16, target: i16) -> i16 {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(TPX, current as u16);
    call(&mut bus, addr, &Entry { a: target as u16, p: 0x00, ..Default::default() });
    bus.read16(TPX) as i16
}

#[test]
fn sr16_achase_all_rates_vs_chase_proportional() {
    let syms = load_symbols();
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no built ROM");
        return;
    };
    // Broad signed magnitudes incl. the exact values that make diff = i16::MIN.
    let vals: [i16; 33] = [
        i16::MIN, -32767, -32000, -20000, -16385, -16384, -10000, -1000, -353, -128, -64, -17, -16,
        -9, -8, -1, 0, 1, 7, 8, 9, 16, 17, 64, 128, 941, 1000, 10000, 16384, 20000, 32000, 32766,
        i16::MAX,
    ];
    let mut checked = 0usize;
    let mut diffs_reachable = 0usize; // |diff| != 32768
    let mut diffs_antipodal = 0usize; // |current-target| == 32768 (16-bit MIN)
    let mut first: Vec<String> = Vec::new();
    for rate in 1u32..=7 {
        let name = format!("SR16_ACHASE_ALVAR{rate}");
        let Some(&addr) = syms.get(&name) else {
            eprintln!("skip {name}: no symbol");
            continue;
        };
        for &current in &vals {
            for &target in &vals {
                let rom = rom_achase16(&rom, addr, current, target);
                let rust = strat_chase_proportional(current, target, rate);
                checked += 1;
                if rom != rust {
                    let diff = (current as i32) - (target as i32);
                    if diff.abs() == 32768 {
                        diffs_antipodal += 1;
                    } else {
                        diffs_reachable += 1;
                    }
                    if first.len() < 12 {
                        first.push(format!(
                            "rate{rate} cur={current} tgt={target} (diff={diff}): ROM={rom} RUST={rust}"
                        ));
                    }
                }
            }
        }
    }
    eprintln!("PROBE sr16_achase: checked {checked} (rate,cur,tgt) triples over rates 1..7");
    for l in &first {
        eprintln!("DIVERGE {l}");
    }
    eprintln!(
        "test result note: sr16_achase diffs reachable={diffs_reachable} antipodal(|diff|=32768)={diffs_antipodal}"
    );
    // Reachable regime (any diff whose magnitude != 32768) is bit-exact. The
    // exact ±32768 diff is the 16-bit antipodal boundary — see the #[ignore]d
    // sr16_achase_antipodal_divergence test + AUDIT_PURE_FNS_FINDINGS.md.
    assert_eq!(
        diffs_reachable, 0,
        "strat_chase_proportional diverges from ROM sr16_achase for |diff| != 32768"
    );
    assert!(diffs_antipodal > 0, "expected the ±32768 boundary to still diverge");
}

/// LATENT DIVERGENCE (finding #2). Same class as finding #1 but 16-bit.
/// `strat_chase_proportional` (common.rs:304) computes `diff = current - target`
/// as an i16 `wrapping_sub`, then branches on `diff >= 0`. When the true diff is
/// exactly ±32768 the i16 wraps to i16::MIN (-32768), so the SIGN test uses the
/// wrong sign and the chase steps the wrong direction (the i32 widening in the
/// negative branch does not help — the branch was already chosen from the
/// wrapped value). Only reachable if two 16-bit quantities sit exactly 32768
/// apart (e.g. worldx +16384 vs -16384) — effectively never in normal play, but
/// a real boundary bug. TODO: compute diff in i32 and branch on the i32 sign.
#[test]
#[ignore = "latent: sr16 achase diverges from ROM at the exact ±32768 diff boundary"]
fn sr16_achase_antipodal_divergence() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("SR16_ACHASE_ALVAR1"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let rom_new = rom_achase16(&rom, addr, 0, -32768); // true diff +32768
    let rust = strat_chase_proportional(0, -32768, 1);
    eprintln!("sr16 antipodal cur=0 tgt=-32768 rate1: ROM={rom_new} RUST={rust}");
    assert_eq!(rom_new, rust, "ROM steps to -16384, chase_proportional to +16384");
}

// ===========================================================================
// 3. addvecs_l  (STRATROU.ASM:497)   pos += vec
//    ABI: shorta/longi (p=$20). X = alien base; adds x1/y1/z1 (16-bit) into
//    al_worldx/y/z. Rust equiv: strat_add_to_pos(al, dx, dy, dz).
// ===========================================================================
fn rom_addvecs(rom: &[u8], addr: u32, pos: (i16, i16, i16), vec: (i16, i16, i16)) -> (i16, i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(XB + AL_WORLDX, pos.0 as u16);
    bus.write16(XB + AL_WORLDY, pos.1 as u16);
    bus.write16(XB + AL_WORLDZ, pos.2 as u16);
    bus.write16(X1, vec.0 as u16);
    bus.write16(Y1, vec.1 as u16);
    bus.write16(Z1, vec.2 as u16);
    call(&mut bus, addr, &Entry { x: XB as u16, p: 0x20, ..Default::default() });
    (
        bus.read16(XB + AL_WORLDX) as i16,
        bus.read16(XB + AL_WORLDY) as i16,
        bus.read16(XB + AL_WORLDZ) as i16,
    )
}

/// Grid of position/vector triples incl. 16-bit wrap boundaries.
const AV_POS: [i16; 7] = [i16::MIN, -20000, -1, 0, 1, 20000, i16::MAX];
const AV_VEC: [i16; 8] = [i16::MIN, -12345, -100, -1, 0, 1, 100, 12345];

#[test]
fn addvecs_l_vs_add_to_pos() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("ADDVECS_L"), load_built_rom()) else {
        eprintln!("skip: no ADDVECS_L symbol / built ROM");
        return;
    };
    let mut checked = 0usize;
    let mut diffs = 0usize;
    let mut first: Vec<String> = Vec::new();
    for &px in &AV_POS {
        for &vx in &AV_VEC {
            for &py in &AV_POS {
                for &vy in &AV_VEC {
                    let pos = (px, py, px ^ py); // spread pz too
                    let vec = (vx, vy, vx.wrapping_add(vy));
                    let rom = rom_addvecs(&rom, addr, pos, vec);
                    let mut al = Alien::default();
                    (al.worldx, al.worldy, al.worldz) = pos;
                    strat_add_to_pos(&mut al, vec.0, vec.1, vec.2);
                    let rust = (al.worldx, al.worldy, al.worldz);
                    checked += 1;
                    if rom != rust {
                        diffs += 1;
                        if first.len() < 12 {
                            first.push(format!("pos={pos:?} vec={vec:?}: ROM={rom:?} RUST={rust:?}"));
                        }
                    }
                }
            }
        }
    }
    eprintln!("PROBE addvecs_l: checked {checked} pos/vec combos");
    for l in &first {
        eprintln!("DIVERGE {l}");
    }
    eprintln!("test result note: addvecs_l diffs={diffs}");
    assert_eq!(diffs, 0, "strat_add_to_pos diverges from ROM addvecs_l");
}

// ===========================================================================
// 4. addvecs2_l (STRATROU.ASM:491) / addvecs4_l (STRATROU.ASM:482)
//    Same as addvecs_l but pre-shift the vec 16-bit words: <<1 (x2) / <<2 (x4)
//    via `asl` before adding. No dedicated Rust fn (callers pre-scale the vec
//    or use apply_velocity), so this validates the ROM contract against a local
//    spec = strat_add_to_pos with the vec doubled/quadrupled (wrapping).
// ===========================================================================
fn spec_addvecs_shift(pos: (i16, i16, i16), vec: (i16, i16, i16), sh: u32) -> (i16, i16, i16) {
    let mut al = Alien::default();
    (al.worldx, al.worldy, al.worldz) = pos;
    strat_add_to_pos(
        &mut al,
        vec.0.wrapping_shl(sh),
        vec.1.wrapping_shl(sh),
        vec.2.wrapping_shl(sh),
    );
    (al.worldx, al.worldy, al.worldz)
}

#[test]
fn addvecs2_and_4_shift_contract() {
    let syms = load_symbols();
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no built ROM");
        return;
    };
    let mut diffs = 0usize;
    let mut checked = 0usize;
    let mut first: Vec<String> = Vec::new();
    for (name, sh) in [("ADDVECS2_L", 1u32), ("ADDVECS4_L", 2u32)] {
        let Some(&addr) = syms.get(name) else {
            eprintln!("skip {name}: no symbol");
            continue;
        };
        for &px in &AV_POS {
            for &vx in &AV_VEC {
                for &vy in &AV_VEC {
                    let pos = (px, vy, px ^ vy);
                    let vec = (vx, vy, vx.wrapping_add(vy));
                    let rom = rom_addvecs(&rom, addr, pos, vec);
                    let spec = spec_addvecs_shift(pos, vec, sh);
                    checked += 1;
                    if rom != spec {
                        diffs += 1;
                        if first.len() < 12 {
                            first.push(format!("{name} pos={pos:?} vec={vec:?}: ROM={rom:?} SPEC={spec:?}"));
                        }
                    }
                }
            }
        }
    }
    eprintln!("PROBE addvecs2/4: checked {checked} combos");
    for l in &first {
        eprintln!("DIVERGE {l}");
    }
    eprintln!("test result note: addvecs2/4 diffs={diffs}");
    assert_eq!(diffs, 0, "addvecs2_l/addvecs4_l diverge from vec<<{{1,2}} + add spec");
}

// ===========================================================================
// 5. add_objvecs_l  (STRATROU.ASM:1576)   obj2.vel += obj1.vel
//    ABI: a8i16 (p=$20). X = obj2, Y = obj1. Reads/writes al_vx/vy/vz (16-bit).
//    Adds obj1's velocity into obj2's velocity. No dedicated Rust fn (inlined
//    at the handful of call sites); validated against a local spec.
// ===========================================================================
fn rom_add_objvecs(
    rom: &[u8],
    addr: u32,
    v1: (i16, i16, i16),
    v2: (i16, i16, i16),
) -> (i16, i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    // obj1 = Y base, obj2 = X base.
    bus.write16(YB + AL_VX, v1.0 as u16);
    bus.write16(YB + AL_VY, v1.1 as u16);
    bus.write16(YB + AL_VZ, v1.2 as u16);
    bus.write16(XB + AL_VX, v2.0 as u16);
    bus.write16(XB + AL_VY, v2.1 as u16);
    bus.write16(XB + AL_VZ, v2.2 as u16);
    call(&mut bus, addr, &Entry { x: XB as u16, y: YB as u16, p: 0x20, ..Default::default() });
    (
        bus.read16(XB + AL_VX) as i16,
        bus.read16(XB + AL_VY) as i16,
        bus.read16(XB + AL_VZ) as i16,
    )
}

#[test]
fn add_objvecs_l_sum_contract() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("ADD_OBJVECS_L"), load_built_rom()) else {
        eprintln!("skip: no ADD_OBJVECS_L symbol / built ROM");
        return;
    };
    let vals: [i16; 6] = [i16::MIN, -12345, -1, 0, 1, i16::MAX];
    let mut checked = 0usize;
    let mut diffs = 0usize;
    let mut first: Vec<String> = Vec::new();
    for &a in &vals {
        for &b in &vals {
            for &c in &vals {
                let v1 = (a, b, c);
                let v2 = (b, c, a);
                let rom = rom_add_objvecs(&rom, addr, v1, v2);
                let spec = (
                    v2.0.wrapping_add(v1.0),
                    v2.1.wrapping_add(v1.1),
                    v2.2.wrapping_add(v1.2),
                );
                checked += 1;
                if rom != spec {
                    diffs += 1;
                    if first.len() < 12 {
                        first.push(format!("v1={v1:?} v2={v2:?}: ROM={rom:?} SPEC={spec:?}"));
                    }
                }
            }
        }
    }
    eprintln!("PROBE add_objvecs_l: checked {checked} combos");
    for l in &first {
        eprintln!("DIVERGE {l}");
    }
    eprintln!("test result note: add_objvecs_l diffs={diffs}");
    assert_eq!(diffs, 0, "add_objvecs_l diverges from obj2.vel += obj1.vel spec");
}
