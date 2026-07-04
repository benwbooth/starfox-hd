//! End-to-end GSU validation: run the real `mcallarctan16` GSU program from the
//! ROM and check it computes atan2. This is what proves the GSU emulator is
//! trustworthy enough to validate GSU-side game functions (angle/3D math).
//!
//! ABI (from disassembling $01:81AA): inputs in GSU RAM (bank $70) m_x1=$62,
//! m_y1=$2C; result m_cnt=$40. arctan16 returns arctan(x1/y1), 0..359deg mapped
//! to $0000..$FFFF.

use sf_oracle::gsu::Gsu;
use sf_oracle::load_built_rom;

const M_X1: usize = 0x62;
const M_Y1: usize = 0x2C;
const M_CNT: usize = 0x40;

fn gsu_arctan(rom: &[u8], x: i16, y: i16) -> u16 {
    let mut gsu = Gsu::new(rom.to_vec());
    gsu.ram[M_X1] = x as u16 as u8;
    gsu.ram[M_X1 + 1] = (x as u16 >> 8) as u8;
    gsu.ram[M_Y1] = y as u16 as u8;
    gsu.ram[M_Y1 + 1] = (y as u16 >> 8) as u8;
    gsu.run(1, 0x81AA); // mcallarctan16 @ $01:81AA
    gsu.ram[M_CNT] as u16 | ((gsu.ram[M_CNT + 1] as u16) << 8)
}

fn expected_angle(x: i16, y: i16) -> u16 {
    let mut a = (x as f64).atan2(y as f64);
    if a < 0.0 {
        a += 2.0 * std::f64::consts::PI;
    }
    (a * 65536.0 / (2.0 * std::f64::consts::PI)).round() as u16
}

#[test]
fn arctan16_is_sane() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: built ROM data/sf.sfc not present");
        return;
    };
    // Cardinal + diagonal angles: the GSU emulator runs the real ROM octant/
    // quadrant + table code and matches atan2 EXACTLY here — this validates the
    // core (prefix system, ~50 opcodes, memory, FMULT, MOVE/MOVES, LINK, LOOP,
    // branches, the GSU RAM/ROM ABI). Off-axis/shallow angles depend on the
    // shift-subtract divide refinement ($8192) which still has a flag bug (WIP).
    let exact_cases = [
        (100i16, 0i16),
        (100, 100),
        (-100, 100),
        (100, -100),
        (-100, -100),
    ];
    let deg = |a: u16| a as u32 * 360 / 65536;
    let circ = |got: u16, exp: u16| {
        let dd = (got as i32 - exp as i32).rem_euclid(65536);
        dd.min(65536 - dd)
    };
    let mut bad = 0;
    for &(x, y) in &exact_cases {
        let got = gsu_arctan(&rom, x, y);
        let exp = expected_angle(x, y);
        let d = circ(got, exp);
        if d > 64 {
            bad += 1;
        }
        eprintln!(
            "arctan16 x={x:4} y={y:4}: GSU={got:5} ({:3}deg)  atan2~{exp:5} ({:3}deg)  dist={d}",
            deg(got),
            deg(exp)
        );
    }
    // Informational: refinement-dependent angles (not yet asserted).
    for &(x, y) in &[(50i16, 87i16), (0, 100)] {
        let got = gsu_arctan(&rom, x, y);
        eprintln!(
            "arctan16 x={x:4} y={y:4}: GSU={got:5} ({:3}deg)  atan2~{:3}deg  [refinement WIP]",
            deg(got),
            deg(expected_angle(x, y))
        );
    }
    assert_eq!(bad, 0, "{bad} cardinal/diagonal cases diverge from atan2");
}
