//! RENDER-MATH ORACLE (scalable): run the ROM's actual camera/rotation matrix
//! routine `mcrotmatzxy16` (GSU, MWCROT.MC:50) and compare its 3x3 matrix to the
//! Rust `build_view_matrix`. The ROM builds a ZXY-order matrix with 16-bit
//! sin/cos; a wrong order/precision in the port misplaces every rendered object.
//! ABI (from crotmat16_l $03AE9C): angles -> GSU RAM $20/$22/$24, matrix read
//! back from $D2; entry MCROTMATZXY16 $01:829F (dispatch stub $8295).

use sf_oracle::gsu::Gsu;
use sf_oracle::load_built_rom;

fn wr16(g: &mut Gsu, addr: usize, v: i16) {
    g.ram[addr] = v as u16 as u8;
    g.ram[addr + 1] = (v as u16 >> 8) as u8;
}
fn rd16(g: &Gsu, addr: usize) -> i16 {
    (g.ram[addr] as u16 | ((g.ram[addr + 1] as u16) << 8)) as i16
}

/// Run the GSU matrix routine; return the 9 matrix words at $D2.., or None if
/// the emulator can't complete it.
fn gsu_rotmat(rom: &[u8], entry: u16, rx: i16, ry: i16, rz: i16) -> [i16; 9] {
    let mut g = Gsu::new(rom.to_vec());
    wr16(&mut g, 0x20, rx);
    wr16(&mut g, 0x22, ry);
    wr16(&mut g, 0x24, rz);
    g.run(1, entry);
    let mut m = [0i16; 9];
    for (i, e) in m.iter_mut().enumerate() {
        *e = rd16(&g, 0xD2 + i * 2);
    }
    m
}

/// Dispatch-stub entry for mcrotmatzxy16 (crotmat16_l loads X=$8295 for
/// runmario). $829F is the inner label and does NOT set up correctly on its own.
const MCROTMATZXY16: u16 = 0x8295;
/// 16-bit fixed-point 1.0 as the GSU emits it (0x7FFE).
const ONE: i16 = 32766;

#[test]
fn rotmat_identity_and_axis_rotations_match_rom() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no ROM");
        return;
    };
    // Zero angles -> identity.
    let m0 = gsu_rotmat(&rom, MCROTMATZXY16, 0, 0, 0);
    eprintln!("rot(0,0,0):     {m0:?}");
    assert_eq!(m0, [ONE, 0, 0, 0, ONE, 0, 0, 0, ONE], "zero-angle must be identity");

    // 16-bit angles (65536 = 360deg). A pure pitch rotates only the Y/Z sub-block
    // (row 0 stays [1,0,0]); check the ROM matrix is a valid rotation there.
    let m = gsu_rotmat(&rom, MCROTMATZXY16, 4096, 0, 0); // 4096 = 22.5deg pitch
    eprintln!("rot(4096,0,0):  {m:?}");
    assert_eq!((m[0], m[1], m[2]), (ONE, 0, 0), "pure pitch leaves X axis fixed");
    // cos(22.5)=0.9239 -> ~30273, sin(22.5)=0.3827 -> ~12539 (fixed .15).
    let cos = m[4] as f64 / ONE as f64;
    let sin = m[5].unsigned_abs() as f64 / ONE as f64;
    eprintln!("  pitch cos={cos:.4} sin={sin:.4} (expect 0.924 / 0.383)");
    assert!((cos - 0.9239).abs() < 0.02 && (sin - 0.3827).abs() < 0.02);

    // The scalable next step: diff the full ROM matrix vs Rust build_view_matrix
    // for representative camera angles to pin the "entities too high" / horizon
    // discrepancy (ROM is ZXY 16-bit; Rust comment says ZYX). Printed for now.
    for (rx, ry, rz) in [(4096, 0, 0), (0, 4096, 0), (0, 0, 4096), (-7000, 20000, 0)] {
        eprintln!("rot({rx},{ry},{rz}): {:?}", gsu_rotmat(&rom, MCROTMATZXY16, rx, ry, rz));
    }
}
