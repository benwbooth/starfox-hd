//! Native camera-relative placement and ordered draw preparation (`$01:D28B`).
//! Inputs are semantic state, not machine RAM or captured display lists.

use sf_core::snes_trig::matrix_rotate_q15;

use super::object::Vector3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewTransform {
    pub position: Vector3,
    /// Signed Q15 coefficients, indexed by input axis then output axis.
    pub matrix: [[i16; 3]; 3],
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ShadowPlacement {
    #[default]
    None,
    Ground {
        height: i16,
    },
    /// A shadow-shaped object copies its own camera-space position. This
    /// takes precedence over a ground shadow when both source flags are set.
    Object,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawPlacement {
    pub position: Vector3,
    pub shadow: ShadowPlacement,
    pub shape_sort_bias: i16,
    pub object_sort_bias: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedPlacement {
    pub position: Vector3,
    pub shadow: Option<Vector3>,
    pub sort_depth: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDrawList {
    /// Placements retain input order and identity.
    pub placements: Vec<PreparedPlacement>,
    /// Indices into `placements`, in source submission order.
    pub order: Vec<usize>,
}

impl ViewTransform {
    fn rotate(self, position: Vector3) -> Vector3 {
        let (x, y, z) = matrix_rotate_q15(self.matrix, position.x, position.y, position.z);
        Vector3 { x, y, z }
    }

    pub fn prepare(self, placement: DrawPlacement) -> PreparedPlacement {
        let relative = Vector3 {
            x: placement.position.x.wrapping_sub(self.position.x),
            y: placement.position.y.wrapping_sub(self.position.y),
            z: placement.position.z.wrapping_sub(self.position.z),
        };
        let position = self.rotate(relative);
        let shadow = match placement.shadow {
            ShadowPlacement::None => None,
            ShadowPlacement::Ground { height } => Some(self.rotate(Vector3 {
                y: height.wrapping_sub(self.position.y),
                ..relative
            })),
            ShadowPlacement::Object => Some(position),
        };
        // D3E6 computes |y| but D3EE explicitly clears it. D401 adds |x|
        // to Z, then D402 adds it to the header's low byte in r0. GETBH
        // replaces that partial sum's high byte, discarding its carry.
        // MIN.wrapping_abs() deliberately stays negative.
        let horizontal = position.x.wrapping_abs();
        let biased_shape = (placement.shape_sort_bias & !255)
            | (placement.shape_sort_bias.wrapping_add(horizontal) & 255);
        let sort_depth = position
            .z
            .wrapping_add(horizontal)
            .wrapping_add(biased_shape)
            .wrapping_add(placement.object_sort_bias);
        PreparedPlacement {
            position,
            shadow,
            sort_depth,
        }
    }
}

/// Reproduce source insertion order, including equal-key reversal and signed
/// subtraction overflow. A conventional stable/comparison sort is not equal
/// to this post-decrement, sign-bit-only comparison at word boundaries.
pub fn prepare_draw_list(view: ViewTransform, objects: &[DrawPlacement]) -> PreparedDrawList {
    let placements: Vec<_> = objects.iter().map(|object| view.prepare(*object)).collect();
    let mut order: Vec<usize> = Vec::with_capacity(objects.len());
    for (index, object) in placements.iter().enumerate() {
        let insertion = order
            .iter()
            .position(|&earlier| {
                placements[earlier]
                    .sort_depth
                    .wrapping_sub(1)
                    .wrapping_sub(object.sort_depth)
                    < 0
            })
            .unwrap_or(order.len());
        order.insert(insertion, index);
    }
    PreparedDrawList { placements, order }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_sort_bias_discards_low_byte_carry() {
        let view = ViewTransform {
            position: Vector3::default(),
            matrix: [[32767, 0, 0], [0, 32767, 0], [0, 0, 32767]],
        };
        let prepared = view.prepare(DrawPlacement {
            position: Vector3 {
                x: 257,
                y: -300,
                z: 0,
            },
            shadow: ShadowPlacement::None,
            shape_sort_bias: 0x1FE,
            object_sort_bias: 0,
        });
        assert_eq!(prepared.position.x, 256);
        assert_eq!(prepared.sort_depth, 766);
    }

    #[test]
    fn world_subtraction_wraps_before_rotation() {
        let view = ViewTransform {
            position: Vector3 {
                x: i16::MAX,
                y: i16::MAX,
                z: i16::MAX,
            },
            matrix: [[i16::MIN, 0, 0], [0, i16::MIN, 0], [0, 0, i16::MIN]],
        };
        let prepared = view.prepare(DrawPlacement {
            position: Vector3 {
                x: i16::MIN,
                y: i16::MIN,
                z: i16::MIN,
            },
            shadow: ShadowPlacement::Ground { height: i16::MIN },
            shape_sort_bias: 0,
            object_sort_bias: 0,
        });
        assert_eq!(
            prepared.position,
            Vector3 {
                x: -1,
                y: -1,
                z: -1
            }
        );
        assert_eq!(prepared.shadow, Some(prepared.position));
    }

    #[test]
    fn equal_depths_reverse_insertion_and_empty_list_stays_empty() {
        let view = ViewTransform {
            position: Vector3::default(),
            matrix: [[0; 3]; 3],
        };
        assert!(prepare_draw_list(view, &[]).order.is_empty());
        let object = DrawPlacement {
            position: Vector3::default(),
            shadow: ShadowPlacement::None,
            shape_sort_bias: 0,
            object_sort_bias: 5,
        };
        assert_eq!(prepare_draw_list(view, &[object; 4]).order, [3, 2, 1, 0]);
    }

    #[test]
    fn ordering_uses_wrapping_difference_not_signed_key_comparison() {
        let view = ViewTransform {
            position: Vector3::default(),
            matrix: [[0; 3]; 3],
        };
        let objects = [i16::MIN, i16::MAX].map(|object_sort_bias| DrawPlacement {
            position: Vector3::default(),
            shadow: ShadowPlacement::None,
            shape_sort_bias: 0,
            object_sort_bias,
        });
        assert_eq!(prepare_draw_list(view, &objects).order, [0, 1]);
    }
}
