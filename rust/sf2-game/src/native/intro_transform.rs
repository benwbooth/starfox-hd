//! Authored SF2 point-block rotation. Encoding and mirror boundaries are
//! semantic inputs: independently rotating the expanded vertex catalog loses
//! source rounding. Object matrices retain the original shortcut rounding.

use sf2_data::{
    point_program::{PointBlock, PointFormat, PointProgram},
    shape_data::{ShapeDataEntry, ShapeVertex},
};
use sf_core::snes_trig::{gsu_fmult_q15, zxy_matrix_q15_fine};

/// Original `$01:964E..96BA` object-local light vector, in signed byte units.
/// The source dots each input-axis row of the composed object/view matrix
/// with its fixed view-space light. Each product is truncated before the
/// wrapping sum; extracting the signed high byte happens only at the end.
pub fn object_light_direction(object_view: [[i16; 3]; 3]) -> [i8; 3] {
    object_view.map(|row| {
        let terms = row.map(|coefficient| gsu_fmult_q15(0x49E5, coefficient));
        (terms[0].wrapping_add(terms[1]).wrapping_add(terms[2]) >> 8) as i8
    })
}

/// Original `$01:9539..964E` object/view composition. Angles are the raw
/// draw-record words: low bytes encode rotations, while nonzero high bytes
/// also affect shortcut eligibility. Both matrices use input-axis-first order.
pub fn object_view_matrix(view: [[i16; 3]; 3], angles: [u16; 3], flags: u16) -> [[i16; 3]; 3] {
    let flatten = flags & 0x2000 != 0;
    if angles[0] | angles[2] == 0 && matches!(angles[1], 0 | 128) {
        let mut result = view;
        if angles[1] == 128 {
            result[0] = result[0].map(i16::wrapping_neg);
            result[2] = result[2].map(i16::wrapping_neg);
        }
        if flatten {
            result[1] = [0; 3];
        }
        return result;
    }
    let [pitch, yaw, roll] = angles.map(|angle| ((angle & 255) << 8).wrapping_neg());
    let rotation = zxy_matrix_q15_fine(pitch, yaw, roll);
    let mut local: [[i16; 3]; 3] =
        std::array::from_fn(|input| std::array::from_fn(|output| rotation[output][input]));
    if flatten {
        for row in &mut local {
            row[1] = 0;
        }
    }
    std::array::from_fn(|input| {
        std::array::from_fn(|output| {
            gsu_fmult_q15(local[input][0], view[0][output])
                .wrapping_add(gsu_fmult_q15(local[input][1], view[1][output]))
                .wrapping_add(gsu_fmult_q15(local[input][2], view[2][output]))
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointTransform {
    /// First index is input axis; second is output axis. This is the source
    /// m11,m12,m13,m21,... storage order, with signed Q15 coefficients.
    columns: [[i16; 3]; 3],
    shift: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointTransformError {
    InvalidShift(u8),
    FrameCount,
    NoncontiguousBlock(u16),
    ZeroCount,
    MissingVertices,
    ByteCoordinate,
    MirrorMismatch,
}

impl PointTransform {
    pub fn new(columns: [[i16; 3]; 3], shift: u8) -> Result<Self, PointTransformError> {
        if shift > 15 {
            return Err(PointTransformError::InvalidShift(shift));
        }
        Ok(Self { columns, shift })
    }

    fn rotate(
        self,
        point: ShapeVertex,
        format: PointFormat,
        mirrored: bool,
    ) -> ([i16; 3], [i16; 3]) {
        let point = [point.x, point.y, point.z];
        let scale = (1u16 << self.shift) as i8;
        let mut plus = [0; 3];
        let mut minus = [0; 3];
        for output in 0..3 {
            if format == PointFormat::Bytes && self.shift < 3 {
                let products: [i16; 3] = std::array::from_fn(|axis| {
                    i16::from((self.columns[axis][output] >> 8) as i8)
                        * i16::from(point[axis] as i8)
                });
                let yz = products[1].wrapping_add(products[2]);
                let finish =
                    |sum: i16| i16::from((sum.wrapping_mul(2) >> 8) as i8) * i16::from(scale);
                plus[output] = finish(yz.wrapping_add(products[0]));
                minus[output] = finish(yz.wrapping_sub(products[0]));
            } else {
                let products: [i32; 3] = std::array::from_fn(|axis| {
                    let coordinate = if format == PointFormat::Bytes {
                        i16::from(point[axis] as i8) * i16::from(scale)
                    } else {
                        point[axis]
                    };
                    i32::from(coordinate) * i32::from(self.columns[axis][output])
                });
                if format == PointFormat::Bytes && !mirrored {
                    plus[output] = (products[0]
                        .wrapping_add(products[1])
                        .wrapping_add(products[2])
                        >> 15) as i16;
                } else {
                    let terms = products.map(|product| (product >> 15) as i16);
                    plus[output] = terms[0].wrapping_add(terms[1]).wrapping_add(terms[2]);
                    minus[output] = plus[output].wrapping_sub(terms[0].wrapping_mul(2));
                }
            }
        }
        (plus, minus)
    }
}

/// Transform one complete decoded frame, checking that metadata covers it
/// exactly. Mirrored companions are validated but never independently rotated.
pub fn transform_points(
    vertices: &[ShapeVertex],
    blocks: &[PointBlock],
    transform: PointTransform,
) -> Result<Vec<[i16; 3]>, PointTransformError> {
    let mut result = Vec::with_capacity(vertices.len());
    for block in blocks {
        if usize::from(block.first_vertex) != result.len() {
            return Err(PointTransformError::NoncontiguousBlock(block.first_vertex));
        }
        if block.count == 0 {
            return Err(PointTransformError::ZeroCount);
        }
        for _ in 0..block.count {
            let point = *vertices
                .get(result.len())
                .ok_or(PointTransformError::MissingVertices)?;
            if block.format == PointFormat::Bytes
                && [point.x, point.y, point.z]
                    .into_iter()
                    .any(|coordinate| i8::try_from(coordinate).is_err())
            {
                return Err(PointTransformError::ByteCoordinate);
            }
            let (plus, minus) = transform.rotate(point, block.format, block.mirrored);
            result.push(plus);
            if block.mirrored {
                let companion = vertices
                    .get(result.len())
                    .ok_or(PointTransformError::MissingVertices)?;
                if *companion
                    != (ShapeVertex {
                        x: point.x.wrapping_neg(),
                        y: point.y,
                        z: point.z,
                    })
                {
                    return Err(PointTransformError::MirrorMismatch);
                }
                result.push(minus);
            }
        }
    }
    if result.len() != vertices.len() {
        return Err(PointTransformError::MissingVertices);
    }
    Ok(result)
}

pub fn transform_shape(
    shape: &ShapeDataEntry,
    program: &PointProgram,
    animation: u16,
    columns: [[i16; 3]; 3],
) -> Result<Vec<[i16; 3]>, PointTransformError> {
    let frame_count = shape.animation_frames.len().max(1);
    if program.frames.len() != frame_count {
        return Err(PointTransformError::FrameCount);
    }
    let vertices = if shape.animation_frames.is_empty() {
        shape.vertices
    } else {
        shape.animation_frames[usize::from(animation & 63) % frame_count]
    };
    transform_points(
        vertices,
        program
            .frame(animation)
            .ok_or(PointTransformError::FrameCount)?,
        PointTransform::new(columns, shape.shift)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_shortcuts_preserve_coefficients_without_identity_rounding() {
        let view = [[i16::MIN, i16::MAX, -1], [255, 0, -256], [1, -2, 3]];
        assert_eq!(object_view_matrix(view, [0; 3], 0), view);
        assert_eq!(
            object_view_matrix(view, [0, 128, 0], 0x2000),
            [[i16::MIN, -32767, 1], [0; 3], [-1, 2, -3]]
        );
    }

    #[test]
    fn high_angle_flags_force_general_rounding_even_at_zero_rotation() {
        let view = [[32767; 3]; 3];
        assert_eq!(object_view_matrix(view, [0; 3], 0), view);
        assert_eq!(object_view_matrix(view, [256, 0, 0], 0), [[32765; 3]; 3]);
    }

    fn block(format: PointFormat, mirrored: bool) -> PointBlock {
        PointBlock {
            source_address: 0,
            format,
            mirrored,
            first_vertex: 0,
            count: 1,
        }
    }

    #[test]
    fn ordinary_full_bytes_round_once_but_mirrors_round_each_term() {
        let transform = PointTransform::new([[32767, 0, 0]; 3], 3).unwrap();
        let point = ShapeVertex { x: 1, y: 1, z: 1 };
        let companion = ShapeVertex { x: -1, ..point };
        assert_eq!(
            transform_points(&[point], &[block(PointFormat::Bytes, false)], transform).unwrap(),
            [[23, 0, 0]]
        );
        assert_eq!(
            transform_points(
                &[point, companion],
                &[block(PointFormat::Bytes, true)],
                transform
            )
            .unwrap(),
            [[21, 0, 0], [7, 0, 0]]
        );
    }

    #[test]
    fn mirrored_words_reuse_the_rounded_positive_x_contribution() {
        let transform = PointTransform::new([[32767, 0, 0]; 3], 0).unwrap();
        let point = ShapeVertex { x: 1, y: 0, z: 0 };
        let companion = ShapeVertex { x: -1, ..point };
        assert_eq!(
            transform_points(
                &[point, companion],
                &[block(PointFormat::Words, true)],
                transform
            )
            .unwrap(),
            [[0; 3], [0; 3]]
        );
        assert_eq!(
            transform_points(&[companion], &[block(PointFormat::Words, false)], transform).unwrap(),
            [[-1, 0, 0]]
        );
    }

    #[test]
    fn metadata_errors_do_not_silently_skip_or_synthesize_vertices() {
        let transform = PointTransform::new([[0; 3]; 3], 0).unwrap();
        let point = ShapeVertex { x: 128, y: 0, z: 0 };
        assert_eq!(
            PointTransform::new([[0; 3]; 3], 16),
            Err(PointTransformError::InvalidShift(16))
        );
        assert_eq!(
            transform_points(&[point], &[block(PointFormat::Bytes, false)], transform),
            Err(PointTransformError::ByteCoordinate)
        );
        assert_eq!(
            transform_points(&[], &[block(PointFormat::Words, false)], transform),
            Err(PointTransformError::MissingVertices)
        );
        assert_eq!(
            transform_points(&[point], &[], transform),
            Err(PointTransformError::MissingVertices)
        );
        assert_eq!(
            transform_points(
                &[],
                &[PointBlock {
                    count: 0,
                    ..block(PointFormat::Words, false)
                }],
                transform
            ),
            Err(PointTransformError::ZeroCount)
        );
        assert_eq!(
            transform_points(
                &[point],
                &[PointBlock {
                    first_vertex: 1,
                    ..block(PointFormat::Words, false)
                }],
                transform
            ),
            Err(PointTransformError::NoncontiguousBlock(1))
        );
        assert_eq!(
            transform_points(
                &[point, point],
                &[block(PointFormat::Words, true)],
                transform
            ),
            Err(PointTransformError::MirrorMismatch)
        );
    }

    #[test]
    fn frame_count_disagreement_is_rejected() {
        let shape = &sf2_data::shape_data::SHAPE_DATA[0];
        let program = PointProgram { frames: &[] };
        assert_eq!(
            transform_shape(shape, &program, 0, [[0; 3]; 3]),
            Err(PointTransformError::FrameCount)
        );
    }
}
