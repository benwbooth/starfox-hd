//! TIER-1 leaf fuzz: `rotate_8xz/yz/yx_l` vs Rust (gen_weapon muzzle chain).
//!
//! ROM: STRATROU.ASM:986 / 1057 / 1128. ABI a8i16:
//!   A = angle; x1=$02, y1=$08, z1=$8A are **signed bytes**;
//!   outs x2=$04, y2=$0A, z2=$1647 are sign-extended i16.
//! `rotate_8xz` negates the angle before the table lookup.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::snes_trig::{rotate_8xz, rotate_8yx, rotate_8yz};

const X1: u32 = 0x0002;
const Y1: u32 = 0x0008;
const X2: u32 = 0x0004;
const Y2: u32 = 0x000A;
const Z1: u32 = 0x008A;
const Z2: u32 = 0x1647;

fn rom_r8xz(rom: &[u8], addr: u32, angle: u8, x: i8, z: i8) -> (i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(X1, x as u8);
    bus.write8(Z1, z as u8);
    call(
        &mut bus,
        addr,
        &Entry {
            a: angle as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    (bus.read16(X2) as i16, bus.read16(Z2) as i16)
}

fn rom_r8yz(rom: &[u8], addr: u32, angle: u8, y: i8, z: i8) -> (i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(Y1, y as u8);
    bus.write8(Z1, z as u8);
    call(
        &mut bus,
        addr,
        &Entry {
            a: angle as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    (bus.read16(Y2) as i16, bus.read16(Z2) as i16)
}

fn rom_r8yx(rom: &[u8], addr: u32, angle: u8, x: i8, y: i8) -> (i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(X1, x as u8);
    bus.write8(Y1, y as u8);
    call(
        &mut bus,
        addr,
        &Entry {
            a: angle as u16,
            p: 0x20,
            ..Default::default()
        },
    );
    (bus.read16(X2) as i16, bus.read16(Y2) as i16)
}

fn sample_angles() -> Vec<u8> {
    let mut v: Vec<u8> = (0..=255u8).step_by(11).collect();
    v.extend([0, 1, 63, 64, 65, 127, 128, 129, 192, 255]);
    v.sort_unstable();
    v.dedup();
    v
}

fn sample_bytes() -> Vec<i8> {
    vec![
        0, 1, -1, 2, -2, 7, -7, 16, -16, 32, -32, 64, -64, 100, -100, 127, -128,
    ]
}

#[test]
fn rotate_8_family_matches_rom() {
    let syms = load_symbols();
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no ROM");
        return;
    };
    let angles = sample_angles();
    let bytes = sample_bytes();
    let mut checked = 0usize;
    let mut diffs = 0usize;
    let mut first: Vec<String> = Vec::new();

    if let Some(&addr) = syms.get("ROTATE_8XZ_L") {
        for &a in &angles {
            for &x in &bytes {
                for &z in &bytes {
                    let r = rom_r8xz(&rom, addr, a, x, z);
                    let u = rotate_8xz(a, x, z);
                    checked += 1;
                    if r != u {
                        diffs += 1;
                        if first.len() < 12 {
                            first.push(format!("8xz a={a} x={x} z={z}: ROM={r:?} RUST={u:?}"));
                        }
                    }
                }
            }
        }
    }
    if let Some(&addr) = syms.get("ROTATE_8YZ_L") {
        for &a in &angles {
            for &y in &bytes {
                for &z in &bytes {
                    let r = rom_r8yz(&rom, addr, a, y, z);
                    let u = rotate_8yz(a, y, z);
                    checked += 1;
                    if r != u {
                        diffs += 1;
                        if first.len() < 12 {
                            first.push(format!("8yz a={a} y={y} z={z}: ROM={r:?} RUST={u:?}"));
                        }
                    }
                }
            }
        }
    }
    if let Some(&addr) = syms.get("ROTATE_8YX_L") {
        for &a in &angles {
            for &x in &bytes {
                for &y in &bytes {
                    let r = rom_r8yx(&rom, addr, a, x, y);
                    let u = rotate_8yx(a, x, y);
                    checked += 1;
                    if r != u {
                        diffs += 1;
                        if first.len() < 12 {
                            first.push(format!("8yx a={a} x={x} y={y}: ROM={r:?} RUST={u:?}"));
                        }
                    }
                }
            }
        }
    }

    eprintln!("PROBE rotate_8 family: checked {checked}; diffs={diffs}");
    for s in &first {
        eprintln!("DIVERGE {s}");
    }
    assert_eq!(diffs, 0, "rotate_8 diverged; first={first:?}");
}
