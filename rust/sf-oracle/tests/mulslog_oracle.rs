//! Differential test: the real ROM `muls816log16_l` vs the Rust `mulslog`
//! (sf-strat::snes_trig). This is the fixed-point signed multiply behind every
//! `mulslog168` in the 3D-vector / steering math.
//!
//! ROM chain (assembled under `ifeq 0`, which is TRUE):
//!   mulslog168 \1,\2   MACROS.INC:2540 — loads \1 as a 16-bit word into m1/m2,
//!                      \2 as one byte into m3, then `jsl muls816log16_l`.
//!   muls816log16       GAME.ASM:543    — stz m6; stz m7; mla16mac m1,m3,m4,m6.
//!   mla16mac \1,\2,\3,\4  MACROS.INC:1303 — \3 = \1*\2 + \4, i.e. m4 = m1*m3.
//!
//! In mla16mac, \1 (m1/m2) is a SIGNED 16-bit word (`bmi` on m2) and \2 (m3) is
//! a SIGNED 8-bit byte (`bmi` on m3). It doubles |\2| in an 8-bit accumulator
//! (`asl a` — so \2 == -128 overflows to fr=0), runs the SNES hardware
//! multiplier byte-wise, and accumulates `xh*fr + hi_byte(xl*fr)` = |m1*m3|>>7,
//! then adds (sign=XOR positive) or subtracts (sign=XOR negative) into m6=0.
//!
//! REQUIRES 8-bit index: `ldy rdmpyhr` reads only $4217; with a 16-bit index it
//! would slurp $4218 (JOY1L) into the high byte and corrupt the result. So the
//! call runs with p=$30 (M=1, X=1), the state the game is in at each call site.
//!
//! ABI: m1=$5D, m2=$5E (16-bit `a`), m3=$5F (byte `b`), result m4=$60 (16-bit).

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const M1: u32 = 0x005D; // 16-bit word: m1 (low) / m2 (high)
const M3: u32 = 0x005F; // 8-bit byte
const M4: u32 = 0x0060; // 16-bit result

fn rom_mulslog(rom: &[u8], addr: u32, a: u16, b: u8) -> i16 {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(M1, a); // m1 = low(a), m2 = high(a)
    bus.write8(M3, b); //  m3 = b (signed byte)
                       // p=$30: 8-bit A AND 8-bit index — mla16mac's `ldy rdmpyhr` demands i8.
    call(
        &mut bus,
        addr,
        &Entry {
            p: 0x30,
            ..Default::default()
        },
    );
    bus.read16(M4) as i16
}

#[test]
fn mulslog_matches_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("MULS816LOG16_L"), load_built_rom()) else {
        eprintln!("skip: no MULS816LOG16_L symbol / built ROM");
        return;
    };

    // `a` (the 16-bit word) across every regime: small, bit-7 set, >=256,
    // >=32768 (bit 15 set -> ROM sees it negative), and a few explicit i32
    // negatives (which, masked to 16-bit, must agree with their u16 twins).
    let a_words: Vec<u16> = vec![
        0, 1, 2, 3, 4, 5, 7, 8, 15, 16, 31, 32, 63, 64, 100, 127, 128, 129, 200, 255, 256, 257,
        300, 511, 512, 1000, 4095, 4096, 16383, 16384, 32000, 32767, 32768, 32769, 40000, 49152,
        65280, 65535,
    ];
    // A handful of raw i32 `a` inputs whose low 16 bits equal a word above, to
    // prove mulslog reinterprets bit-15 the same way regardless of how the
    // caller framed it.
    let a_raw_extra: Vec<i32> = vec![-1, -100, -32768, -40000, 100_000, -70000];

    let mut diffs = 0usize;
    let mut checked = 0usize;
    let mut first_diffs: Vec<String> = Vec::new();

    // `b` (the m3 byte): EXHAUSTIVE over all 256 byte values — this is the crux
    // of the "≥128 latent" (bit 7 set => ROM treats it as a signed negative).
    let mut check = |a_word: u16, a_i32: i32| {
        for b in 0u8..=255u8 {
            let rom = rom_mulslog(&rom, addr, a_word, b) as i32;
            // Caller may pass `b` as a raw positive byte (0..=255) OR pre-signed;
            // mulslog must yield the ROM's signed-byte result either way. Test the
            // raw-positive framing (the risky one).
            let rust = sf_strat::snes_trig::mulslog(a_i32, b as i32);
            checked += 1;
            if rom != rust {
                diffs += 1;
                if first_diffs.len() < 20 {
                    first_diffs.push(format!(
                        "a=0x{a_word:04X}({}) b=0x{b:02X}({}): ROM={rom} RUST={rust}",
                        a_word as i16, b as i8
                    ));
                }
            }
        }
    };

    for &a in &a_words {
        check(a, a as i32);
    }
    for &a in &a_raw_extra {
        check((a as i32 as u32 & 0xFFFF) as u16, a);
    }

    eprintln!("checked {checked} (a,b) pairs across the full grid");
    if diffs != 0 {
        eprintln!("{diffs} divergence(s); first up to 20:");
        for d in &first_diffs {
            eprintln!("  {d}");
        }
    } else {
        eprintln!("bit-exact across the full grid");
    }
    assert_eq!(diffs, 0, "mulslog diverges from ROM muls816log16_l");
}
