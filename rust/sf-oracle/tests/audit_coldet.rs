//! Audit: object lifecycle / collision / distance routines vs the ROM.
//!
//! Differential tests added while auditing the Rust reimplementation against
//! the 65816 ROM (reference/ultrastarfox/SF). Each test runs the real ROM
//! function via the w65c816 oracle and compares it to the Rust port.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const XB: u32 = 0x0100; // obj1 base
const YB: u32 = 0x0300; // obj2 base (0x0200 is clobbered by the oracle stub)
const WX: u32 = 0x0C;
const WZ: u32 = 0x10;
const RANGEXZ: u32 = 0x12DB;

// ---------------------------------------------------------------------------
// xzdiffs_l  (STRATROU.ASM:1796) -> sf_strat::common::strat_dist_xz
//
// The ROM computes a *scaled Euclidean-ish magnitude*, not Manhattan:
//   x1=|dx|; y1=|dz|; x1>>=1; y1>>=1
//   rangexz=(x1+y1)<<1 ; m=max(x1,y1)
//   T=m+rangexz ; result = ((T>>1)+(T<<2))>>3   -> stored in `rangexz`
// ---------------------------------------------------------------------------

fn rom_xzdiffs(rom: &[u8], addr: u32, ax: i16, az: i16, bx: i16, bz: i16) -> i16 {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(XB + WX, ax as u16);
    bus.write16(XB + WZ, az as u16);
    bus.write16(YB + WX, bx as u16);
    bus.write16(YB + WZ, bz as u16);
    // a8i16 on entry (shorta/longi); routine does its own `a16`.
    call(
        &mut bus,
        addr,
        &Entry {
            x: XB as u16,
            y: YB as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    bus.read16(RANGEXZ) as i16
}

/// Faithful re-implementation of the ROM's xzdiffs_l fixed-point magnitude.
fn rust_faithful_xzdiffs(ax: i16, az: i16, bx: i16, bz: i16) -> i16 {
    let mut x1 = bx.wrapping_sub(ax);
    if x1 < 0 {
        x1 = x1.wrapping_neg();
    }
    let mut y1 = bz.wrapping_sub(az);
    if y1 < 0 {
        y1 = y1.wrapping_neg();
    }
    x1 >>= 1; // asra
    y1 >>= 1;
    let rangexz = (y1.wrapping_add(x1)).wrapping_shl(1);
    let m = if y1 < x1 { x1 } else { y1 };
    let t = m.wrapping_add(rangexz);
    let a = (t >> 1).wrapping_add(t.wrapping_shl(2)); // (T>>1)+(T<<2)
    ((a >> 1) >> 1) >> 1 // asra x3
}

/// Current Rust port: plain Manhattan distance (coldet common.rs:466).
fn rust_current_xzdiffs(ax: i16, az: i16, bx: i16, bz: i16) -> i16 {
    let dx = ((bx as i32 - ax as i32).abs()) as i16;
    let dz = ((bz as i32 - az as i32).abs()) as i16;
    dx.wrapping_add(dz)
}

#[test]
fn xzdiffs_vs_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("XZDIFFS_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let cases: [(i16, i16, i16, i16); 9] = [
        (0, 0, 100, 0),
        (0, 0, 0, 100),
        (0, 0, 100, 100),
        (0, 0, 300, 400),
        (1000, 2000, 1300, 2400),
        (0, 0, -300, -400),
        (500, -500, -500, 500),
        (0, 0, 20, 20),
        (0, 0, 1, 1),
    ];
    let (mut cur_diffs, mut fix_diffs) = (0, 0);
    for (ax, az, bx, bz) in cases {
        let o = rom_xzdiffs(&rom, addr, ax, az, bx, bz);
        let cur = rust_current_xzdiffs(ax, az, bx, bz);
        let fix = rust_faithful_xzdiffs(ax, az, bx, bz);
        if o != cur {
            cur_diffs += 1;
        }
        if o != fix {
            fix_diffs += 1;
        }
        eprintln!(
            "a=({ax},{az}) b=({bx},{bz}): ROM={o} current(Manhattan)={cur}{} faithful={fix}{}",
            if o == cur { "" } else { " <DIFF>" },
            if o == fix { "" } else { " <DIFF>" }
        );
    }
    eprintln!("current port: {cur_diffs} diffs vs ROM; faithful: {fix_diffs} diffs");
    assert_eq!(fix_diffs, 0, "faithful re-impl must match ROM");
    assert!(
        cur_diffs > 0,
        "current Manhattan port deviates from ROM (documents the bug)"
    );
}

// ---------------------------------------------------------------------------
// init_objvars_l (STRATROU.ASM:2311) -> sf_game::obj::strat_init_obj_vars.
// The ROM sets, on EVERY spawned object:
//   s_set_alflag  x,inviewpl   -> al_flags   |= $10 (AFINVIEWPL)
//   s_setremove_behind x       -> al_type    |= atzremove
//   s_set_alcollflag x,firstframe -> al_collflags |= $?? (ACF_FIRSTFRAME)
//   s_set_alsflag x,realobj    -> al_sflags3 |= $08 (ASF_REALOBJ)
// The Rust port sets collflags=FIRSTFRAME and type=ATZREMOVE, but leaves
// sflags3 (realobj) and the inviewpl bit CLEAR.
// ---------------------------------------------------------------------------

const AL_FLAGS: u32 = 0x08;
const AL_SFLAGS3: u32 = 0x1F;
const AL_COLLFLAGS: u32 = 0x2E;
const AFINVIEWPL: u8 = 0x10;
const ASF_REALOBJ: u8 = 0x08;

#[test]
fn init_objvars_sets_realobj() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("INIT_OBJVARS_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let mut bus = SnesBus::new(rom.to_vec());
    // Poison the flag bytes so we can see the ROM set them.
    bus.write8(XB + AL_FLAGS, 0);
    bus.write8(XB + AL_SFLAGS3, 0);
    bus.write8(XB + AL_COLLFLAGS, 0);
    // init_objvars_l does exg_XY; pass alien base in both X and Y so it is
    // unambiguous regardless of the swap.
    call(
        &mut bus,
        addr,
        &Entry {
            x: XB as u16,
            y: XB as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    let flags = bus.read8(XB + AL_FLAGS);
    let sflags3 = bus.read8(XB + AL_SFLAGS3);
    let collflags = bus.read8(XB + AL_COLLFLAGS);
    eprintln!(
        "ROM init_objvars: al_flags={flags:#04x} sflags3={sflags3:#04x} collflags={collflags:#04x}"
    );
    eprintln!(
        "  -> ROM sets realobj (sflags3 bit3): {}",
        sflags3 & ASF_REALOBJ != 0
    );
    eprintln!(
        "  -> ROM sets inviewpl (flags bit4):  {}",
        flags & AFINVIEWPL != 0
    );
    eprintln!("  Rust strat_init_obj_vars sets neither (sflags3 untouched, flags untouched).");
    assert!(
        sflags3 & ASF_REALOBJ != 0,
        "ROM init sets ASF_REALOBJ; Rust port omits it"
    );
    assert!(
        flags & AFINVIEWPL != 0,
        "ROM init sets AFINVIEWPL; Rust port omits it"
    );
    assert!(collflags != 0, "ROM sets firstframe collflag");
}

// ---------------------------------------------------------------------------
// kill_list_l (MAIN.ASM:1992, FmtFreeLst MACROS.INC:3750). The ROM's ONLY
// free-list format routine. It sets alfreelst = alblks (slot 0) and chains
// slot0->slot1->...->slot69->null. i.e. the free head is slot 0, FORWARD.
//
// Rust obj.rs: `Obj_Init` matches (head 0, forward), but `Obj_KillAll` pushes
// 0..69 front-to-back giving head=slot 69, REVERSED. After a kill_all the
// first Obj_Alloc therefore returns slot 69, not slot 0 -> breaks the
// "player == slot 0" invariant (Obj_GetPlayer checks slot 0).
// ---------------------------------------------------------------------------

const ALFREELST: u32 = 0x12AF;
const ALLST: u32 = 0x12AD;
const ALBLKS: u32 = 0x0338;
const AL_SIZE_ROM: u32 = 0x38;
const NUMBER_AL: u32 = 0x46;

#[test]
fn kill_list_free_order_is_forward() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("KILL_LIST_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let mut bus = SnesBus::new(rom.to_vec());
    call(
        &mut bus,
        addr,
        &Entry {
            p: 0x00,
            ..Default::default()
        },
    );
    let allst = bus.read16(ALLST);
    let mut node = bus.read16(ALFREELST) as u32;
    let mut order = Vec::new();
    for _ in 0..(NUMBER_AL + 2) {
        if node == 0 {
            break;
        }
        let slot = (node.wrapping_sub(ALBLKS)) / AL_SIZE_ROM;
        order.push(slot);
        node = bus.read16(node) as u32; // _next is offset 0
    }
    eprintln!("ROM kill_list_l: allst={allst:#06x} (empty active list)");
    eprintln!(
        "  free-list head slot = {}",
        order.first().copied().unwrap_or(9999)
    );
    eprintln!("  first 6 free slots = {:?}", &order[..order.len().min(6)]);
    eprintln!("  total chained = {}", order.len());
    eprintln!("  Rust Obj_KillAll produces head=slot 69 REVERSED (69,68,...) -> DIVERGES");
    assert_eq!(allst, 0, "active list must be empty after kill_list");
    assert_eq!(
        order.first().copied(),
        Some(0),
        "ROM free head is slot 0 (forward)"
    );
    assert_eq!(order.len() as u32, NUMBER_AL, "all 70 slots chained");
    // Forward order 0,1,2,...
    assert_eq!(
        &order[..4],
        &[0, 1, 2, 3],
        "ROM chains forward 0->1->2->3..."
    );
}
