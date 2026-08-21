//! End-to-end GSU validation: run the real `mcallarctan16` GSU program from the
//! ROM and check it computes atan2. This is what proves the GSU emulator is
//! trustworthy enough to validate GSU-side game functions (angle/3D math).
//!
//! ABI (from disassembling $01:81AA): inputs in GSU RAM (bank $70) m_x1=$62,
//! m_y1=$2C; result m_cnt=$40. arctan16 returns arctan(x1/y1), 0..359deg mapped
//! to $0000..$FFFF.

use sf_core::aim_angle::atan16;
use sf_oracle::gsu::Gsu;
use sf_oracle::load_built_rom;

const M_X1: usize = 0x62;
const M_Y1: usize = 0x2C;
const M_CNT: usize = 0x40;
const GSU_PROGRAM_BANK: u8 = 1;
const GSU_ARCTAN_ENTRY: u16 = 0x81AA;

fn gsu_arctan(rom: &[u8], x: i16, y: i16) -> u16 {
    let mut gsu = Gsu::new(rom.to_vec());
    gsu.ram[M_X1] = x as u16 as u8;
    gsu.ram[M_X1 + 1] = (x as u16 >> 8) as u8;
    gsu.ram[M_Y1] = y as u16 as u8;
    gsu.ram[M_Y1 + 1] = (y as u16 >> 8) as u8;
    gsu.run(GSU_PROGRAM_BANK, GSU_ARCTAN_ENTRY);
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
    // branches, the GSU RAM/ROM ABI). Off-axis/shallow angles exercise the
    // shift-subtract divide refinement ($8192); see `arctan16_off_axis_grid`.
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
    // Off-axis/shallow angles now verified too (see off_axis_grid below).
    for &(x, y) in &[(50i16, 87i16), (0, 100)] {
        let got = gsu_arctan(&rom, x, y);
        eprintln!(
            "arctan16 x={x:4} y={y:4}: GSU={got:5} ({:3}deg)  atan2~{:3}deg",
            deg(got),
            deg(expected_angle(x, y))
        );
    }
    assert_eq!(bad, 0, "{bad} cardinal/diagonal cases diverge from atan2");
}

/// Circular distance between two 16-bit angles, in [0, 32768].
fn circ(got: u16, exp: u16) -> i32 {
    let dd = (got as i32 - exp as i32).rem_euclid(65536);
    dd.min(65536 - dd)
}

/// Off-axis grid: the divide (`mdivu3115` @ $8192) + arctan table lookup path
/// that the cardinal/diagonal cases skip. Previously "WIP" because the GSU
/// emulator had SuperFX branch opcodes $06/$07 (BGE/BLT) swapped, so
/// `marctan16` took `blt marctan3` on positive CMP results and skipped the
/// mandatory operand swap — dividing by zero on any |y|>|x| input. With the
/// opcode fix ($06=BGE S==OV, $07=BLT S!=OV), the ROM's own arctan is now
/// exercised end-to-end. It matches atan2:
///   * within 64/65536 in the raw 16-bit angle (ROM `arctantab` is 512 entries
///     + `quotient>>5`, so the low bits are quantized — this is ROM precision,
///     not an emulator defect), and
///   * within +/-1 in the 8-bit angle the game actually uses (`arctan16>>8`).
#[test]
fn arctan16_off_axis_grid() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: built ROM data/sf.sfc not present");
        return;
    };
    // Mix of magnitudes, shallow ratios, both signs, all quadrants.
    let vals: [i16; 24] = [
        -12345, -4000, -1000, -300, -173, -100, -37, -13, -7, -3, -1, 0, 1, 3, 7, 13, 37, 91, 100,
        300, 1000, 4000, 12345, 173,
    ];
    let mut max16 = 0i32;
    let mut max8 = 0i32;
    let mut worst = (0i16, 0i16, 0u16, 0u16);
    let mut n = 0;
    for &x in &vals {
        for &y in &vals {
            if x == 0 && y == 0 {
                continue;
            }
            let got = gsu_arctan(&rom, x, y);
            let exp = expected_angle(x, y);
            let d16 = circ(got, exp);
            // 8-bit angle the game uses: arctan16 >> 8 (256 = full circle).
            let g8 = (got >> 8) as i32;
            let e8 = (exp >> 8) as i32;
            let d8 = {
                let dd = (g8 - e8).rem_euclid(256);
                dd.min(256 - dd)
            };
            if d16 > max16 {
                max16 = d16;
                worst = (x, y, got, exp);
            }
            max8 = max8.max(d8);
            n += 1;
        }
    }
    eprintln!(
        "off-axis grid n={n}: max 16-bit delta={max16} (worst x={} y={} GSU={} atan2={}), max 8-bit delta={max8}",
        worst.0, worst.1, worst.2, worst.3
    );
    assert!(
        max16 <= 64,
        "ROM arctan16 diverges from atan2 by {max16} (>64) in raw 16-bit units"
    );
    assert!(
        max8 <= 1,
        "ROM arctan16>>8 diverges from atan2 by {max8} 8-bit units (>1)"
    );
}

/// Port parity: the pure Rust integer routine must reproduce every raw bit from
/// the retail GSU implementation, including table quantization.
#[test]
fn arctan16_matches_port_angle() {
    let Some(rom) = load_built_rom() else {
        eprintln!("skip: built ROM data/sf.sfc not present");
        return;
    };
    let vals: [i16; 20] = [
        -4000, -1000, -300, -100, -37, -13, -3, -1, 0, 1, 3, 13, 37, 100, 300, 1000, 4000, 7, 91,
        173,
    ];
    for &x in &vals {
        for &y in &vals {
            if x == 0 && y == 0 {
                continue;
            }
            assert_eq!(
                atan16(x, y),
                gsu_arctan(&rom, x, y),
                "integer arctangent mismatch for x={x}, y={y}"
            );
        }
    }

    const OPENING_VERTICAL_DELTA: i16 = 730;
    const OPENING_HORIZONTAL_RANGE: i16 = 615;
    const RETAIL_SIGNED_PITCH: i8 = -36;
    let oracle_angle = gsu_arctan(&rom, OPENING_VERTICAL_DELTA, OPENING_HORIZONTAL_RANGE);
    assert_eq!(
        atan16(OPENING_VERTICAL_DELTA, OPENING_HORIZONTAL_RANGE),
        oracle_angle
    );
    assert_eq!(
        (oracle_angle.wrapping_neg() >> 8) as u8 as i8,
        RETAIL_SIGNED_PITCH,
        "fixed-view camera negates the fine angle before taking its high byte"
    );
}
