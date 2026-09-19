//! Source conditional path statements and their shared IFNOT latch.
//! Operands here are typed values sampled by the command's world adapter.

use super::{Angle, Vector3};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BranchState {
    /// Shared across actors and callback entry, not an actor-local property.
    pub invert_next: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Predicate {
    EqualByte {
        value: u8,
        expected: u8,
    },
    EqualWord {
        value: u16,
        expected: u16,
    },
    BetweenByte {
        value: u8,
        lower: u8,
        upper: u8,
    },
    BetweenWord {
        value: u16,
        lower: u16,
        upper: u16,
    },
    NonzeroByte(u8),
    NonzeroWord(u16),
    HorizontalDistanceLess {
        position: Vector3,
        target: Vector3,
        limit: u16,
    },
    LinkedDistanceLess {
        position: Vector3,
        target: Option<Vector3>,
        limit: u16,
    },
    GroundThreshold {
        height: i16,
        offset: i16,
    },
    WithinTargetRange {
        position: Vector3,
        target: Vector3,
        limit: u16,
    },
    TargetWithinYawArc {
        position: Vector3,
        target: Vector3,
        yaw: Angle,
        radius: u8,
    },
    TargetRelativeYawBetween {
        position: Vector3,
        target: Vector3,
        target_yaw: Angle,
        lower: u8,
        upper: u8,
    },
}

impl BranchState {
    /// `$7F:A320`: repeated IFNOT sets the latch again; it does not toggle.
    pub fn invert_next_condition(&mut self) {
        self.invert_next = true;
    }

    pub fn test(&mut self, predicate: Predicate) -> bool {
        use Predicate::*;
        let value = match predicate {
            EqualByte { value, expected } => value == expected,
            EqualWord { value, expected } => value == expected,
            // Source CMP/BMI tests the sign of a bounded subtraction, not
            // widened signed order. The lower edge is excluded, upper included.
            BetweenByte {
                value,
                lower,
                upper,
            } => (lower.wrapping_sub(value) as i8) < 0 && (upper.wrapping_sub(value) as i8) >= 0,
            BetweenWord {
                value,
                lower,
                upper,
            } => (lower.wrapping_sub(value) as i16) < 0 && (upper.wrapping_sub(value) as i16) >= 0,
            HorizontalDistanceLess {
                position,
                target,
                limit,
            } => horizontal_distance(position, target) < limit,
            LinkedDistanceLess {
                position,
                target,
                limit,
            } => {
                // `$7F:8C7F`: missing link skips before checking IFNOT.
                let Some(target) = target else { return false };
                horizontal_distance(position, target) < limit
            }
            GroundThreshold { height, offset } => height.wrapping_add(offset) >= 0,
            // These predicates deliberately leave a pending IFNOT untouched.
            NonzeroByte(value) => return value != 0,
            NonzeroWord(value) => return value != 0,
            WithinTargetRange {
                position,
                target,
                limit,
            } => return within_target_range(position, target, limit),
            TargetWithinYawArc {
                position,
                target,
                yaw,
                radius,
            } => return target_within_yaw_arc(position, target, yaw, radius),
            TargetRelativeYawBetween {
                position,
                target,
                target_yaw,
                lower,
                upper,
            } => return target_relative_yaw_between(position, target, target_yaw, lower, upper),
        };
        value ^ std::mem::take(&mut self.invert_next)
    }
}

/// `$7F:8C25` supplies zero vertical displacement to the source geometry
/// length routine. This is not the approximate range used by pitch aiming.
pub fn horizontal_distance(position: Vector3, target: Vector3) -> u16 {
    super::path_math::vector_length(Vector3 {
        x: target.x.wrapping_sub(position.x),
        y: 0,
        z: target.z.wrapping_sub(position.z),
    })
}

/// `$7F:A7CE` first bounds depth, then the wrapped X/Y Manhattan distance.
/// The second distance must be nonnegative, and both limit comparisons use
/// the sign of word subtraction. This is NOT a Euclidean distance predicate.
pub fn within_target_range(position: Vector3, target: Vector3, limit: u16) -> bool {
    let depth = target.z.wrapping_sub(position.z).wrapping_abs();
    if depth.wrapping_sub(limit as i16) >= 0 {
        return false;
    }
    let horizontal = target.x.wrapping_sub(position.x).wrapping_abs();
    let vertical = target.y.wrapping_sub(position.y).wrapping_abs();
    let plane = horizontal.wrapping_add(vertical);
    plane >= 0 && plane.wrapping_sub(limit as i16) < 0
}

/// `$7F:AB7E`: bearing FROM the selected actor TO this actor, plus the
/// selected actor's yaw, in the wrapped half-open interval [lower, upper).
pub fn target_relative_yaw_between(
    position: Vector3,
    target: Vector3,
    target_yaw: Angle,
    lower: u8,
    upper: u8,
) -> bool {
    let bearing = (sf_core::aim_angle::sf2_atan16(
        position.x.wrapping_sub(target.x),
        position.z.wrapping_sub(target.z),
    ) >> u8::BITS) as u8;
    bearing.wrapping_add(target_yaw.units()).wrapping_sub(lower) < upper.wrapping_sub(lower)
}

/// `$7F:AB48`: bearing TO the target plus this actor's heading. The
/// target's own orientation is immaterial, unlike the relative-yaw interval.
pub fn target_within_yaw_arc(position: Vector3, target: Vector3, yaw: Angle, radius: u8) -> bool {
    let bearing = (sf_core::aim_angle::sf2_atan16(
        target.x.wrapping_sub(position.x),
        target.z.wrapping_sub(position.z),
    ) >> u8::BITS) as u8;
    radius.wrapping_add(yaw.units()).wrapping_add(bearing) < radius.wrapping_mul(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inversion_is_set_not_toggled_and_only_consumed_by_eligible_predicates() {
        let mut state = BranchState::default();
        state.invert_next_condition();
        state.invert_next_condition();
        assert!(state.test(Predicate::NonzeroByte(1)));
        assert!(!state.test(Predicate::NonzeroWord(0)));
        assert!(state.invert_next);
        assert!(!state.test(Predicate::EqualByte {
            value: 7,
            expected: 7
        }));
        assert!(!state.invert_next);
        assert!(state.test(Predicate::EqualWord {
            value: 65535,
            expected: 65535
        }));
    }

    #[test]
    fn missing_link_preserves_inversion_for_the_next_comparison() {
        let mut state = BranchState { invert_next: true };
        assert!(!state.test(Predicate::LinkedDistanceLess {
            position: Vector3::default(),
            target: None,
            limit: 1
        }));
        assert!(state.invert_next);
        assert!(!state.test(Predicate::LinkedDistanceLess {
            position: Vector3::default(),
            target: Some(Vector3::default()),
            limit: 1
        }));
        assert!(!state.invert_next);
    }

    #[test]
    fn numeric_intervals_exclude_lower_include_upper_and_wrap_before_sign_test() {
        let mut state = BranchState::default();
        for value in 0..=255u8 {
            let expected = matches!(value, 251..=255 | 0..=5);
            assert_eq!(
                state.test(Predicate::BetweenByte {
                    value,
                    lower: 250,
                    upper: 5
                }),
                expected
            );
        }
        for value in 0..=65535u16 {
            let expected = value > 65530 || value <= 5;
            assert_eq!(
                state.test(Predicate::BetweenWord {
                    value,
                    lower: 65530,
                    upper: 5
                }),
                expected
            );
        }
        state.invert_next_condition();
        assert!(state.test(Predicate::BetweenByte {
            value: 10,
            lower: 10,
            upper: 20
        }));
        assert!(state.test(Predicate::BetweenByte {
            value: 20,
            lower: 10,
            upper: 20
        }));
        assert!(!state.test(Predicate::BetweenByte {
            value: 21,
            lower: 10,
            upper: 20
        }));
    }

    #[test]
    fn ground_threshold_uses_wrapped_height_sum_including_zero() {
        let mut state = BranchState::default();
        assert!(state.test(Predicate::GroundThreshold {
            height: -10,
            offset: 10
        }));
        assert!(!state.test(Predicate::GroundThreshold {
            height: i16::MAX,
            offset: 1
        }));
        state.invert_next_condition();
        assert!(state.test(Predicate::GroundThreshold {
            height: i16::MAX,
            offset: 1
        }));
    }

    #[test]
    fn horizontal_distance_ignores_height_but_range_test_includes_it() {
        let origin = Vector3::default();
        let target = Vector3 { x: 80, y: 80, z: 0 };
        assert_eq!(horizontal_distance(origin, target), 80);
        assert!(!within_target_range(origin, target, 100));
        assert!(within_target_range(origin, target, 161));
        assert!(!within_target_range(
            origin,
            Vector3 { x: 0, y: 0, z: 100 },
            100
        ));
        assert!(!within_target_range(
            origin,
            Vector3 {
                x: 20_000,
                y: 20_000,
                z: 0
            },
            30_000
        ));
        let target = Vector3 {
            x: i16::MIN,
            y: 0,
            z: i16::MIN,
        };
        assert_eq!(
            horizontal_distance(origin, target),
            super::super::path_math::vector_length(target)
        );
    }

    #[test]
    fn target_relative_arc_faces_from_target_and_uses_target_yaw() {
        let actor = Vector3::default();
        let target = Vector3 { x: 100, y: 0, z: 0 };
        let yaw = Angle::from_units(64);
        assert!(target_relative_yaw_between(actor, target, yaw, 0, 1));
        assert!(!target_relative_yaw_between(actor, target, yaw, 255, 0));
        assert!(!target_relative_yaw_between(actor, target, yaw, 0, 0));
        let mut state = BranchState { invert_next: true };
        assert!(state.test(Predicate::TargetRelativeYawBetween {
            position: actor,
            target,
            target_yaw: yaw,
            lower: 0,
            upper: 1
        }));
        assert!(state.invert_next);
    }

    #[test]
    fn facing_arc_uses_target_bearing_plus_actor_heading_without_consuming_inversion() {
        let position = Vector3::default();
        let target = Vector3 { x: 100, y: 0, z: 0 };
        for yaw in 0..=255u8 {
            let expected = 32u8.wrapping_add(yaw).wrapping_add(64) < 64;
            let mut state = BranchState { invert_next: true };
            assert_eq!(
                state.test(Predicate::TargetWithinYawArc {
                    position,
                    target,
                    yaw: Angle::from_units(yaw),
                    radius: 32,
                }),
                expected
            );
            assert!(state.invert_next);
        }
        assert!(!target_within_yaw_arc(
            position,
            target,
            Angle::from_units(192),
            0
        ));
        assert!(!target_within_yaw_arc(
            position,
            target,
            Angle::from_units(192),
            128
        ));
    }
}
