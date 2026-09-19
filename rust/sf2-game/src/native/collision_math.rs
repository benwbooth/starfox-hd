//! Integer collision geometry transcribed from the SF2 geometry leaves.
//!
//! These are ordinary Rust geometry operations, with the source's truncation
//! and wrapping retained. They neither execute instructions nor use an
//! addressed scratch buffer.

use super::Angle;

const COEFFICIENT_SHIFT: u32 = 8;
const WORD_BITS: u32 = 16;
const PROBE_PRECISION_SHIFT: u32 = 2;

/// Rotate a world probe into a collider's local yaw frame (`$01:FD62`,
/// `$01:FE74`). The host bypasses the kernel at exactly zero yaw.
/// Input words are multiplied by four *before* the signed wide products;
/// the products are combined before their high-word/half truncation.
pub fn local_probe(yaw: Angle, x: i16, z: i16) -> (i16, i16) {
    if yaw == Angle::ZERO {
        return (x, z);
    }
    let angle = usize::from(yaw.units().wrapping_neg());
    let sin = i16::from(sf_core::snes_trig::SINTAB[angle]) << COEFFICIENT_SHIFT;
    let cos = i16::from(sf_core::snes_trig::COSTAB[angle]) << COEFFICIENT_SHIFT;
    let x = i32::from(x.wrapping_shl(PROBE_PRECISION_SHIFT));
    let z = i32::from(z.wrapping_shl(PROBE_PRECISION_SHIFT));
    let local_x = (x * i32::from(cos)).wrapping_sub(z * i32::from(sin));
    let local_z = (x * i32::from(sin)).wrapping_add(z * i32::from(cos));
    (
        (local_x >> (WORD_BITS + 1)) as i16,
        (local_z >> (WORD_BITS + 1)) as i16,
    )
}

/// Closed, clockwise convex footprint (`$01:FCD7`). Edges are inclusive.
/// The source stores the closing vertex explicitly; do not invent a final
/// edge that is absent from the authored list.
pub fn polygon_contains(vertices: &[[i8; 2]], scale: u8, x: i16, z: i16) -> bool {
    if vertices.len() < 2 {
        return false;
    }
    let scale_component = |value: i8| {
        if u32::from(scale) >= WORD_BITS {
            0
        } else {
            i16::from(value).wrapping_shl(u32::from(scale))
        }
    };
    for edge in vertices.windows(2) {
        let [start_x, start_z] = edge[0].map(scale_component);
        let [end_x, end_z] = edge[1].map(scale_component);
        let edge_x = end_x.wrapping_sub(start_x);
        let edge_z = end_z.wrapping_sub(start_z);
        let offset_x = x.wrapping_sub(start_x);
        let offset_z = z.wrapping_sub(start_z);
        let cross = (i32::from(offset_x) * i32::from(edge_z))
            .wrapping_sub(i32::from(edge_x) * i32::from(offset_z));
        if cross < 0 {
            return false;
        }
    }
    true
}

/// Signed division leaf (`$01:FA8A`) for a word numerator followed by a zero
/// fractional word. Restoring division has a word-sized remainder, including
/// overflow; division by zero produces the source's all-one quotient.
fn signed_fractional_quotient(numerator: i16, denominator: i16) -> i16 {
    let negative = (numerator < 0) != (denominator < 0);
    let mut remainder = numerator.unsigned_abs();
    let denominator = denominator.unsigned_abs();
    let mut quotient = 0_u16;
    for _ in 0..WORD_BITS {
        remainder = (remainder << 1) | (quotient >> (WORD_BITS - 1));
        quotient <<= 1;
        if remainder >= denominator {
            remainder = remainder.wrapping_sub(denominator);
            quotient |= 1;
        }
    }
    if negative {
        quotient.wrapping_neg() as i16
    } else {
        quotient as i16
    }
}

/// Project a local probe onto an authored collision plane (`$01:FA31`).
/// Each signed product is truncated separately and doubled afterward. The
/// final signed fractional quotient is halved with arithmetic rounding.
pub fn plane_height(normal: [i8; 3], offset: i16, x: i16, z: i16) -> i16 {
    let [normal_x, normal_y, normal_z] = normal.map(|axis| i16::from(axis) << COEFFICIENT_SHIFT);
    let product_x = ((i32::from(x) * i32::from(normal_x)) >> WORD_BITS) as i16;
    let product_z = ((i32::from(z) * i32::from(normal_z)) >> WORD_BITS) as i16;
    let numerator = product_x
        .wrapping_add(product_x)
        .wrapping_add(product_z.wrapping_add(product_z))
        .wrapping_sub(offset);
    signed_fractional_quotient(numerator, normal_y) >> 1
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: [[i8; 2]; 5] = [[-1, -1], [-1, 1], [1, 1], [1, -1], [-1, -1]];

    #[test]
    fn clockwise_polygon_includes_edges_and_rejects_outside_points() {
        for x in -5..=5 {
            for z in -5..=5 {
                assert_eq!(
                    polygon_contains(&SQUARE, 2, x, z),
                    x.abs() <= 4 && z.abs() <= 4
                );
            }
        }
        let reversed: Vec<_> = SQUARE.into_iter().rev().collect();
        assert!(!polygon_contains(&reversed, 2, 0, 0));
    }

    #[test]
    fn polygon_scale_keeps_word_wrap_and_does_not_mask_the_shift_count() {
        assert!(polygon_contains(&SQUARE, 16, 200, -300));
        assert!(polygon_contains(&SQUARE, 255, -100, 123));
        assert!(!polygon_contains(&SQUARE, 0, 200, -300));
        assert!(!polygon_contains(&[], 0, 0, 0));
    }

    #[test]
    fn yaw_zero_bypasses_quantization_and_quarter_turns_keep_signed_rounding() {
        assert_eq!(
            local_probe(Angle::ZERO, i16::MAX, i16::MIN),
            (i16::MAX, i16::MIN)
        );
        assert_eq!(local_probe(Angle::from_units(64), 100, 200), (198, -100));
        assert_eq!(local_probe(Angle::from_units(128), 100, 200), (-100, -199));
        assert_eq!(local_probe(Angle::from_units(64), 8192, 0), (0, 8128));
    }

    #[test]
    fn fractional_division_matches_exact_arithmetic_before_remainder_overflow() {
        for denominator in [-32_768_i16, -32_512, -16_384, -256, 256, 16_384, 32_512] {
            for numerator in -200_i16..=200 {
                assert_eq!(
                    signed_fractional_quotient(numerator, denominator),
                    ((i64::from(numerator) << WORD_BITS) / i64::from(denominator)) as i16
                );
            }
        }
        assert_eq!(signed_fractional_quotient(1, 0), -1);
        assert_eq!(signed_fractional_quotient(-1, 0), 1);
    }

    #[test]
    fn plane_projection_truncates_negative_products_before_doubling() {
        assert_eq!(plane_height([0, 64, 0], 128, 0, 0), -256);
        assert_eq!(plane_height([0, -64, 0], 128, 0, 0), 256);
        assert_eq!(plane_height([64, 64, 0], 0, 1, 0), 0);
        assert_eq!(plane_height([64, 64, 0], 0, -1, 0), -4);
    }
}
