//! Source path movement primitives (`$7F:9DDE`, `$7F:855F`, `$7F:306E`).
//!
//! Ordinary position integration (`$7F:2C24`) precedes path callbacks;
//! attached-relative integration (`$7F:9E9F`) follows them. These operations
//! are separate so callers cannot accidentally collapse those two phases.

use super::{Angle, Vector3};

const BANK_TURN_DIVISOR: i8 = 4;

/// Accelerate using the source's signed *byte subtraction* tests. Testing
/// widened integers or saturating the intermediate result is not equivalent
/// when the authored speed or acceleration crosses a byte boundary.
///
/// Reaching the target also disables acceleration. A zero acceleration does
/// nothing, even when the current speed differs from the target.
pub fn accelerate(speed: &mut u8, target: u8, acceleration: &mut u8) {
    if *acceleration == 0 {
        return;
    }
    let below = (speed.wrapping_sub(target) as i8) < 0;
    let next = if below {
        speed.wrapping_add(*acceleration)
    } else {
        speed.wrapping_sub(*acceleration)
    };
    let still_below = (next.wrapping_sub(target) as i8) < 0;
    if below == still_below {
        *speed = next;
    } else {
        *speed = target;
        *acceleration = 0;
    }
}

/// Two signed halves rounded toward zero (`$7F:9E29..9E3C`).
pub fn bank_turn(yaw: Angle, roll: Angle) -> Angle {
    yaw.wrapping_add((roll.units() as i8) / BANK_TURN_DIVISOR)
}

/// Byte-by-byte signed products, retaining the source's intermediate
/// truncation and doubled-byte overflow. Pitch is positive; yaw is negated.
pub fn direction_velocity(pitch: Angle, yaw: Angle, speed: u8, scale: i16) -> Vector3 {
    use sf_core::snes_trig::{mulslog_mac8, COSTAB, SINTAB};
    let yaw = usize::from(yaw.units().wrapping_neg());
    let pitch = usize::from(pitch.units());
    let speed = speed as i8;
    Vector3 {
        x: i16::from(mulslog_mac8(
            mulslog_mac8(speed, SINTAB[yaw]),
            COSTAB[pitch],
        ))
        .wrapping_mul(scale),
        y: i16::from(mulslog_mac8(speed, SINTAB[pitch])).wrapping_mul(scale),
        z: i16::from(mulslog_mac8(
            mulslog_mac8(speed, COSTAB[yaw]),
            COSTAB[pitch],
        ))
        .wrapping_mul(scale),
    }
}

pub fn integrate(position: &mut Vector3, velocity: Vector3) {
    position.x = position.x.wrapping_add(velocity.x);
    position.y = position.y.wrapping_add(velocity.y);
    position.z = position.z.wrapping_add(velocity.z);
}

/// Selected-player world displacement (`$7F:9F16`). Source auxiliary flag
/// bit 2 suppresses horizontal compensation only; depth still advances.
/// Vertical player displacement is never copied by this operation.
pub fn follow_selected_displacement(
    position: &mut Vector3,
    selected_displacement: Vector3,
    suppress_horizontal: bool,
) {
    if !suppress_horizontal {
        position.x = position.x.wrapping_add(selected_displacement.x);
    }
    position.z = position.z.wrapping_add(selected_displacement.z);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acceleration_stops_on_crossing_but_not_on_an_exact_landing_from_above() {
        let mut speed = 10;
        let mut amount = 3;
        accelerate(&mut speed, 12, &mut amount);
        assert_eq!((speed, amount), (12, 0));
        speed = 15;
        amount = 3;
        accelerate(&mut speed, 12, &mut amount);
        assert_eq!((speed, amount), (12, 3));
        accelerate(&mut speed, 12, &mut amount);
        assert_eq!((speed, amount), (12, 0));
    }

    #[test]
    fn acceleration_uses_wrapped_byte_differences_and_intermediates() {
        for (initial, target, amount, expected) in [
            (0, 255, 1, (255, 1)),
            (255, 0, 1, (0, 0)),
            (0, 128, 1, (1, 1)),
            (127, 128, 255, (126, 255)),
            (64, 64, 0, (64, 0)),
        ] {
            let mut speed = initial;
            let mut acceleration = amount;
            accelerate(&mut speed, target, &mut acceleration);
            assert_eq!((speed, acceleration), expected);
        }
    }

    #[test]
    fn bank_turn_rounds_negative_roll_toward_zero_and_wraps_yaw() {
        for roll in i8::MIN..=i8::MAX {
            let turned = bank_turn(Angle::from_units(255), Angle::from_units(roll as u8));
            assert_eq!(turned.units(), 255_u8.wrapping_add((roll / 4) as u8));
        }
    }

    #[test]
    fn velocity_keeps_two_truncations_and_signed_byte_speed_overflow() {
        assert_eq!(
            direction_velocity(Angle::ZERO, Angle::ZERO, 63, 4),
            Vector3 { x: 0, y: 0, z: 244 }
        );
        assert_eq!(
            direction_velocity(Angle::ZERO, Angle::ZERO, 128, 4),
            Vector3::default()
        );
        assert_eq!(
            direction_velocity(Angle::ZERO, Angle::from_units(64), 63, 4),
            Vector3 {
                x: -244,
                y: 0,
                z: 0
            }
        );
    }

    #[test]
    fn position_additions_wrap_and_selected_displacement_never_changes_altitude() {
        let mut position = Vector3 {
            x: i16::MAX,
            y: 10,
            z: i16::MIN,
        };
        integrate(
            &mut position,
            Vector3 {
                x: 1,
                y: -20,
                z: -1,
            },
        );
        assert_eq!(
            position,
            Vector3 {
                x: i16::MIN,
                y: -10,
                z: i16::MAX
            }
        );
        let displacement = Vector3 {
            x: 13,
            y: 25,
            z: 10,
        };
        follow_selected_displacement(&mut position, displacement, true);
        assert_eq!(
            position,
            Vector3 {
                x: i16::MIN,
                y: -10,
                z: -32759
            }
        );
        follow_selected_displacement(&mut position, displacement, false);
        assert_eq!(
            position,
            Vector3 {
                x: -32755,
                y: -10,
                z: -32749
            }
        );
    }
}
