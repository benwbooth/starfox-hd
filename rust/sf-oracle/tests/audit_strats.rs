//! Enemy-strategy accuracy audit — differential evidence for the distance
//! metric that several ground/fighter fire-gates use.
//!
//! ROM `xzdiffs_l` ($1FD0C3) computes `rangexz` ($12DB): an octagonal
//! approximation of the Euclidean distance in the **XZ plane only** between
//! two objects (al_worldx $0C, al_worldz $10). It is the exact function behind
//! the `s_jmp_distless` / `s_jmp_distmore` strat macros (STRATMAC.INC:3295 ->
//! `jsl xzdiffs_l; lda rangexz`).
//!
//! Two ported strategies gate their fire on the WRONG metric:
//!   * `houdai_strat` (enemy_a.rs:3639) — uses Z-only `|dz|` for the 800-unit
//!     player fire gate, but GASTRATS.ASM:1308 `s_jmp_distless x,y,#800` is XZ.
//!   * `zaco3_attack` (enemy_a.rs:3728) — uses Manhattan `|dx|+|dy|+|dz|` for
//!     the 1300-unit target fire gate, but KSTRATS.ASM:115
//!     `s_jmp_distless y,x,#1300` is XZ.
//!
//! This test calls the ROM to obtain the ground-truth `rangexz` for a spread
//! of offsets and shows it diverges from both the Z-only and Manhattan-3D
//! values the port currently computes — so the gates trip at different ranges.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const OBJ1: u32 = 0x0100;
// 0x0200 (bootstrap stub) / 0x0300 (RTS trap) are reserved by the oracle
// harness, so place the second object well clear of them.
const OBJ2: u32 = 0x0500;
const AL_WORLDX: u32 = 0x0C;
const AL_WORLDY: u32 = 0x0E;
const AL_WORLDZ: u32 = 0x10;
const RANGEXZ: u32 = 0x12DB;

/// Ground truth: ROM XZ distance between two objects.
fn rom_rangexz(rom: &[u8], addr: u32, o1: [i16; 3], o2: [i16; 3]) -> u16 {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(OBJ1 + AL_WORLDX, o1[0] as u16);
    bus.write16(OBJ1 + AL_WORLDY, o1[1] as u16);
    bus.write16(OBJ1 + AL_WORLDZ, o1[2] as u16);
    bus.write16(OBJ2 + AL_WORLDX, o2[0] as u16);
    bus.write16(OBJ2 + AL_WORLDY, o2[1] as u16);
    bus.write16(OBJ2 + AL_WORLDZ, o2[2] as u16);
    // 16-bit index registers so X/Y can address 0x0100 / 0x0200.
    call(
        &mut bus,
        addr,
        &Entry { x: OBJ1 as u16, y: OBJ2 as u16, p: 0x00, ..Default::default() },
    );
    bus.read16(RANGEXZ)
}

fn z_only(o1: [i16; 3], o2: [i16; 3]) -> i32 {
    (o1[2] as i32 - o2[2] as i32).abs()
}

fn manhattan3d(o1: [i16; 3], o2: [i16; 3]) -> i32 {
    (o1[0] as i32 - o2[0] as i32).abs()
        + (o1[1] as i32 - o2[1] as i32).abs()
        + (o1[2] as i32 - o2[2] as i32).abs()
}

#[test]
fn houdai_and_zaco3_gates_use_wrong_distance_metric() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("XZDIFFS_L"), load_built_rom()) else {
        eprintln!("skip: no XZDIFFS_L symbol / built ROM");
        return;
    };

    // (self, target) world coords [x, y, z].
    let cases: [([i16; 3], [i16; 3]); 6] = [
        // Player dead ahead (pure +Z): all three metrics agree here.
        ([0, 0, 0], [0, 0, 800]),
        // Player off to the side: Z-only under-reads the true XZ range.
        ([0, 0, 0], [700, 0, 400]),
        ([0, 0, 0], [1200, 0, 0]),
        // Big Y separation: Manhattan-3D over-reads vs the XZ-plane truth.
        ([0, 0, 0], [200, 1500, 900]),
        // zaco3 vs a houdai target at an angle.
        ([500, 100, 500], [-400, 900, 1600]),
        ([0, 0, 0], [900, 0, 900]),
    ];

    let mut z_mismatch = 0;
    let mut man_mismatch = 0;
    for (o1, o2) in cases {
        let rom_xz = rom_rangexz(&rom, addr, o1, o2) as i32;
        let z = z_only(o1, o2);
        let man = manhattan3d(o1, o2);
        println!(
            "self={:?} tgt={:?} -> ROM rangexz={:>5}  z_only={:>5} (Δ{:>+5})  manhattan3d={:>5} (Δ{:>+5})",
            o1, o2, rom_xz, z, z - rom_xz, man, man - rom_xz
        );
        if z != rom_xz {
            z_mismatch += 1;
        }
        if man != rom_xz {
            man_mismatch += 1;
        }
    }

    // The current port's two metrics both diverge from the ROM XZ truth on
    // multiple cases -> the 800/1300 fire gates trip at the wrong ranges.
    assert!(
        z_mismatch >= 3,
        "expected houdai's Z-only metric to diverge from ROM rangexz on \
         several cases (got {z_mismatch})"
    );
    assert!(
        man_mismatch >= 4,
        "expected zaco3's Manhattan-3D metric to diverge from ROM rangexz on \
         several cases (got {man_mismatch})"
    );
}
