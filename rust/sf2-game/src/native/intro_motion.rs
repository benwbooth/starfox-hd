//! Source-derived building blocks for the native attract-scene reconstruction.
//!
//! These are game-domain state and arithmetic, not captured animation tracks.
//! Scene scheduling and presentation are separate work; these kernels alone
//! do not replace the current recorded front end.

use super::object::{Angle, Vector3};
use super::render::Rotation;
use sf_core::snes_trig::{cos_q15, gsu_fmult_q15 as multiply_q15, matrix_rotate_q15, sin_q15};

const ANGLE_FRACTION_BITS: u32 = 8;
const TITLE_VIEW_FINE_YAW: u16 = 49_152;
const ANGLE_CHASE_DIVISOR: i16 = 8;
const INTRO_ROLL_CHASE_DIVISOR: i16 = 4;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IntroScenePose {
    pub position: Vector3,
    pub rotation: Rotation,
}

/// Retained attachment coordinates are independent of the published world
/// pose. Their velocity is integrated after the parent's pose publication.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IntroAttachment {
    pub offset: Vector3,
    pub rotation: Rotation,
}

impl IntroAttachment {
    pub fn world_pose(self, parent: IntroScenePose) -> IntroScenePose {
        let (x, y, z) = matrix_rotate_q15(
            attachment_matrix(parent.rotation),
            self.offset.x,
            self.offset.y,
            self.offset.z,
        );
        IntroScenePose {
            position: Vector3 {
                x: parent.position.x.wrapping_add(x),
                y: parent.position.y.wrapping_add(y),
                z: parent.position.z.wrapping_add(z),
            },
            rotation: Rotation {
                pitch: parent
                    .rotation
                    .pitch
                    .wrapping_add(self.rotation.pitch.units() as i8),
                yaw: parent
                    .rotation
                    .yaw
                    .wrapping_add(self.rotation.yaw.units() as i8),
                roll: parent
                    .rotation
                    .roll
                    .wrapping_add(self.rotation.roll.units() as i8),
            },
        }
    }

    pub fn advance(&mut self, velocity: Vector3) {
        self.offset.x = self.offset.x.wrapping_add(velocity.x);
        self.offset.y = self.offset.y.wrapping_add(velocity.y);
        self.offset.z = self.offset.z.wrapping_add(velocity.z);
    }
}

/// Attachment rotation uses a different composition from view rotation.
/// Build the complete fixed-point matrix before transforming the point;
/// separate successive rotations have different truncation.
fn attachment_matrix(rotation: Rotation) -> [[i16; 3]; 3] {
    let pitch_sine = sin_q15(rotation.pitch.units());
    let pitch_cosine = cos_q15(rotation.pitch.units());
    let yaw_sine = sin_q15(rotation.yaw.units());
    let yaw_cosine = cos_q15(rotation.yaw.units());
    let roll_sine = sin_q15(rotation.roll.units());
    let roll_cosine = cos_q15(rotation.roll.units());
    let cosine_sine = multiply_q15(roll_cosine, yaw_sine);
    let cosine_cosine = multiply_q15(roll_cosine, yaw_cosine);
    let sine_sine = multiply_q15(roll_sine, yaw_sine);
    let sine_cosine = multiply_q15(roll_sine, yaw_cosine);
    [
        [
            cosine_cosine.wrapping_sub(multiply_q15(sine_sine, pitch_sine)),
            multiply_q15(pitch_cosine, roll_sine).wrapping_neg(),
            multiply_q15(sine_cosine, pitch_sine).wrapping_add(cosine_sine),
        ],
        [
            multiply_q15(cosine_sine, pitch_sine).wrapping_add(sine_cosine),
            multiply_q15(pitch_cosine, roll_cosine),
            sine_sine.wrapping_sub(multiply_q15(cosine_cosine, pitch_sine)),
        ],
        [
            multiply_q15(pitch_cosine, yaw_sine).wrapping_neg(),
            pitch_sine,
            multiply_q15(pitch_cosine, yaw_cosine),
        ],
    ]
}

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
    chase_wrapping_value(current, target, ANGLE_CHASE_DIVISOR)
}

/// Level the opening camera's roll by one quarter of its signed displacement,
/// rounding toward zero with a minimum nonzero step of one fractional unit.
pub fn settle_intro_camera_roll(roll: u16) -> u16 {
    chase_wrapping_value(roll, 0, INTRO_ROLL_CHASE_DIVISOR)
}

/// Source scene coordinate easing shares the wrapped one-eighth arithmetic.
pub fn chase_intro_coordinate(current: i16, target: i16) -> i16 {
    chase_wrapping_value(current as u16, target as u16, ANGLE_CHASE_DIVISOR) as i16
}

fn chase_wrapping_value(current: u16, target: u16, divisor: i16) -> u16 {
    let displacement = target.wrapping_sub(current) as i16;
    if displacement == 0 {
        return current;
    }
    let numerator = if displacement < 0 {
        displacement.min(-divisor)
    } else {
        displacement.max(divisor)
    };
    current.wrapping_add((numerator / divisor) as u16)
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
