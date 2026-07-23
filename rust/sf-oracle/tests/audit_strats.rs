//! Enemy-strategy accuracy audit — houdai / zaco3 fire gates vs ROM `xzdiffs_l`.
//!
//! ROM `xzdiffs_l` ($1FD0C3) computes `rangexz` ($12DB): an octagonal
//! approximation of the Euclidean distance in the **XZ plane only**. It backs
//! `s_jmp_distless` / `s_jmp_distmore` (STRATMAC.INC:3295).
//!
//! Both gates now use [`sf_strat::common::strat_dist_xz`] (alias `dist_xz`):
//!   * `houdai_strat` — GASTRATS.ASM:1308 `#800`
//!   * `zaco3_attack` — KSTRATS.ASM:115 `#1300`
//!
//! This test proves the port metric matches ROM `rangexz` bit-exactly, and
//! that the old Z-only / Manhattan-3D mistakes still diverge (regression guard).

use sf_game::alien::Alien;
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::common::strat_dist_xz;

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
    // 16-bit index registers so X/Y can address 0x0100 / 0x0500.
    call(
        &mut bus,
        addr,
        &Entry {
            x: OBJ1 as u16,
            y: OBJ2 as u16,
            p: 0x00,
            ..Default::default()
        },
    );
    bus.read16(RANGEXZ)
}

fn alien_at(pos: [i16; 3]) -> Alien {
    let mut a = Alien::default();
    a.worldx = pos[0];
    a.worldy = pos[1];
    a.worldz = pos[2];
    a
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
fn houdai_zaco3_gates_match_rom_rangexz() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("XZDIFFS_L"), load_built_rom()) else {
        eprintln!("skip: no XZDIFFS_L symbol / built ROM");
        return;
    };

    let cases: [([i16; 3], [i16; 3]); 6] = [
        ([0, 0, 0], [0, 0, 800]),
        ([0, 0, 0], [700, 0, 400]),
        ([0, 0, 0], [1200, 0, 0]),
        ([0, 0, 0], [200, 1500, 900]),
        ([500, 100, 500], [-400, 900, 1600]),
        ([0, 0, 0], [900, 0, 900]),
    ];

    let mut port_mismatch = 0;
    let mut z_mismatch = 0;
    let mut man_mismatch = 0;
    for (o1, o2) in cases {
        let rom_xz = rom_rangexz(&rom, addr, o1, o2) as i32;
        let port = strat_dist_xz(&alien_at(o1), &alien_at(o2)) as i32;
        let z = z_only(o1, o2);
        let man = manhattan3d(o1, o2);
        println!(
            "self={:?} tgt={:?} -> ROM={:>5}  port={:>5} (Δ{:>+5})  z_only={:>5}  man3d={:>5}",
            o1,
            o2,
            rom_xz,
            port,
            port - rom_xz,
            z,
            man
        );
        if port != rom_xz {
            port_mismatch += 1;
        }
        if z != rom_xz {
            z_mismatch += 1;
        }
        if man != rom_xz {
            man_mismatch += 1;
        }
    }

    assert_eq!(
        port_mismatch, 0,
        "houdai/zaco3 dist_xz must match ROM xzdiffs_l / rangexz"
    );
    // Regression guard: the old wrong metrics still diverge on this grid.
    assert!(
        z_mismatch >= 3,
        "Z-only must still diverge (got {z_mismatch})"
    );
    assert!(
        man_mismatch >= 4,
        "Manhattan-3D must still diverge (got {man_mismatch})"
    );
}
