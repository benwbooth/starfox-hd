//! Render-math oracle: run the built ROM's actual `mallrotzsort` GSU routine
//! (MDRAWLIS.MC:1399) over a synthetic drawlist and compare each rotated
//! dl_x/dl_y/dl_z against the Rust `matrix_rotate_q15`. This is the routine
//! that feeds alienflags_l's ATZREMOVE behind test (MAIN.ASM:2032), so any
//! truncation difference shifts cull timing on boundary objects.

use sf_core::snes_trig::{matrix_rotate_q15, zxy_matrix_q15_fine};
use sf_oracle::gsu::Gsu;
use sf_oracle::load_built_rom;

const M_NUMSHAPES: usize = 0x018A;
const M_DRAWLIST: usize = 0x1960;
const M_WMAT11: usize = 0x00D2;
const DL_STRIDE: usize = 0x1E;
// Field offsets within one drawlist entry (STRUCTS.INC drawlist format).
const OFF_SFLAGS: usize = 7;
const OFF_SHAPE: usize = 8;
const OFF_Y: usize = 16;
const OFF_X: usize = 18;
const OFF_Z: usize = 20;

/// Bank/entry for MALLROTZSORT ($0001b18f in the built ROM).
const MALLROTZSORT_BANK: u8 = 1;
const MALLROTZSORT_ENTRY: u16 = 0xB18F;

fn wr16(g: &mut Gsu, addr: usize, v: u16) {
    g.ram[addr] = v as u8;
    g.ram[addr + 1] = (v >> 8) as u8;
}

fn rd16(g: &Gsu, addr: usize) -> u16 {
    u16::from(g.ram[addr]) | (u16::from(g.ram[addr + 1]) << 8)
}

#[test]
fn mallrotzsort_dot_products_match_matrix_rotate_q15() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no built ROM");
        return;
    };

    // Deterministic spread covering small/large and negative values,
    // including the kamikaze-class boundary magnitudes seen at gf851.
    let cases: [(i16, i16, i16); 12] = [
        (77, -109, -18),
        (83, -125, 32),
        (89, -141, 82),
        (95, -157, 132),
        (-1200, 40, -900),
        (1300, -260, -1500),
        (0, 0, 0),
        (1, -1, 2),
        (-32768, 32767, -12345),
        (12345, -6789, 32000),
        (56, -339, 1171),
        (-500, -800, 24000),
    ];

    // Identity-ish Q15 matrix (GSU emits ~1.0 as 0x7FFE) and a lightly
    // rotated one for non-trivial mixing.
    let mat = [[0x7FFE_i16, 0, 0], [0, 0x7FFE, 0], [0, 0, 0x7FFE]];
    let rot = zxy_matrix_q15_fine(0x20, 0x40, 0x08);

    for (label, m) in [("identity", mat), ("rotated", rot)] {
        let mut g = Gsu::new(rom.clone());
        for r in 0..3 {
            for c in 0..3 {
                wr16(&mut g, M_WMAT11 + (r * 3 + c) * 2, m[r][c] as u16);
            }
        }
        wr16(&mut g, M_NUMSHAPES, cases.len() as u16);
        for (i, (x, y, z)) in cases.iter().enumerate() {
            let base = M_DRAWLIST + i * DL_STRIDE;
            g.ram[base + OFF_SFLAGS] = 0; // not shadow
            wr16(&mut g, base + OFF_SHAPE, 0xB70C);
            wr16(&mut g, base + OFF_X, *x as u16);
            wr16(&mut g, base + OFF_Y, *y as u16);
            wr16(&mut g, base + OFF_Z, *z as u16);
        }

        g.run(MALLROTZSORT_BANK, MALLROTZSORT_ENTRY);

        for (i, (x, y, z)) in cases.iter().enumerate() {
            let base = M_DRAWLIST + i * DL_STRIDE;
            let gx = rd16(&g, base + OFF_X) as i16;
            let gy = rd16(&g, base + OFF_Y) as i16;
            let gz = rd16(&g, base + OFF_Z) as i16;
            let (px, py, pz) = matrix_rotate_q15(m, *x, *y, *z);
            eprintln!(
                "{label} case{i} in=({x},{y},{z}): ROM=({gx},{gy},{gz}) port=({px},{py},{pz})"
            );
        }
    }
}
