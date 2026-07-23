//! ROM aim-angle + XZ distance leaves (`Yanglexy` / `Xanglexy` / `xzdiffs*`).
//!
//! Angles still use the C-port f32 `atan2`→u8 path (GSU `arctan16` is ±1 LSB
//! on off-axis; see `sf-oracle` `gsu_arctan` / `fuzz_pure_fns2`). Distance
//! metrics match the ROM exactly:
//! - [`xzdiffs`] — scaled Euclidean (`xzdiffs_l` / `xzdiffs_diffabs_l`)
//! - [`xzdiffs_abs_manhattan`] — `|dx|+|dz|` (`xzdiffs_abs_l` rangexz)

const SF2_QUARTER_TURN_FINE: u16 = 16_384;
const SF2_HALF_TURN_FINE: u16 = 32_768;
const SF2_DIAGONAL_FINE: u16 = 8_192;
const SF2_RATIO_FRACTION_BITS: u32 = 14;
const SF2_RATIO_TABLE_SHIFT: u32 = 5;
const SF2_RATIO_TABLE_BYTE_MASK: u16 = 0xFFFE;
const SF2_RATIO_MAXIMUM: u16 = 0x7FFF;

/// Quantized first-octant arctangent curve used by Star Fox 2. Values are
/// fine angles where 65,536 units make a full turn. Keeping the authored
/// integer curve avoids the one-unit drift of a floating-point approximation.
static SF2_ARCTANGENT_CURVE: [u16; 256] = [
    0, 32, 80, 112, 160, 192, 240, 272, 320, 352, 400, 432, 480, 528, 560, 608, 640, 688, 720, 768,
    800, 848, 880, 928, 960, 1_008, 1_040, 1_088, 1_136, 1_168, 1_216, 1_248, 1_296, 1_328, 1_376,
    1_408, 1_456, 1_488, 1_536, 1_568, 1_616, 1_648, 1_696, 1_728, 1_760, 1_808, 1_840, 1_888,
    1_920, 1_968, 2_000, 2_048, 2_080, 2_128, 2_160, 2_192, 2_240, 2_272, 2_320, 2_352, 2_400,
    2_432, 2_464, 2_512, 2_544, 2_592, 2_624, 2_656, 2_704, 2_736, 2_768, 2_816, 2_848, 2_896,
    2_928, 2_960, 3_008, 3_040, 3_072, 3_120, 3_152, 3_184, 3_232, 3_264, 3_296, 3_328, 3_376,
    3_408, 3_440, 3_488, 3_520, 3_552, 3_584, 3_632, 3_664, 3_696, 3_728, 3_776, 3_808, 3_840,
    3_872, 3_904, 3_952, 3_984, 4_016, 4_048, 4_080, 4_128, 4_160, 4_192, 4_224, 4_256, 4_288,
    4_320, 4_368, 4_400, 4_432, 4_464, 4_496, 4_528, 4_560, 4_592, 4_624, 4_656, 4_704, 4_736,
    4_768, 4_800, 4_832, 4_864, 4_896, 4_928, 4_960, 4_992, 5_024, 5_056, 5_088, 5_120, 5_152,
    5_184, 5_216, 5_248, 5_280, 5_312, 5_344, 5_360, 5_392, 5_424, 5_456, 5_488, 5_520, 5_552,
    5_584, 5_616, 5_648, 5_664, 5_696, 5_728, 5_760, 5_792, 5_824, 5_840, 5_872, 5_904, 5_936,
    5_968, 6_000, 6_016, 6_048, 6_080, 6_112, 6_128, 6_160, 6_192, 6_224, 6_240, 6_272, 6_304,
    6_336, 6_352, 6_384, 6_416, 6_432, 6_464, 6_496, 6_512, 6_544, 6_576, 6_592, 6_624, 6_656,
    6_672, 6_704, 6_736, 6_752, 6_784, 6_800, 6_832, 6_864, 6_880, 6_912, 6_928, 6_960, 6_992,
    7_008, 7_040, 7_056, 7_088, 7_104, 7_136, 7_152, 7_184, 7_200, 7_232, 7_248, 7_280, 7_296,
    7_328, 7_344, 7_376, 7_392, 7_424, 7_440, 7_472, 7_488, 7_520, 7_536, 7_552, 7_584, 7_600,
    7_632, 7_648, 7_664, 7_696, 7_712, 7_744, 7_760, 7_776, 7_808, 7_824, 7_840, 7_872, 7_888,
    7_920, 7_936, 7_952, 7_984, 8_000, 8_016, 8_032, 8_064, 8_080, 8_096, 8_128, 8_144, 8_160,
];

#[inline]
fn sf2_abs_word(value: u16) -> u16 {
    if value as i16 >= 0 {
        value
    } else {
        value.wrapping_neg()
    }
}

/// Star Fox 2's table-quantized signed arctangent. Inputs are ordinary signed
/// world-space deltas; the result is a fine angle with 65,536 units per turn.
pub fn sf2_atan16(x: i16, y: i16) -> u16 {
    let original_x = x as u16;
    let original_y = y as u16;
    let mut angle = if original_y == 0 {
        SF2_QUARTER_TURN_FINE
    } else {
        let mut numerator = sf2_abs_word(original_x);
        let mut denominator = sf2_abs_word(original_y);
        if numerator == denominator {
            SF2_DIAGONAL_FINE
        } else {
            let swapped = numerator.wrapping_sub(denominator) as i16 >= 0;
            if swapped {
                std::mem::swap(&mut numerator, &mut denominator);
            }
            let ratio = if denominator == 0 {
                SF2_RATIO_MAXIMUM
            } else {
                (((u32::from(numerator)) << SF2_RATIO_FRACTION_BITS) / u32::from(denominator))
                    as u16
            };
            let byte_index = (ratio >> SF2_RATIO_TABLE_SHIFT) & SF2_RATIO_TABLE_BYTE_MASK;
            let sample = SF2_ARCTANGENT_CURVE[usize::from(byte_index / 2)];
            if swapped {
                SF2_QUARTER_TURN_FINE.wrapping_sub(sample)
            } else {
                sample
            }
        }
    };
    if ((original_x ^ original_y) as i16) < 0 {
        angle = angle.wrapping_neg();
    }
    if (original_y as i16) < 0 {
        angle = angle.wrapping_add(SF2_HALF_TURN_FINE);
    }
    angle
}

/// Exact Star Fox 2 byte yaw toward a world-space X/Z delta.
#[inline]
pub fn sf2_yaw_to_target(dx: i16, dz: i16) -> u8 {
    ((sf2_atan16(dx, dz) >> 8) as u8).wrapping_neg()
}

/// Exact Star Fox 2 byte pitch toward a world-space vertical/range delta.
#[inline]
pub fn sf2_pitch_to_target(dy: i16, distance: i16) -> u8 {
    (sf2_atan16(dy, distance) >> 8) as u8
}

/// ROM `xzdiffs_l` / `xzdiffs_diffabs_l` (STRATROU.ASM:1796): scaled Euclidean.
#[inline]
pub fn xzdiffs(dx: i16, dz: i16) -> i16 {
    let mut x1 = dx;
    if x1 < 0 {
        x1 = x1.wrapping_neg();
    }
    let mut y1 = dz;
    if y1 < 0 {
        y1 = y1.wrapping_neg();
    }
    x1 >>= 1;
    y1 >>= 1;
    let rangexz = (y1.wrapping_add(x1)).wrapping_shl(1);
    let m = if y1 < x1 { x1 } else { y1 };
    let t = m.wrapping_add(rangexz);
    let acc = (t >> 1).wrapping_add(t.wrapping_shl(2));
    ((acc >> 1) >> 1) >> 1
}

/// ROM `xzdiffs_abs_l` rangexz (STRATROU.ASM:1488): Manhattan `|dx|+|dz|`.
#[inline]
pub fn xzdiffs_abs_manhattan(dx: i16, dz: i16) -> i16 {
    let mut ax = dx;
    if ax < 0 {
        ax = ax.wrapping_neg();
    }
    let mut az = dz;
    if az < 0 {
        az = az.wrapping_neg();
    }
    ax.wrapping_add(az)
}

/// Map `atan2(opp, adj)` to a SNES 8-bit angle (C/x86 float→uint8 truncation).
#[inline]
pub fn atan2_to_u8(opp: f32, adj: f32) -> u8 {
    let mut angle = opp.atan2(adj);
    if angle < 0.0 {
        angle += 2.0f32 * 3.141_592_65_f32;
    }
    ((angle * (256.0f32 / (2.0f32 * 3.141_592_65_f32))) as i32) as u8
}

/// ROM `Yanglexy_l` / `anglexy_l` / `anglexy_abs_l`: yaw = atan2(dx, dz).
#[inline]
pub fn yanglexy(dx: i16, dz: i16) -> u8 {
    atan2_to_u8(dx as f32, dz as f32)
}

/// ROM `s_obj2WP_angle` / `s_obj2obj_angle` yaw store: `nega(Yanglexy)`.
#[inline]
pub fn yanglexy_nega(dx: i16, dz: i16) -> u8 {
    yanglexy(dx, dz).wrapping_neg()
}

/// ROM `Xanglexy_l`: elevation = atan2(dy, `xzdiffs_l`).
#[inline]
pub fn xanglexy(dy: i16, dx: i16, dz: i16) -> u8 {
    atan2_to_u8(dy as f32, xzdiffs(dx, dz) as f32)
}

/// ROM `Xanglexabs_l`: elevation = atan2(dy, Manhattan from `xzdiffs_abs_l`).
#[inline]
pub fn xanglexabs(dy: i16, dx: i16, dz: i16) -> u8 {
    atan2_to_u8(dy as f32, xzdiffs_abs_manhattan(dx, dz) as f32)
}

/// SF2's fixed-point X/Z length approximation used before calculating a
/// vertical aim angle. All intermediate values retain the original signed
/// 16-bit wrapping behavior, but the result is an ordinary world-space
/// distance rather than an addressed runtime value.
pub fn sf2_xz_angle_distance(dx: i16, dz: i16) -> i16 {
    #[inline]
    fn abs_wrapping(value: i16) -> u16 {
        if value < 0 {
            value.wrapping_neg() as u16
        } else {
            value as u16
        }
    }

    #[inline]
    fn signed_half(value: u16) -> u16 {
        ((value as i16) >> 1) as u16
    }

    let x = signed_half(abs_wrapping(dx));
    let z = signed_half(abs_wrapping(dz));
    let sum = z.wrapping_add(x).wrapping_shl(1);
    let maximum = if (z.wrapping_sub(x) as i16) < 0 { x } else { z };
    let total = maximum.wrapping_add(sum);
    let value = signed_half(total).wrapping_add(total);
    signed_half(signed_half(value)) as i16
}
