//! Differential audit: map-VM NON-SPAWN opcodes, ROM handlers vs the Rust
//! port (`sf-game/src/game.rs` `map_exec`).
//!
//! Harness pattern (extends audit_mapvm.rs): host crafted map bytecode in
//! WRAM at $7E:8000, set the DP var `mapbank` = $7E, and enter the real ROM
//! dispatcher `newobjex` (WORLD.ASM:76-86) with X = mapptr. Because
//! $7E:0000-$1FFF is the same memory as the bank-0 low-RAM mirror, all the
//! executor's absolute stores (`sta mapcnt` etc.) land in the same WRAM
//! cells regardless of DBR. Each crafted script ends in an opcode that RTSes
//! (mapwait with nonzero dist, mapend, mapwait2), so `call_near`'s $0300
//! trap catches the exit and `mapptr`/`mapcnt` + touched WRAM tell us
//! exactly what the handler did.
//!
//! The Rust side runs the very same bytes through `Game::map_exec`.
//!
//! CONFIRMED DIVERGENCES (asserted below so they are machine-checked; fixing
//! game.rs will flip the corresponding `rust_*` assertion and flag the test
//! for update):
//!  - WAIT2 with a zero operand: ROM stores mapcnt=0 and STOPS the frame
//!    (WORLD.ASM:175-187 always RTS); Rust `continue`s into the next opcode.
//!  - SETBGM: ROM skips the music change while pshipflags2 has
//!    psf2_playerHP0 set (WORLD.ASM:196-198); Rust always plays.
//!  - SETVAROBJ with lastmapobj==0: ROM skips the write entirely
//!    (`ifobjinvalid`, WORLD.ASM:744-745); Rust writes 0.
//!  - REMOVE: ROM removes only the FIRST matching alien after the list head
//!    (WORLD.ASM:1977-1988 falls through to .out after removedeadal_l);
//!    Rust frees every active alien with the shape.
//!  - Opcode 136: ROM's `notneededyet` label falls through into `setbgmdo`
//!    (WORLD.ASM:191-194) => it IS a 2-byte setbgm; Rust treats it as a
//!    1-byte nop. (No level emits 136; documented, not load-bearing.)
//!  - FADETOSEA/FADETOGROUND: ROM starts a palette fade
//!    (palfade/palcnt/palnum, WORLD.ASM:371-394); Rust is a no-op (HD
//!    palette lane gap — levels DO emit these, builder.rs:469/473).

use sf_game::game::{Game, Hooks};
use sf_oracle::{call_near, load_built_rom, load_symbols, Entry, SnesBus};
use std::cell::RefCell;
use std::rc::Rc;

// Executor WRAM vars (sf-oracle/data/symbols.txt).
const MAPCNT: u32 = 0x1780;
const MAPPTR: u32 = 0x1782;
const MAPBANK: u32 = 0x1af8;
const LASTMAPOBJ: u32 = 0x177c;
const NUMMAPJSR: u32 = 0x17b7;
const NUMMAPLOOPS: u32 = 0x17d8;
const BGM_MUSIC: u32 = 0x1a4b;
const BGMCNT: u32 = 0x1a4a;
const PSHIPFLAGS2: u32 = 0x1562;
const DOZROT: u32 = 0x1776;
const STAGECNT: u32 = 0x163e;
const CURRENTBG: u32 = 0x17c6;
const BGFLAGS: u32 = 0x1a17;
const PALFADE: u32 = 0x19ef;
const PALNUM: u32 = 0x19f3;
const PALCNT: u32 = 0x19f5;
const SPECIALOBJTOTAL: u32 = 0x17c1;
const ALLST: u32 = 0x12ad;
const ALFREELST: u32 = 0x12af;

// Alien block field offsets (symbols.txt AL_*).
const AL_SHAPE: u32 = 0x04;
const AL_ROTX: u32 = 0x12; // roty +1, rotz +2
const AL_SFLAGS: u32 = 0x1d;
const AL_SFLAGS4: u32 = 0x20;
const AL_SWORD2: u32 = 0x28;

// Harness layout: alien blocks clear of the call stub ($0200) / trap ($0300).
const BLOCK: u32 = 0x0140;
const VAR: u16 = 0x1900; // scratch "external variable" (low WRAM)

fn setup() -> Option<(std::collections::HashMap<String, u32>, Vec<u8>)> {
    let syms = load_symbols();
    let rom = load_built_rom()?;
    Some((syms, rom))
}

/// Write `map` at $7E:8000, seed extra state, then run the ROM dispatcher
/// `newobjex` once from map offset `start` until a handler RTSes.
fn rom_exec(
    rom: &[u8],
    newobjex: u32,
    map: &[u8],
    start: u16,
    seed: impl FnOnce(&mut SnesBus),
) -> SnesBus {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(MAPBANK, 0x7E);
    for (i, b) in map.iter().enumerate() {
        bus.write8(0x7E_0000 | (0x8000 + i as u32), *b);
    }
    seed(&mut bus);
    call_near(
        &mut bus,
        newobjex,
        &Entry { x: start, p: 0x00, ..Default::default() },
    );
    bus
}

/// Resume the dispatcher on an existing bus (loop/jsr multi-step tests).
fn rom_resume(bus: &mut SnesBus, newobjex: u32, start: u16) {
    call_near(
        bus,
        newobjex,
        &Entry { x: start, p: 0x00, ..Default::default() },
    );
}

/// Run the same bytes through the Rust port once (one `map_exec` call).
fn rust_exec(map: &[u8], seed: impl FnOnce(&mut Game)) -> Game {
    let mut g = Game::new();
    g.world.map = map.to_vec();
    g.world.map_loaded = true;
    g.vars.mapptr = 0;
    seed(&mut g);
    g.map_exec();
    g
}

/// Music-recording hook set for the SETBGM tests.
struct RecHooks(Rc<RefCell<Vec<u8>>>);
impl Hooks for RecHooks {
    fn play_music(&mut self, t: u8) {
        self.0.borrow_mut().push(t);
    }
}

// ============================================================
// WAIT (18) / WAIT2 (138)
// ============================================================

#[test]
fn wait_scaling_and_zero_skip_match_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];

    // mapwait 0x1234 -> mapcnt=dist, mapptr=+3 (WORLD.ASM:2004-2013).
    let m1 = [18u8, 0x34, 0x12, 2];
    let bus = rom_exec(&rom, ne, &m1, 0, |_| {});
    let g = rust_exec(&m1, |_| {});
    assert_eq!(bus.read16(MAPCNT), 0x1234);
    assert_eq!(bus.read16(MAPPTR), 3);
    assert_eq!((g.vars.mapcnt, g.vars.mapptr), (0x1234, 3), "rust WAIT");

    // mapwait 0 falls through to the next opcode (WORLD.ASM:2010,2014).
    let m2 = [18u8, 0x00, 0x00, 18, 0x40, 0x00, 2];
    let bus = rom_exec(&rom, ne, &m2, 0, |_| {});
    let g = rust_exec(&m2, |_| {});
    assert_eq!(bus.read16(MAPCNT), 0x40);
    assert_eq!(bus.read16(MAPPTR), 6);
    assert_eq!((g.vars.mapcnt, g.vars.mapptr), (0x40, 6), "rust WAIT-0");
}

#[test]
fn wait2_scaling_matches_but_zero_diverges() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];

    // mapwait2 0x12 -> mapcnt = 0x12<<4, mapptr=+2 (WORLD.ASM:175-187).
    let m1 = [138u8, 0x12, 2];
    let bus = rom_exec(&rom, ne, &m1, 0, |_| {});
    let g = rust_exec(&m1, |_| {});
    assert_eq!(bus.read16(MAPCNT), 0x120);
    assert_eq!(bus.read16(MAPPTR), 2);
    assert_eq!((g.vars.mapcnt, g.vars.mapptr), (0x120, 2), "rust WAIT2");

    // DISCREPANCY: wait2 0 — ROM stores mapcnt=0 and RTSes anyway (no zero
    // check, unlike mapwait). Rust `continue`s into the following opcodes.
    let m2 = [138u8, 0x00, 24, 18, 0x40, 0x00, 2]; // wait2 0; gnddots; wait 0x40
    let bus = rom_exec(&rom, ne, &m2, 0, |_| {});
    assert_eq!(bus.read16(MAPPTR), 2, "ROM stops right after wait2 0");
    assert_eq!(bus.read16(MAPCNT), 0);
    let g = rust_exec(&m2, |_| {});
    eprintln!(
        "DISCREPANCY WAIT2-zero: ROM mapptr=2 mapcnt=0; Rust mapptr={} mapcnt={:#x}",
        g.vars.mapptr, g.vars.mapcnt
    );
    assert_eq!((g.vars.mapptr, g.vars.mapcnt), (6, 0x40), "rust runs on past wait2 0");
}

// ============================================================
// JMPVARLESS/MORE/EQ (124/126/128)
// ============================================================

#[test]
fn jmpvar_family_signed_compare_matches_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];

    // [op, ptr24, cmp, target16] @0 (7 bytes), END @7 (njmp), END @16 (jmp).
    let mut map = vec![0u8; 17];
    map[7] = 2;
    map[16] = 2;
    let cases: [(u8, u8); 10] = [
        (5, 5),
        (4, 5),
        (6, 5),
        (0x00, 0x90),
        (0x90, 0x00),
        (0x7f, 0x80),
        (0x80, 0x7f),
        (0x00, 0x00),
        (0xff, 0x01),
        (0x01, 0xff),
    ];
    let mut bad = 0;
    for op in [124u8, 126, 128] {
        for (var, cmp) in cases {
            map[0] = op;
            map[1] = (VAR & 0xFF) as u8;
            map[2] = (VAR >> 8) as u8;
            map[3] = 0x00; // bank 0 (low-WRAM mirror)
            map[4] = cmp;
            map[5] = 16;
            map[6] = 0;
            let bus = rom_exec(&rom, ne, &map, 0, |b| b.write8(VAR as u32, var));
            let g = rust_exec(&map, |g| g.vars.write_ext8(VAR, var));
            let romp = bus.read16(MAPPTR);
            if romp != g.vars.mapptr {
                bad += 1;
                eprintln!(
                    "JMPVAR op={op} var={var:#04x} cmp={cmp:#04x}: ROM mapptr={romp} RUST={}",
                    g.vars.mapptr
                );
            }
        }
    }
    assert_eq!(bad, 0, "jmpvar decision/advance mismatches");
}

// ============================================================
// SETVARB/W/L (92/94/96)
// ============================================================

#[test]
fn setvar_b_w_l_match_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];
    let vl = (VAR & 0xFF) as u8;
    let vh = (VAR >> 8) as u8;

    // setvarb: value(1) @1, ptr(3) @2 (WORLD.ASM:634-647).
    let m = [92u8, 0xAB, vl, vh, 0x00, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    let g = rust_exec(&m, |_| {});
    assert_eq!(bus.read8(VAR as u32), 0xAB);
    assert_eq!(bus.read16(MAPPTR), 5);
    assert_eq!((g.vars.read_ext8(VAR), g.vars.mapptr), (0xAB, 5), "rust setvarb");

    // setvarw: value(2) @1, ptr(3) @3 (WORLD.ASM:616-629).
    let m = [94u8, 0xCD, 0xAB, vl, vh, 0x00, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    let g = rust_exec(&m, |_| {});
    assert_eq!(bus.read16(VAR as u32), 0xABCD);
    assert_eq!(bus.read16(MAPPTR), 6);
    assert_eq!((g.vars.read_ext16(VAR), g.vars.mapptr), (0xABCD, 6), "rust setvarw");

    // setvarl: ptr(3) @1, lo16 @4, hi8 @6 (WORLD.ASM:590-612).
    let m = [96u8, vl, vh, 0x00, 0x34, 0x12, 0x56, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    let g = rust_exec(&m, |_| {});
    assert_eq!(bus.read16(VAR as u32), 0x1234);
    assert_eq!(bus.read8(VAR as u32 + 2), 0x56);
    assert_eq!(bus.read16(MAPPTR), 7);
    assert_eq!(
        (g.vars.read_ext16(VAR), g.vars.read_ext8(VAR + 2), g.vars.mapptr),
        (0x1234, 0x56, 7),
        "rust setvarl"
    );
}

// ============================================================
// SETALVARB/W/L (54/56/58) + invalid-object skip, SETALXVARB/W (60/62)
// ============================================================

#[test]
fn setalvar_family_matches_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];
    let seed_obj = |b: &mut SnesBus| b.write16(LASTMAPOBJ, BLOCK as u16);
    let rust_obj = |g: &mut Game| {
        let idx = g.objs.alloc().unwrap();
        g.world.last_obj = Some(idx);
        g.world.lastmapobj = idx + 1;
    };

    // setalvarb offset(2) @1, value(1) @3 -> byte at block+off, +4
    // (WORLD.ASM:848-862). Offset 0x12 = al_rotx.
    let m = [54u8, AL_ROTX as u8, 0x00, 0x5A, 2];
    let bus = rom_exec(&rom, ne, &m, 0, seed_obj);
    let g = rust_exec(&m, rust_obj);
    assert_eq!(bus.read8(BLOCK + AL_ROTX), 0x5A);
    assert_eq!(bus.read16(MAPPTR), 4);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!((g.objs.aliens[idx].rotx, g.vars.mapptr), (0x5A, 4), "rust setalvarb");

    // invalid object: write skipped, still advances (ifobjinvalid,
    // WORLD.ASM:849/39-47).
    let bus = rom_exec(&rom, ne, &m, 0, |b| {
        b.write16(LASTMAPOBJ, 0);
        b.write8(BLOCK + AL_ROTX, 0x77);
    });
    let g = rust_exec(&m, |_| {});
    assert_eq!(bus.read8(BLOCK + AL_ROTX), 0x77, "ROM skips write");
    assert_eq!(bus.read16(MAPPTR), 4);
    assert_eq!(g.vars.mapptr, 4, "rust also advances w/o object");

    // setalvarw offset @1, value16 @3, +5 (WORLD.ASM:866-879). 0x28=al_sword2.
    let m = [56u8, AL_SWORD2 as u8, 0x00, 0xEF, 0xBE, 2];
    let bus = rom_exec(&rom, ne, &m, 0, seed_obj);
    let g = rust_exec(&m, rust_obj);
    assert_eq!(bus.read16(BLOCK + AL_SWORD2), 0xBEEF);
    assert_eq!(bus.read16(MAPPTR), 5);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!(
        (g.objs.aliens[idx].sword2 as u16, g.vars.mapptr),
        (0xBEEF, 5),
        "rust setalvarw"
    );

    // setalvarl offset @1, lo16 @3 -> block+off, hi8 @5 -> block+off+2, +6
    // (WORLD.ASM:883-901). Offset 0x0C = al_worldx (hi byte -> worldy lo).
    let m = [58u8, 0x0C, 0x00, 0x34, 0x12, 0x56, 2];
    let bus = rom_exec(&rom, ne, &m, 0, seed_obj);
    let g = rust_exec(&m, rust_obj);
    assert_eq!(bus.read16(BLOCK + 0x0C), 0x1234);
    assert_eq!(bus.read8(BLOCK + 0x0E), 0x56);
    assert_eq!(bus.read16(MAPPTR), 6);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!(g.objs.aliens[idx].worldx as u16, 0x1234, "rust setalvarl lo");
    assert_eq!(g.objs.aliens[idx].worldy as u16 & 0xFF, 0x56, "rust setalvarl hi");
    assert_eq!(g.vars.mapptr, 6);

    // setalxvarb: byte -> xalblks+lastmapobj+off, +4 (WORLD.ASM:906-923).
    // alx offset 0 = alx_swpx1 lo.
    let m = [60u8, 0x00, 0x00, 0x99, 2];
    let bus = rom_exec(&rom, ne, &m, 0, seed_obj);
    let g = rust_exec(&m, rust_obj);
    assert_eq!(bus.read8(0x7E_2000 + BLOCK), 0x99);
    assert_eq!(bus.read16(MAPPTR), 4);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!(
        (g.objs.aliens[idx].swpx1 as u16 & 0xFF, g.vars.mapptr),
        (0x99, 4),
        "rust setalxvarb"
    );

    // setalxvarw: word, +5 (WORLD.ASM:927-944). alx offset 21 = depthoffset.
    let m = [62u8, 21, 0x00, 0x21, 0x43, 2];
    let bus = rom_exec(&rom, ne, &m, 0, seed_obj);
    let g = rust_exec(&m, rust_obj);
    assert_eq!(bus.read16(0x7E_2000 + BLOCK + 21), 0x4321);
    assert_eq!(bus.read16(MAPPTR), 5);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!(
        (g.objs.aliens[idx].depthoffset as u16, g.vars.mapptr),
        (0x4321, 5),
        "rust setalxvarw"
    );
}

// ============================================================
// SETALVARPB/PW (70/72), ADDALVARPB/PW (104/106)
// ============================================================

#[test]
fn alvar_pointer_ops_match_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];
    let vl = (VAR & 0xFF) as u8;
    let vh = (VAR >> 8) as u8;

    // setalvarptrb: offset(2) @1, ptr(3) @3; al[off] = ext8, +6
    // (WORLD.ASM:762-782). 0x14 = al_rotz.
    let m = [70u8, 0x14, 0x00, vl, vh, 0x00, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |b| {
        b.write16(LASTMAPOBJ, BLOCK as u16);
        b.write8(VAR as u32, 0x66);
    });
    let g = rust_exec(&m, |g| {
        let idx = g.objs.alloc().unwrap();
        g.world.last_obj = Some(idx);
        g.world.lastmapobj = idx + 1;
        g.vars.write_ext8(VAR, 0x66);
    });
    assert_eq!(bus.read8(BLOCK + 0x14), 0x66);
    assert_eq!(bus.read16(MAPPTR), 6);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!((g.objs.aliens[idx].rotz, g.vars.mapptr), (0x66, 6), "rust setalvarpb");

    // setalvarptrw (WORLD.ASM:786-807). 0x28 = al_sword2.
    let m = [72u8, AL_SWORD2 as u8, 0x00, vl, vh, 0x00, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |b| {
        b.write16(LASTMAPOBJ, BLOCK as u16);
        b.write16(VAR as u32, 0x7788);
    });
    let g = rust_exec(&m, |g| {
        let idx = g.objs.alloc().unwrap();
        g.world.last_obj = Some(idx);
        g.world.lastmapobj = idx + 1;
        g.vars.write_ext16(VAR, 0x7788);
    });
    assert_eq!(bus.read16(BLOCK + AL_SWORD2), 0x7788);
    assert_eq!(bus.read16(MAPPTR), 6);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!(
        (g.objs.aliens[idx].sword2 as u16, g.vars.mapptr),
        (0x7788, 6),
        "rust setalvarpw"
    );

    // addalvarptrb: al[off] += ext8, +6 (WORLD.ASM:412-434). 0x14 = al_rotz.
    let m = [104u8, 0x14, 0x00, vl, vh, 0x00, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |b| {
        b.write16(LASTMAPOBJ, BLOCK as u16);
        b.write8(BLOCK + 0x14, 10);
        b.write8(VAR as u32, 0xFB); // -5
    });
    let g = rust_exec(&m, |g| {
        let idx = g.objs.alloc().unwrap();
        g.world.last_obj = Some(idx);
        g.world.lastmapobj = idx + 1;
        g.objs.aliens[idx as usize].rotz = 10;
        g.vars.write_ext8(VAR, 0xFB);
    });
    assert_eq!(bus.read8(BLOCK + 0x14), 5);
    assert_eq!(bus.read16(MAPPTR), 6);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!((g.objs.aliens[idx].rotz, g.vars.mapptr), (5, 6), "rust addalvarpb");

    // addalvarptrw: 16-bit add (WORLD.ASM:438-460). 0x28 = al_sword2.
    let m = [106u8, AL_SWORD2 as u8, 0x00, vl, vh, 0x00, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |b| {
        b.write16(LASTMAPOBJ, BLOCK as u16);
        b.write16(BLOCK + AL_SWORD2, 0x0100);
        b.write16(VAR as u32, 0xFFFF); // -1
    });
    let g = rust_exec(&m, |g| {
        let idx = g.objs.alloc().unwrap();
        g.world.last_obj = Some(idx);
        g.world.lastmapobj = idx + 1;
        g.objs.aliens[idx as usize].sword2 = 0x0100;
        g.vars.write_ext16(VAR, 0xFFFF);
    });
    assert_eq!(bus.read16(BLOCK + AL_SWORD2), 0x00FF);
    assert_eq!(bus.read16(MAPPTR), 6);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!(
        (g.objs.aliens[idx].sword2 as u16, g.vars.mapptr),
        (0x00FF, 6),
        "rust addalvarpw"
    );
}

// ============================================================
// SETVAROBJ (74)
// ============================================================

#[test]
fn setvarobj_valid_matches_invalid_diverges() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];
    let vl = (VAR & 0xFF) as u8;
    let vh = (VAR >> 8) as u8;
    let m = [74u8, vl, vh, 0x00, 2];

    // Valid object: var = lastmapobj, +4 (WORLD.ASM:744-757).
    let bus = rom_exec(&rom, ne, &m, 0, |b| b.write16(LASTMAPOBJ, BLOCK as u16));
    assert_eq!(bus.read16(VAR as u32), BLOCK as u16);
    assert_eq!(bus.read16(MAPPTR), 4);
    let g = rust_exec(&m, |g| {
        let idx = g.objs.alloc().unwrap();
        g.world.last_obj = Some(idx);
        g.world.lastmapobj = idx + 1;
    });
    assert_eq!(g.vars.read_ext16(VAR), g.world.lastmapobj, "rust writes obj ref");
    assert_eq!(g.vars.mapptr, 4);

    // DISCREPANCY: invalid object — ROM's ifobjinvalid skips the write
    // (sentinel survives); Rust writes lastmapobj==0 over it.
    let bus = rom_exec(&rom, ne, &m, 0, |b| {
        b.write16(LASTMAPOBJ, 0);
        b.write16(VAR as u32, 0x1234);
    });
    assert_eq!(bus.read16(VAR as u32), 0x1234, "ROM keeps sentinel");
    assert_eq!(bus.read16(MAPPTR), 4);
    let g = rust_exec(&m, |g| g.vars.write_ext16(VAR, 0x1234));
    eprintln!(
        "DISCREPANCY SETVAROBJ-invalid: ROM keeps 0x1234, Rust wrote {:#06x}",
        g.vars.read_ext16(VAR)
    );
    assert_eq!(g.vars.read_ext16(VAR), 0, "rust clobbers with 0");
}

// ============================================================
// SETBGM (20) player-HP0 guard, and opcode 136 fall-through
// ============================================================

#[test]
fn setbgm_hp0_guard_diverges() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];
    let m = [20u8, 5, 2];

    // Player alive: bgm_music=5, bgmcnt=0, +2 (WORLD.ASM:194-206).
    let bus = rom_exec(&rom, ne, &m, 0, |b| {
        b.write8(BGM_MUSIC, 0x77);
        b.write8(BGMCNT, 0x55);
    });
    assert_eq!(bus.read8(BGM_MUSIC), 5);
    assert_eq!(bus.read8(BGMCNT), 0);
    assert_eq!(bus.read16(MAPPTR), 2);

    // Player HP0 (psf2_playerHP0 = $80): ROM skips the store entirely.
    let bus = rom_exec(&rom, ne, &m, 0, |b| {
        b.write8(PSHIPFLAGS2, 0x80);
        b.write8(BGM_MUSIC, 0x77);
        b.write8(BGMCNT, 0x55);
    });
    assert_eq!(bus.read8(BGM_MUSIC), 0x77, "ROM: no music change while dead");
    assert_eq!(bus.read8(BGMCNT), 0x55);
    assert_eq!(bus.read16(MAPPTR), 2);

    // DISCREPANCY: Rust plays the music regardless of pshipflags2.
    let played = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(RecHooks(played.clone())));
    g.world.map = m.to_vec();
    g.world.map_loaded = true;
    g.vars.mapptr = 0;
    g.vars.pshipflags2 = 0x80; // dead
    g.map_exec();
    eprintln!("DISCREPANCY SETBGM-HP0: ROM skips, Rust played {:?}", played.borrow());
    assert_eq!(*played.borrow(), vec![5u8], "rust ignores the HP0 guard");
}

#[test]
fn opcode136_is_setbgm_in_rom_nop_in_rust() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];
    // ROM: dispatch 136 -> notneededyet label, which falls through into
    // setbgmdo (WORLD.ASM:191-194): a 2-byte setbgm.
    let m = [136u8, 42, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    assert_eq!(bus.read8(BGM_MUSIC), 42, "ROM op136 == setbgm");
    assert_eq!(bus.read16(MAPPTR), 2);

    // Rust: 1-byte RESERVED nop, then byte 42 is decoded as an opcode (RTS).
    let played = Rc::new(RefCell::new(Vec::new()));
    let mut g = Game::with_hooks(Box::new(RecHooks(played.clone())));
    g.world.map = m.to_vec();
    g.world.map_loaded = true;
    g.vars.mapptr = 0;
    g.map_exec();
    eprintln!(
        "DISCREPANCY op136 (unused by levels): ROM sets bgm=42; Rust played {:?}",
        played.borrow()
    );
    assert!(played.borrow().is_empty());
}

// ============================================================
// LOOP (4)
// ============================================================

#[test]
fn maploop_iteration_count_matches_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];
    // wait@0 (loop body terminator), loop@3 -> target 0, count C, END@8.
    // Stored count C yields C jumps => the wait runs C+1 times, then END.
    // NB the ROM `maploop lbl,N` macro emits N-1 (MAPMACS.INC:264-268), so
    // ASM "loop N times" == stored N-1. sf-map's MapBuilder::maploop emits
    // the RAW count (builder.rs:211-215) => every Rust level loop runs one
    // extra iteration (see maploop_builder_encoding_off_by_one below).
    for c in [1u16, 2, 5] {
        let map = [
            18u8, 0x40, 0x00, // wait 0x40
            4, 0x00, 0x00, (c & 0xFF) as u8, (c >> 8) as u8, // loop -> 0
            2, // end
        ];
        // ROM: step until mapptr == 8 (the END opcode).
        let mut bus = rom_exec(&rom, ne, &map, 0, |_| {});
        let mut rom_waits = 1u32; // first call ran the wait at 0
        let mut guard = 0;
        while bus.read16(MAPPTR) != 8 && guard < 20 {
            let at = bus.read16(MAPPTR);
            rom_resume(&mut bus, ne, at);
            if bus.read16(MAPPTR) == 3 {
                rom_waits += 1;
            }
            guard += 1;
        }
        assert_eq!(bus.read16(MAPPTR), 8, "ROM reached END (C={c})");
        assert_eq!(bus.read16(NUMMAPLOOPS), 0, "ROM loop slot released");
        assert_eq!(rom_waits, c as u32 + 1, "ROM: stored C => C+1 body runs");

        // Rust: same stepping.
        let mut g = Game::new();
        g.world.map = map.to_vec();
        g.world.map_loaded = true;
        g.vars.mapptr = 0;
        let mut rust_waits = 0u32;
        let mut guard = 0;
        while g.vars.mapptr != 8 && guard < 20 {
            g.map_exec();
            if g.vars.mapptr == 3 {
                rust_waits += 1;
            }
            guard += 1;
        }
        assert_eq!(g.vars.mapptr, 8, "rust reached END (C={c})");
        assert_eq!(g.world.num_loops, 0, "rust loop slot released");
        assert_eq!(rust_waits, rom_waits, "handler parity for stored count {c}");
    }
}

#[test]
fn maploop_builder_encoding_off_by_one() {
    // ROM macro (MAPMACS.INC:264-268):
    //   maploop  macro label:  db ctrlloop / dw (\1)&$7fff / dw \2-1
    // i.e. `maploop .x,8` stores 7. The Rust builder stores 8:
    let mut b = sf_map::builder::MapBuilder::new();
    b.label("x");
    b.mapwait(0x40);
    b.maploop("x", 8);
    let (data, _labels) = b.finish();
    // data: wait(3 bytes) + [4, tgt16, count16]
    assert_eq!(data[3], 4, "loop opcode");
    let stored = data[6] as u16 | ((data[7] as u16) << 8);
    eprintln!(
        "DISCREPANCY MAPLOOP encoding: ROM macro would store 7 for `maploop x,8`; \
         MapBuilder stored {stored} (=> one extra loop iteration in every level)"
    );
    assert_eq!(stored, 8, "builder currently emits the raw count");
}

// ============================================================
// JSR/RTS (40/42), GOTO (46)
// ============================================================

#[test]
fn jsr_rts_goto_match_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];

    // jsr@0 -> 8; wait@4 (the return point 0+4); rts@8.
    // Bank byte must stay $7E on the ROM side (mapjsrdo re-loads mapbank).
    let m = [40u8, 0x08, 0x00, 0x7E, 18, 0x40, 0x00, 0, 42];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    let g = rust_exec(&m, |_| {});
    assert_eq!(bus.read16(MAPPTR), 7, "ROM: rts returns to jsr+4, wait stops at 7");
    assert_eq!(bus.read16(MAPCNT), 0x40);
    assert_eq!(bus.read16(NUMMAPJSR), 0);
    assert_eq!((g.vars.mapptr, g.vars.mapcnt), (7, 0x40), "rust jsr/rts");
    assert_eq!(g.world.num_jsr, 0);
    assert_eq!(g.world.jsr_top, 0);

    // goto@0 -> 5; wait@5.
    let m = [46u8, 0x05, 0x00, 0x7E, 0, 18, 0x40, 0x00, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    let g = rust_exec(&m, |_| {});
    assert_eq!(bus.read16(MAPPTR), 8);
    assert_eq!(bus.read16(MAPCNT), 0x40);
    assert_eq!((g.vars.mapptr, g.vars.mapcnt), (8, 0x40), "rust goto");
}

// ============================================================
// REMOVE (12)
// ============================================================

#[test]
fn remove_takes_first_match_only_in_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];
    const HEAD: u32 = 0x0100;
    const A: u32 = 0x0140;
    const B: u32 = 0x0180;

    // mapremove: ctrl, count(2, unused), shape(2) -> +5 (WORLD.ASM:1973-1993,
    // MAPSTRUC.INC mr_shape=3/mr_sizeof=5).
    let m = [12u8, 0x00, 0x00, 0x07, 0x00, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |b| {
        // Doubly-linked active list head -> A -> B, both shape 7.
        b.write16(ALLST, HEAD as u16);
        b.write16(ALFREELST, 0);
        b.write16(HEAD, A as u16); // _next
        b.write16(HEAD + 2, 0); // _prev
        b.write16(HEAD + AL_SHAPE, 0x9999);
        b.write16(A, B as u16);
        b.write16(A + 2, HEAD as u16);
        b.write16(A + AL_SHAPE, 7);
        b.write16(B, 0);
        b.write16(B + 2, A as u16);
        b.write16(B + AL_SHAPE, 7);
    });
    assert_eq!(bus.read16(MAPPTR), 5);
    // ROM removed exactly ONE object (A): head now links straight to B,
    // and B is still live in the list.
    assert_eq!(bus.read16(HEAD), B as u16, "ROM unlinked only the first match");
    assert_eq!(bus.read16(ALLST), HEAD as u16, "list head (player) never touched");

    // DISCREPANCY: Rust frees every active alien with the shape.
    let g = rust_exec(&m, |g| {
        let a = g.objs.alloc().unwrap();
        let b2 = g.objs.alloc().unwrap();
        g.objs.aliens[a as usize].shape = 7;
        g.objs.aliens[b2 as usize].shape = 7;
    });
    let live = g
        .objs
        .active_indices()
        .iter()
        .filter(|&&i| g.objs.aliens[i as usize].shape == 7)
        .count();
    eprintln!("DISCREPANCY REMOVE: ROM leaves 1 of 2 shape-7 aliens, Rust leaves {live}");
    assert_eq!(live, 0, "rust removes all matches");
    assert_eq!(g.vars.mapptr, 5);
}

// ============================================================
// Small state ops: rot(48/50/52), zrot on/off(88/86), setstage(14),
// setbg(16), special(90)/cspecial(132)
// ============================================================

#[test]
fn small_state_ops_match_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];
    let seed_obj = |b: &mut SnesBus| b.write16(LASTMAPOBJ, BLOCK as u16);
    let rust_obj = |g: &mut Game| {
        let idx = g.objs.alloc().unwrap();
        g.world.last_obj = Some(idx);
        g.world.lastmapobj = idx + 1;
    };

    // setxrot/setyrot/setzrot: byte @1 -> al_rotx/y/z, +2 (WORLD.ASM:986-1021).
    for (op, off) in [(48u8, AL_ROTX), (50, AL_ROTX + 1), (52, AL_ROTX + 2)] {
        let m = [op, 0xA5, 2];
        let bus = rom_exec(&rom, ne, &m, 0, seed_obj);
        assert_eq!(bus.read8(BLOCK + off), 0xA5, "ROM rot op {op}");
        assert_eq!(bus.read16(MAPPTR), 2);
        let g = rust_exec(&m, rust_obj);
        let idx = g.world.last_obj.unwrap() as usize;
        let got = match op {
            48 => g.objs.aliens[idx].rotx,
            50 => g.objs.aliens[idx].roty,
            _ => g.objs.aliens[idx].rotz,
        };
        assert_eq!((got, g.vars.mapptr), (0xA5, 2), "rust rot op {op}");
    }

    // setzroton (88) / setzrotoff (86) -> dozrot $1776 (WORLD.ASM:681-696).
    let m = [88u8, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    let g = rust_exec(&m, |_| {});
    assert_eq!(bus.read8(DOZROT), 1);
    assert_eq!(g.vars.read_ext8(DOZROT as u16), 1, "rust zroton");
    let m = [86u8, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |b| b.write8(DOZROT, 1));
    let g = rust_exec(&m, |g| g.vars.write_ext8(DOZROT as u16, 1));
    assert_eq!(bus.read8(DOZROT), 0);
    assert_eq!(g.vars.read_ext8(DOZROT as u16), 0, "rust zrotoff");

    // setstage (14): stagecnt=50, +1 (WORLD.ASM:1334-1339).
    let m = [14u8, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    let g = rust_exec(&m, |_| {});
    assert_eq!(bus.read16(STAGECNT), 50);
    assert_eq!(bus.read16(MAPPTR), 1);
    assert_eq!((g.vars.stagecnt, g.vars.mapptr), (50, 1), "rust setstage");

    // setbg (16): currentbg=word, bgflags|=4 (bgf_bg), +3
    // (WORLD.ASM:1273-1293).
    let m = [16u8, 0x34, 0x02, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    let g = rust_exec(&m, |_| {});
    assert_eq!(bus.read16(CURRENTBG), 0x0234);
    assert_eq!(bus.read8(BGFLAGS) & 0x04, 0x04);
    assert_eq!(bus.read16(MAPPTR), 3);
    assert_eq!((g.vars.currentbg, g.vars.mapptr), (0x0234, 3), "rust setbg");
    assert_eq!(g.vars.bgflags & 0x04, 0x04);

    // mapspecial (90): ROM stores asf_special(1) INTO al_sflags ($1D) and
    // increments specialobjtotal (WORLD.ASM:654-663). The Rust port keeps
    // the marker in its own sflags4 bit (ASF4_SPECIAL=0x40, port-wide
    // relocation; consistent because every Rust reader checks sflags4).
    let m = [90u8, 2];
    let bus = rom_exec(&rom, ne, &m, 0, seed_obj);
    assert_eq!(bus.read8(BLOCK + AL_SFLAGS), 0x01, "ROM sflags overwrite");
    assert_eq!(bus.read8(SPECIALOBJTOTAL), 1);
    assert_eq!(bus.read16(MAPPTR), 1);
    let g = rust_exec(&m, rust_obj);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!(g.objs.aliens[idx].sflags4 & 0x40, 0x40, "rust ASF4_SPECIAL");
    assert_eq!(g.world.specialobjtotal, 1);
    assert_eq!(g.vars.mapptr, 1);

    // mapCspecial (132): asf_Cspecial = bit31 -> $80 in al_sflags4 ($20)
    // (WORLD.ASM:668-677, STRATEQU.INC make_sflag Cspecial).
    let m = [132u8, 2];
    let bus = rom_exec(&rom, ne, &m, 0, seed_obj);
    assert_eq!(bus.read8(BLOCK + AL_SFLAGS4), 0x80, "ROM sflags4 Cspecial");
    assert_eq!(bus.read8(SPECIALOBJTOTAL), 1);
    let g = rust_exec(&m, rust_obj);
    let idx = g.world.last_obj.unwrap() as usize;
    assert_eq!(g.objs.aliens[idx].sflags4 & 0x80, 0x80, "rust ASF4_CSPECIAL");
    assert_eq!(g.world.specialobjtotal, 1);
}

// ============================================================
// FADETOSEA/FADETOGROUND (108/110) — ROM palette fade, Rust no-op
// ============================================================

#[test]
fn fadetosea_ground_write_palette_fade_in_rom_only() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];

    // fadetosea: palfade=lastpalfade=30, palcnt=2, palnum=30, +1
    // (WORLD.ASM:371-380).
    let m = [108u8, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    assert_eq!(bus.read16(PALFADE), 30);
    assert_eq!(bus.read16(PALCNT), 2);
    assert_eq!(bus.read16(PALNUM) & 0xFF, 30);
    assert_eq!(bus.read16(MAPPTR), 1);

    // fadetoground: palfade = groundpal-seapal+30 = 0x20+30 = 62
    // (WORLD.ASM:384-394, symbols SEAPAL $2f362 / GROUNDPAL $2f382).
    let m = [110u8, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |_| {});
    assert_eq!(bus.read16(PALFADE), 62);
    assert_eq!(bus.read16(MAPPTR), 1);

    // Rust: documented no-op (advance only). Levels DO emit these
    // (sf-map builder.rs:469/473) — HD palette-fade gap, colors lane.
    let g = rust_exec(&m, |_| {});
    assert_eq!(g.vars.mapptr, 1, "rust advances but performs no palette fade");
    eprintln!("DISCREPANCY FADETOSEA/GROUND: ROM starts palette fade; Rust no-op");
}

// ============================================================
// IF (44) carry semantics + CODEJSL (122) advance
// ============================================================

#[test]
fn mapif_carry_semantics_match_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];

    // Callback stubs in low WRAM (executable through bank 0):
    //   $0500: SEC / RTL      $0504: CLC / RTL
    // mapifdo pushes func-? no: stored word is pushed DEC'd, so store the
    // real entry address (WORLD.ASM:1048-1053).
    let sec_stub: u16 = 0x0500;
    let clc_stub: u16 = 0x0504;
    let seed_stubs = |b: &mut SnesBus| {
        b.write8(0x0500, 0x38); // SEC
        b.write8(0x0501, 0x6B); // RTL
        b.write8(0x0504, 0x18); // CLC
        b.write8(0x0505, 0x6B); // RTL
    };

    // Carry SET -> jump to else target @+4 (WORLD.ASM:1069,1090-1100).
    let m = [
        44u8,
        (sec_stub & 0xFF) as u8,
        (sec_stub >> 8) as u8,
        0x00, // bank 0
        0x10,
        0x00, // else -> 16
        2, // @6 END (fallthrough)
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        2, // @16 END (else)
    ];
    let bus = rom_exec(&rom, ne, &m, 0, seed_stubs);
    assert_eq!(bus.read16(MAPPTR), 16, "ROM: carry set => else branch");

    // Carry CLEAR -> advance +6, mapcnt=1, stop (WORLD.ASM:1076-1088).
    let mut m2 = m;
    m2[1] = (clc_stub & 0xFF) as u8;
    m2[2] = (clc_stub >> 8) as u8;
    let bus = rom_exec(&rom, ne, &m2, 0, seed_stubs);
    assert_eq!(bus.read16(MAPPTR), 6, "ROM: carry clear => continue");
    assert_eq!(bus.read16(MAPCNT), 1);

    // Rust: unknown callback defaults to carry=true => else branch. Parity
    // with the ROM's carry-set path.
    let g = rust_exec(&m, |_| {});
    assert_eq!(g.vars.mapptr, 16, "rust unknown-callback == ROM carry-set");

    // CODEJSL (122): stored word is func-1, callee at bank:(word+1); RTL
    // stub => pure advance +4 (WORLD.ASM:250-279).
    let m3 = [
        122u8,
        ((sec_stub - 1) & 0xFF) as u8,
        ((sec_stub - 1) >> 8) as u8,
        0x00,
        18,
        0x40,
        0x00,
        2,
    ];
    let bus = rom_exec(&rom, ne, &m3, 0, seed_stubs);
    assert_eq!(bus.read16(MAPPTR), 7, "ROM codejsl advances 4 then waits");
    let g = rust_exec(&m3, |_| {});
    assert_eq!(g.vars.mapptr, 7, "rust codejsl (no callback) advances 4");
}

// ============================================================
// SETPATH (140) — advance parity; value goes through the path hook
// ============================================================

#[test]
fn setpath_advance_matches_rom() {
    let Some((syms, rom)) = setup() else { return };
    let ne = syms["NEWOBJEX"];
    // ROM: al_sword2 = raw word @+1, +3 (WORLD.ASM:162-170). The Rust port
    // stores Paths_ResolveStart(path_id) instead of a ROM data pointer —
    // representation-level abstraction, advance must match.
    let m = [140u8, 0x34, 0x12, 2];
    let bus = rom_exec(&rom, ne, &m, 0, |b| b.write16(LASTMAPOBJ, BLOCK as u16));
    assert_eq!(bus.read16(BLOCK + AL_SWORD2), 0x1234, "ROM raw path word");
    assert_eq!(bus.read16(MAPPTR), 3);
    let g = rust_exec(&m, |g| {
        let idx = g.objs.alloc().unwrap();
        g.world.last_obj = Some(idx);
        g.world.lastmapobj = idx + 1;
    });
    assert_eq!(g.vars.mapptr, 3, "rust setpath advance");
}
