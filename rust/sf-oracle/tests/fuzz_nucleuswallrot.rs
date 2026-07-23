//! TIER-1: `nucleuswallrot_srou_l` / `nucleuswallrot2_srou_l` vs Rust.
//!
//! ROM (GASTRATS.ASM:157/195): x1=0, z1=al_sword2, A=al_sbyte2 →
//! `rotate_16xz_l` → worldx=x2, worldz=(z2<<1)+zbase+zref; sbyte2+=gsvar_byte1.
//! zbase = 160<<3 (wallrot) / 210<<3 (wallrot2); zref = player_posz / pviewposz.
//!
//! Rust: `sf_strat` bosses `b8_wallrot` / `b8_wallrot2` (via rotate_16xz).

use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};
use sf_strat::snes_trig::rotate_16xz;

const XBASE: u32 = 0x0100;
const AL_WORLDX: u32 = 0x0C;
const AL_WORLDZ: u32 = 0x10;
const AL_SBYTE2: u32 = 0x23;
const AL_SWORD2: u32 = 0x28;
const PLAYER_POSZ: u32 = 0x159C;
const PVIEWPOSZ: u32 = 0x1585;
const GSVAR_BYTE1: u32 = 0x15DA;

fn rom_wallrot(
    rom: &[u8],
    addr: u32,
    angle: u8,
    dist: i16,
    zref: i16,
    zref_addr: u32,
    gsv: u8,
) -> (i16, i16, u8) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write8(XBASE + AL_SBYTE2, angle);
    bus.write16(XBASE + AL_SWORD2, dist as u16);
    bus.write16(zref_addr, zref as u16);
    bus.write8(GSVAR_BYTE1, gsv);
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
        bus.read16(XBASE + AL_WORLDX) as i16,
        bus.read16(XBASE + AL_WORLDZ) as i16,
        bus.read8(XBASE + AL_SBYTE2),
    )
}

fn rust_wallrot(angle: u8, dist: i16, zbase: i16, zref: i16, gsv: u8) -> (i16, i16, u8) {
    let (x2, z2) = rotate_16xz(angle, 0, dist);
    let wx = x2;
    let wz = z2.wrapping_mul(2).wrapping_add(zbase).wrapping_add(zref);
    let ang = angle.wrapping_add(gsv);
    (wx, wz, ang)
}

#[test]
fn nucleuswallrot_matches_rom() {
    let syms = load_symbols();
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no ROM");
        return;
    };
    let cases: &[(u8, i16)] = &[
        (0, 100),
        (64, 100),
        (128, 80),
        (192, 50),
        (32, 200),
        (1, 1),
        (255, 127),
        (90, 300),
    ];
    let zrefs = [0i16, 100, -50, 1000];
    let gsvs = [0u8, 1, 5, 16];
    let mut checked = 0usize;
    let mut diffs = 0usize;
    let mut first: Vec<String> = Vec::new();

    let specs = [
        ("NUCLEUSWALLROT_SROU_L", 160i16 << 3, PLAYER_POSZ),
        ("NUCLEUSWALLROT2_SROU_L", 210i16 << 3, PVIEWPOSZ),
    ];
    for &(name, zbase, zref_addr) in &specs {
        let Some(&addr) = syms.get(name) else {
            eprintln!("skip {name}: no symbol");
            continue;
        };
        for &(angle, dist) in cases {
            for &zref in &zrefs {
                for &gsv in &gsvs {
                    let (rx, rz, ra) = rom_wallrot(&rom, addr, angle, dist, zref, zref_addr, gsv);
                    let (ux, uz, ua) = rust_wallrot(angle, dist, zbase, zref, gsv);
                    checked += 1;
                    if (rx, rz, ra) != (ux, uz, ua) {
                        diffs += 1;
                        if first.len() < 12 {
                            first.push(format!(
                                "{name} a={angle} d={dist} zref={zref} gsv={gsv}: ROM=({rx},{rz},{ra}) RUST=({ux},{uz},{ua})"
                            ));
                        }
                    }
                }
            }
        }
    }
    eprintln!("PROBE nucleuswallrot: checked {checked}; diffs={diffs}");
    for s in &first {
        eprintln!("DIVERGE {s}");
    }
    assert_eq!(diffs, 0, "nucleuswallrot diverged; first={first:?}");
}
