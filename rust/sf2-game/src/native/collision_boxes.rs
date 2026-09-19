//! Object-contact box arithmetic (`$7F:4100..43D4`, `$7F:4538..48F1`).
//! These are not the downward-contact plane/polygon profiles.

use super::{Angle, Rotation, Vector3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterRotation {
    /// All three offsets are signed words; no scale is applied.
    None,
    /// Rotated axes use signed LOW bytes and the supplied scale. The third
    /// axis retains its full signed word and is not scaled.
    Roll,
    Yaw,
    Pitch,
    Full,
}

impl CenterRotation {
    /// Decode the authored rotation selector at the data boundary. The
    /// source tests these bits in priority order, not as independent axes.
    pub fn from_authored_flags(flags: u8) -> Self {
        if flags & 0x10 != 0 {
            Self::Roll
        } else if flags & 0x20 != 0 {
            Self::Yaw
        } else if flags & 0x40 != 0 {
            Self::Pitch
        } else if flags & 0x80 != 0 {
            Self::Full
        } else {
            Self::None
        }
    }
}

fn scale_byte(value: i8, shift: u32) -> i16 {
    i16::from(value).checked_shl(shift).unwrap_or(0)
}

/// Unlike weapon muzzle offsets, EACH zero-angle rotation is bypassed. That
/// matters because the byte cosine table's identity coefficient is 127/128.
pub fn center(
    position: Vector3,
    rotation: Rotation,
    offset: Vector3,
    mode: CenterRotation,
    shift: u32,
) -> Vector3 {
    use sf_core::snes_trig::{rotate_8xz, rotate_8yx, rotate_8yz};
    let (mut x, mut y, mut z) = (offset.x as i8, offset.y as i8, offset.z as i8);
    if matches!(mode, CenterRotation::Roll | CenterRotation::Full) && rotation.roll != Angle::ZERO {
        let rotated = rotate_8yx(rotation.roll.units(), x, y);
        (x, y) = (rotated.0 as i8, rotated.1 as i8);
    }
    if matches!(mode, CenterRotation::Pitch | CenterRotation::Full) && rotation.pitch != Angle::ZERO
    {
        let rotated = rotate_8yz(rotation.pitch.units(), y, z);
        (y, z) = (rotated.0 as i8, rotated.1 as i8);
    }
    if matches!(mode, CenterRotation::Yaw | CenterRotation::Full) && rotation.yaw != Angle::ZERO {
        let rotated = rotate_8xz(rotation.yaw.units(), x, z);
        (x, z) = (rotated.0 as i8, rotated.1 as i8);
    }
    let transformed = match mode {
        CenterRotation::None => offset,
        CenterRotation::Roll => Vector3 {
            x: scale_byte(x, shift),
            y: scale_byte(y, shift),
            z: offset.z,
        },
        CenterRotation::Yaw => Vector3 {
            x: scale_byte(x, shift),
            y: offset.y,
            z: scale_byte(z, shift),
        },
        CenterRotation::Pitch => Vector3 {
            x: offset.x,
            y: scale_byte(y, shift),
            z: scale_byte(z, shift),
        },
        CenterRotation::Full => Vector3 {
            x: scale_byte(x, shift),
            y: scale_byte(y, shift),
            z: scale_byte(z, shift),
        },
    };
    Vector3 {
        x: position.x.wrapping_add(transformed.x),
        y: position.y.wrapping_add(transformed.y),
        z: position.z.wrapping_add(transformed.z),
    }
}

/// The source takes a word-sized absolute difference, subtracts the wrapped
/// extent sum, and tests its sign. It does not widen either intermediate.
pub fn axis_overlaps(first: i16, second: i16, first_extent: u16, second_extent: u16) -> bool {
    let distance = first.wrapping_sub(second).wrapping_abs();
    distance.wrapping_sub(first_extent.wrapping_add(second_extent) as i16) < 0
}

/// Contact-box animation uses a MASK, unlike downward-surface animation's
/// out-of-range fallback. A high-bit actor frame selects an explicit frame;
/// otherwise the shared clock is used. Zero variants means a single record.
pub fn animation_variant(variants: u8, actor_frame: u8, strategy_clock: u8) -> usize {
    if variants == 0 {
        return 0;
    }
    let frame = if actor_frame & 0x80 != 0 {
        actor_frame
    } else {
        strategy_clock
    };
    usize::from(frame & variants.wrapping_sub(1) & 0x7F)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_angles_preserve_bytes_without_identity_table_shrinkage() {
        let offset = Vector3 {
            x: 25,
            y: -15,
            z: -40,
        };
        assert_eq!(
            center(
                Vector3::default(),
                Rotation::default(),
                offset,
                CenterRotation::Full,
                2
            ),
            Vector3 {
                x: 100,
                y: -60,
                z: -160
            }
        );
    }

    #[test]
    fn each_zero_axis_bypasses_independently_between_nonzero_rotations() {
        let offset = Vector3 {
            x: 100,
            y: 20,
            z: -30,
        };
        let rotation = Rotation {
            roll: Angle::from_units(64),
            ..Rotation::default()
        };
        assert_eq!(
            center(
                Vector3::default(),
                rotation,
                offset,
                CenterRotation::Full,
                0
            ),
            Vector3 {
                x: 19,
                y: -99,
                z: -30
            }
        );
        let rotation = Rotation {
            pitch: Angle::from_units(64),
            ..Rotation::default()
        };
        assert_eq!(
            center(
                Vector3::default(),
                rotation,
                offset,
                CenterRotation::Full,
                0
            ),
            Vector3 {
                x: 100,
                y: -29,
                z: -19
            }
        );
        let rotation = Rotation {
            yaw: Angle::from_units(64),
            ..Rotation::default()
        };
        assert_eq!(
            center(
                Vector3::default(),
                rotation,
                offset,
                CenterRotation::Full,
                0
            ),
            Vector3 {
                x: 29,
                y: 20,
                z: 99
            }
        );
    }

    #[test]
    fn partial_rotation_scales_only_its_two_byte_axes() {
        let offset = Vector3 {
            x: 257,
            y: 514,
            z: 771,
        };
        let origin = Vector3::default();
        let rotation = Rotation::default();
        assert_eq!(
            center(origin, rotation, offset, CenterRotation::None, 2),
            offset
        );
        assert_eq!(
            center(origin, rotation, offset, CenterRotation::Roll, 2),
            Vector3 { x: 4, y: 8, z: 771 }
        );
        assert_eq!(
            center(origin, rotation, offset, CenterRotation::Pitch, 2),
            Vector3 {
                x: 257,
                y: 8,
                z: 12
            }
        );
        assert_eq!(
            center(origin, rotation, offset, CenterRotation::Yaw, 2),
            Vector3 {
                x: 4,
                y: 514,
                z: 12
            }
        );
    }

    #[test]
    fn shifts_and_world_additions_retain_word_overflow() {
        let offset = Vector3 { x: 1, y: -1, z: 3 };
        let origin = Vector3 {
            x: i16::MAX,
            y: i16::MIN,
            z: 0,
        };
        assert_eq!(
            center(origin, Rotation::default(), offset, CenterRotation::Full, 0),
            Vector3 {
                x: i16::MIN,
                y: i16::MAX,
                z: 3
            }
        );
        assert_eq!(
            center(
                Vector3::default(),
                Rotation::default(),
                offset,
                CenterRotation::Full,
                15
            ),
            Vector3 {
                x: i16::MIN,
                y: i16::MIN,
                z: i16::MIN
            }
        );
        assert_eq!(
            center(
                origin,
                Rotation::default(),
                offset,
                CenterRotation::Full,
                16
            ),
            origin
        );
    }

    #[test]
    fn touching_edges_do_not_overlap_and_half_turn_difference_retains_source_sign() {
        assert!(axis_overlaps(0, 19, 10, 10));
        assert!(!axis_overlaps(0, 20, 10, 10));
        assert!(!axis_overlaps(0, -20, 10, 10));
        assert!(axis_overlaps(i16::MAX, i16::MIN, 1, 1));
        assert!(axis_overlaps(0, i16::MIN, 0, 0));
        assert!(!axis_overlaps(0, i16::MIN, 1, 0));
        assert!(axis_overlaps(0, 0, 40_000, 40_000));
        assert!(!axis_overlaps(0, 0, 40_000, 0));
    }

    #[test]
    fn all_rotation_selector_combinations_use_source_priority() {
        for flags in 0..=u8::MAX {
            let expected = match (flags >> 4).trailing_zeros() {
                0 => CenterRotation::Roll,
                1 => CenterRotation::Yaw,
                2 => CenterRotation::Pitch,
                3 => CenterRotation::Full,
                _ => CenterRotation::None,
            };
            assert_eq!(CenterRotation::from_authored_flags(flags), expected);
        }
    }

    #[test]
    fn animation_selection_masks_instead_of_modulo_or_clamp() {
        assert_eq!(animation_variant(0, 255, 255), 0);
        assert_eq!(animation_variant(4, 1, 6), 2);
        assert_eq!(animation_variant(4, 0x81, 6), 1);
        assert_eq!(animation_variant(3, 0, 3), 2);
        assert_eq!(animation_variant(3, 0x81, 2), 0);
        for count in 1..=u8::MAX {
            for clock in 0..=u8::MAX {
                assert!(animation_variant(count, 0, clock) < usize::from(count));
            }
        }
    }
}
