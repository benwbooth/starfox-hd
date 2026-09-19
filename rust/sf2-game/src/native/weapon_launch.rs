//! Shared weapon launch geometry (`$03:AB2A..AC10`).
//!
//! The weapon's bank is cleared, but the muzzle offset still rotates through
//! the firing actor's bank, pitch and yaw. Rotation truncates between stages;
//! a combined floating-point matrix is not equivalent. Object allocation,
//! reciprocal links, path initialization and cue dispatch belong to callers.

use super::{path_control, Angle, Rotation, Vector3};

const MUZZLE_WORLD_SCALE_SHIFT: u32 = 2;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MuzzleOffset {
    pub x: i8,
    pub y: i8,
    pub z: i8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LaunchParameters {
    pub muzzle: MuzzleOffset,
    pub pitch_offset: i8,
    pub yaw_offset: i8,
    /// Aim from the rotated muzzle position, not from the firing actor.
    /// With no selected target, inherit the firing actor's pitch and yaw.
    pub target: Option<Vector3>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchPose {
    pub position: Vector3,
    pub rotation: Rotation,
}

pub fn format_pose(
    source_position: Vector3,
    source_rotation: Rotation,
    parameters: LaunchParameters,
) -> LaunchPose {
    let (x, y, z) = sf_core::snes_trig::strat_roffs_full_scaled(
        source_rotation.roll.units(),
        source_rotation.pitch.units(),
        source_rotation.yaw.units(),
        parameters.muzzle.x,
        parameters.muzzle.y,
        parameters.muzzle.z,
        MUZZLE_WORLD_SCALE_SHIFT,
    );
    let position = Vector3 {
        x: source_position.x.wrapping_add(x),
        y: source_position.y.wrapping_add(y),
        z: source_position.z.wrapping_add(z),
    };
    let (pitch, yaw) = parameters
        .target
        .map_or((source_rotation.pitch, source_rotation.yaw), |target| {
            path_control::target_angles(position, target)
        });
    LaunchPose {
        position,
        rotation: Rotation {
            pitch: pitch.wrapping_add(parameters.pitch_offset),
            yaw: yaw.wrapping_add(parameters.yaw_offset),
            roll: Angle::ZERO,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_muzzle_inherits_position_and_heading_but_not_bank() {
        let position = Vector3 {
            x: 30,
            y: -40,
            z: 90,
        };
        let rotation = Rotation {
            pitch: Angle::from_units(17),
            yaw: Angle::from_units(245),
            roll: Angle::from_units(63),
        };
        let pose = format_pose(position, rotation, LaunchParameters::default());
        assert_eq!(pose.position, position);
        assert_eq!(pose.rotation.pitch, rotation.pitch);
        assert_eq!(pose.rotation.yaw, rotation.yaw);
        assert_eq!(pose.rotation.roll, Angle::ZERO);
    }

    #[test]
    fn even_zero_angles_retain_each_byte_product_truncation() {
        let pose = format_pose(
            Vector3 {
                x: 32_760,
                y: -32_760,
                z: 5,
            },
            Rotation::default(),
            LaunchParameters {
                muzzle: MuzzleOffset {
                    x: 10,
                    y: -20,
                    z: 30,
                },
                ..LaunchParameters::default()
            },
        );
        // cos(0) is 127/128, not exactly one: x=10 -> 9 -> 8,
        // y=-20 -> -19 -> -18, z=30 -> 29 -> 28, then scale by four.
        assert_eq!(
            pose.position,
            Vector3 {
                x: -32_744,
                y: 32_704,
                z: 117
            }
        );
    }

    #[test]
    fn source_bank_rotates_muzzle_even_though_weapon_bank_is_zero() {
        let pose = format_pose(
            Vector3::default(),
            Rotation {
                roll: Angle::from_units(64),
                ..Rotation::default()
            },
            LaunchParameters {
                muzzle: MuzzleOffset { x: 100, y: 0, z: 0 },
                ..LaunchParameters::default()
            },
        );
        assert_eq!(
            pose.position,
            Vector3 {
                x: 0,
                y: -392,
                z: 0
            }
        );
        assert_eq!(pose.rotation.roll, Angle::ZERO);
    }

    #[test]
    fn target_aim_uses_muzzle_and_offsets_are_applied_after_aim() {
        let position = Vector3 {
            x: 100,
            y: -100,
            z: 200,
        };
        let rotation = Rotation {
            pitch: Angle::from_units(20),
            yaw: Angle::from_units(70),
            roll: Angle::from_units(10),
        };
        let parameters = LaunchParameters {
            muzzle: MuzzleOffset {
                x: 40,
                y: -10,
                z: 80,
            },
            ..LaunchParameters::default()
        };
        let muzzle = format_pose(position, rotation, parameters).position;
        let pose = format_pose(
            position,
            rotation,
            LaunchParameters {
                target: Some(Vector3 {
                    z: muzzle.z.wrapping_add(1_000),
                    ..muzzle
                }),
                pitch_offset: -1,
                yaw_offset: 1,
                ..parameters
            },
        );
        assert_eq!(pose.rotation.pitch.units(), 255);
        assert_eq!(pose.rotation.yaw.units(), 1);
        assert_eq!(pose.position, muzzle);
    }

    #[test]
    fn inherited_angle_offsets_wrap_as_bytes() {
        let pose = format_pose(
            Vector3::default(),
            Rotation {
                pitch: Angle::from_units(250),
                yaw: Angle::from_units(2),
                roll: Angle::ZERO,
            },
            LaunchParameters {
                pitch_offset: 10,
                yaw_offset: -10,
                ..LaunchParameters::default()
            },
        );
        assert_eq!(pose.rotation.pitch.units(), 4);
        assert_eq!(pose.rotation.yaw.units(), 248);
    }
}
