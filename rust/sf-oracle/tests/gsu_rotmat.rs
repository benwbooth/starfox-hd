//! RENDER-MATH ORACLE (scalable): run the ROM's actual camera/rotation matrix
//! routine `mcrotmatzxy16` (GSU, MWCROT.MC:50) and compare its 3x3 matrix to the
//! Rust `build_view_matrix`. The ROM builds a ZXY-order matrix with 16-bit
//! sin/cos; a wrong order/precision in the port misplaces every rendered object.
//! ABI (from crotmat16_l $03AE9C): angles -> GSU RAM $20/$22/$24, matrix read
//! back from $D2; entry MCROTMATZXY16 $01:829F (dispatch stub $8295).

use sf_core::snes_trig::zxy_matrix_q15_fine;
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
    assert_eq!(
        m0,
        [ONE, 0, 0, 0, ONE, 0, 0, 0, ONE],
        "zero-angle must be identity"
    );

    // 16-bit angles (65536 = 360deg). A pure pitch rotates only the Y/Z sub-block
    // (row 0 stays [1,0,0]); check the ROM matrix is a valid rotation there.
    let m = gsu_rotmat(&rom, MCROTMATZXY16, 4096, 0, 0); // 4096 = 22.5deg pitch
    eprintln!("rot(4096,0,0):  {m:?}");
    assert_eq!(
        (m[0], m[1], m[2]),
        (ONE, 0, 0),
        "pure pitch leaves X axis fixed"
    );
    // cos(22.5)=0.9239 -> ~30273, sin(22.5)=0.3827 -> ~12539 (fixed .15).
    let cos = m[4] as f64 / ONE as f64;
    let sin = m[5].unsigned_abs() as f64 / ONE as f64;
    eprintln!("  pitch cos={cos:.4} sin={sin:.4} (expect 0.924 / 0.383)");
    assert!((cos - 0.9239).abs() < 0.02 && (sin - 0.3827).abs() < 0.02);

    // --- ROM-vs-Rust matrix DIFF (finds the camera-matrix defect) -----------
    // Rust build_view_matrix_f rotation[] (transform.rs), reconstructed here.
    // Angle in radians is representation-independent; sincos_frac(-crx)/(cry)/
    // (-crz) -> the negation pattern the port applies.
    use std::f64::consts::PI;
    let rust_rot = |rx: i16, ry: i16, rz: i16| -> [f64; 9] {
        let a = |v: i16| v as f64 / 65536.0 * 2.0 * PI;
        let (sx, cx) = (-a(rx)).sin_cos();
        let (sy, cy) = (a(ry)).sin_cos();
        let (sz, cz) = (-a(rz)).sin_cos();
        [
            cy * cz,
            cy * sz,
            -sy,
            sx * sy * cz - cx * sz,
            sx * sy * sz + cx * cz,
            sx * cy,
            cx * sy * cz + sx * sz,
            cx * sy * sz - sx * cz,
            cx * cy,
        ]
    };
    let cmp = |label: &str, rx: i16, ry: i16, rz: i16| {
        let rom = gsu_rotmat(&rom, MCROTMATZXY16, rx, ry, rz);
        let romf: Vec<f64> = rom.iter().map(|&v| v as f64 / ONE as f64).collect();
        let rust = rust_rot(rx, ry, rz);
        let maxd = romf
            .iter()
            .zip(rust.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        eprintln!("{label} maxΔ={maxd:.3}");
        eprintln!(
            "  ROM : {}",
            romf.iter()
                .map(|v| format!("{v:+.3}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        eprintln!(
            "  RUST: {}",
            rust.iter()
                .map(|v| format!("{v:+.3}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        maxd
    };
    let _ = cmp("pure yaw  (0,4096,0)", 0, 4096, 0);
    let _ = cmp("pure pitch(4096,0,0)", 4096, 0, 0);
    let _ = cmp("pure roll (0,0,4096)", 0, 0, 4096);
    let _ = cmp("combined  (-7000,20000,0)", -7000, 20000, 0);

    // Candidate CORRECT formula: ZXY order = Ry·Rx·Rz with the ROM's per-axis
    // signs (Ry has +sin[0][2]/-sin[2][0]). If this reproduces the ROM for ALL
    // cases, it's exactly what build_view_matrix should use.
    let mul = |a: [f64; 9], b: [f64; 9]| -> [f64; 9] {
        let mut m = [0.0; 9];
        for r in 0..3 {
            for c in 0..3 {
                m[r * 3 + c] = (0..3).map(|k| a[r * 3 + k] * b[k * 3 + c]).sum();
            }
        }
        m
    };
    let zxy = |rx: i16, ry: i16, rz: i16| -> [f64; 9] {
        let a = |v: i16| v as f64 / 65536.0 * 2.0 * PI;
        let (sx, cx) = a(rx).sin_cos();
        let (sy, cy) = a(ry).sin_cos();
        let (sz, cz) = a(rz).sin_cos();
        let rxm = [1., 0., 0., 0., cx, -sx, 0., sx, cx];
        let rym = [cy, 0., sy, 0., 1., 0., -sy, 0., cy];
        let rzm = [cz, -sz, 0., sz, cz, 0., 0., 0., 1.];
        mul(rym, mul(rxm, rzm)) // Ry·Rx·Rz  (apply Z, then X, then Y)
    };
    eprintln!("--- candidate ZXY (Ry*Rx*Rz) vs ROM ---");
    let mut worst = 0.0f64;
    for (rx, ry, rz) in [
        (0, 4096, 0),
        (4096, 0, 0),
        (0, 0, 4096),
        (-7000, 20000, 0),
        (5000, -12000, 8000),
    ] {
        let rom = gsu_rotmat(&rom, MCROTMATZXY16, rx, ry, rz);
        let romf: Vec<f64> = rom.iter().map(|&v| v as f64 / ONE as f64).collect();
        let cand = zxy(rx, ry, rz);
        let d = romf
            .iter()
            .zip(cand.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        worst = worst.max(d);
        eprintln!("  ({rx},{ry},{rz}) maxΔ={d:.3}");
    }
    eprintln!(
        "=> candidate ZXY worst Δ = {worst:.3} (near 0 => this is the correct formula to port)"
    );
    assert!(
        worst < 0.02,
        "ZXY = Ry*Rx*Rz should reproduce the ROM matrix"
    );
}

#[test]
fn fractional_angle_matrix_matches_the_gsu_bit_for_bit() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: no ROM");
        return;
    };
    const ESCAPE_PITCH: i16 = -420;
    const ESCAPE_YAW: i16 = -1_365;
    for (pitch, yaw, roll) in [
        (1, 2, 3),
        (ESCAPE_PITCH, ESCAPE_YAW, 0),
        (5_123, -12_345, 8_765),
        (16_383, 16_385, -32_767),
    ] {
        let oracle = gsu_rotmat(&rom, MCROTMATZXY16, pitch, yaw, roll);
        let port = zxy_matrix_q15_fine(pitch as u16, yaw as u16, roll as u16);
        let flattened = [
            port[0][0], port[0][1], port[0][2], port[1][0], port[1][1], port[1][2], port[2][0],
            port[2][1], port[2][2],
        ];
        assert_eq!(
            flattened, oracle,
            "matrix mismatch at ({pitch},{yaw},{roll})"
        );
    }
}
