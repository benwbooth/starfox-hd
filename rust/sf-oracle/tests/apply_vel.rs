//! Differential test: ROM `addalvecs_l` (position += velocity, every object
//! every tick) vs Rust `strat_apply_velocity`. ABI: X = alien base; adds
//! al_vx/vy/vz ($2F/$31/$33) into al_worldx/y/z ($0C/$0E/$10), 16-bit.

use sf_game::alien::Alien;
use sf_oracle::{call, load_built_rom, load_symbols, Entry, SnesBus};

const XB: u32 = 0x0100;
const WX: u32 = 0x0C;
const WY: u32 = 0x0E;
const WZ: u32 = 0x10;
const VX: u32 = 0x2F;
const VY: u32 = 0x31;
const VZ: u32 = 0x33;

fn rom_apply(rom: &[u8], addr: u32, p: [i16; 6]) -> (i16, i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(XB + WX, p[0] as u16);
    bus.write16(XB + WY, p[1] as u16);
    bus.write16(XB + WZ, p[2] as u16);
    bus.write16(XB + VX, p[3] as u16);
    bus.write16(XB + VY, p[4] as u16);
    bus.write16(XB + VZ, p[5] as u16);
    call(&mut bus, addr, &Entry { x: XB as u16, p: 0x00, ..Default::default() });
    (
        bus.read16(XB + WX) as i16,
        bus.read16(XB + WY) as i16,
        bus.read16(XB + WZ) as i16,
    )
}

#[test]
fn apply_velocity_matches_rom() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("ADDALVECS_L"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    let cases: [[i16; 6]; 6] = [
        [1000, 500, 8000, 100, -50, 200],
        [0, 0, 0, 0, 0, 0],
        [-1000, -500, -8000, -100, 50, -200],
        [32000, 0, 0, 1000, 0, 0], // wrap
        [-32000, 0, 0, -1000, 0, 0],
        [12345, -6789, 4321, -111, 222, -333],
    ];
    let mut bad = 0;
    for p in cases {
        let r = rom_apply(&rom, addr, p);
        let mut al = Alien::default();
        al.worldx = p[0];
        al.worldy = p[1];
        al.worldz = p[2];
        al.vx = p[3];
        al.vy = p[4];
        al.vz = p[5];
        sf_strat::common::strat_apply_velocity(&mut al);
        let g = (al.worldx, al.worldy, al.worldz);
        if r != g {
            bad += 1;
        }
        eprintln!("pos{:?} vel{:?} -> ROM{r:?} RUST{g:?} {}", &p[0..3], &p[3..6], if r == g { "ok" } else { "DIFF" });
    }
    assert_eq!(bad, 0, "{bad} apply_velocity cases differ");
}
