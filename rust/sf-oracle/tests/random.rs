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
        let e = call(&mut bus, addr, &Entry { p: 0x20, ..Default::default() });
        out.push(e.a); // RANDOM returns A (new $DE)
    }
    out
}

// Faithful port of the SWB chain (8-bit, 6502 SBC borrow semantics).
fn rust_random(s: &mut [u8; 4]) -> u8 {
    let de = s[0];
    let mut a = de as u16;
    let mut carry = 0u16; // CLC
    for &m in &[s[1], s[2], s[3], de] {
        let t = a.wrapping_sub(m as u16).wrapping_sub(1 - carry);
        carry = if (a as i32) - (m as i32) - (1 - carry as i32) >= 0 { 1 } else { 0 };
        a = t & 0xFF;
    }
    // stores happen in order df,e0,e1,de — recompute with side effects:
    let de0 = s[0];
    let mut aa = de0 as u16;
    let mut c = 0u16;
    let idxs = [1usize, 2, 3, 0];
    let subs = [s[1], s[2], s[3], de0];
    for k in 0..4 {
        let m = subs[k] as u16;
        let raw = (aa as i32) - (m as i32) - (1 - c as i32);
        c = if raw >= 0 { 1 } else { 0 };
        aa = (raw as u16) & 0xFF;
        s[idxs[k]] = aa as u8;
    }
    let _ = a;
    s[0]
}

#[test]
fn random_swb_matches_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("RANDOM_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    for seed in [[1u8, 2, 3, 4], [0xAB, 0xCD, 0xEF, 0x12], [0, 0, 0, 0], [0xFF, 0xFF, 0xFF, 0xFF]] {
        let romv = rom_seq(&rom, addr, seed, 8);
        let mut st = seed;
        let rustv: Vec<u8> = (0..8).map(|_| rust_random(&mut st)).collect();
        eprintln!("seed {seed:02x?}\n  ROM  {romv:02x?}\n  RUST {rustv:02x?}  {}",
            if romv == rustv { "ok" } else { "DIFF" });
        assert_eq!(romv, rustv, "SWB port must match ROM RANDOM for seed {seed:02x?}");
    }
}
