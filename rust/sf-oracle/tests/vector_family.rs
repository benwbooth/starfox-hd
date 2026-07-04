//! Differential tests for the "alien-based" velocity-vector family against the
//! real ROM. These take X = alien slot base and read `al_roty`=$13 / `al_rotx`
//! =$12 / `al_vel`=$15; outputs x1=$02, y1=$08, z1=$8A (16-bit). Confirms the
//! ROM-exact fixed-point sweep (sidevecs/frontvecs/2d) is bit-exact and that
//! their yaw conventions (no negation, unlike n3dvecs) are right.

use sf_game::alien::Alien;
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const XBASE: u32 = 0x0100; // test alien slot (clear of DP scratch $02..$8A)
const AL_ROTX: u32 = 0x12;
const AL_ROTY: u32 = 0x13;
const AL_VEL: u32 = 0x15;
const X1: u32 = 0x02;
const Y1: u32 = 0x08;
const Z1: u32 = 0x8A;

/// Run an alien-based vector routine. `a_in` is the entry A register (speed for
/// front/side; ignored by alvelvecs which reads al_vel).
fn rom_vec(rom: &[u8], addr: u32, roty: u8, rotx: u8, vel: u8, a_in: u16) -> (i16, i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(XBASE + AL_ROTY, roty);
    bus.write8(XBASE + AL_ROTX, rotx);
    bus.write8(XBASE + AL_VEL, vel);
    let entry = Entry { a: a_in, x: XBASE as u16, p: 0x20, ..Default::default() };
    call(&mut bus, addr, &entry);
    (
        bus.read16(X1) as i16,
        bus.read16(Y1) as i16,
        bus.read16(Z1) as i16,
    )
}

fn alien(roty: u8, rotx: u8, vel: u8) -> Alien {
    let mut a = Alien::default();
    a.roty = roty;
    a.rotx = rotx;
    a.vel = vel;
    a
}

const CASES: [(u8, u8, u8); 7] = [
    (0, 0, 100),
    (64, 0, 100),
    (192, 0, 90),
    (32, 16, 80),
    (96, 0, 64),
    (10, 0, 120),
    (250, 0, 90),
];

#[test]
fn frontvecs_matches_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("FRONTVECS_L"), load_built_rom()) else {
        eprintln!("skip: no symbols/ROM");
        return;
    };
    let mut bad = 0;
    for &(roty, rotx, vel) in &CASES {
        // frontvecs_l takes speed in A.
        let (x1, _y1, z1) = rom_vec(&rom, addr, roty, rotx, vel, vel as u16);
        let (rx, rz) = sf_strat::common::strat_gen_front_vecs(&alien(roty, rotx, vel));
        let ok = rx == x1 && rz == z1;
        if !ok {
            bad += 1;
        }
        eprintln!("front roty={roty:3} vel={vel:3}  ROM=({x1},{z1}) RUST=({rx},{rz}) {}", if ok { "EXACT" } else { "DIFF" });
    }
    assert_eq!(bad, 0, "{bad} frontvecs cases differ");
}

#[test]
fn sidevecs_matches_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("SIDEVECS_L"), load_built_rom()) else {
        eprintln!("skip: no symbols/ROM");
        return;
    };
    let mut bad = 0;
    for &(roty, rotx, vel) in &CASES {
        let (x1, _y1, z1) = rom_vec(&rom, addr, roty, rotx, vel, vel as u16);
        let (rx, rz) = sf_strat::common::strat_gen_side_vecs(&alien(roty, rotx, vel));
        let ok = rx == x1 && rz == z1;
        if !ok {
            bad += 1;
        }
        eprintln!("side  roty={roty:3} vel={vel:3}  ROM=({x1},{z1}) RUST=({rx},{rz}) {}", if ok { "EXACT" } else { "DIFF" });
    }
    assert_eq!(bad, 0, "{bad} sidevecs cases differ");
}

#[test]
fn genvecs2d_matches_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("ALVELVECS_L"), load_built_rom()) else {
        eprintln!("skip: no symbols/ROM");
        return;
    };
    let mut bad = 0;
    for &(roty, rotx, vel) in &CASES {
        // alvelvecs_l reads speed from al_vel (A ignored).
        let (x1, y1, z1) = rom_vec(&rom, addr, roty, rotx, vel, 0);
        let mut al = alien(roty, rotx, vel);
        sf_strat::common::strat_gen_vecs_2d(&mut al);
        let ok = al.vx == x1 && al.vy == y1 && al.vz == z1;
        if !ok {
            bad += 1;
        }
        eprintln!("2d    roty={roty:3} vel={vel:3}  ROM=({x1},{y1},{z1}) RUST=({},{},{}) {}", al.vx, al.vy, al.vz, if ok { "EXACT" } else { "DIFF" });
    }
    assert_eq!(bad, 0, "{bad} genvecs2d cases differ");
}
