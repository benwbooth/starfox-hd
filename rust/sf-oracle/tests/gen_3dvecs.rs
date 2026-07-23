//! Differential test: the real ROM `n3dvecs_l` (STRATROU.ASM) vs the Rust
//! `strat_gen_vecs_3d`. This is the steering/3D-vector math I repeatedly
//! mis-fixed by hand — now checked against the actual ROM.
//!
//! ABI (from the disassembly): inputs `troty`=$1631, `trotx`=$1630 (angle
//! bytes), `tmpz`=$78 (magnitude byte); outputs `x1`=$02, `y1`=$08, `z1`=$8A
//! (16-bit signed). Enters with 8-bit A (M=1), 16-bit index.

use sf_game::alien::Alien;
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const TROTX: u32 = 0x1630;
const TROTY: u32 = 0x1631;
const TMPZ: u32 = 0x0078;
const X1: u32 = 0x0002;
const Y1: u32 = 0x0008;
const Z1: u32 = 0x008A;

fn rom_vecs(rom: &[u8], addr: u32, roty: u8, rotx: u8, vel: u8) -> (i16, i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(TROTX, rotx);
    bus.write8(TROTY, roty);
    bus.write8(TMPZ, vel);
    call(
        &mut bus,
        addr,
        &Entry {
            p: 0x20,
            ..Default::default()
        },
    );
    (
        bus.read16(X1) as i16,
        bus.read16(Y1) as i16,
        bus.read16(Z1) as i16,
    )
}

fn rust_vecs(roty: u8, rotx: u8, vel: u8) -> (i16, i16, i16) {
    let mut al = Alien::default();
    al.roty = roty;
    al.rotx = rotx;
    al.vel = vel;
    sf_strat::common::strat_gen_vecs_3d(&mut al);
    (al.vx, al.vy, al.vz)
}

#[test]
fn gen_3dvecs_matches_rom() {
    let syms = load_symbols();
    let Some(&addr) = syms.get("N3DVECS_L") else {
        eprintln!("skip: symbols.txt not present");
        return;
    };
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: built ROM data/sf.sfc not present");
        return;
    };

    // Spread of yaw/pitch/speed, incl. the LEFT/RIGHT-relevant yaws.
    let cases = [
        (0u8, 0u8, 100u8),
        (64, 0, 100),
        (192, 0, 100),
        (32, 16, 80),
        (96, 32, 64),
        (128, 0, 100),
        (10, 5, 120),
        (250, 8, 90),
    ];
    let mut bad = 0;
    for &(roty, rotx, vel) in &cases {
        let (x1, y1, z1) = rom_vecs(&rom, addr, roty, rotx, vel);
        let (vx, vy, vz) = rust_vecs(roty, rotx, vel);
        let exact = (vx, vy, vz) == (x1, y1, z1);
        if !exact {
            bad += 1;
        }
        eprintln!(
            "roty={roty:3} rotx={rotx:3} vel={vel:3}  ROM={:?}  RUST={:?}  {}",
            (x1, y1, z1),
            (vx, vy, vz),
            if exact { "EXACT" } else { "DIFF" },
        );
    }
    assert_eq!(bad, 0, "{bad}/{} cases differ from the ROM", cases.len());
}
