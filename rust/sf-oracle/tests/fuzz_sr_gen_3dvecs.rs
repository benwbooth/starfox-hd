//! TIER-1 leaf fuzz: `sr_gen_3dvecs` / `sr_gen_3dvecs1..3` vs Rust.
//!
//! ROM (STRATROU.ASM:2624): `jsl n3dvecs_l` then
//! `al_vx/vy/vz = (x1/y1/z1) << {0,1,2,3}`.
//! Rust: `strat_gen_vecs_3d_scaled`.
//!
//! All three velocity components, including the pitch/Y sign, are bit-exact.

use sf_game::alien::Alien;
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::common::strat_gen_vecs_3d_scaled;

const XBASE: u32 = 0x0100;
const AL_ROTX: u32 = 0x12;
const AL_ROTY: u32 = 0x13;
const AL_VEL: u32 = 0x15;
const AL_VX: u32 = 0x2F;
const AL_VY: u32 = 0x31;
const AL_VZ: u32 = 0x33;
const TROTX: u32 = 0x1630;
const TROTY: u32 = 0x1631;
const TMPZ: u32 = 0x0078;

fn rom_sr_gen(rom: &[u8], addr: u32, roty: u8, rotx: u8, vel: u8) -> (i16, i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    // sr_gen reads al_rotx/roty/vel via the s_gen_3dvecs macro path… but the
    // labeled entry itself expects trotx/troty/tmpz already loaded (macro does
    // that). Seed both the alien fields and the DP temps the body uses.
    bus.write8(XBASE + AL_ROTX, rotx);
    bus.write8(XBASE + AL_ROTY, roty);
    bus.write8(XBASE + AL_VEL, vel);
    bus.write8(TROTX, rotx);
    bus.write8(TROTY, roty);
    bus.write8(TMPZ, vel);
    call(
        &mut bus,
        addr,
        &Entry {
            x: XBASE as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    (
        bus.read16(XBASE + AL_VX) as i16,
        bus.read16(XBASE + AL_VY) as i16,
        bus.read16(XBASE + AL_VZ) as i16,
    )
}

fn rust_sr_gen(roty: u8, rotx: u8, vel: u8, shift: u32) -> (i16, i16, i16) {
    let mut al = Alien::default();
    al.roty = roty;
    al.rotx = rotx;
    al.vel = vel;
    strat_gen_vecs_3d_scaled(&mut al, shift);
    (al.vx, al.vy, al.vz)
}

#[test]
fn sr_gen_3dvecs_family_matches_rom() {
    let syms = load_symbols();
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no ROM");
        return;
    };
    let cases = [
        (0u8, 0u8, 100u8),
        (64, 0, 100),
        (192, 0, 100),
        (32, 16, 80),
        (96, 32, 64),
        (128, 0, 100),
        (10, 5, 120),
        (250, 8, 90),
        (1, 1, 1),
        (127, 64, 127),
        // Signed-byte velocity case (vel>=128).
        (200, 200, 50),
    ];
    let names = [
        ("SR_GEN_3DVECS", 0u32),
        ("SR_GEN_3DVECS1", 1),
        ("SR_GEN_3DVECS2", 2),
        ("SR_GEN_3DVECS3", 3),
    ];
    let mut checked = 0usize;
    let mut diffs_lo = 0usize;
    let mut diffs_hi = 0usize;
    let mut first: Vec<String> = Vec::new();
    for &(name, shift) in &names {
        let Some(&addr) = syms.get(name) else {
            eprintln!("skip {name}: no symbol");
            continue;
        };
        for &(roty, rotx, vel) in &cases {
            let (rx, ry, rz) = rom_sr_gen(&rom, addr, roty, rotx, vel);
            let (ux, uy, uz) = rust_sr_gen(roty, rotx, vel, shift);
            checked += 1;
            if (ux, uy, uz) != (rx, ry, rz) {
                if vel < 128 {
                    diffs_lo += 1;
                    if first.len() < 16 {
                        first.push(format!(
                            "{name} roty={roty} rotx={rotx} vel={vel}: ROM=({rx},{ry},{rz}) RUST=({ux},{uy},{uz})"
                        ));
                    }
                } else {
                    diffs_hi += 1;
                }
            }
        }
        // Explicit vel>=128 signed-byte probes.
        for vel in [128u8, 150, 200, 255] {
            let (rx, ry, rz) = rom_sr_gen(&rom, addr, 64, 0, vel);
            let (ux, uy, uz) = rust_sr_gen(64, 0, vel, shift);
            if (ux, uy, uz) != (rx, ry, rz) {
                diffs_hi += 1;
            }
        }
    }
    eprintln!(
        "PROBE sr_gen_3dvecs family: checked {checked}; diffs_lo(vel<128)={diffs_lo}; diffs_hi(vel>=128)={diffs_hi}"
    );
    for s in &first {
        eprintln!("DIVERGE {s}");
    }
    assert_eq!(
        diffs_lo, 0,
        "sr_gen_3dvecs diverged for vel<128; first={first:?}"
    );
    assert_eq!(diffs_hi, 0, "sr_gen_3dvecs diverged for vel>=128");
}
