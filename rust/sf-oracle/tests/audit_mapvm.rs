//! Differential audit: map-VM spawn opcodes' coordinate decompression &
//! positioning, ROM handlers vs the Rust port (`sf-game/src/game.rs`
//! `map_exec` QOBJ/OBJ8/DOBJ/MAPOBJ/MOTHER arms).
//!
//! The ROM handlers (MAPQOBJDO $3e981, MAPOBJ8DO $3eab1, MAPDOBJDO $3ec33,
//! MAPOBJDO $3eb80, MAPMOTHER $3ee8f) each: alloc a block off the free list,
//! `init_objvars_l` it, then decompress x/y/z from the map struct into
//! al_worldx/y/z ($0C/$0E/$10) with z taken relative to the player's z
//! (via `playpt`). They end with `lda mapcnt; bne .listfull(rts)`, so a
//! *nonzero* frame byte makes them process exactly one object and RTS —
//! which is what lets us isolate a single spawn.
//!
//! Struct/field refs: WORLD.ASM:1343-1960, MAPSTRUC.INC, MAPMACS.INC.

use sf_oracle::{call_near, load_built_rom, load_symbols, Entry, SnesBus};

// Direct-page executor vars (symbols.txt).
const ALLST: u32 = 0x12ad;
const ALFREELST: u32 = 0x12af;
const PLAYPT: u32 = 0x12c3;
const MAPCNT: u32 = 0x1780;

// Alien struct field offsets (symbols.txt): al_worldx/y/z, al_shape.
const AL_SHAPE: u32 = 0x04;
const AL_WORLDX: u32 = 0x0c;
const AL_WORLDY: u32 = 0x0e;
const AL_WORLDZ: u32 = 0x10;

// WRAM layout for the harness (avoid stub $0200-$0213 and RTS trap $0300).
const PLAYER: u32 = 0x0100; // player block (playpt)
const OBJ: u32 = 0x0140; // free block that will be allocated for the spawn
                         // The map struct base is `mapbase&WM` = $8000 (symbols.txt MAP_CTRL=$8000);
                         // handlers read fields as absolute `$8000+field+X` under DBR=mapbank. We host
                         // the map in WRAM bank $7E ($7E:8000 = wram index 0x8000) and set DBR=$7E via a
                         // trampoline. mapptr (Y) = 0, so the opcode sits at $7E:8000.
const MAPWRAM: u32 = 0x8000; // wram index that $7E:8000 maps to
const TRAMPOLINE: u32 = 0x0400; // DBR-setter stub

struct Spawn {
    x: i16,
    y: i16,
    z: i16,
    mapcnt: u16,
    shape: u16,
}

/// Seed the free list / player, write `map_bytes` at MAP, call the ROM handler
/// at `addr` with Y=MAP, and read back the spawned block's world coords.
fn rom_spawn(rom: &[u8], addr: u32, player_z: i16, map_bytes: &[u8]) -> Spawn {
    let mut bus = SnesBus::new(rom.to_vec());
    // Player block: only its z matters (z-add source).
    bus.write16(PLAYER + AL_WORLDZ, player_z as u16);
    // Free list: allst empty, alfreelst -> OBJ, OBJ._next = 0 (single block).
    bus.write16(ALLST, 0x0000);
    bus.write16(ALFREELST, OBJ as u16);
    bus.write16(OBJ + 0x00, 0x0000); // _next
    bus.write16(PLAYPT, PLAYER as u16);
    for (i, b) in map_bytes.iter().enumerate() {
        // $7E:8000+i  (wram index 0x8000+i).
        bus.write8(0x7E_0000 | (MAPWRAM + i as u32), *b);
    }
    // Trampoline: set DBR=$7E (so `$8000,x` reads our WRAM map) then JML the
    // real handler. Enters with 16-bit A (call_near REP #$30); balance PHA/PLB
    // in 8-bit A. Y (mapptr) is preserved into the handler.
    let tramp: [u8; 9] = [
        0xE2, 0x20, // SEP #$20  (8-bit A)
        0xA9, 0x7E, // LDA #$7E
        0x48, // PHA
        0xAB, // PLB           -> DBR = $7E
        0xC2, 0x20, // REP #$20 (16-bit A)
        0x5C, // JML ... (target bytes appended below)
    ];
    for (i, b) in tramp.iter().enumerate() {
        bus.write8(TRAMPOLINE + i as u32, *b);
    }
    bus.write8(TRAMPOLINE + 9, addr as u8);
    bus.write8(TRAMPOLINE + 10, (addr >> 8) as u8);
    bus.write8(TRAMPOLINE + 11, (addr >> 16) as u8);
    // Handlers RTS (near) when mapcnt != 0; entered a16i16 with Y = mapptr = 0.
    call_near(
        &mut bus,
        TRAMPOLINE,
        &Entry {
            y: 0x0000,
            p: 0x00,
            ..Default::default()
        },
    );
    Spawn {
        x: bus.read16(OBJ + AL_WORLDX) as i16,
        y: bus.read16(OBJ + AL_WORLDY) as i16,
        z: bus.read16(OBJ + AL_WORLDZ) as i16,
        mapcnt: bus.read16(MAPCNT),
        shape: bus.read16(OBJ + AL_SHAPE),
    }
}

// ---- Rust port's decompression formulas (mirrors game.rs map_exec) ----

fn rust_qobj(xr: u8, yr: u8, zr: u8, fr: u8, player_z: i16) -> (i16, i16, i16, u16) {
    let x = (xr as i8 as i16) << 2;
    let y = (yr as i8 as i16) << 2;
    let z = ((zr as u16) << 4) as i16; // zero-extended
    let mapcnt = (fr as u16) << 4;
    (x, y, player_z.wrapping_add(z), mapcnt)
}

fn rust_obj8(xr: u8, yr: u8, zr: u8, fr: u8, player_z: i16) -> (i16, i16, i16, u16) {
    let x = (xr as i8 as i16) << 2;
    let y = (yr as i8 as i16) << 2;
    let z = (zr as i8 as i16) << 4; // sign-extended
    let mapcnt = (fr as u16) << 2;
    (x, y, player_z.wrapping_add(z), mapcnt)
}

fn setup() -> Option<(std::collections::HashMap<String, u32>, Vec<u8>)> {
    let syms = load_symbols();
    let rom = load_built_rom()?;
    Some((syms, rom))
}

#[test]
fn qobj_coords_match_rom() {
    let Some((syms, rom)) = setup() else {
        eprintln!("skip: no ROM/symbols");
        return;
    };
    let Some(&addr) = syms.get("MAPQOBJDO") else {
        eprintln!("skip: no MAPQOBJDO");
        return;
    };
    // (frame, x, y, z, shape, strat) raw bytes; frame nonzero so it RTSs.
    let cases: [(u8, u8, u8, u8, u8, u8); 6] = [
        (2, 0x05, 0x0A, 0x10, 0, 0), // small positive
        (1, 0xFC, 0xF6, 0x08, 0, 0), // negative x/y (-4, -10)
        (3, 0x7F, 0x80, 0xF0, 0, 0), // x=+127 y=-128 z=240 (zero-ext!)
        (1, 0x00, 0x00, 0xFF, 0, 0), // z=255 -> 0x0FF0 zero-ext
        (5, 0x40, 0xC0, 0x00, 0, 0), // z=0
        (2, 0x81, 0x7F, 0x01, 0, 0), // x=-127 y=+127
    ];
    let player_z: i16 = 0x1000;
    let mut bad = 0;
    for (fr, xr, yr, zr, sh, st) in cases {
        let map = [112u8, fr, xr, yr, zr, sh, st];
        let got = rom_spawn(&rom, addr, player_z, &map);
        let (ex, ey, ez, emc) = rust_qobj(xr, yr, zr, fr, player_z);
        if (got.x, got.y, got.z, got.mapcnt) != (ex, ey, ez, emc) {
            bad += 1;
            eprintln!(
                "QOBJ x={xr:02x} y={yr:02x} z={zr:02x} f={fr:02x}: ROM=({},{},{},mc={}) RUST=({ex},{ey},{ez},mc={emc})",
                got.x, got.y, got.z, got.mapcnt
            );
        }
    }
    assert_eq!(bad, 0, "QOBJ coordinate decompression mismatches vs ROM");
}

#[test]
fn obj8_coords_match_rom() {
    let Some((syms, rom)) = setup() else {
        eprintln!("skip: no ROM/symbols");
        return;
    };
    let Some(&addr) = syms.get("MAPOBJ8DO") else {
        eprintln!("skip: no MAPOBJ8DO");
        return;
    };
    // OBJ8 layout: ctrl,frame,x,y,z, shape(2), strat(2), bank(1).
    let cases: [(u8, u8, u8, u8); 6] = [
        (2, 0x05, 0x0A, 0x10),
        (1, 0xFC, 0xF6, 0x08),
        (3, 0x7F, 0x80, 0xF0), // z=0xF0 SIGN-extended here (-16<<4=-256)
        (1, 0x00, 0x00, 0xFF), // z=-1<<4 = -16
        (5, 0x40, 0xC0, 0x7F), // z=+127<<4
        (2, 0x81, 0x7F, 0x80), // z=-128<<4
    ];
    let player_z: i16 = 0x1000;
    let mut bad = 0;
    for (fr, xr, yr, zr) in cases {
        let map = [114u8, fr, xr, yr, zr, 0, 0, 0, 0, 0];
        let got = rom_spawn(&rom, addr, player_z, &map);
        let (ex, ey, ez, emc) = rust_obj8(xr, yr, zr, fr, player_z);
        if (got.x, got.y, got.z, got.mapcnt) != (ex, ey, ez, emc) {
            bad += 1;
            eprintln!(
                "OBJ8 x={xr:02x} y={yr:02x} z={zr:02x} f={fr:02x}: ROM=({},{},{},mc={}) RUST=({ex},{ey},{ez},mc={emc})",
                got.x, got.y, got.z, got.mapcnt
            );
        }
    }
    assert_eq!(bad, 0, "OBJ8 coordinate decompression mismatches vs ROM");
}

#[test]
fn mapobj_coords_match_rom() {
    let Some((syms, rom)) = setup() else {
        eprintln!("skip: no ROM/symbols");
        return;
    };
    let Some(&addr) = syms.get("MAPOBJDO") else {
        eprintln!("skip: no MAPOBJDO");
        return;
    };
    // MAPOBJ: ctrl, frame(2), x(2), y(2), z(2), shape(1), strat(1). Full i16.
    let cases: [(u16, i16, i16, i16); 5] = [
        (10, 1000, -500, 8000),
        (1, -32000, 32000, -100),
        (255, 0, 0, 0),
        (5, 12345, -6789, 4321),
        (100, -1, 1, -32768),
    ];
    let player_z: i16 = 0x1000;
    let mut bad = 0;
    for (frame, x, y, z) in cases {
        let mut map = vec![0u8; 11];
        map[0] = 0; // ctrlmapobj
        map[1..3].copy_from_slice(&frame.to_le_bytes());
        map[3..5].copy_from_slice(&(x as u16).to_le_bytes());
        map[5..7].copy_from_slice(&(y as u16).to_le_bytes());
        map[7..9].copy_from_slice(&(z as u16).to_le_bytes());
        let got = rom_spawn(&rom, addr, player_z, &map);
        let ez = player_z.wrapping_add(z);
        if (got.x, got.y, got.z, got.mapcnt) != (x, y, ez, frame) {
            bad += 1;
            eprintln!(
                "MAPOBJ x={x} y={y} z={z} f={frame}: ROM=({},{},{},mc={}) RUST=({x},{y},{ez},mc={frame})",
                got.x, got.y, got.z, got.mapcnt
            );
        }
    }
    assert_eq!(bad, 0, "MAPOBJ coordinate mismatches vs ROM");
}

#[test]
fn dobj_coords_match_rom() {
    let Some((syms, rom)) = setup() else {
        eprintln!("skip: no ROM/symbols");
        return;
    };
    let Some(&addr) = syms.get("MAPDOBJDO") else {
        eprintln!("skip: no MAPDOBJDO");
        return;
    };
    // DOBJ: ctrl, frame(2), x(2), y(2), z(2), strat(1). Full i16, no shape byte.
    let cases: [(u16, i16, i16, i16); 4] = [
        (10, 1000, -500, 8000),
        (1, -32000, 32000, -100),
        (5, 12345, -6789, 4321),
        (7, -1, 1, -20000),
    ];
    let player_z: i16 = 0x1000;
    let mut bad = 0;
    for (frame, x, y, z) in cases {
        let mut map = vec![0u8; 10];
        map[0] = 116; // ctrlmapdobj
        map[1..3].copy_from_slice(&frame.to_le_bytes());
        map[3..5].copy_from_slice(&(x as u16).to_le_bytes());
        map[5..7].copy_from_slice(&(y as u16).to_le_bytes());
        map[7..9].copy_from_slice(&(z as u16).to_le_bytes());
        map[9] = 0; // strat idx 0
        let got = rom_spawn(&rom, addr, player_z, &map);
        let ez = player_z.wrapping_add(z);
        if (got.x, got.y, got.z, got.mapcnt) != (x, y, ez, frame) {
            bad += 1;
            eprintln!(
                "DOBJ x={x} y={y} z={z} f={frame}: ROM=({},{},{},mc={}) RUST=({x},{y},{ez},mc={frame})",
                got.x, got.y, got.z, got.mapcnt
            );
        }
    }
    assert_eq!(bad, 0, "DOBJ coordinate mismatches vs ROM");
}

#[test]
fn mother_coords_and_shape_match_rom() {
    let Some((syms, rom)) = setup() else {
        eprintln!("skip: no ROM/symbols");
        return;
    };
    let Some(&addr) = syms.get("MAPMOTHER") else {
        eprintln!("skip: no MAPMOTHER");
        return;
    };
    // MOTHER: ctrl, frame(2), x(2), y(2), z(2), shape(2), strat(3), map(2).
    let cases: [(u16, i16, i16, i16, u16); 3] = [
        (10, 1000, -500, 8000, 100),
        (1, -32000, 32000, -100, 278),
        (5, 12345, -6789, 4321, 241),
    ];
    let player_z: i16 = 0x1000;
    let mut bad = 0;
    for (frame, x, y, z, shape) in cases {
        let mut map = vec![0u8; 16];
        map[0] = 10; // ctrlmapmother
        map[1..3].copy_from_slice(&frame.to_le_bytes());
        map[3..5].copy_from_slice(&(x as u16).to_le_bytes());
        map[5..7].copy_from_slice(&(y as u16).to_le_bytes());
        map[7..9].copy_from_slice(&(z as u16).to_le_bytes());
        map[9..11].copy_from_slice(&shape.to_le_bytes());
        // strat(3)+map(2) left 0
        let got = rom_spawn(&rom, addr, player_z, &map);
        let ez = player_z.wrapping_add(z);
        // ROM stores mm_shape raw (no shapes-table / resolve).
        if (got.x, got.y, got.z, got.mapcnt, got.shape) != (x, y, ez, frame, shape) {
            bad += 1;
            eprintln!(
                "MOTHER x={x} y={y} z={z} f={frame} sh={shape}: ROM=({},{},{},mc={},sh={}) want=({x},{y},{ez},mc={frame},sh={shape})",
                got.x, got.y, got.z, got.mapcnt, got.shape
            );
        }
    }
    assert_eq!(bad, 0, "MOTHER coordinate/shape mismatches vs ROM");
}
