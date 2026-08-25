//! Exact fixed-point projection used at native fixed-update boundaries.
//!
//! The HD path remains smooth and floating-point between updates. At an exact
//! source boundary, this module preserves the authored integer matrix,
//! coordinate scaling, projection, and visibility decisions so the
//! source-resolution conformance image has deterministic geometry.

use crate::shape_data::ShapeVertex;
use sf_core::snes_trig::{
    matrix_rotate_q15, rotate_packed_point, zxy_matrix_q15, zxy_matrix_q15_fine,
};

const PACKED_MATRIX_SHIFT: u32 = 8;
const FULL_PRECISION_POINT_SHIFT: u8 = 3;
pub const MIN_FRONT_DEPTH: i16 = 1;
const PROJECTION_MAX_DEPTH: i16 = 12_288;
const NEAR_PROJECTION_DEPTH: i16 = 256;
const NEAR_PROJECTION_SCALE: i16 = 16;
const NEAR_PROJECTION_COMPONENT_LIMIT: i16 = 1_024;
const RECIPROCAL_NUMERATOR: i32 = 32_767 * 256;
const RECIPROCAL_FRACTION_BITS: u32 = 15;
const MAX_RECIPROCAL: i16 = 32_767;
const PROJECTION_CENTER_X: i16 = 112;
const PROJECTION_CENTER_Y: i16 = 96;
const SCALED_SPRITE_NEAR_DEPTH: i16 = 128;
const SCALED_SPRITE_MAX_SIZE: i16 = 240;
const PLAYFIELD_LEFT: i16 = 16;
const PLAYFIELD_RIGHT: i16 = 239;
const PLAYFIELD_TOP: i16 = 16;
const PLAYFIELD_BOTTOM: i16 = 207;
const LIGHT_COMPONENT_Q15: i16 = 18_917;
const LIGHT_QUANTIZATION_SHIFT: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePose {
    pub world_position: [i16; 3],
    pub rotation: [u8; 3],
    pub view_position: [i16; 3],
    pub view_rotation: [u16; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedPoint {
    /// Final 256-by-224 presentation coordinate, with Y increasing downward.
    pub x: i16,
    pub y: i16,
    pub depth: i16,
}

#[derive(Debug, Clone)]
pub struct ProjectedShape {
    pub points: Vec<ProjectedPoint>,
    pub object_light: [i8; 3],
    /// Source view-space object origin used for distance-color selection.
    pub object_depth: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedSprite {
    /// Final 256-by-224 presentation coordinate of the upper-left pixel.
    pub top_left: [i16; 2],
    pub size: u16,
}

/// Project the typed flat fields consumed by the source scaled-sprite path.
/// The authored extent is doubled before the signed strategy adjustment is
/// applied, matching the source ShapeHdr sizing contract.
pub fn project_scaled_sprite(
    pose: SourcePose,
    authored_extent: u16,
    coordinate_shift: u8,
    size_adjustment: u8,
) -> Option<ProjectedSprite> {
    let (_, view_position) = source_object_transform(pose, false);
    if view_position.2 <= SCALED_SPRITE_NEAR_DEPTH {
        return None;
    }

    let shifted_adjustment = i16::from(size_adjustment as i8)
        .wrapping_shl(u32::from(coordinate_shift));
    let mut world_size = (authored_extent as i16)
        .wrapping_mul(2)
        .wrapping_add(shifted_adjustment);
    if world_size == 0 {
        world_size = 1;
    }
    let depth = view_position.2.min(PROJECTION_MAX_DEPTH - 1);
    let reciprocal = source_reciprocal(depth);
    let projected_size = project_component(world_size, reciprocal)
        .clamp(0, SCALED_SPRITE_MAX_SIZE);
    if projected_size == 0 {
        return None;
    }

    let center = project_point([view_position.0, view_position.1, depth]);
    let half_size = (projected_size + 1) / 2;
    Some(ProjectedSprite {
        top_left: [
            center.x.wrapping_sub(half_size),
            center.y.wrapping_sub(half_size),
        ],
        size: projected_size as u16,
    })
}

pub fn project_shape(
    vertices: &[ShapeVertex],
    coordinate_shift: u8,
    pose: SourcePose,
) -> ProjectedShape {
    project_shape_with_flattened_height(vertices, coordinate_shift, pose, false)
}

/// Project the source shadow pass. The source retains the object's authored
/// X/Z rotation but removes its world-height contribution before applying the
/// camera matrix, producing a fixed-point ground-plane silhouette.
pub fn project_shadow_shape(
    vertices: &[ShapeVertex],
    coordinate_shift: u8,
    pose: SourcePose,
) -> ProjectedShape {
    project_shape_with_flattened_height(vertices, coordinate_shift, pose, true)
}

fn project_shape_with_flattened_height(
    vertices: &[ShapeVertex],
    coordinate_shift: u8,
    pose: SourcePose,
    flatten_height: bool,
) -> ProjectedShape {
    let (object_matrix, view_position) = source_object_transform(pose, flatten_height);
    let points = vertices
        .iter()
        .map(|vertex| {
            let source = [
                vertex.x.round() as i16,
                (-vertex.y).round() as i16,
                vertex.z.round() as i16,
            ];
            let rotated = if coordinate_shift < FULL_PRECISION_POINT_SHIFT {
                rotate_shape_point(object_matrix, source, coordinate_shift)
            } else {
                matrix_rotate_q15(object_matrix, source[0], source[1], source[2])
            };
            let position = [
                rotated.0.wrapping_add(view_position.0),
                rotated.1.wrapping_add(view_position.1),
                rotated.2.wrapping_add(view_position.2),
            ];
            project_point(position)
        })
        .collect();

    ProjectedShape {
        points,
        object_light: quantized_object_light(object_matrix),
        object_depth: view_position.2,
    }
}

fn source_object_transform(
    pose: SourcePose,
    flatten_height: bool,
) -> ([[i16; 3]; 3], (i16, i16, i16)) {
    let view_matrix = zxy_matrix_q15_fine(
        pose.view_rotation[0],
        pose.view_rotation[1],
        pose.view_rotation[2],
    );
    let relative = [
        pose.world_position[0].wrapping_sub(pose.view_position[0]),
        pose.world_position[1].wrapping_sub(pose.view_position[1]),
        pose.world_position[2].wrapping_sub(pose.view_position[2]),
    ];
    let view_position = matrix_rotate_q15(view_matrix, relative[0], relative[1], relative[2]);
    let direct_object = zxy_matrix_q15(
        pose.rotation[0].wrapping_neg(),
        pose.rotation[1].wrapping_neg(),
        pose.rotation[2].wrapping_neg(),
    );
    let mut transposed_object: [[i16; 3]; 3] =
        std::array::from_fn(|row| std::array::from_fn(|column| direct_object[column][row]));
    if flatten_height {
        for row in &mut transposed_object {
            row[1] = 0;
        }
    }
    let object_matrix = std::array::from_fn(|row| {
        let source = transposed_object[row];
        let transformed = matrix_rotate_q15(view_matrix, source[0], source[1], source[2]);
        [transformed.0, transformed.1, transformed.2]
    });
    (object_matrix, view_position)
}

fn quantized_object_light(object_matrix: [[i16; 3]; 3]) -> [i8; 3] {
    let transposed: [[i16; 3]; 3] =
        std::array::from_fn(|row| std::array::from_fn(|column| object_matrix[column][row]));
    let rotated = matrix_rotate_q15(
        transposed,
        LIGHT_COMPONENT_Q15,
        LIGHT_COMPONENT_Q15,
        LIGHT_COMPONENT_Q15,
    );
    let source_basis = [
        (rotated.0 >> LIGHT_QUANTIZATION_SHIFT) as i8,
        (rotated.1 >> LIGHT_QUANTIZATION_SHIFT) as i8,
        (rotated.2 >> LIGHT_QUANTIZATION_SHIFT) as i8,
    ];
    // Shape vertices and normals use a conventional upward-positive Y axis.
    [
        source_basis[0],
        source_basis[1].wrapping_neg(),
        source_basis[2],
    ]
}

fn rotate_shape_point(
    matrix: [[i16; 3]; 3],
    point: [i16; 3],
    coordinate_shift: u8,
) -> (i16, i16, i16) {
    let packed_matrix = matrix.map(|row| {
        row.map(|coefficient| (coefficient >> PACKED_MATRIX_SHIFT) as i8)
    });
    let encoded = point.map(|component| (component >> coordinate_shift) as i8);
    let scale = 1i8.wrapping_shl(u32::from(coordinate_shift));
    rotate_packed_point(
        packed_matrix,
        scale,
        encoded[0],
        encoded[1],
        encoded[2],
    )
}

fn project_point(point: [i16; 3]) -> ProjectedPoint {
    if point[2] < MIN_FRONT_DEPTH {
        return ProjectedPoint {
            x: i16::MIN,
            y: i16::MIN,
            depth: point[2],
        };
    }
    let depth = point[2].min(PROJECTION_MAX_DEPTH - 1);
    let (projection_depth, project_x, project_y) = if depth < NEAR_PROJECTION_DEPTH {
        let scale_component = |component: i16| {
            component
                .clamp(
                    -NEAR_PROJECTION_COMPONENT_LIMIT,
                    NEAR_PROJECTION_COMPONENT_LIMIT,
                )
                .wrapping_mul(NEAR_PROJECTION_SCALE)
        };
        (
            depth.wrapping_mul(NEAR_PROJECTION_SCALE),
            scale_component(point[0]),
            scale_component(point[1]),
        )
    } else {
        (depth, point[0], point[1])
    };
    let reciprocal = source_reciprocal(projection_depth);
    ProjectedPoint {
        x: project_component(project_x, reciprocal)
            .wrapping_add(PROJECTION_CENTER_X)
            .wrapping_add(PLAYFIELD_LEFT),
        y: project_component(project_y, reciprocal)
            .wrapping_add(PROJECTION_CENTER_Y)
            .wrapping_add(PLAYFIELD_TOP),
        depth: point[2],
    }
}

/// Reciprocal-table lookup used by the source renderer. Entries are indexed at
/// even depths, saturate below 256, and retain the table generator's integer
/// truncation. This is ordinary typed projection math, not source-machine
/// execution state.
fn source_reciprocal(depth: i16) -> i16 {
    let even_depth = depth & !1;
    if even_depth < NEAR_PROJECTION_DEPTH {
        MAX_RECIPROCAL
    } else {
        (RECIPROCAL_NUMERATOR / i32::from(even_depth)) as i16
    }
}

fn project_component(component: i16, reciprocal: i16) -> i16 {
    ((i32::from(component) * i32::from(reciprocal)) >> RECIPROCAL_FRACTION_BITS) as i16
}

pub fn face_is_visible(points: &[ProjectedPoint], indices: [u16; 3]) -> bool {
    let Some(a) = points.get(usize::from(indices[0])) else {
        return true;
    };
    let Some(b) = points.get(usize::from(indices[1])) else {
        return true;
    };
    let Some(c) = points.get(usize::from(indices[2])) else {
        return true;
    };
    if [a, b, c]
        .into_iter()
        .any(|point| point.depth < MIN_FRONT_DEPTH)
    {
        return true;
    }
    let ab_x = b.x.wrapping_sub(a.x);
    let ab_y = b.y.wrapping_sub(a.y);
    let ac_x = c.x.wrapping_sub(a.x);
    let ac_y = c.y.wrapping_sub(a.y);
    let visible = if shape_is_fully_inside_playfield(points) {
        source_winding_high_byte(ab_x, ab_y, ac_x, ac_y) < 0
    } else {
        i32::from(ab_x) * i32::from(ac_y) - i32::from(ab_y) * i32::from(ac_x) < 0
    };
    // Projected source coordinates increase downward. The source-visible
    // winding is therefore negative here, opposite the renderer's Y-up clip
    // coordinates.
    visible
}

/// Source `mtestoutcodes`: reject an object when every projected point lies
/// beyond the same playfield edge. Individual faces are clipped only after
/// this whole-shape test, which is why a near-edge object can still leave the
/// retail rasterizer's clamped rightmost column.
pub fn shape_is_outside_playfield(points: &[ProjectedPoint]) -> bool {
    points.is_empty()
        || points.iter().all(|point| point.x < PLAYFIELD_LEFT)
        || points.iter().all(|point| point.x >= PLAYFIELD_RIGHT)
        || points.iter().all(|point| point.y < PLAYFIELD_TOP)
        || points.iter().all(|point| point.y >= PLAYFIELD_BOTTOM)
}

fn shape_is_fully_inside_playfield(points: &[ProjectedPoint]) -> bool {
    points.iter().all(|point| {
        (PLAYFIELD_LEFT..=PLAYFIELD_RIGHT).contains(&point.x)
            && (PLAYFIELD_TOP..=PLAYFIELD_BOTTOM).contains(&point.y)
    })
}

/// Reproduce the source renderer's deliberately low-precision winding test.
/// Each screen-space delta is halved before signed-byte multiplication, the
/// two products wrap as a signed word, and only the high byte is retained.
/// Keeping this as typed geometry arithmetic avoids exposing source processor
/// state in the port while preserving edge-on face decisions exactly.
fn source_winding_high_byte(ab_x: i16, ab_y: i16, ac_x: i16, ac_y: i16) -> i8 {
    let half = |delta: i16| delta >> 1;
    let product = |left: i16, right: i16| {
        i16::from(left as u8 as i8).wrapping_mul(i16::from(right as u8 as i8))
    };
    let cross = product(half(ab_x), half(ac_y)).wrapping_sub(product(half(ac_x), half(ab_y)));
    (cross >> 8) as i8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_shape_rotation_keeps_word_and_signed_byte_wrapping() {
        let matrix = [
            [32_767, 32_767, 32_767],
            [32_767, 32_767, 32_767],
            [32_767, 32_767, 32_767],
        ];
        assert_eq!(rotate_shape_point(matrix, [127, 127, 127], 0), (122, 122, 122));
    }

    #[test]
    fn near_projection_preserves_the_source_reciprocal_table_rounding() {
        let scaled_depth = 231 * NEAR_PROJECTION_SCALE;
        let reciprocal = source_reciprocal(scaled_depth);
        assert_eq!(reciprocal, 2_269);
        assert_eq!(
            project_component(37 * NEAR_PROJECTION_SCALE, reciprocal),
            40
        );
        assert_eq!(
            project_component(-37 * NEAR_PROJECTION_SCALE, reciprocal),
            -41
        );
    }

    #[test]
    fn visibility_preserves_the_source_half_delta_and_high_byte_quantization() {
        assert_eq!(source_winding_high_byte(-28, 25, -41, -8), 1);
        assert_eq!(source_winding_high_byte(70, 34, 83, 67), 1);
        assert_eq!(source_winding_high_byte(26, -8, -85, -50), -2);
        assert_eq!(source_winding_high_byte(1, 0, 1, -29), 0);
    }

    #[test]
    fn offscreen_shapes_keep_full_precision_visibility() {
        let points = [
            ProjectedPoint {
                x: PLAYFIELD_RIGHT + 1,
                y: 100,
                depth: 100,
            },
            ProjectedPoint {
                x: PLAYFIELD_RIGHT,
                y: 100,
                depth: 100,
            },
            ProjectedPoint {
                x: PLAYFIELD_RIGHT,
                y: 129,
                depth: 100,
            },
        ];
        assert!(!shape_is_fully_inside_playfield(&points));
        assert!(face_is_visible(&points, [0, 1, 2]));
    }

    #[test]
    fn title_demo_light_is_quantized_before_face_shading() {
        let (matrix, _) = source_object_transform(SourcePose {
            world_position: [20, 20, 1_261],
            rotation: [239, 96, 14],
            view_position: [0, 0, 1_021],
            view_rotation: [0; 3],
        }, false);
        assert_eq!(quantized_object_light(matrix), [-9, -23, -126]);
    }

    #[test]
    fn wingman_boost_sprite_matches_the_retail_frame_333_capture() {
        let sprite = project_scaled_sprite(
            SourcePose {
                world_position: [-46, -48, 292],
                rotation: [0; 3],
                view_position: [0; 3],
                view_rotation: [0; 3],
            },
            20,
            1,
            u8::MAX,
        )
        .expect("visible boost sprite");
        assert_eq!(sprite.top_left, [70, 52]);
        assert_eq!(sprite.size, 33);
    }

    #[test]
    fn corneria_edge_building_projects_like_the_retail_capture() {
        let building = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == 61)
            .expect("compiled Corneria building");
        let projected = project_shape(
            building.vertices,
            2,
            SourcePose {
                world_position: [1_000, 0, 4_800],
                rotation: [0, 128, 0],
                view_position: [-14, -55, 2_686],
                view_rotation: [160, 160, 0],
            },
        );
        assert_eq!(projected.object_light, [-76, -72, -74]);
        assert_eq!(
            projected.points[9..12]
                .iter()
                .map(|point| (point.x, point.y))
                .collect::<Vec<_>>(),
            [(258, 122), (242, 72), (257, 72)]
        );
        assert!(building.faces[5]
            .visibility_vertices
            .is_some_and(|selector| face_is_visible(&projected.points, selector)));
        assert!(!shape_is_outside_playfield(&projected.points));
    }

    #[test]
    fn corneria_far_edge_building_is_rejected_by_source_object_outcodes() {
        let building = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == 61)
            .expect("compiled Corneria building");
        let projected = project_shape(
            building.vertices,
            2,
            SourcePose {
                world_position: [800, 0, 3_300],
                rotation: [0, 128, 0],
                view_position: [-41, -61, 2_296],
                view_rotation: [352, 352, 0],
            },
        );
        assert_eq!(
            projected.points.iter().map(|point| point.x).min(),
            Some(293)
        );
        assert!(shape_is_outside_playfield(&projected.points));
    }

    #[test]
    fn corneria_near_edge_building_matches_retail_projection() {
        let building = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == 61)
            .expect("compiled Corneria building");
        let projected = project_shape(
            building.vertices,
            2,
            SourcePose {
                world_position: [1_000, 0, 4_800],
                rotation: [0, 128, 0],
                view_position: [-24, -57, 2_504],
                view_rotation: [272, 240, 0],
            },
        );
        assert_eq!(projected.object_light, [-76, -71, -75]);
        assert_eq!(
            projected.points.iter().map(|point| point.x).collect::<Vec<_>>(),
            [239, 222, 239, 222, 247, 229, 246, 228, 232, 245, 231, 245]
        );
        assert!(!shape_is_outside_playfield(&projected.points));
    }

    #[test]
    fn corneria_player_shadow_matches_the_retail_projection() {
        let arwing = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == 2)
            .expect("compiled player Arwing");
        let projected = project_shadow_shape(
            arwing.vertices,
            0,
            SourcePose {
                world_position: [-3, 0, 3_208],
                rotation: [0; 3],
                view_position: [-6, -53, 3_040],
                view_rotation: [168, 98, 0],
            },
        );
        assert_eq!(
            projected
                .points
                .iter()
                .map(|point| (point.x, point.y))
                .collect::<Vec<_>>(),
            [
                (128, 198),
                (128, 195),
                (128, 195),
                (125, 169),
                (201, 219),
                (54, 219),
                (163, 204),
                (92, 204),
                (99, 197),
                (155, 196),
                (112, 195),
                (141, 195),
                (157, 195),
                (96, 195),
                (107, 186),
                (145, 186),
            ]
        );
    }

    #[test]
    fn complete_title_demo_projection_matches_the_independent_retail_capture() {
        let demo = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == 225)
            .expect("compiled title demo shape");
        let projected = project_shape(
            demo.vertices,
            1,
            SourcePose {
                world_position: [20, 20, 946],
                rotation: [239, 96, 4],
                view_position: [0, 0, 711],
                view_rotation: [0; 3],
            },
        );
        let screen_points: Vec<_> = projected
            .points
            .iter()
            .map(|point| (point.x, point.y))
            .collect();
        assert_eq!(
            screen_points,
            [
                (172, 142),
                (167, 141),
                (129, 146),
                (53, 83),
                (146, 169),
                (257, 209),
                (138, 156),
                (225, 182),
                (135, 155),
                (231, 186),
                (158, 127),
                (223, 139),
                (201, 174),
                (137, 154),
                (164, 126),
                (172, 129),
                (167, 147),
                (151, 143),
                (136, 139),
                (186, 151),
                (176, 153),
                (138, 144),
                (142, 142),
                (173, 150),
                (128, 139),
                (193, 155),
                (175, 155),
                (132, 144),
                (184, 156),
                (127, 141),
                (136, 126),
                (184, 136),
                (140, 122),
                (145, 124),
                (103, 143),
                (150, 158),
            ]
        );
    }

    #[test]
    fn title_demo_projection_matches_the_independent_retail_capture() {
        let vertices = [
            ShapeVertex {
                x: 0.0,
                y: 4.0,
                z: -40.0,
            },
            ShapeVertex {
                x: 0.0,
                y: 2.0,
                z: -32.0,
            },
            ShapeVertex {
                x: 0.0,
                y: -20.0,
                z: 16.0,
            },
            ShapeVertex {
                x: 0.0,
                y: -4.0,
                z: 100.0,
            },
        ];
        let projected = project_shape(
            &vertices,
            1,
            SourcePose {
                world_position: [20, 20, 2_080],
                rotation: [239, 96, 40],
                view_position: [0, 0, 1_833],
                view_rotation: [0; 3],
            },
        );
        assert_eq!(
            projected.points,
            [
                ProjectedPoint {
                    x: 172,
                    y: 143,
                    depth: 270,
                },
                ProjectedPoint {
                    x: 167,
                    y: 142,
                    depth: 264,
                },
                ProjectedPoint {
                    x: 120,
                    y: 134,
                    depth: 244,
                },
                ProjectedPoint {
                    x: 59,
                    y: 82,
                    depth: 182,
                },
            ]
        );
    }
}
