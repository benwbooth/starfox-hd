//! Integer face visibility from source-resolution projected shape points.
//! HD interpolation and floating-point screen coordinates do not participate
//! in these signs or in the resulting BSP submission order.

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedShapePoint {
    pub x: i16,
    pub y: i16,
    pub outcode: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityMode {
    FullPrecision,
    OnScreen,
}

/// Select the ordinary mesh path from the complete projected-point set.
/// Source outcodes carry outside-plane bits in low bits 0..4 and their
/// complemented inside-plane bits in high bits 8..12. None means every point
/// is outside at least one shared plane. Recomputing avoids sticky mode flags.
pub fn select_visibility_mode(points: &[ProjectedShapePoint]) -> Option<VisibilityMode> {
    let aggregate = points.iter().fold(0u16, |bits, point| bits | point.outcode);
    if aggregate & 0x1F00 != 0x1F00 {
        None
    } else if aggregate & 0x001F == 0 {
        Some(VisibilityMode::OnScreen)
    } else {
        Some(VisibilityMode::FullPrecision)
    }
}

/// Preserve the complete byte, not only its sign: source face and sprite
/// consumers share this table. BSP decisions use the negative bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeVisibility(pub u8);

impl ShapeVisibility {
    pub const fn is_negative(self) -> bool {
        self.0 & 0x80 != 0
    }
}

pub fn triangle_visibility(
    [a, b, c]: [ProjectedShapePoint; 3],
    mode: VisibilityMode,
) -> ShapeVisibility {
    let packed = match mode {
        VisibilityMode::FullPrecision => {
            let ab_x = i32::from(b.x.wrapping_sub(a.x));
            let ab_y = i32::from(b.y.wrapping_sub(a.y));
            let ac_x = i32::from(c.x.wrapping_sub(a.x));
            let ac_y = i32::from(c.y.wrapping_sub(a.y));
            let area = ab_x
                .wrapping_mul(ac_y)
                .wrapping_sub(ab_y.wrapping_mul(ac_x));
            // Two signed long products and a low/high subtract produce the
            // 32-bit determinant. Source stores its top byte, XORed with the
            // low outcode bits shifted three places (including the Z sign).
            (area >> 24) as u8 ^ ((a.outcode ^ b.outcode ^ c.outcode) << 3) as u8
        }
        VisibilityMode::OnScreen => {
            // This path reads low coordinate bytes before subtracting, then
            // arithmetic-shifts each delta before signed byte multiplies.
            let delta = |to: i16, from: i16| (i16::from(to as u8) - i16::from(from as u8)) >> 1;
            let ab_x = delta(b.x, a.x);
            let ab_y = delta(b.y, a.y);
            let ac_x = delta(c.x, a.x);
            let ac_y = delta(c.y, a.y);
            let area = ab_x
                .wrapping_mul(ac_y)
                .wrapping_sub(ab_y.wrapping_mul(ac_x));
            (area as u16 >> 8) as u8
        }
    };
    ShapeVisibility(packed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingVisibilityPoint(pub u8);

pub fn visibility_table(
    points: &[ProjectedShapePoint],
    triangles: &[[u8; 3]],
    mode: VisibilityMode,
) -> Result<Vec<ShapeVisibility>, MissingVisibilityPoint> {
    triangles
        .iter()
        .map(|indices| {
            let mut triangle = [ProjectedShapePoint::default(); 3];
            for (destination, index) in triangle.iter_mut().zip(indices) {
                *destination = *points
                    .get(usize::from(*index))
                    .ok_or(MissingVisibilityPoint(*index))?;
            }
            Ok(triangle_visibility(triangle, mode))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedBspSubmission {
    pub mode: VisibilityMode,
    pub visibility: Vec<ShapeVisibility>,
    pub bsp: super::intro_bsp_work::BspSubmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectedBspError {
    MissingRoot,
    ExpectedVisibilityTable,
    ExpectedBspRoot,
    InvalidTableLength(usize),
    MissingPoint(MissingVisibilityPoint),
    Bsp(super::intro_bsp_work::BspSubmissionError),
}

/// Run the ordinary visibility-table/BSP prefix of an authored shape program
/// using native projected points. Culled objects submit no lists. Programs
/// without this prefix belong to other shape paths and are reported explicitly.
pub fn submit_projected_bsp(
    program: &sf2_data::shape_program::FaceProgram,
    points: &[ProjectedShapePoint],
) -> Result<Option<ProjectedBspSubmission>, ProjectedBspError> {
    use sf2_data::shape_program::FaceCommand;
    let Some(mode) = select_visibility_mode(points) else {
        return Ok(None);
    };
    let root = program.root.ok_or(ProjectedBspError::MissingRoot)?;
    let node = program.node(root).ok_or(ProjectedBspError::MissingRoot)?;
    let FaceCommand::Visibility { triangles, next } = node.command else {
        return Err(ProjectedBspError::ExpectedVisibilityTable);
    };
    if !(1..=255).contains(&triangles.len()) {
        return Err(ProjectedBspError::InvalidTableLength(triangles.len()));
    }
    if !program
        .node(next)
        .is_some_and(|node| matches!(node.command, FaceCommand::BeginBsp { .. }))
    {
        return Err(ProjectedBspError::ExpectedBspRoot);
    }
    let visibility =
        visibility_table(points, triangles, mode).map_err(ProjectedBspError::MissingPoint)?;
    let signs: Vec<_> = visibility.iter().map(|flag| flag.is_negative()).collect();
    let bsp =
        super::intro_bsp_work::submit_bsp(program, next, &signs).map_err(ProjectedBspError::Bsp)?;
    Ok(Some(ProjectedBspSubmission {
        mode,
        visibility,
        bsp,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_path_quantization_can_change_the_visibility_sign() {
        let points =
            [(50, 100), (51, 100), (51, 71)].map(|(x, y)| ProjectedShapePoint { x, y, outcode: 0 });
        assert_eq!(
            triangle_visibility(points, VisibilityMode::FullPrecision),
            ShapeVisibility(0xFF)
        );
        assert_eq!(
            triangle_visibility(points, VisibilityMode::OnScreen),
            ShapeVisibility(0)
        );
    }

    #[test]
    fn full_path_preserves_outcode_bits_and_wrapped_coordinate_deltas() {
        let mut points = [(32767, 0), (-32768, 0), (32767, -1)].map(|(x, y)| ProjectedShapePoint {
            x,
            y,
            outcode: 0,
        });
        assert_eq!(
            triangle_visibility(points, VisibilityMode::FullPrecision),
            ShapeVisibility(0xFF)
        );
        points[1].outcode = 0x13;
        assert_eq!(
            triangle_visibility(points, VisibilityMode::FullPrecision),
            ShapeVisibility(0x67)
        );
    }

    #[test]
    fn missing_points_are_errors_not_default_visible_faces() {
        assert_eq!(
            visibility_table(&[], &[[0, 1, 2]], VisibilityMode::FullPrecision),
            Err(MissingVisibilityPoint(0))
        );
    }

    #[test]
    fn every_projected_point_participates_in_mode_selection() {
        use sf2_data::shape_program::{FaceCommand, FaceNode, FaceProgram, NodeId};
        const fn node(command: FaceCommand) -> FaceNode {
            FaceNode {
                source_address: 0,
                command,
            }
        }
        static NODES: [FaceNode; 5] = [
            node(FaceCommand::Visibility {
                triangles: &[[0, 1, 2]],
                next: NodeId(1),
            }),
            node(FaceCommand::BeginBsp { root: NodeId(2) }),
            node(FaceCommand::Bsp {
                visibility: 0,
                coplanar: NodeId(3),
                left: NodeId(4),
                right: None,
            }),
            node(FaceCommand::Quit),
            node(FaceCommand::ReturnBsp),
        ];
        let program = FaceProgram {
            root: Some(NodeId(0)),
            nodes: &NODES,
        };
        let mut points: Vec<_> = [(50, 100), (51, 100), (51, 71)]
            .map(|(x, y)| ProjectedShapePoint {
                x,
                y,
                outcode: 0x1F00,
            })
            .into();
        let inside = submit_projected_bsp(&program, &points).unwrap().unwrap();
        assert_eq!(inside.mode, VisibilityMode::OnScreen);
        assert!(inside.bsp.face_lists.is_empty());
        // This vertex is not used by the visibility triangle, but changes
        // the whole object's source mode and therefore its quantized sign.
        points.push(ProjectedShapePoint {
            x: -1,
            y: 100,
            outcode: 0x1B04,
        });
        let crossing = submit_projected_bsp(&program, &points).unwrap().unwrap();
        assert_eq!(crossing.mode, VisibilityMode::FullPrecision);
        assert_eq!(crossing.bsp.face_lists, [NodeId(3)]);
        for point in &mut points {
            point.outcode = 0x1B04;
        }
        assert_eq!(submit_projected_bsp(&program, &points), Ok(None));
    }
}
