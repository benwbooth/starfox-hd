//! ROM do_coll_l (STRATROU.ASM, $1FD252, RTL) — collision damage/cooldown.
//! Disasm:
//!   s_decbne_alvar collcount,.skip   ; DEC collcount; BNE exit
//!   if intunnel && x1==hardAP: x1 >>= 1
//!   LDA hp; BMI .o2c                  ; hp bit7 set (>=$80) => indestructible
//!   hp -= x1 (clamp 0)
//!   .o2c: collcount = tpa
//! ABI: X=alien (collcount=$2D, hp=$2A), x1=AP@$02, tpa@$1550, pshipflags3@$1563.
//! Confirms the port's two deviations, then validates the faithful fix.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const XB: u32 = 0x0100;
const COLLCOUNT: u32 = 0x2D;
const HP: u32 = 0x2A;
const X1: u32 = 0x02;
const TPA: u32 = 0x1550;
const PSHIPFLAGS3: u32 = 0x1563;
const FRAMESPERAP: u8 = 10;
const HARD_AP: u8 = 8;

fn rom(rom: &[u8], addr: u32, collcount: u8, hp: u8, ap: u8, tunnel: bool) -> (u8, u8) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(XB + COLLCOUNT, collcount);
    bus.write8(XB + HP, hp);
    bus.write8(X1, ap);
    bus.write8(TPA, FRAMESPERAP);
    bus.write8(PSHIPFLAGS3, if tunnel { 1 } else { 0 });
    call(
        &mut bus,
        addr,
        &Entry {
            x: XB as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    (bus.read8(XB + COLLCOUNT), bus.read8(XB + HP))
}

// Current port (coldet.rs:113).
fn rust_current(collcount: u8, hp: u8, ap: u8, tunnel: bool) -> (u8, u8) {
    let (mut cc, mut h) = (collcount, hp);
    if cc > 0 {
        cc -= 1;
        return (cc, h);
    }
    if h == 0xFF {
        return (FRAMESPERAP, h);
    }
    let mut dmg = ap;
    if tunnel && dmg == HARD_AP {
        dmg >>= 1;
    }
    h = if h <= dmg { 0 } else { h - dmg };
    (FRAMESPERAP, h)
}

// Faithful fix (matches disasm exactly).
fn rust_fixed(collcount: u8, hp: u8, ap: u8, tunnel: bool) -> (u8, u8) {
    let (mut cc, mut h) = (collcount, hp);
    cc = cc.wrapping_sub(1);
    if cc != 0 {
        return (cc, h);
    }
    let mut dmg = ap;
    if tunnel && dmg == HARD_AP {
        dmg >>= 1;
    }
    if (h as i8) >= 0 {
        h = h.saturating_sub(dmg);
    }
    (FRAMESPERAP, h)
}

#[test]
fn do_coll_vs_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(r)) = (syms.get("DO_COLL_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let cases: [(u8, u8, u8, bool); 6] = [
        (10, 5, 1, false),
        (1, 5, 1, false),    // ROM damages (dec->0); current port returns
        (0, 5, 1, false),    // ROM wraps to 255; current port damages immediately
        (1, 0x80, 1, false), // ROM indestructible (bit7); current port not
        (1, 3, 5, false),    // underflow clamp
        (1, 10, 8, true),    // tunnel hardAP halving
    ];
    let (mut cur_diffs, mut fix_diffs) = (0, 0);
    for (cc, hp, ap, tun) in cases {
        let o = rom(&r, addr, cc, hp, ap, tun);
        let c = rust_current(cc, hp, ap, tun);
        let f = rust_fixed(cc, hp, ap, tun);
        if o != c {
            cur_diffs += 1;
        }
        if o != f {
            fix_diffs += 1;
        }
        eprintln!(
            "cc={cc} hp={hp:#04x} ap={ap} tun={tun}: ROM{o:?} cur{c:?}{} fixed{f:?}{}",
            if o == c { "" } else { " <DIFF>" },
            if o == f { "" } else { " <DIFF>" }
        );
    }
    eprintln!("current port: {cur_diffs} diffs vs ROM; faithful fix: {fix_diffs} diffs");
    assert!(
        cur_diffs > 0,
        "expected the current port to deviate (documents the bug)"
    );
    assert_eq!(fix_diffs, 0, "faithful fix must match ROM on every case");
}
