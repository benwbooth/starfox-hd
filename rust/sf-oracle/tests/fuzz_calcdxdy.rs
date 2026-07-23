//! TIER-1 leaf fuzz: `dividebynum` (CALCDXDY helper) vs Rust.
//!
//! ROM: PLANETS.ASM:1884 (`DIVIDEBYNUM` $03:BE31), near RTS — use `call_near`.
//! ABI: a16 on entry = delta; `scrframesb`=$1C (divisor); `remainder`=$22
//! (in/out); result A = quotient, `remainder` updated.
//!
//! Rust: `sf_game::planets::divide_by_num`.

use sf_game::planets::{calc_scroll_step, divide_by_num};
use sf_oracle::{call_near, load_built_rom, load_symbols, Entry, SnesBus};

const SCRFRAMESB: u32 = 0x001C;
const REMAINDER: u32 = 0x0022;

fn rom_divide(rom: &[u8], addr: u32, delta: i16, frames: u16, rem: u16) -> (i16, u16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(SCRFRAMESB, frames);
    bus.write16(REMAINDER, rem);
    let exit = call_near(
        &mut bus,
        addr,
        &Entry {
            a: delta as u16,
            p: 0x00, // 16-bit A/X/Y (routine does `i16` / longa)
            ..Default::default()
        },
    );
    (exit.c as i16, bus.read16(REMAINDER))
}

#[test]
fn dividebynum_matches_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("DIVIDEBYNUM"), load_built_rom()) else {
        eprintln!("skip: no DIVIDEBYNUM / ROM");
        return;
    };
    let deltas: Vec<i16> = {
        let mut v = vec![
            0, 1, -1, 2, -2, 7, -7, 10, -10, 31, 32, 33, -31, -32, -33, 64, -64, 100, -100, 127,
            -128, 255, -255, 256, -256, 1000, -1000, 4096, -4096, 16000, -16000, 32767, -32768,
        ];
        v.extend((-200..=200i16).step_by(17));
        v.sort_unstable();
        v.dedup();
        v
    };
    let frames_list: &[u16] = &[1, 2, 3, 4, 8, 16, 31, 32, 33, 64];
    let rems: &[u16] = &[0, 1, 7, 15, 31];
    let mut checked = 0usize;
    let mut diffs = 0usize;
    let mut first: Vec<String> = Vec::new();
    for &frames in frames_list {
        for &rem0 in rems {
            if rem0 >= frames {
                continue;
            }
            for &delta in &deltas {
                let (rq, rr) = rom_divide(&rom, addr, delta, frames, rem0);
                let (uq, ur) = divide_by_num(delta, frames, rem0);
                checked += 1;
                if (rq, rr) != (uq, ur) {
                    diffs += 1;
                    if first.len() < 20 {
                        first.push(format!(
                            "d={delta} fr={frames} rem={rem0}: ROM=({rq},{rr}) RUST=({uq},{ur})"
                        ));
                    }
                }
                // calc_scroll_step zero-path
                let (zq, zr) = calc_scroll_step(0, frames, rem0);
                assert_eq!((zq, zr), (0, rem0));
            }
        }
    }
    eprintln!("PROBE dividebynum: checked {checked}; diffs={diffs}");
    for s in &first {
        eprintln!("DIVERGE {s}");
    }
    assert_eq!(diffs, 0, "dividebynum diverged; first={first:?}");
}
