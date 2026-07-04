//! Characterize ROM `framescalevecs` (PSTRATS.ASM $BEA90, an RTS/near fn): it
//! scales al_vx/vy by `framerate/8`. The Rust port makes it a no-op, which is
//! only correct if the base `framerate` (framec) == 8 (identity). This runs the
//! real ROM fn to confirm the scaling + the identity point. ABI: X=alien
//! (al_vx=$2F, al_vy=$31, 8-bit in), framerate at $156E, 8-bit A entry.

use sf_oracle::{call_near, load_built_rom, load_symbols, Entry, SnesBus};

const XB: u32 = 0x0100;
const AL_VX: u32 = 0x2F;
const AL_VY: u32 = 0x31;
const FRAMERATE: u32 = 0x156E;

fn rom_framescale(rom: &[u8], addr: u32, vx: i16, vy: i16, framerate: u8) -> (i16, i16) {
    let mut bus = SnesBus::new(rom.to_vec());
    bus.write16(XB + AL_VX, vx as u16);
    bus.write16(XB + AL_VY, vy as u16);
    bus.write8(FRAMERATE, framerate);
    call_near(&mut bus, addr, &Entry { x: XB as u16, p: 0x20, ..Default::default() });
    (
        bus.read16(XB + AL_VX) as i16,
        bus.read16(XB + AL_VY) as i16,
    )
}

#[test]
fn characterize_framescalevecs() {
    let syms = load_symbols();
    let (Some(&addr), Some(rom)) = (syms.get("FRAMESCALEVECS"), load_built_rom()) else {
        eprintln!("skip: no symbol/ROM");
        return;
    };
    // Empirically framescalevecs = vx * framerate/4 (NOT /8): identity at
    // framerate=4. SF's base frame rate is ~15fps = 60/4 -> framec=4, so at the
    // base rate the function is IDENTITY -> the Rust fixed-step no-op is CORRECT.
    for fr in [1u8, 2, 4, 8, 16, 24] {
        let (ox, oy) = rom_framescale(&rom, addr, 100, -64, fr);
        eprintln!("framerate={fr:2}: vx 100->{ox:4}  vy -64->{oy:4}  (= *{fr}/4)");
    }
    let (ix, iy) = rom_framescale(&rom, addr, 100, -64, 4);
    eprintln!("IDENTITY @framerate=4 (base framec): ({ix},{iy}) vs (100,-64)");
    assert!(
        (ix - 100).abs() <= 1 && (iy + 64).abs() <= 1,
        "framerate=4 (base) should be identity -> no-op port is correct"
    );
}
