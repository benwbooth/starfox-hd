//! Object-contact box arithmetic (`$7F:4100..43D4`, `$7F:4538..48F1`).
//! These are not the downward-contact plane/polygon profiles.

use super::{Angle, Rotation, Vector3};
use sf2_data::contact_box_data::{contact_boxes_by_index, ContactBoxGroupId};

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

#[derive(Debug, Clone, Copy)]
pub struct Collider {
    pub position: Vector3,
    pub rotation: Rotation,
    pub half_extents: [u16; 3],
    pub boxes: Option<ContactBoxGroupId>,
    /// High bit selects an explicit frame; otherwise the shared clock wins.
    pub animation_frame: u8,
}

impl Collider {
    pub fn from_shape(
        shape: super::ShapeId,
        position: Vector3,
        rotation: Rotation,
        animation_frame: u8,
    ) -> Option<Self> {
        Some(Self {
            position,
            rotation,
            half_extents: shape.catalog_entry()?.bounds,
            boxes: contact_boxes_by_index(usize::from(shape.catalog_index())),
            animation_frame,
        })
    }

    pub fn world_boxes(self, strategy_clock: u8) -> WorldBoxes {
        let frame = if self.animation_frame & 0x80 != 0 {
            self.animation_frame
        } else {
            strategy_clock
        };
        WorldBoxes {
            collider: self,
            next: self.boxes,
            frame,
            ordinary: self.boxes.is_none(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldBox {
    pub center: Vector3,
    pub half_extents: [u16; 3],
    pub hit_flags: u8,
}

pub struct WorldBoxes {
    collider: Collider,
    next: Option<ContactBoxGroupId>,
    frame: u8,
    ordinary: bool,
}

impl Iterator for WorldBoxes {
    type Item = WorldBox;

    fn next(&mut self) -> Option<Self::Item> {
        if std::mem::take(&mut self.ordinary) {
            return Some(WorldBox {
                center: self.collider.position,
                half_extents: self.collider.half_extents,
                hit_flags: 0,
            });
        }
        let group = self.next?.group();
        let record = group.variants[usize::from(self.frame & group.variant_mask)];
        self.next = record.next;
        let [x, y, z] = record.center;
        Some(WorldBox {
            center: center(
                self.collider.position,
                self.collider.rotation,
                Vector3 { x, y, z },
                CenterRotation::from_authored_flags(record.rotation_flags),
                u32::from(record.rotation_flags & 0x0F),
            ),
            half_extents: record.half_extents,
            hit_flags: record.hit_flags,
        })
    }
}

pub fn boxes_overlap(first: WorldBox, second: WorldBox) -> bool {
    // Source comparison order is depth, horizontal, vertical.
    axis_overlaps(
        first.center.z,
        second.center.z,
        first.half_extents[2],
        second.half_extents[2],
    ) && axis_overlaps(
        first.center.x,
        second.center.x,
        first.half_extents[0],
        second.half_extents[0],
    ) && axis_overlaps(
        first.center.y,
        second.center.y,
        first.half_extents[1],
        second.half_extents[1],
    )
}

/// A compound target accumulates all overlapping box flags. An overlap with
/// only zero-flag boxes does NOT create a contact. An ordinary target instead
/// leaves its previous directional flags untouched (the inner None).
pub fn hit_by_probe(probe: WorldBox, target: Collider, strategy_clock: u8) -> Option<Option<u8>> {
    if !boxes_overlap(
        probe,
        WorldBox {
            center: target.position,
            half_extents: target.half_extents,
            hit_flags: 0,
        },
    ) {
        return None;
    }
    if target.boxes.is_none() {
        return Some(None);
    }
    let mut flags = 0;
    for candidate in target.world_boxes(strategy_clock) {
        if boxes_overlap(probe, candidate) {
            flags |= candidate.hit_flags;
        }
    }
    (flags != 0).then_some(Some(flags))
}

/// Boolean query only; the full pass must still visit every probe box so
/// repeated hits refresh the contact count in the source order.
pub fn any_overlap(first: Collider, second: Collider, strategy_clock: u8) -> bool {
    first
        .world_boxes(strategy_clock)
        .any(|probe| hit_by_probe(probe, second, strategy_clock).is_some())
}

/// Existing geometry-only callers do not own the shared strategy clock.
/// They may query nonanimated profiles, but cannot silently substitute a
/// frame for an animated one. All currently extracted contact boxes are static.
pub fn static_overlap(first: Collider, second: Collider) -> Option<bool> {
    for collider in [first, second] {
        let mut next = collider.boxes;
        while let Some(id) = next {
            let group = id.group();
            if group.variant_mask != 0 {
                return None;
            }
            next = group.variants[0].next;
        }
    }
    // The validation above proves this clock value is never used to select
    // a different variant; animated gameplay must call any_overlap instead.
    Some(any_overlap(first, second, 0))
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

    #[test]
    fn extracted_contact_graphs_are_static_and_have_exact_zero_angle_laser_centers() {
        let collider = Collider::from_shape(
            super::super::ShapeId::ENEMY_LASER,
            Vector3::default(),
            Rotation::default(),
            0,
        )
        .unwrap();
        let boxes: Vec<_> = collider.world_boxes(255).collect();
        assert_eq!(boxes.len(), 3);
        assert_eq!(
            boxes.iter().map(|item| item.center.z).collect::<Vec<_>>(),
            vec![0, -80, -160]
        );
        assert!(boxes
            .iter()
            .all(|item| item.half_extents == [40; 3] && item.hit_flags == 7));
        assert_eq!(sf2_data::contact_box_data::CONTACT_BOX_SHAPE_COUNT, 61);
        for shape in sf2_data::shape_data::SHAPE_DATA {
            let boxes = contact_boxes_by_index(usize::from(shape.header_index));
            let collider = Collider { boxes, ..collider };
            assert!(static_overlap(collider, collider).is_some());
        }
    }

    #[test]
    fn compound_targets_accumulate_flags_while_ordinary_targets_leave_flags_unchanged() {
        let player = Collider::from_shape(
            super::super::ShapeId::FOX_FALCO_FLIGHT_CRAFT,
            Vector3::default(),
            Rotation::default(),
            0,
        )
        .unwrap();
        let probe = WorldBox {
            center: Vector3::default(),
            half_extents: [40; 3],
            hit_flags: 7,
        };
        assert_eq!(hit_by_probe(probe, player, 0), Some(Some(7)));
        let ordinary = Collider {
            boxes: None,
            ..player
        };
        assert_eq!(hit_by_probe(probe, ordinary, 0), Some(None));
        let distant = WorldBox {
            center: Vector3 { x: 500, y: 0, z: 0 },
            ..probe
        };
        assert_eq!(hit_by_probe(distant, ordinary, 0), None);
    }
}
