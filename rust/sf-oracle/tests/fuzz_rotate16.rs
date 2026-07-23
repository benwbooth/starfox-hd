//! TIER-1 leaf fuzz: `rotate_16xz_l` / `rotate_16yz_l` vs Rust ports.
//!
//! ROM: STRATROU.ASM:1198 / 1276. ABI (a8/i16 entry, routine does `i8`):
//!   - A = angle (8-bit)
//!   - x1=$02, z1=$8A (xz) or y1=$08, z1=$8A (yz) — 16-bit vectors
//!   - out: x2=$04, z2=$1647 (xz) or y2=$0A, z2=$1647 (yz)
//!
//! Rust: `sf_strat::snes_trig::{rotate_16xz, rotate_16yz}`.

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::snes_trig::{rotate_16xz, rotate_16yz};

const X1: u32 = 0x0002;
const Y1: u32 = 0x0008;
const X2: u32 = 0x0004;
const Y2: u32 = 0x000A;
const Z1: u32 = 0x008A;
const Z2: u32 = 0x1647;

fn rom_rotate_xz(rom: &[u8], addr: u32, angle: u8, x: i16, z: i16) -> (i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(X1, x as u16);
    bus.write16(Z1, z as u16);
    // SHORTA LONGI on entry (p=$20); routine itself does `i8` before `tax`.
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

fn rom_rotate_yz(rom: &[u8], addr: u32, angle: u8, y: i16, z: i16) -> (i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(Y1, y as u16);
    bus.write16(Z1, z as u16);
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

fn sample_angles() -> Vec<u8> {
    let mut v: Vec<u8> = (0..=255u8).step_by(7).collect();
    v.extend([0, 1, 63, 64, 65, 127, 128, 129, 191, 192, 193, 254, 255]);
    v.sort_unstable();
    v.dedup();
    v
}

fn sample_vecs() -> Vec<i16> {
    vec![
        0, 1, -1, 2, -2, 16, -16, 64, -64, 100, -100, 127, -128, 255, -255, 256, -256, 1000, -1000,
        4096, -4096, 16000, -16000, 32767, -32768,
    ]
}

#[test]
fn rotate_16xz_matches_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("ROTATE_16XZ_L"), load_built_rom()) else {
        eprintln!("skip: no ROTATE_16XZ_L / ROM");
        return;
    };
    let angles = sample_angles();
    let vecs = sample_vecs();
    let mut checked = 0usize;
    let mut diffs = 0usize;
    let mut first: Vec<String> = Vec::new();
    for &angle in &angles {
        for &x in &vecs {
            for &z in &vecs {
                let rom_r = rom_rotate_xz(&rom, addr, angle, x, z);
                let rust_r = rotate_16xz(angle, x, z);
                checked += 1;
                if rom_r != rust_r {
                    diffs += 1;
                    if first.len() < 16 {
                        first.push(format!(
                            "ang={angle} x={x} z={z}: ROM={rom_r:?} RUST={rust_r:?}"
                        ));
                    }
                }
            }
        }
    }
    eprintln!("PROBE rotate_16xz: checked {checked}; diffs={diffs}");
    for s in &first {
        eprintln!("DIVERGE {s}");
    }
    assert_eq!(diffs, 0, "rotate_16xz diverged; first={first:?}");
}

#[test]
fn rotate_16yz_matches_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("ROTATE_16YZ_L"), load_built_rom()) else {
        eprintln!("skip: no ROTATE_16YZ_L / ROM");
        return;
    };
    let angles = sample_angles();
    let vecs = sample_vecs();
    let mut checked = 0usize;
    let mut diffs = 0usize;
    let mut first: Vec<String> = Vec::new();
    for &angle in &angles {
        for &y in &vecs {
            for &z in &vecs {
                let rom_r = rom_rotate_yz(&rom, addr, angle, y, z);
                let rust_r = rotate_16yz(angle, y, z);
                checked += 1;
                if rom_r != rust_r {
                    diffs += 1;
                    if first.len() < 16 {
                        first.push(format!(
                            "ang={angle} y={y} z={z}: ROM={rom_r:?} RUST={rust_r:?}"
                        ));
                    }
                }
            }
        }
    }
    eprintln!("PROBE rotate_16yz: checked {checked}; diffs={diffs}");
    for s in &first {
        eprintln!("DIVERGE {s}");
    }
    assert_eq!(diffs, 0, "rotate_16yz diverged; first={first:?}");
}
