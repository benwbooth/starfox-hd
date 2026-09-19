//! SF2 path geometry arithmetic, transcribed from the original assembly.
//!
//! These routines deliberately retain the source's bounded intermediate
//! arithmetic. Host square root, wide dot products, or floating-point
//! normalization are not interchangeable at the word-wrap boundaries.

use super::object::Vector3;

const WORD_BITS: u32 = 16;
const RADICAND_PAIR_SHIFT: u32 = 30;
const NORMALIZED_UNIT: u32 = 32_767;

/// The sixteen digit-pair iterations at `$01:FA64..FA89`.
///
/// The trial and remainder are words. For large inputs this is observably
/// different from an unbounded integer square root; widening the remainder
/// would silently change source behavior.
pub fn word_remainder_square_root(mut value: u32) -> u16 {
    let mut root = 0_u16;
    let mut remainder = 0_u16;
    for _ in 0..WORD_BITS {
        remainder = remainder.wrapping_shl(2) | (value >> RADICAND_PAIR_SHIFT) as u16;
        value = value.wrapping_shl(2);
        root = root.wrapping_add(root);
        let trial = root.wrapping_add(root);
        if remainder > trial {
            remainder = remainder.wrapping_sub(trial).wrapping_sub(1);
            root = root.wrapping_add(1);
        }
    }
    root
}

/// Wrapped world-space vector length (`$01:FB72..FBAA`).
pub fn vector_length(delta: Vector3) -> u16 {
    let square = [delta.x, delta.y, delta.z]
        .into_iter()
        .fold(0_u32, |sum, component| {
            let component = i32::from(component);
            sum.wrapping_add((component * component) as u32)
        });
    word_remainder_square_root(square)
}

/// `$01:FBAB..FBD1`: division used by vector normalization. Its sign-bit
/// preconditioning and word-sized restoring remainder are part of the
/// algorithm, not saturation or ordinary host-language unsigned division.
fn normalization_quotient(numerator: u32, mut denominator: u16) -> u16 {
    let high = (numerator >> WORD_BITS) as u16;
    let mut quotient = numerator as u16;
    if (high as i16) < 0 || (denominator as i16) < 0 {
        quotient = (quotient >> 1) | ((denominator & 2) << 14);
        denominator >>= 2;
    }
    let mut remainder = high.wrapping_shl(1) | (quotient >> (WORD_BITS - 1));
    quotient = quotient.wrapping_shl(1);
    for _ in 0..WORD_BITS {
        let take = remainder >= denominator;
        if take {
            remainder = remainder.wrapping_sub(denominator);
        }
        let next_bit = quotient >> (WORD_BITS - 1);
        quotient = quotient.wrapping_shl(1) | u16::from(take);
        remainder = remainder.wrapping_shl(1) | next_bit;
    }
    quotient
}

/// Change the radius around the selected target (`$7F:ADC7` calling
/// `$01:FB72`, `$01:FBD2` and `$01:FC5A`). Positive amounts contract.
///
/// The source computes a quantized unit direction, wraps the new radius to a
/// word, doubles that word, then uses three separately truncated signed high
/// products. In particular a radius below `amount` reverses the direction;
/// it must not become a large positive host integer.
pub fn change_radius(position: Vector3, target: Vector3, amount: i16) -> Vector3 {
    let delta = Vector3 {
        x: position.x.wrapping_sub(target.x),
        y: position.y.wrapping_sub(target.y),
        z: position.z.wrapping_sub(target.z),
    };
    let radius = vector_length(delta);
    if radius == 0 {
        return position;
    }
    let precision = u16::BITS - 1 - radius.leading_zeros();
    let reciprocal = normalization_quotient(NORMALIZED_UNIT << precision, radius) as i16;
    let doubled_radius = (radius as i16).wrapping_sub(amount).wrapping_mul(2);
    let component = |delta: i16, target: i16| {
        let direction = ((i32::from(delta) * i32::from(reciprocal)) >> precision) as i16;
        let offset = (i32::from(direction) * i32::from(doubled_radius)) >> WORD_BITS;
        target.wrapping_add(offset as i16)
    };
    Vector3 {
        x: component(delta.x, target.x),
        y: component(delta.y, target.y),
        z: component(delta.z, target.z),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_root_preserves_the_source_word_remainder_boundaries() {
        for (input, expected) in [
            (0, 0),
            (1, 1),
            (2, 1),
            (3, 1),
            (4, 2),
            (1_000_000, 1_000),
            (0x3FFF_FFFF, 32_766),
            (0x4000_0000, 32_768),
            (0x7FFF_FFFF, 46_340),
            (0x8000_0000, 46_340),
            (0xBFFF_FFFF, 56_752),
            (0xC000_0000, 56_752),
            (0xFFFF_FFFF, 65_532),
        ] {
            assert_eq!(word_remainder_square_root(input), expected, "{input:08x}");
        }
    }

    #[test]
    fn square_root_agrees_with_exact_squares_before_remainder_overflow() {
        for root in 1..=16_383_u32 {
            let square = root * root;
            assert_eq!(word_remainder_square_root(square), root as u16);
            assert_eq!(word_remainder_square_root(square - 1), root as u16 - 1);
        }
    }

    #[test]
    fn normal_reciprocals_agree_with_unsigned_division_before_sign_preconditioning() {
        for radius in 1..=i16::MAX as u16 {
            let precision = u16::BITS - 1 - radius.leading_zeros();
            let numerator = NORMALIZED_UNIT << precision;
            assert_eq!(
                normalization_quotient(numerator, radius),
                (numerator / u32::from(radius)) as u16
            );
        }
    }

    #[test]
    fn contracting_past_zero_reverses_instead_of_wrapping_positive() {
        let target = Vector3::default();
        assert_eq!(
            change_radius(Vector3 { x: 1, ..target }, target, 127),
            Vector3 { x: -126, ..target }
        );
        assert_eq!(
            change_radius(Vector3 { x: -1, ..target }, target, 127),
            Vector3 { x: 125, ..target }
        );
        assert_eq!(change_radius(target, target, 127), target);
    }

    #[test]
    fn large_radius_doubles_as_a_signed_word_before_the_product() {
        let target = Vector3::default();
        assert_eq!(
            change_radius(
                Vector3 {
                    x: 20_000,
                    ..target
                },
                target,
                127
            ),
            Vector3 {
                x: -12_895,
                ..target
            }
        );
    }
}
