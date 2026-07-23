//! ROM RANDOM ($2F7BF, via RANDOM_L $2F7BB RTL) — the RUNTIME RNG (called 32x
//! in strat code). Disasm is a 4-byte subtract-with-borrow chain over $DE-$E1:
//!   LDA $DE; CLC; SBC $DF->$DF; SBC $E0->$E0; SBC $E1->$E1; SBC $DE->$DE; ret A
//! NOT the ×91+$61D7 LCG (that's a BUILD-TIME macro, MACROS.INC, for baking
//! static data). The Rust sf_random uses the LCG at runtime -> wrong. This
//! captures the true sequence + validates a faithful SWB port.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const RAND: u32 = 0x00DE; // $DE..$E1

fn rom_seq(rom: &[u8], addr: u32, seed: [u8; 4], n: usize) -> Vec<u8> {
    let mut bus = SnesBus::new(rom.to_vec());
    for (i, &b) in seed.iter().enumerate() {
        bus.write8(RAND + i as u32, b);
    }
    let mut out = Vec::new();
    for _ in 0..n {
        let e = call(
            &mut bus,
            addr,
            &Entry {
                p: 0x20,
                ..Default::default()
            },
        );
        out.push(e.a); // RANDOM returns A (new $DE)
    }
    out
}

/// Faithful port of the ROM's SWB chain — proves the CORRECT runtime RNG
/// algorithm. NOT yet wired into sf_random (which stays on the C-oracle LCG to
/// preserve the frozen boss parity fixtures — see rom-oracle-plan.md for the
/// C-oracle-vs-ROM-oracle decision this is pending).
#[test]
fn sf_random_matches_rom() {
    use sf_game::vars::GameVars;
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("RANDOM_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    for seed in [
        [1u8, 2, 3, 4],
        [0xAB, 0xCD, 0xEF, 0x12],
        [0, 0, 0, 0],
        [0xFF, 0xFF, 0xFF, 0xFF],
    ] {
        let romv = rom_seq(&rom, addr, seed, 8);
        // The real, wired port: seed GameVars.rng, call sf_random (returns the
        // new low byte widened to u16; callers mask, so the low byte is truth).
        let mut vars = GameVars::default();
        vars.rng = seed;
        let rustv: Vec<u8> = (0..8)
            .map(|_| sf_strat::common::sf_random(&mut vars) as u8)
            .collect();
        eprintln!(
            "seed {seed:02x?}\n  ROM  {romv:02x?}\n  RUST {rustv:02x?}  {}",
            if romv == rustv { "ok" } else { "DIFF" }
        );
        assert_eq!(
            romv, rustv,
            "sf_random must match ROM RANDOM for seed {seed:02x?}"
        );
    }
}
