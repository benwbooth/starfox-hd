//! ROM playerlimitx_srou (PSTRATS.ASM $BDF1C, RTS). Disassembly @$BDF1C:
//!   LDA $1ACB; AND #$F3; STA $1ACB      ; arrows &= ~(left|right)  [8-bit A]
//!   REP #$20; LDA $0C,X; CMP $15F9; SEP #$20
//!   BEQ clamp; BMI clamp; JML .nminX     ; clamp when worldX <= minpmoveX
//!   clamp: worldX = minpmoveX; arrows |= $04 (left)
//! => INCLUSIVE boundary (worldX==min sets left arrow). Entry is 8-bit A
//! (p=$20), 16-bit X. The Rust port uses `worldx < minx` (exclusive) -> misses
//! the == arrow. This confirms the ROM truth bit-exactly.

use sf_oracle::{call_near, load_built_rom, load_symbols, Entry, SnesBus};

const XB: u32 = 0x0100;
const AL_WORLDX: u32 = 0x0C;
const AL_WORLDY: u32 = 0x0E;
const MINPMOVEX: u32 = 0x15F9;
const MAXPMOVEX: u32 = 0x15FB;
const PMOVELIMITAND: u32 = 0x156A;
const MINPMOVEY: u32 = 0x1601;
const MAXPMOVEY: u32 = 0x1603;
const ARROWS: u32 = 0x1ACB;
const SPRAR_LEFT: u8 = 0x04;
const SPRAR_RIGHT: u8 = 0x08;
const SPRAR_UP: u8 = 0x01;
const SPRAR_DOWN: u8 = 0x02;
const BODY_BOTTOM_COLLISION: u8 = 128;

const PLAYER_Y_LIMIT_ENTRY: u32 = 0x0B_D4E0;
const PLAYER_Y_LIMIT_RETURN: usize = 0x05_D52D;

fn rom_limit(rom: &[u8], addr: u32, worldx: i16, min: i16, max: i16) -> (i16, u8) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(XB + AL_WORLDX, worldx as u16);
    bus.write16(MINPMOVEX, min as u16);
    bus.write16(MAXPMOVEX, max as u16);
    bus.write8(ARROWS, 0);
    // 8-bit A (p=$20), 16-bit X — as the ROM caller enters it.
    call_near(
        &mut bus,
        addr,
        &Entry {
            x: XB as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    (
        bus.read16(XB + AL_WORLDX) as i16,
        bus.read8(ARROWS) & (SPRAR_LEFT | SPRAR_RIGHT),
    )
}

/// Rust port's current logic (player.rs playerlimit_x_srou, X axis only).
fn rust_limit(worldx: i16, min: i16, max: i16) -> (i16, u8) {
    // Fixed port: inclusive boundary (matches ROM BEQ+BMI / BEQ+BPL).
    let mut wx = worldx;
    let mut arr = 0u8;
    if wx <= min {
        wx = min;
        arr |= SPRAR_LEFT;
    }
    if wx >= max {
        wx = max;
        arr |= SPRAR_RIGHT;
    }
    (wx, arr)
}

#[test]
fn player_bounds_vs_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("PLAYERLIMITX_SROU"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let (min, max) = (-500i16, 500i16);
    let mut diffs = 0;
    for wx in [-600i16, -500, -499, 0, 499, 500, 600] {
        let (ox, oarr) = rom_limit(&rom, addr, wx, min, max);
        let (gx, garr) = rust_limit(wx, min, max);
        let d = if ox == gx && oarr == garr {
            "ok"
        } else {
            diffs += 1;
            "DIFF"
        };
        eprintln!("worldX {wx:5}: ROM(clamp {ox:5}, arr {oarr:#04x})  RUST(clamp {gx:5}, arr {garr:#04x})  {d}");
    }
    // ROM truth: clamp is correct; the port differs only at the inclusive edge
    // (worldX==min/max), where the ROM sets the arrow and the port does not.
    let (_, arr_min) = rom_limit(&rom, addr, min, min, max);
    let (_, arr_max) = rom_limit(&rom, addr, max, min, max);
    assert!(
        arr_min & SPRAR_LEFT != 0,
        "ROM sets LEFT arrow at worldX==min"
    );
    assert!(
        arr_max & SPRAR_RIGHT != 0,
        "ROM sets RIGHT arrow at worldX==max"
    );
    eprintln!("=> {diffs} case(s) where the Rust port differs from the ROM");
    assert_eq!(diffs, 0, "fixed port must match ROM at every boundary");
}

fn rom_y_limit(rom: &[u8], worldy: i16, min: i16, max: i16, movement_limits: u8) -> (i16, u8) {
    let mut isolated = rom.to_vec();
    // Stop immediately after the two authored Y-bound checks. This changes
    // only the continuation instruction; every tested branch and write is
    // executed directly from the retail ROM bytes.
    isolated[PLAYER_Y_LIMIT_RETURN] = 0x6B; // RTL

    let mut bus = SnesBus::new(isolated);
    bus.write16(XB + AL_WORLDY, worldy as u16);
    bus.write16(MINPMOVEY, min as u16);
    bus.write16(MAXPMOVEY, max as u16);
    bus.write8(PMOVELIMITAND, movement_limits);
    bus.write8(ARROWS, 0);
    sf_oracle::call(
        &mut bus,
        PLAYER_Y_LIMIT_ENTRY,
        &Entry {
            x: XB as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    (
        bus.read16(XB + AL_WORLDY) as i16,
        bus.read8(ARROWS) & (SPRAR_UP | SPRAR_DOWN),
    )
}

#[test]
fn player_vertical_bounds_match_the_retail_collision_gate() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: built ROM data/sf.sfc not present");
        return;
    };

    let min = -100;
    let max = 50;

    assert_eq!(
        rom_y_limit(&rom, 80, min, max, 0),
        (max, SPRAR_DOWN),
        "clear body-bottom lane must apply the ordinary floor clamp"
    );
    assert_eq!(
        rom_y_limit(&rom, 80, min, max, BODY_BOTTOM_COLLISION),
        (80, 0),
        "enabled body-bottom collision lane must bypass the ordinary floor clamp"
    );
    assert_eq!(
        rom_y_limit(&rom, min, min, max, 0),
        (min, SPRAR_UP),
        "the upper-screen boundary is inclusive"
    );
    assert_eq!(
        rom_y_limit(&rom, 0, -50, -70, 0),
        (-50, SPRAR_UP | SPRAR_DOWN),
        "retail applies the lower clamp before the upper clamp"
    );
}
