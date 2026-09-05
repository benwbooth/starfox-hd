//! Source-derived building blocks for the native attract-scene reconstruction.
//!
//! These are game-domain state and arithmetic, not captured animation tracks.
//! Scene scheduling and presentation are separate work; these kernels alone
//! do not replace the current recorded front end.

use super::object::{Angle, Vector3};
use super::render::Rotation;

const ANGLE_FRACTION_BITS: u32 = 8;
const TITLE_VIEW_FINE_YAW: u16 = 49_152;
const ANGLE_CHASE_DIVISOR: i16 = 8;

/// The attract camera keeps a fractional angle for every axis. This is
/// distinct from the selected player's retained, integral-roll orientation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AttractCameraAngles {
    pub pitch: u16,
    pub yaw: u16,
    pub roll: u16,
}

impl AttractCameraAngles {
    pub fn chase(&mut self, target: Rotation) {
        self.pitch = chase_fine_angle(
            self.pitch,
            u16::from(target.pitch.units()) << ANGLE_FRACTION_BITS,
        );
        self.yaw = chase_fine_angle(
            self.yaw,
            u16::from(target.yaw.units()) << ANGLE_FRACTION_BITS,
        );
        self.roll = chase_fine_angle(
            self.roll,
            u16::from(target.roll.units()) << ANGLE_FRACTION_BITS,
        );
    }

    pub fn coarse_rotation(self) -> Rotation {
        Rotation {
            pitch: Angle::from_units((self.pitch >> ANGLE_FRACTION_BITS) as u8),
            yaw: Angle::from_units((self.yaw >> ANGLE_FRACTION_BITS) as u8),
            roll: Angle::from_units((self.roll >> ANGLE_FRACTION_BITS) as u8),
        }
    }
}

/// Move one eighth of the shortest signed angular displacement, rounding
/// toward zero, with a minimum nonzero step of one fractional-angle unit.
pub fn chase_fine_angle(current: u16, target: u16) -> u16 {
    let displacement = target.wrapping_sub(current) as i16;
    if displacement == 0 {
        return current;
    }
    let numerator = if displacement < 0 {
        displacement.min(-ANGLE_CHASE_DIVISOR)
    } else {
        displacement.max(ANGLE_CHASE_DIVISOR)
    };
    current.wrapping_add((numerator / ANGLE_CHASE_DIVISOR) as u16)
}

/// The retained orientation has fractional pitch/yaw but an integral roll.
/// This asymmetry is part of the source's player-pose layout.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RetainedRotation {
    pub pitch: u16,
    pub yaw: u16,
    pub roll: Angle,
}

/// The selected player pose and its retained movement origins. Source scene
/// commands update both copies; updating only the visible pose leaves stale
/// origins for the next movement calculation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IntroPlayerAnchor {
    pub position: Vector3,
    pub rotation: Rotation,
    pub retained_rotation: RetainedRotation,
    pub retained_position: Vector3,
}

impl IntroPlayerAnchor {
    pub fn capture_rotation(&mut self, rotation: Rotation) {
        self.rotation = rotation;
        self.retained_rotation = RetainedRotation {
            pitch: u16::from(rotation.pitch.units()) << ANGLE_FRACTION_BITS,
            yaw: u16::from(rotation.yaw.units()) << ANGLE_FRACTION_BITS,
            roll: rotation.roll,
        };
    }

    pub fn capture_position(&mut self, position: Vector3) {
        self.position = position;
        self.retained_position = position;
    }
}

/// One attract-camera yaw settling step, in full-turn fractional-angle units.
/// The two successive signed halves round down separately. A single
/// multiply by three quarters changes odd and negative source results.
pub fn settle_title_view_yaw(yaw: u16) -> u16 {
    let displacement = yaw.wrapping_sub(TITLE_VIEW_FINE_YAW) as i16;
    let half = displacement >> 1;
    let quarter = half >> 1;
    TITLE_VIEW_FINE_YAW.wrapping_add(half.wrapping_add(quarter) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_keeps_position_and_rotation_channels_separate() {
        let position = Vector3 {
            x: i16::MIN,
            y: -1,
            z: i16::MAX,
        };
        let rotation = Rotation {
            pitch: Angle::from_units(1),
            yaw: Angle::HALF_TURN,
            roll: Angle::from_units(u8::MAX),
        };
        let mut anchor = IntroPlayerAnchor::default();
        anchor.capture_position(position);
        anchor.capture_rotation(rotation);
        assert_eq!(anchor.position, position);
        assert_eq!(anchor.rotation, rotation);
        assert_eq!(anchor.retained_position, position);
        assert_eq!(anchor.retained_rotation.pitch, 1 << ANGLE_FRACTION_BITS);
        assert_eq!(anchor.retained_rotation.yaw, 32_768);
        assert_eq!(anchor.retained_rotation.roll, rotation.roll);
    }

    #[test]
    fn settling_preserves_the_source_signed_rounding() {
        assert_eq!(
            settle_title_view_yaw(TITLE_VIEW_FINE_YAW),
            TITLE_VIEW_FINE_YAW
        );
        assert_eq!(
            settle_title_view_yaw(TITLE_VIEW_FINE_YAW + 3),
            TITLE_VIEW_FINE_YAW + 1
        );
        assert_eq!(
            settle_title_view_yaw(TITLE_VIEW_FINE_YAW - 1),
            TITLE_VIEW_FINE_YAW - 2
        );
    }
}
