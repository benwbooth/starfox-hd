//! Shared weapon launch geometry (`$03:AB2A..AC10`).
//!
//! The weapon's bank is cleared, but the muzzle offset still rotates through
//! the firing actor's bank, pitch and yaw. Rotation truncates between stages;
//! a combined floating-point matrix is not equivalent. Object allocation,
//! reciprocal links, path initialization and cue dispatch belong to callers.

use super::{path_control, Angle, Rotation, Vector3};

const MUZZLE_WORLD_SCALE_SHIFT: u32 = 2;
const REFLECTION_SCATTER_MASK: u8 = 0x3F;
const REFLECTION_SCATTER_BIAS: u8 = 32;
const HOSTILE_DISABLE_RANDOM_MASK: u8 = 0x03;
const QUARTER_TURN: u8 = 64;
const HALF_TURN: u8 = 128;

/// `$07:F1E7..F25A`: offsets for the ordinary weapon formatter. Pitch is
/// negated without subtracting the reflector's pitch; yaw subtracts its yaw.
/// Consequently the formatted pitch includes the reflector's pitch, whereas
/// formatted yaw is the incoming yaw plus half a turn. Scatter bytes must be
/// drawn in pitch-then-yaw order only after the caller's player-mode gates.
pub fn reflection_parameters(
    incoming: Rotation,
    reflector_yaw: Angle,
    scatter: Option<[u8; 2]>,
) -> LaunchParameters {
    let mut pitch = incoming.pitch.units().wrapping_neg();
    let mut yaw = incoming
        .yaw
        .units()
        .wrapping_add(HALF_TURN)
        .wrapping_sub(reflector_yaw.units());
    if let Some([pitch_random, yaw_random]) = scatter {
        pitch = pitch
            .wrapping_add(pitch_random & REFLECTION_SCATTER_MASK)
            .wrapping_sub(REFLECTION_SCATTER_BIAS);
        yaw = yaw
            .wrapping_add(yaw_random & REFLECTION_SCATTER_MASK)
            .wrapping_sub(REFLECTION_SCATTER_BIAS);
    }
    LaunchParameters {
        pitch_offset: pitch as i8,
        yaw_offset: yaw as i8,
        ..LaunchParameters::default()
    }
}

/// `$0D:DE38..DE49`: classify using the PRIMARY player's yaw, even when the
/// weapon was launched by a different actor. Only the aligned half-plane draws
/// a random byte. Wrapped unsigned comparison is deliberate at the boundary.
pub fn hostile_launch_needs_random(primary_yaw: Angle, weapon_yaw: Angle) -> bool {
    primary_yaw
        .units()
        .wrapping_add(HALF_TURN)
        .wrapping_sub(weapon_yaw.units())
        .wrapping_add(QUARTER_TURN)
        >= HALF_TURN
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HostileLaunchCounts {
    pub collision_disabled: u8,
    pub aligned_half_plane: u8,
}

impl HostileLaunchCounts {
    /// Call only when hostile_launch_needs_random is true, after consuming
    /// exactly one shared random byte. Existing collision disable is never
    /// cleared. Both source counters are wrapping bytes, not lifetime totals.
    pub fn classify_aligned_launch(&mut self, collision_disabled: &mut bool, random: u8) {
        if random & HOSTILE_DISABLE_RANDOM_MASK != 0 {
            *collision_disabled = true;
            self.collision_disabled = self.collision_disabled.wrapping_add(1);
        }
        self.aligned_half_plane = self.aligned_half_plane.wrapping_add(1);
    }
}

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

    #[test]
    fn reflection_preserves_the_sources_asymmetric_pitch_and_yaw_offsets() {
        for incoming in 0..=u8::MAX {
            for source in 0..=u8::MAX {
                let source_rotation = Rotation {
                    pitch: Angle::from_units(source),
                    yaw: Angle::from_units(source),
                    roll: Angle::from_units(73),
                };
                let parameters = reflection_parameters(
                    Rotation {
                        pitch: Angle::from_units(incoming),
                        yaw: Angle::from_units(incoming),
                        roll: Angle::from_units(19),
                    },
                    source_rotation.yaw,
                    None,
                );
                let pose = format_pose(Vector3::default(), source_rotation, parameters);
                assert_eq!(pose.rotation.pitch.units(), source.wrapping_sub(incoming));
                assert_eq!(pose.rotation.yaw.units(), incoming.wrapping_add(128));
                assert_eq!(pose.rotation.roll, Angle::ZERO);
                assert_eq!(parameters.muzzle, MuzzleOffset::default());
                assert_eq!(parameters.target, None);
            }
        }
    }

    #[test]
    fn reflection_scatter_masks_each_random_byte_independently() {
        for pitch_random in 0..=u8::MAX {
            for yaw_random in 0..=u8::MAX {
                let parameters = reflection_parameters(
                    Rotation {
                        pitch: Angle::from_units(128),
                        yaw: Angle::from_units(250),
                        roll: Angle::ZERO,
                    },
                    Angle::from_units(2),
                    Some([pitch_random, yaw_random]),
                );
                assert_eq!(
                    parameters.pitch_offset as u8,
                    128u8.wrapping_add(pitch_random & 63).wrapping_sub(32)
                );
                assert_eq!(
                    parameters.yaw_offset as u8,
                    120u8.wrapping_add(yaw_random & 63).wrapping_sub(32)
                );
            }
        }
    }

    #[test]
    fn hostile_launch_gate_is_a_wrapped_half_plane() {
        for player in 0..=u8::MAX {
            for weapon in 0..=u8::MAX {
                let separation = weapon.wrapping_sub(player);
                assert_eq!(
                    hostile_launch_needs_random(
                        Angle::from_units(player),
                        Angle::from_units(weapon)
                    ),
                    separation <= 64 || separation > 192
                );
            }
        }
    }

    #[test]
    fn hostile_launch_random_disable_preserves_existing_flags_and_wraps_counters() {
        for random in 0..=u8::MAX {
            for initial in [false, true] {
                let mut disabled = initial;
                let mut counts = HostileLaunchCounts {
                    collision_disabled: 255,
                    aligned_half_plane: 255,
                };
                counts.classify_aligned_launch(&mut disabled, random);
                let selected = random & 3 != 0;
                assert_eq!(disabled, initial || selected);
                assert_eq!(counts.collision_disabled, if selected { 0 } else { 255 });
                assert_eq!(counts.aligned_half_plane, 0);
            }
        }
    }
}
