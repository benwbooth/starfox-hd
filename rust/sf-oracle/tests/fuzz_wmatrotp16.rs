//! TIER-1 GSU leaf fuzz: `mwmatrotp16` vs Rust `wmat_rot_point`.
//!
//! ROM: MWROT.MC:19 (entry MWMATROTP16 $01:823E). ABI:
//!   in:  m_x1=$62, m_y1=$2C, m_z1=$2E; matrix m_wmat11..33 @ $D2
//!   out: m_bigx=$26, m_bigy=$28, m_bigz=$2A
//!
//! Each output is three GSU FMULT+ROL terms (`(a*b)>>15`) summed with 16-bit wrap.

use sf_oracle::gsu::Gsu;
use sf_oracle::load_built_rom;
use sf_strat::snes_trig::wmat_rot_point;

const M_X1: usize = 0x62;
const M_Y1: usize = 0x2C;
const M_Z1: usize = 0x2E;
const M_BIGX: usize = 0x26;
const M_BIGY: usize = 0x28;
const M_BIGZ: usize = 0x2A;
const M_WMAT11: usize = 0xD2;
/// Dispatch entry used by `wmatrotp16_l` (`ldx #mwmatrotp16&$ffff`).
const MWMATROTP16: u16 = 0x823E;
const ONE: i16 = 32766; // GSU fixed-point ~1.0

fn wr16(g: &mut Gsu, addr: usize, v: i16) {
    g.ram[addr] = v as u16 as u8;
    g.ram[addr + 1] = (v as u16 >> 8) as u8;
}

fn rd16(g: &Gsu, addr: usize) -> i16 {
    (g.ram[addr] as u16 | ((g.ram[addr + 1] as u16) << 8)) as i16
}

fn gsu_wmatrot(rom: &[u8], mat: [[i16; 3]; 3], x: i16, y: i16, z: i16) -> (i16, i16, i16) {
    let mut g = Gsu::new(rom.to_vec());
    wr16(&mut g, M_X1, x);
    wr16(&mut g, M_Y1, y);
    wr16(&mut g, M_Z1, z);
    for r in 0..3 {
        for c in 0..3 {
            wr16(&mut g, M_WMAT11 + (r * 3 + c) * 2, mat[r][c]);
        }
    }
    g.run(1, MWMATROTP16);
    (rd16(&g, M_BIGX), rd16(&g, M_BIGY), rd16(&g, M_BIGZ))
}

fn sample_i16() -> Vec<i16> {
    vec![
        0, 1, -1, 2, -2, 16, -16, 64, -64, 100, -100, 127, -128, 255, -255, 256, -256, 1000, -1000,
        4096, -4096, 16384, -16384, 20000, -20000, 32767, -32768,
    ]
}

fn sample_mats() -> Vec<[[i16; 3]; 3]> {
    let mut mats = vec![
        // Identity (~1.0)
        [[ONE, 0, 0], [0, ONE, 0], [0, 0, ONE]],
        // Zero
        [[0; 3]; 3],
        // Pure scale 0.5
        [[16384, 0, 0], [0, 16384, 0], [0, 0, 16384]],
        // Swap X/Z with signs (common view-ish)
        [[0, 0, ONE], [0, ONE, 0], [-ONE, 0, 0]],
        // Dense small
        [[100, -200, 300], [-400, 500, -600], [700, -800, 900]],
        // Near-full Q15 mix
        [[ONE, 1000, -2000], [-3000, ONE, 4000], [5000, -6000, ONE]],
    ];
    // A few deterministic pseudo-random matrices
    let mut s: u32 = 0xC0FFEE;
    for _ in 0..8 {
        let mut m = [[0i16; 3]; 3];
        for r in 0..3 {
            for c in 0..3 {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                m[r][c] = (s as i32 >> 16) as i16;
            }
        }
        mats.push(m);
    }
    mats
}

#[test]
fn wmatrotp16_matches_rust() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no built ROM");
        return;
    };

    let mats = sample_mats();
    let vals = sample_i16();
    let mut n = 0u32;
    let mut mismatches = 0u32;
    let mut first: Option<String> = None;

    for mat in &mats {
        for &x in &vals {
            for &y in &vals {
                // Sparse z to keep runtime bounded (~14 mats * 27^2 * ~9 z)
                for &z in &[0i16, 1, -1, 100, -100, 1000, -1000, 16384, -16384] {
                    let rom_out = gsu_wmatrot(&rom, *mat, x, y, z);
                    let rust_out = wmat_rot_point(*mat, x, y, z);
                    n += 1;
                    if rom_out != rust_out {
                        mismatches += 1;
                        if first.is_none() {
                            first = Some(format!(
                                "mat={mat:?} p=({x},{y},{z}) rom={rom_out:?} rust={rust_out:?}"
                            ));
                        }
                    }
                }
            }
        }
    }

    eprintln!("wmatrotp16: {n} cases, {mismatches} mismatches");
    if let Some(f) = first {
        eprintln!("first: {f}");
    }
    assert_eq!(mismatches, 0, "wmatrotp16 must be bit-exact vs GSU");
}
