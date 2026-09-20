//! Primary-player target selection and marker projection (`$07:B1EA..B3AE`).
//! The fixed view marker has fine angles, unlike ordinary byte-angle actors.

use super::{ObjectId, Vector3};
use sf_core::aim_angle::{sf2_atan16, sf2_xz_angle_distance};

const LOCKED: u8 = 0x10;
const PATH_REQUESTED: u8 = 0x08;
const DISPLAY_HIGH: u8 = 0x80;
const ANGLE_FRACTION_BITS: u32 = 8;
const HALF_SAMPLE: u16 = 0x0080;
const CURVE_MASK: u8 = 0x1F;
const NORMALIZED_RANGE_MASK: u16 = 0xF000;
const HIGH_BYTE_MASK: u16 = 0xFF00;
const CENTER_X: i16 = 112;
const CENTER_Y: i16 = 96;
const QUARTER_SHIFT: u32 = 2;
const LEFT_VERTICAL_GROUP: i16 = 5376;
const RIGHT_VERTICAL_GROUP: i16 = -4608;

/// Published by target-lock retention ($07:A5E3/A653), cleared by its
/// cancellation routes. A projectile copies this snapshot once; it does not
/// substitute the fresh target candidate or path-selected player.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PublishedHomingTarget {
    pub object: Option<ObjectId>,
}

/// Pilot-relative targeting upgrade flags ($1DDD). The active pilot's high
/// bit gates target-lock tracking at $07:A50A; pilot exchange swaps the high
/// two bits at $06:A399. Acquiring this upgrade preserves every other bit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TargetingUpgradeState {
    pub pilot_flags: u8,
}

impl TargetingUpgradeState {
    const ACTIVE_PILOT: u8 = 0x80;

    pub fn active_pilot_has_upgrade(self) -> bool {
        self.pilot_flags & Self::ACTIVE_PILOT != 0
    }

    pub fn acquire_for_active_pilot(&mut self) {
        self.pilot_flags |= Self::ACTIVE_PILOT;
    }
}

#[derive(Clone, Copy)]
struct EdgeLimits {
    lower: i16,
    upper: i16,
    negative_pixel: i16,
    positive_pixel: i16,
}

// $07:B47D..B49C, decoded once into named angle limits and screen edges.
const HORIZONTAL: EdgeLimits = EdgeLimits {
    lower: -6144,
    upper: 6144,
    negative_pixel: 208,
    positive_pixel: 16,
};
const VERTICAL_CENTER: EdgeLimits = EdgeLimits {
    lower: -4608,
    upper: 5120,
    negative_pixel: 32,
    positive_pixel: 176,
};
const VERTICAL_LEFT: EdgeLimits = EdgeLimits {
    lower: -5120,
    upper: 3584,
    negative_pixel: 32,
    positive_pixel: 154,
};
const VERTICAL_RIGHT: EdgeLimits = EdgeLimits {
    lower: -3328,
    upper: 3328,
    negative_pixel: 60,
    positive_pixel: 132,
};

// $07:B4BD/B4DD: two interleaved half-angle samples, not a float tangent.
const WHOLE_CURVE: [u8; 32] = [
    0, 3, 6, 9, 12, 16, 19, 22, 25, 28, 32, 35, 39, 42, 46, 49, 53, 56, 60, 64, 68, 72, 76, 81, 85,
    90, 94, 100, 105, 110, 116, 121,
];
const HALF_CURVE: [u8; 32] = [
    1, 4, 8, 11, 14, 17, 20, 24, 27, 30, 33, 37, 40, 44, 47, 51, 54, 58, 62, 66, 70, 74, 79, 83,
    88, 92, 97, 102, 107, 113, 119, 124,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetAnchor {
    pub position: Vector3,
    pub pitch: u16,
    pub yaw: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TargetSelection {
    pub display_status: u8,
    pub control_flags: u8,
    pub forced_owner: Option<ObjectId>,
    pub candidate: Option<ObjectId>,
    /// Stored signed source distance bits; comparison uses absolute NEW
    /// distance against the existing unsigned word, not its absolute value.
    pub distance: u16,
    /// The source retains normalized range's high byte and replaces its low
    /// byte with a separately wrapped byte-coordinate Manhattan distance.
    pub auxiliary_distance: u16,
    pub position: Vector3,
    pub pitch: u16,
    pub yaw: u16,
    pub screen: [u8; 2],
    pub clipped_yaw: u8,
}

pub struct PrimaryTarget<'a> {
    pub anchor: TargetAnchor,
    pub selection: &'a mut TargetSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetProjection {
    pub screen: [u8; 2],
    pub clipped_yaw: u8,
}

fn curve(angle: u16, half: bool) -> i16 {
    let whole = (angle >> ANGLE_FRACTION_BITS) as u8;
    let negative = (angle as i16) < 0;
    let index = if negative {
        whole.wrapping_neg()
    } else {
        whole
    } & CURVE_MASK;
    let samples = if half != negative {
        &HALF_CURVE
    } else {
        &WHOLE_CURVE
    };
    i16::from(samples[usize::from(index)])
}

/// `$07:B258..B32C`: Y's half-sample choice deliberately comes from YAW's
/// fractional byte, due to the source's overlapping word loads.
pub fn project(pitch: u16, yaw: u16) -> TargetProjection {
    let horizontal = yaw as i16;
    let vertical = pitch as i16;
    let half = yaw & HALF_SAMPLE != 0;
    let mut clipping_count = 0u8;
    let x = if horizontal >= HORIZONTAL.upper {
        clipping_count += 1;
        HORIZONTAL.positive_pixel
    } else if horizontal < HORIZONTAL.lower {
        clipping_count += 1;
        HORIZONTAL.negative_pixel
    } else {
        let sample = curve(yaw, half);
        CENTER_X + if horizontal < 0 { sample } else { -sample }
    };
    // $07:B485/B48D/B495: yaw-dependent vertical limits and edge pixels.
    let edges = if horizontal < RIGHT_VERTICAL_GROUP {
        VERTICAL_RIGHT
    } else if horizontal >= LEFT_VERTICAL_GROUP {
        VERTICAL_LEFT
    } else {
        VERTICAL_CENTER
    };
    let y = if vertical >= edges.upper {
        clipping_count += 1;
        edges.positive_pixel
    } else if vertical < edges.lower {
        clipping_count += 1;
        edges.negative_pixel
    } else {
        let magnitude = curve(pitch, half);
        let sample = if vertical < 0 { -magnitude } else { magnitude };
        // Preserve each arithmetic shift, including the negation before
        // the final half. Combining this into a float scale changes pixels.
        let quarter = sample >> QUARTER_SHIFT;
        let negative_eighth = -(quarter >> 1);
        CENTER_Y + sample + quarter + negative_eighth + (negative_eighth >> 1)
    };
    TargetProjection {
        screen: [x as u8, y as u8],
        clipped_yaw: (yaw as u8).wrapping_add(clipping_count),
    }
}

fn angles_and_auxiliary_range(position: Vector3, anchor: TargetAnchor) -> (u16, u16, u16) {
    // Half the coordinates BEFORE subtracting, not half the wrapped delta.
    let dx = (position.x >> 1).wrapping_sub(anchor.position.x >> 1);
    let dz = (position.z >> 1).wrapping_sub(anchor.position.z >> 1);
    let mut range = dx.unsigned_abs().wrapping_add(dz.unsigned_abs());
    let mut vertical = (position.y >> 1).wrapping_sub(anchor.position.y >> 1);
    while range & NORMALIZED_RANGE_MASK != 0 {
        range >>= 1;
        vertical >>= 1;
    }
    let pitch = sf2_atan16(vertical, range as i16).wrapping_add(anchor.pitch);
    let yaw = sf2_atan16(
        position.x.wrapping_sub(anchor.position.x),
        position.z.wrapping_sub(anchor.position.z),
    )
    .wrapping_neg()
    .wrapping_add(anchor.yaw);
    let byte_delta =
        |value: i16, origin: i16| ((value as u8).wrapping_sub(origin as u8) as i8).unsigned_abs();
    let near = byte_delta(position.x, anchor.position.x)
        .wrapping_add(byte_delta(position.z, anchor.position.z));
    (pitch, yaw, (range & HIGH_BYTE_MASK) | u16::from(near))
}

/// Consider the calling actor for the primary player's target, using the
/// fixed view anchor rather than the selected player's world transform.
/// Returns whether it replaced the target; rejected candidates still clear
/// the display high bit unless selection was already locked.
pub fn consider(
    selection: &mut TargetSelection,
    owner: ObjectId,
    position: Vector3,
    anchor: TargetAnchor,
) -> bool {
    if selection.control_flags & LOCKED != 0 {
        return false;
    }
    selection.display_status &= !DISPLAY_HIGH;
    let distance = sf2_xz_angle_distance(
        anchor.position.x.wrapping_sub(position.x),
        anchor.position.z.wrapping_sub(position.z),
    );
    let forced = selection.forced_owner == Some(owner);
    if !forced && distance.unsigned_abs() >= selection.distance {
        return false;
    }
    let (pitch, yaw, auxiliary_distance) = angles_and_auxiliary_range(position, anchor);
    let projection = project(pitch, yaw);
    if forced {
        selection.control_flags |= LOCKED;
    }
    selection.control_flags |= PATH_REQUESTED;
    selection.candidate = Some(owner);
    selection.distance = distance as u16;
    selection.auxiliary_distance = auxiliary_distance;
    selection.position = position;
    selection.pitch = pitch;
    selection.yaw = yaw;
    selection.screen = projection.screen;
    selection.clipped_yaw = projection.clipped_yaw;
    true
}

#[cfg(test)]
mod tests {
    use super::super::{Behavior, Object, ObjectKind, ObjectStore, ShapeId};
    use super::*;

    fn candidate() -> ObjectId {
        ObjectStore::new()
            .allocate(Object::new(
                ObjectKind::Enemy,
                ShapeId::EMPTY,
                Behavior::FollowPath,
            ))
            .unwrap()
    }

    // Independent table/word equation: no source execution or gameplay trace.
    fn expected_projection(pitch: u16, yaw: u16) -> TargetProjection {
        let lookup = |angle: u16| -> i32 {
            let signed = angle as i16;
            let whole = i32::from(angle >> 8);
            let index = if signed < 0 { -whole } else { whole }.rem_euclid(32) as usize;
            let upper = (yaw & 128 != 0) ^ (signed < 0);
            i32::from(if upper {
                HALF_CURVE[index]
            } else {
                WHOLE_CURVE[index]
            })
        };
        let y = i32::from(yaw as i16);
        let p = i32::from(pitch as i16);
        let x_clip = y < -6144 || y >= 6144;
        let x = if y >= 6144 {
            16
        } else if y < -6144 {
            208
        } else {
            112 + if y < 0 { lookup(yaw) } else { -lookup(yaw) }
        };
        let groups = [
            [176, 32, 5120, -4608],
            [154, 32, 3584, -5120],
            [132, 60, 3328, -3328],
        ];
        let edges = groups[if y < -4608 {
            2
        } else if y >= 5376 {
            1
        } else {
            0
        }];
        let y_clip = p >= edges[2] || p < edges[3];
        let screen_y = if p >= edges[2] {
            edges[0]
        } else if p < edges[3] {
            edges[1]
        } else {
            let sample = if p < 0 { -lookup(pitch) } else { lookup(pitch) };
            let quarter = sample.div_euclid(4);
            let correction = -quarter.div_euclid(2);
            96 + sample + quarter + correction + correction.div_euclid(2)
        };
        TargetProjection {
            screen: [x as u8, screen_y as u8],
            clipped_yaw: ((u32::from(yaw) + u32::from(x_clip) + u32::from(y_clip)) % 256) as u8,
        }
    }

    #[test]
    fn projection_covers_every_fine_angle_and_all_clip_group_boundaries() {
        let edges: [i16; 24] = [
            i16::MIN,
            -6145,
            -6144,
            -6143,
            -5121,
            -5120,
            -4609,
            -4608,
            -4607,
            -3329,
            -3328,
            -1,
            0,
            1,
            127,
            128,
            3327,
            3328,
            3583,
            3584,
            5119,
            5120,
            5375,
            5376,
        ];
        for bits in 0..=u16::MAX {
            for edge in edges {
                assert_eq!(
                    project(edge as u16, bits),
                    expected_projection(edge as u16, bits)
                );
                assert_eq!(
                    project(bits, edge as u16),
                    expected_projection(bits, edge as u16)
                );
            }
        }
        assert_eq!(project(0, 0).screen, [112, 96]);
        assert_eq!(project(0, 128).screen, [111, 97]);
        // Pitch fraction changes alone do not select the half table.
        assert_eq!(project(127, 0).screen, project(128, 0).screen);
    }

    #[test]
    fn selection_preserves_rejection_side_effects_and_forced_ownership_for_every_flag_byte() {
        let owner = candidate();
        let anchor = TargetAnchor {
            position: Vector3::default(),
            pitch: 0,
            yaw: 0,
        };
        let position = Vector3 { x: 0, y: 0, z: 100 };
        for flags in 0..=u8::MAX {
            for display_status in [0, 127, 128, 255] {
                for forced in [false, true] {
                    for distance in [0, 55, 56, 57, 32768, 65535] {
                        let mut selection = TargetSelection {
                            display_status,
                            control_flags: flags,
                            forced_owner: forced.then_some(owner),
                            distance,
                            candidate: None,
                            auxiliary_distance: 0xA539,
                            position: Vector3 { x: -1, y: 2, z: -3 },
                            pitch: 13,
                            yaw: 17,
                            screen: [19, 23],
                            clipped_yaw: 29,
                        };
                        let mut expected = selection;
                        let accepted = flags & 16 == 0 && (forced || distance > 56);
                        if flags & 16 == 0 {
                            expected.display_status &= 127;
                        }
                        if accepted {
                            expected.control_flags |= if forced { 24 } else { 8 };
                            expected.candidate = Some(owner);
                            expected.distance = 56;
                            expected.auxiliary_distance = 100;
                            expected.position = position;
                            expected.pitch = 0;
                            expected.yaw = 0;
                            expected.screen = [112, 96];
                            expected.clipped_yaw = 0;
                        }
                        assert_eq!(consider(&mut selection, owner, position, anchor), accepted);
                        assert_eq!(selection, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn geometry_preserves_half_before_subtraction_and_mixed_width_distance() {
        for word in 0..=u16::MAX {
            let anchor = TargetAnchor {
                position: Vector3 {
                    x: i16::MIN,
                    y: i16::MAX,
                    z: -137,
                },
                pitch: word.wrapping_mul(19),
                yaw: word.wrapping_mul(53),
            };
            let position = Vector3 {
                x: word as i16,
                y: (word ^ 0xC3A5) as i16,
                z: (word as i16).wrapping_neg(),
            };
            let difference = |a: i16, b: i16| -> i32 {
                (i32::from(a).div_euclid(2) - i32::from(b).div_euclid(2)) as i16 as i32
            };
            let mut distance = (difference(position.x, anchor.position.x).abs()
                + difference(position.z, anchor.position.z).abs())
                as u16;
            let mut vertical = difference(position.y, anchor.position.y);
            while distance >= 4096 {
                distance /= 2;
                vertical = vertical.div_euclid(2);
            }
            let pitch = sf2_atan16(vertical as i16, distance as i16).wrapping_add(anchor.pitch);
            let yaw = anchor.yaw.wrapping_sub(sf2_atan16(
                position.x.wrapping_sub(anchor.position.x),
                position.z.wrapping_sub(anchor.position.z),
            ));
            let byte_distance =
                |a: i16, b: i16| ((i32::from(a) - i32::from(b)) as i8 as i16).abs() as u16;
            let near = byte_distance(position.x, anchor.position.x)
                + byte_distance(position.z, anchor.position.z);
            assert_eq!(
                angles_and_auxiliary_range(position, anchor),
                (pitch, yaw, distance / 256 * 256 + near % 256)
            );
        }
    }

    #[test]
    fn accepted_distance_keeps_signed_bits_and_rejection_compares_existing_unsigned_word() {
        let owner = candidate();
        let anchor = TargetAnchor {
            position: Vector3::default(),
            pitch: 0,
            yaw: 0,
        };
        let mut negative_distances = 0;
        for bits in 0..=u16::MAX {
            let position = Vector3 {
                x: bits as i16,
                y: 0,
                z: (bits as i16).wrapping_neg(),
            };
            let distance =
                sf2_xz_angle_distance(position.x.wrapping_neg(), position.z.wrapping_neg());
            if distance < 0 {
                negative_distances += 1;
            }
            for previous in [
                distance.unsigned_abs(),
                distance.unsigned_abs().wrapping_add(1),
                65535,
            ] {
                let mut state = TargetSelection {
                    distance: previous,
                    ..TargetSelection::default()
                };
                let accepted = distance.unsigned_abs() < previous;
                assert_eq!(consider(&mut state, owner, position, anchor), accepted);
                assert_eq!(
                    state.distance,
                    if accepted { distance as u16 } else { previous }
                );
            }
        }
        assert!(negative_distances > 0);
    }
}
