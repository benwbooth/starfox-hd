//! Direct source transcriptions of shared SF2 path control operations.
//!
//! Source: the copied path handlers and their host/geometry callees. No values
//! here come from gameplay recordings. Time is measured in path invocations,
//! not presentation frames; the caller owns the gameplay scheduler.

use super::object::Vector3;
use super::render::Rotation;

const FORWARD_AXIS_LENGTH: i8 = 127;
const NORMAL_FRACTION_BITS: u32 = 8;
const PRODUCT_HIGH_WORD_SHIFT: u32 = 16;

/// Signed forward-plane projection (`$0D:B751`, geometry leaf `$01:FC86`).
///
/// The host rotates an authored signed-byte forward vector in roll/pitch/yaw
/// order, retaining only a byte between stages. The geometry leaf doubles
/// each wrapped delta *before* taking the signed high word of its product.
/// Replacing this with a floating-point dot product changes the branch at
/// both rounding and overflow boundaries.
pub fn forward_plane_projection(position: Vector3, rotation: Rotation, target: Vector3) -> i16 {
    let (x, y, z) = sf_core::snes_trig::strat_roffs_full(
        rotation.roll.units(),
        rotation.pitch.units(),
        rotation.yaw.units(),
        0,
        0,
        FORWARD_AXIS_LENGTH,
    );
    let normal = [x, y, z].map(|component| component.wrapping_shl(NORMAL_FRACTION_BITS));
    let delta = [
        target.x.wrapping_sub(position.x),
        target.y.wrapping_sub(position.y),
        target.z.wrapping_sub(position.z),
    ];
    delta
        .into_iter()
        .zip(normal)
        .fold(0_i16, |sum, (delta, normal)| {
            let doubled = delta.wrapping_add(delta);
            let product = i32::from(doubled) * i32::from(normal);
            sum.wrapping_add((product >> PRODUCT_HIGH_WORD_SHIFT) as i16)
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerTarget {
    Primary,
    Secondary,
}

/// Persistent sign latch for the passed-player trigger (`$7F:9C02..9CE5`).
///
/// This is not a test that movement's dot product with the target is negative.
/// The source stores each player's initial forward-plane sign, then fires on
/// *either* sign change, including one caused by rotation or target motion.
/// The first player's crossing takes precedence and a crossing rearms the
/// latch for the next invocation. Missing players leave their saved sign alone.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerCrossing {
    initialized: bool,
    negative: [bool; 2],
}

impl PlayerCrossing {
    pub fn sample(&mut self, projections: [Option<i16>; 2]) -> Option<PlayerTarget> {
        if !self.initialized {
            self.initialized = true;
            for (saved, current) in self.negative.iter_mut().zip(projections) {
                if let Some(current) = current {
                    *saved = current < 0;
                }
            }
        }
        for (index, current) in projections.into_iter().enumerate() {
            if current.is_some_and(|current| (current < 0) != self.negative[index]) {
                self.initialized = false;
                return Some(if index == 0 {
                    PlayerTarget::Primary
                } else {
                    PlayerTarget::Secondary
                });
            }
        }
        None
    }
}

/// The counted-loop continuation (`$7F:96AE`) decrements before deciding
/// whether to move/yield. A count of one falls through without a movement;
/// a zero word wraps to 65,535 rather than being treated as an empty loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CountedLoop {
    remaining: u16,
}

impl CountedLoop {
    pub const fn new(iterations: u16) -> Self {
        Self {
            remaining: iterations,
        }
    }

    /// True means branch back to the loop body; false means fall through.
    pub fn repeat(&mut self) -> bool {
        self.remaining = self.remaining.wrapping_sub(1);
        self.remaining != 0
    }

    pub const fn remaining(self) -> u16 {
        self.remaining
    }
}

/// Byte wait counter (`$7F:84FB`). A successful wait advances immediately;
/// unsuccessful calls increment the byte and yield to ordinary movement.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PathWait {
    elapsed: u8,
}

impl PathWait {
    pub const fn with_elapsed(elapsed: u8) -> Self {
        Self { elapsed }
    }

    pub fn ready(&mut self, duration: u8) -> bool {
        if self.elapsed == duration {
            self.elapsed = 0;
            true
        } else {
            self.elapsed = self.elapsed.wrapping_add(1);
            false
        }
    }
}

/// Source periodic trigger (`$7F:9D46..9D66`). This uses the low byte of the
/// shared strategy clock, not an age measured from an object's spawn time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerPeriod {
    Two,
    Four,
    Eight,
    Sixteen,
    ThirtyTwo,
    SixtyFour,
    OneTwentyEight,
}

impl TriggerPeriod {
    pub const fn due(self, strategy_tick: u8) -> bool {
        let mask = match self {
            Self::Two => 1,
            Self::Four => 3,
            Self::Eight => 7,
            Self::Sixteen => 15,
            Self::ThirtyTwo => 31,
            Self::SixtyFour => 63,
            Self::OneTwentyEight => 127,
        };
        strategy_tick & mask == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::Angle;

    #[test]
    fn source_loop_decrements_before_yielding() {
        for count in 1..=255 {
            let mut state = CountedLoop::new(count);
            for remaining in (1..count).rev() {
                assert!(state.repeat());
                assert_eq!(state.remaining(), remaining);
            }
            assert!(!state.repeat());
            assert_eq!(state.remaining(), 0);
        }
        let mut wrapped = CountedLoop::new(0);
        assert!(wrapped.repeat());
        assert_eq!(wrapped.remaining(), u16::MAX);
    }

    #[test]
    fn source_wait_uses_equality_and_byte_wrap_not_greater_than() {
        for duration in 0..=u8::MAX {
            let mut state = PathWait::default();
            for _ in 0..duration {
                assert!(!state.ready(duration));
            }
            assert!(state.ready(duration));
        }
        let mut wrapped = PathWait::with_elapsed(u8::MAX);
        assert!(!wrapped.ready(0));
        assert!(wrapped.ready(0));
    }

    #[test]
    fn crossing_latches_sign_and_prioritizes_the_primary_player() {
        let mut state = PlayerCrossing::default();
        assert_eq!(state.sample([Some(1), Some(-1)]), None);
        assert_eq!(state.sample([Some(0), Some(-2)]), None);
        assert_eq!(
            state.sample([Some(-1), Some(0)]),
            Some(PlayerTarget::Primary)
        );
        assert_eq!(state.sample([Some(-1), Some(0)]), None);
        assert_eq!(
            state.sample([Some(-2), Some(-1)]),
            Some(PlayerTarget::Secondary)
        );
        assert_eq!(state.sample([None, Some(-1)]), None);
        assert_eq!(state.sample([Some(1), None]), Some(PlayerTarget::Primary));
    }

    #[test]
    fn source_projection_preserves_double_before_multiply_overflow() {
        let origin = Vector3::default();
        let rotation = Rotation::default();
        for depth in [
            i16::MIN,
            -16_385,
            -16_384,
            -1,
            0,
            1,
            16_383,
            16_384,
            i16::MAX,
        ] {
            let target = Vector3 { z: depth, ..origin };
            // Two byte rotations shorten the authored 127 forward axis to
            // 125. Its encoded coefficient is 32,000, not a unit float.
            let doubled = depth.wrapping_mul(2);
            let expected = ((i32::from(doubled) * 32_000) >> 16) as i16;
            assert_eq!(forward_plane_projection(origin, rotation, target), expected);
        }
    }

    #[test]
    fn rotation_can_cross_a_stationary_target() {
        let origin = Vector3::default();
        let target = Vector3 { z: 1_000, ..origin };
        let mut rotation = Rotation::default();
        let mut state = PlayerCrossing::default();
        assert_eq!(
            state.sample([
                Some(forward_plane_projection(origin, rotation, target)),
                None
            ]),
            None
        );
        rotation.yaw = Angle::HALF_TURN;
        assert_eq!(
            state.sample([
                Some(forward_plane_projection(origin, rotation, target)),
                None
            ]),
            Some(PlayerTarget::Primary)
        );
    }

    #[test]
    fn periodic_triggers_use_the_shared_clock_phase() {
        for (period, duration) in [
            (TriggerPeriod::Two, 2),
            (TriggerPeriod::Four, 4),
            (TriggerPeriod::Eight, 8),
            (TriggerPeriod::Sixteen, 16),
            (TriggerPeriod::ThirtyTwo, 32),
            (TriggerPeriod::SixtyFour, 64),
            (TriggerPeriod::OneTwentyEight, 128),
        ] {
            for tick in 0..=u8::MAX {
                assert_eq!(period.due(tick), tick % duration == 0);
            }
        }
    }
}
