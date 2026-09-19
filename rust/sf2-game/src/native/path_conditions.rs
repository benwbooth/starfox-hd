//! Source conditional path statements and their shared IFNOT latch.
//! Operands here are typed values sampled by the command's world adapter.

use super::{Angle, ObjectId, ObjectStore, Vector3};

/// Actor/world comparisons retain their operands until dispatch, so a prior
/// callback or command can move or reorient either actor before the test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialCondition {
    SelectedDistanceLess(u16),
    LinkedDistanceLess(u16),
    GroundThreshold(i16),
    WithinSelectedRange(u16),
    SelectedWithinYawArc(u8),
    SelectedRelativeYawBetween { lower: u8, upper: u8 },
    SelectedAbove,
    SelectedAtOrBelow,
    NegativeSelectedPlane(super::path_control::PlaneAxis),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionError {
    MissingActor(ObjectId),
    MissingSelected,
}

pub fn sample_spatial(
    objects: &ObjectStore,
    owner: ObjectId,
    selected: Option<ObjectId>,
    condition: SpatialCondition,
) -> Result<Predicate, ConditionError> {
    let actor = objects
        .get(owner)
        .ok_or(ConditionError::MissingActor(owner))?;
    let position = actor.base.position;
    // Resolve only predicates that actually need selection. In particular,
    // absent links must not accidentally require a selected actor.
    let target = || {
        let id = selected.ok_or(ConditionError::MissingSelected)?;
        objects.get(id).ok_or(ConditionError::MissingActor(id))
    };
    Ok(match condition {
        SpatialCondition::SelectedDistanceLess(limit) => Predicate::HorizontalDistanceLess {
            position,
            target: target()?.base.position,
            limit,
        },
        SpatialCondition::LinkedDistanceLess(limit) => Predicate::LinkedDistanceLess {
            position,
            target: actor
                .base
                .attachment
                .map(|id| {
                    objects
                        .get(id)
                        .map(|actor| actor.base.position)
                        .ok_or(ConditionError::MissingActor(id))
                })
                .transpose()?,
            limit,
        },
        SpatialCondition::GroundThreshold(offset) => Predicate::GroundThreshold {
            height: position.y,
            offset,
        },
        SpatialCondition::WithinSelectedRange(limit) => Predicate::WithinTargetRange {
            position,
            target: target()?.base.position,
            limit,
        },
        SpatialCondition::SelectedWithinYawArc(radius) => Predicate::TargetWithinYawArc {
            position,
            target: target()?.base.position,
            yaw: actor.base.yaw,
            radius,
        },
        SpatialCondition::SelectedRelativeYawBetween { lower, upper } => {
            let target = target()?;
            Predicate::TargetRelativeYawBetween {
                position,
                target: target.base.position,
                target_yaw: target.base.yaw,
                lower,
                upper,
            }
        }
        SpatialCondition::SelectedAbove => Predicate::TargetAbove {
            height: position.y,
            target_height: target()?.base.position.y,
        },
        SpatialCondition::SelectedAtOrBelow => Predicate::TargetAtOrBelow {
            height: position.y,
            target_height: target()?.base.position.y,
        },
        SpatialCondition::NegativeSelectedPlane(axis) => {
            let target = target()?;
            Predicate::NegativeSelectedPlane {
                position,
                target_position: target.base.position,
                target_rotation: super::Rotation {
                    pitch: target.base.pitch,
                    yaw: target.base.yaw,
                    roll: target.base.roll,
                },
                axis,
            }
        }
    })
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BranchState {
    /// Shared across actors and callback entry, not an actor-local property.
    pub invert_next: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Predicate {
    /// Both bytes belong to the selected actor's auxiliary record. The
    /// current path actor contributes neither byte (`$7F:B9BC..B9F3`).
    SelectedAuxiliaryContinuation {
        mode: u8,
        action_flags: u8,
    },
    NegativeSelectedPlane {
        position: Vector3,
        target_position: Vector3,
        target_rotation: super::Rotation,
        axis: super::path_control::PlaneAxis,
    },
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
    ZeroByte(u8),
    ZeroWord(u16),
    /// Variable comparison reads the first operand, then tests second-first.
    SecondByteLess {
        first: u8,
        second: u8,
    },
    SecondWordLess {
        first: u16,
        second: u16,
    },
    AnyByteBitsSet {
        value: u8,
        mask: u8,
    },
    AnyWordBitsSet {
        value: u16,
        mask: u16,
    },
    TargetAbove {
        height: i16,
        target_height: i16,
    },
    TargetAtOrBelow {
        height: i16,
        target_height: i16,
    },
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
            NegativeSelectedPlane {
                position,
                target_position,
                target_rotation,
                axis,
            } => {
                // Both handlers orient the plane with the selected actor,
                // then project this actor minus the selected position.
                return super::path_control::plane_projection(
                    target_position,
                    target_rotation,
                    position,
                    axis,
                ) < 0;
            }
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
            SelectedAuxiliaryContinuation { mode, action_flags } => {
                return selected_auxiliary_continuation(mode, action_flags);
            }
            NonzeroByte(value) => return value != 0,
            NonzeroWord(value) => return value != 0,
            ZeroByte(value) => return value == 0,
            ZeroWord(value) => return value == 0,
            SecondByteLess { first, second } => return (second.wrapping_sub(first) as i8) < 0,
            SecondWordLess { first, second } => return (second.wrapping_sub(first) as i16) < 0,
            AnyByteBitsSet { value, mask } => return value & mask != 0,
            AnyWordBitsSet { value, mask } => return value & mask != 0,
            TargetAbove {
                height,
                target_height,
            } => return target_height.wrapping_sub(height) < 0,
            TargetAtOrBelow {
                height,
                target_height,
            } => return target_height.wrapping_sub(height) >= 0,
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

/// Mode bit 40 overrides the mode-bit-80 rejection. Either eligible mode
/// still requires action bit 20. This is not an OR of the two flag bytes.
pub fn selected_auxiliary_continuation(mode: u8, action_flags: u8) -> bool {
    const MODE_OVERRIDE: u8 = 0x40;
    const MODE_REJECT: u8 = 0x80;
    const ACTION_CONTINUE: u8 = 0x20;
    (mode & MODE_OVERRIDE != 0 || mode & MODE_REJECT == 0) && action_flags & ACTION_CONTINUE != 0
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
    fn selected_auxiliary_gate_covers_all_flags_and_preserves_inversion() {
        for mode in 0..=u8::MAX {
            for action_flags in 0..=u8::MAX {
                let expected = match mode & 0xC0 {
                    0x80 => false,
                    _ => action_flags & 0x20 != 0,
                };
                let mut branch = BranchState { invert_next: true };
                assert_eq!(
                    branch.test(Predicate::SelectedAuxiliaryContinuation { mode, action_flags }),
                    expected
                );
                assert!(branch.invert_next);
            }
        }
    }

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
    fn zero_less_bit_and_height_branches_preserve_inversion() {
        let mut state = BranchState { invert_next: true };
        for value in 0..=u8::MAX {
            assert_eq!(state.test(Predicate::ZeroByte(value)), value == 0);
            assert_eq!(
                state.test(Predicate::AnyByteBitsSet { value, mask: 0x81 }),
                value & 0x81 != 0
            );
            for first in 0..=u8::MAX {
                let difference = (i16::from(value) - i16::from(first)).rem_euclid(256);
                assert_eq!(
                    state.test(Predicate::SecondByteLess {
                        first,
                        second: value
                    }),
                    difference >= 128
                );
            }
        }
        for value in 0..=u16::MAX {
            assert_eq!(state.test(Predicate::ZeroWord(value)), value == 0);
            assert_eq!(
                state.test(Predicate::AnyWordBitsSet {
                    value,
                    mask: 0x8100
                }),
                value & 0x8100 != 0
            );
            let first = 32760;
            let expected = (i32::from(value) - i32::from(first)).rem_euclid(65536) >= 32768;
            assert_eq!(
                state.test(Predicate::SecondWordLess {
                    first,
                    second: value
                }),
                expected
            );
            assert_eq!(
                state.test(Predicate::TargetAbove {
                    height: first as i16,
                    target_height: value as i16
                }),
                expected
            );
            assert_eq!(
                state.test(Predicate::TargetAtOrBelow {
                    height: first as i16,
                    target_height: value as i16
                }),
                !expected
            );
        }
        assert!(state.invert_next);
    }

    #[test]
    fn selected_plane_uses_selected_orientation_and_wrapped_projection() {
        use super::super::path_control::PlaneAxis;
        let mut state = BranchState { invert_next: true };
        let position = Vector3 {
            x: -100,
            y: 0,
            z: 100,
        };
        for (axis, yaw, expected) in [
            (PlaneAxis::Right, 0, true),
            (PlaneAxis::Forward, 0, false),
            (PlaneAxis::Right, 128, false),
            (PlaneAxis::Forward, 128, true),
        ] {
            assert_eq!(
                state.test(Predicate::NegativeSelectedPlane {
                    position,
                    target_position: Vector3::default(),
                    target_rotation: super::super::Rotation {
                        yaw: Angle::from_units(yaw),
                        ..Default::default()
                    },
                    axis,
                }),
                expected
            );
        }
        for axis in [PlaneAxis::Right, PlaneAxis::Forward] {
            // Doubling the source word precedes multiplication, so both
            // extreme displacements wrap to zero instead of remaining negative.
            assert!(!state.test(Predicate::NegativeSelectedPlane {
                position: Vector3 {
                    x: i16::MIN,
                    y: 0,
                    z: i16::MIN
                },
                target_position: Vector3::default(),
                target_rotation: Default::default(),
                axis,
            }));
        }
        assert!(state.invert_next);
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
