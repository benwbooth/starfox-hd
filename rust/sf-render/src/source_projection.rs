//! Exact fixed-point projection used at native fixed-update boundaries.
//!
//! The HD path remains smooth and floating-point between updates. At an exact
//! source boundary, this module preserves the authored integer matrix,
//! coordinate scaling, projection, and visibility decisions so the
//! source-resolution conformance image has deterministic geometry.

use crate::shape_data::ShapeVertex;
use sf_core::snes_trig::{gsu_fmult_q15, matrix_rotate_q15, zxy_matrix_q15, zxy_matrix_q15_fine};

const PACKED_MATRIX_SHIFT: u32 = 8;
const FULL_PRECISION_POINT_SHIFT: u8 = 3;
pub const MIN_FRONT_DEPTH: i16 = 0;
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
const SCALED_SPRITE_NEAR_SCALE: i16 = 2;
const SCALED_SPRITE_MAX_SIZE: i16 = 240;
const INDIVIDUAL_PROJECTION_SCALE: u32 = 256;
const INDIVIDUAL_PROJECTION_LIMIT: u32 = 16_383;
const PLAYFIELD_LEFT: i16 = 16;
const PLAYFIELD_RIGHT: i16 = 239;
const PLAYFIELD_TOP: i16 = 16;
const PLAYFIELD_BOTTOM: i16 = 207;
const LIGHT_COMPONENT_Q15: i16 = 18_917;
const LIGHT_QUANTIZATION_SHIFT: u32 = 8;
const OBJECT_Y_HALF_TURN: u8 = 128;

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
    /// Typed view-space geometry retained for per-face near-plane clipping.
    pub view_points: Vec<[i16; 3]>,
    pub object_light: [i8; 3],
    pub view_position: [i16; 3],
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

    let shifted_adjustment =
        i16::from(size_adjustment as i8).wrapping_shl(u32::from(coordinate_shift));
    let mut world_size = (authored_extent as i16)
        .wrapping_mul(2)
        .wrapping_add(shifted_adjustment);
    if world_size == 0 {
        world_size = 1;
    }
    let depth = view_position.2.min(PROJECTION_MAX_DEPTH - 1);
    // The scaled-sprite projector has a dedicated close-range path. Sprites
    // are rejected at depth 128, so one doubling brings every surviving near
    // sprite into the reciprocal table's ordinary range while preserving the
    // world-size/depth ratio and its fixed-point rounding.
    let (projection_size, projection_depth) = if depth < NEAR_PROJECTION_DEPTH {
        (
            world_size.wrapping_mul(SCALED_SPRITE_NEAR_SCALE),
            depth.wrapping_mul(SCALED_SPRITE_NEAR_SCALE),
        )
    } else {
        (world_size, depth)
    };
    let reciprocal = source_reciprocal(projection_depth);
    let projected_size =
        project_component(projection_size, reciprocal).clamp(0, SCALED_SPRITE_MAX_SIZE);
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
    reflected_pair_starts: &[u16],
    coordinate_shift: u8,
    pose: SourcePose,
) -> ProjectedShape {
    project_shape_with_flattened_height(
        vertices,
        reflected_pair_starts,
        coordinate_shift,
        pose,
        false,
    )
}

/// Project the source shadow pass. The source retains the object's authored
/// X/Z rotation but removes its world-height contribution before applying the
/// camera matrix, producing a fixed-point ground-plane silhouette.
pub fn project_shadow_shape(
    vertices: &[ShapeVertex],
    reflected_pair_starts: &[u16],
    coordinate_shift: u8,
    pose: SourcePose,
) -> ProjectedShape {
    project_shape_with_flattened_height(
        vertices,
        reflected_pair_starts,
        coordinate_shift,
        pose,
        true,
    )
}

pub fn project_exploded_face(
    vertices: &[ShapeVertex],
    reflected_pair_starts: &[u16],
    face_indices: &[u16],
    face_normal: [i16; 3],
    coordinate_shift: u8,
    explosion_state: u8,
    pose: SourcePose,
) -> ProjectedShape {
    project_exploded_face_with_height(
        vertices,
        reflected_pair_starts,
        face_indices,
        face_normal,
        coordinate_shift,
        explosion_state,
        pose,
        false,
    )
}

pub fn project_exploded_shadow_face(
    vertices: &[ShapeVertex],
    reflected_pair_starts: &[u16],
    face_indices: &[u16],
    face_normal: [i16; 3],
    coordinate_shift: u8,
    explosion_state: u8,
    pose: SourcePose,
) -> ProjectedShape {
    project_exploded_face_with_height(
        vertices,
        reflected_pair_starts,
        face_indices,
        face_normal,
        coordinate_shift,
        explosion_state,
        pose,
        true,
    )
}

fn rotate_shape_points(
    vertices: &[ShapeVertex],
    reflected_pair_starts: &[u16],
    coordinate_shift: u8,
    object_matrix: [[i16; 3]; 3],
) -> Vec<[i16; 3]> {
    let source = |vertex: &ShapeVertex| {
        [
            vertex.x.round() as i16,
            (-vertex.y).round() as i16,
            vertex.z.round() as i16,
        ]
    };
    let mut rotated_points = Vec::with_capacity(vertices.len());
    let mut index = 0;
    let mut reflected_pair_cursor = 0;
    while index < vertices.len() {
        let first = source(&vertices[index]);
        let reflected_pair = reflected_pair_starts.get(reflected_pair_cursor)
            == Some(&u16::try_from(index).expect("source vertex index"));
        if reflected_pair {
            debug_assert_eq!(
                source(&vertices[index + 1]),
                [first[0].wrapping_neg(), first[1], first[2]],
            );
            reflected_pair_cursor += 1;
        }
        let (transformed, transformed_count) = if reflected_pair {
            (
                rotate_reflected_shape_pair(object_matrix, first, coordinate_shift),
                2,
            )
        } else {
            (
                [
                    rotate_independent_shape_point(object_matrix, first, coordinate_shift),
                    [0; 3],
                ],
                1,
            )
        };
        rotated_points.extend(transformed.into_iter().take(transformed_count));
        index += if reflected_pair { 2 } else { 1 };
    }
    debug_assert_eq!(reflected_pair_cursor, reflected_pair_starts.len());
    rotated_points
}

fn project_exploded_face_with_height(
    vertices: &[ShapeVertex],
    reflected_pair_starts: &[u16],
    face_indices: &[u16],
    face_normal: [i16; 3],
    coordinate_shift: u8,
    explosion_state: u8,
    pose: SourcePose,
    flatten_height: bool,
) -> ProjectedShape {
    let (object_matrix, view_position) = source_object_transform(pose, flatten_height);
    let rotated_points = rotate_shape_points(
        vertices,
        reflected_pair_starts,
        coordinate_shift,
        object_matrix,
    );

    // Generated normals use the HD coordinate convention. Negating all three
    // components reconstructs the normal consumed by the authored exploding-
    // face translation after its encoded axis adjustments.
    let normal = face_normal.map(i16::wrapping_neg);
    let rotated_normal = matrix_rotate_q15(object_matrix, normal[0], normal[1], normal[2]);
    let scale_component = |component: i16| {
        i16::from(component as u8 as i8).wrapping_mul(i16::from(explosion_state as i8)) >> 2
    };
    let vertical_normal = if rotated_normal.1 < 0 {
        rotated_normal.1
    } else {
        rotated_normal.1.wrapping_neg()
    };
    let displaced_position = (
        view_position
            .0
            .wrapping_add(scale_component(rotated_normal.0)),
        view_position
            .1
            .wrapping_add(scale_component(vertical_normal)),
        view_position
            .2
            .wrapping_add(scale_component(rotated_normal.2)),
    );
    let view_points: Vec<_> = face_indices
        .iter()
        .filter_map(|index| rotated_points.get(usize::from(*index)))
        .map(|rotated| {
            [
                rotated[0].wrapping_add(displaced_position.0),
                rotated[1].wrapping_add(displaced_position.1),
                rotated[2].wrapping_add(displaced_position.2),
            ]
        })
        .collect();
    let points = view_points
        .iter()
        .copied()
        .map(project_individual_point)
        .collect();

    ProjectedShape {
        points,
        view_points,
        object_light: quantized_object_light(object_matrix),
        view_position: [
            displaced_position.0,
            displaced_position.1,
            displaced_position.2,
        ],
        object_depth: view_position.2,
    }
}

fn project_shape_with_flattened_height(
    vertices: &[ShapeVertex],
    reflected_pair_starts: &[u16],
    coordinate_shift: u8,
    pose: SourcePose,
    flatten_height: bool,
) -> ProjectedShape {
    let (object_matrix, view_position) = source_object_transform(pose, flatten_height);
    let view_points: Vec<_> = rotate_shape_points(
        vertices,
        reflected_pair_starts,
        coordinate_shift,
        object_matrix,
    )
    .into_iter()
    .map(|rotated| {
        [
            rotated[0].wrapping_add(view_position.0),
            rotated[1].wrapping_add(view_position.1),
            rotated[2].wrapping_add(view_position.2),
        ]
    })
    .collect();
    let points = view_points.iter().copied().map(project_point).collect();

    ProjectedShape {
        points,
        view_points,
        object_light: quantized_object_light(object_matrix),
        view_position: [view_position.0, view_position.1, view_position.2],
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
    let exact_axis_matrix = if pose.rotation == [0; 3] {
        Some(view_matrix)
    } else if pose.rotation == [0, OBJECT_Y_HALF_TURN, 0] {
        let mut matrix = view_matrix;
        matrix[0] = matrix[0].map(i16::wrapping_neg);
        matrix[2] = matrix[2].map(i16::wrapping_neg);
        Some(matrix)
    } else {
        None
    };
    if let Some(mut matrix) = exact_axis_matrix {
        // The source has dedicated zero-rotation and Y-half-turn paths which
        // copy or sign-flip the view matrix. Avoiding an approximate identity
        // multiply changes fixed-point projections at visible pixel edges.
        if flatten_height {
            matrix[1] = [0; 3];
        }
        return (matrix, view_position);
    }
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
    // Generated normals use the renderer's upward-positive Y convention.
    [
        source_basis[0],
        source_basis[1].wrapping_neg(),
        source_basis[2],
    ]
}

/// Transform one authored point and its X reflection as the source
/// `PointsXb` command does. Computing the reflected point by subtracting the
/// first point's X contribution is observably different from independently
/// multiplying its negated X coordinate at fixed-point boundaries.
fn rotate_reflected_shape_pair(
    matrix: [[i16; 3]; 3],
    point: [i16; 3],
    coordinate_shift: u8,
) -> [[i16; 3]; 2] {
    if coordinate_shift < FULL_PRECISION_POINT_SHIFT {
        let packed_matrix =
            matrix.map(|row| row.map(|coefficient| (coefficient >> PACKED_MATRIX_SHIFT) as i8));
        let encoded = point.map(|component| (component >> coordinate_shift) as i8);
        let scale = 1i8.wrapping_shl(u32::from(coordinate_shift));
        let axis = |column: usize| {
            let x_product = i16::from(packed_matrix[0][column]).wrapping_mul(i16::from(encoded[0]));
            let other_products = i16::from(packed_matrix[1][column])
                .wrapping_mul(i16::from(encoded[1]))
                .wrapping_add(
                    i16::from(packed_matrix[2][column]).wrapping_mul(i16::from(encoded[2])),
                );
            let scaled_high_byte = |sum: i16| {
                let high_byte = (((i32::from(sum)) << 1) >> 8) as i8;
                i16::from(high_byte).wrapping_mul(i16::from(scale))
            };
            [
                scaled_high_byte(other_products.wrapping_add(x_product)),
                scaled_high_byte(other_products.wrapping_sub(x_product)),
            ]
        };
        let x = axis(0);
        let y = axis(1);
        let z = axis(2);
        return [[x[0], y[0], z[0]], [x[1], y[1], z[1]]];
    }

    let axis = |column: usize| {
        let x_product = gsu_fmult_q15(point[0], matrix[0][column]);
        let other_products = gsu_fmult_q15(point[1], matrix[1][column])
            .wrapping_add(gsu_fmult_q15(point[2], matrix[2][column]));
        [
            other_products.wrapping_add(x_product),
            other_products.wrapping_sub(x_product),
        ]
    };
    let x = axis(0);
    let y = axis(1);
    let z = axis(2);
    [[x[0], y[0], z[0]], [x[1], y[1], z[1]]]
}

/// Transform a point authored by an independent `PointsB`/`PointsW` stream.
/// Larger scaled-byte points accumulate the complete fixed-point dot product
/// before truncation; smaller points use the packed matrix path.
fn rotate_independent_shape_point(
    matrix: [[i16; 3]; 3],
    point: [i16; 3],
    coordinate_shift: u8,
) -> [i16; 3] {
    if coordinate_shift < FULL_PRECISION_POINT_SHIFT {
        return rotate_reflected_shape_pair(matrix, point, coordinate_shift)[0];
    }
    std::array::from_fn(|column| {
        let product =
            |row: usize| i32::from(point[row]).wrapping_mul(i32::from(matrix[row][column]));
        let sum = product(0).wrapping_add(product(1)).wrapping_add(product(2));
        (sum >> 15) as i16
    })
}

fn project_point(point: [i16; 3]) -> ProjectedPoint {
    if point[2] < MIN_FRONT_DEPTH {
        // Visibility is evaluated before per-face near clipping.  The source
        // therefore retains the signed projection of a behind-camera point;
        // replacing it with a sentinel changes both BSP traversal and face
        // winding at the camera plane.
        return project_individual_point(point);
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

fn source_divide_by_two(value: i16) -> i16 {
    if value == -1 {
        0
    } else {
        value >> 1
    }
}

fn source_midpoint(first: [i16; 3], second: [i16; 3]) -> [i16; 3] {
    std::array::from_fn(|axis| source_divide_by_two(first[axis].wrapping_add(second[axis])))
}

fn near_plane_intersection(behind: [i16; 3], front: [i16; 3]) -> [i16; 3] {
    debug_assert!(behind[2] < MIN_FRONT_DEPTH);
    debug_assert!(front[2] >= MIN_FRONT_DEPTH);
    let mut behind = behind;
    let mut front = front;
    loop {
        let midpoint = source_midpoint(behind, front);
        if midpoint[2] == MIN_FRONT_DEPTH {
            return midpoint;
        }
        if midpoint[2] < MIN_FRONT_DEPTH {
            behind = midpoint;
        } else {
            front = midpoint;
        }
    }
}

/// Clip one authored flat face against the source near plane and project the
/// resulting typed polygon. The source uses midpoint refinement rather than a
/// general division, so this deliberately preserves its integer intersections.
pub fn project_near_clipped_face(view_points: &[[i16; 3]], indices: &[u16]) -> Vec<ProjectedPoint> {
    if indices.len() < 2 {
        return Vec::new();
    }
    let input = indices
        .iter()
        .map(|index| view_points.get(usize::from(*index)).copied())
        .collect::<Option<Vec<_>>>();
    let Some(input) = input else {
        return Vec::new();
    };
    if let [first, second] = input.as_slice() {
        let clipped = match (first[2] >= MIN_FRONT_DEPTH, second[2] >= MIN_FRONT_DEPTH) {
            (true, true) => Some([*first, *second]),
            (true, false) => Some([*first, near_plane_intersection(*second, *first)]),
            (false, true) => Some([near_plane_intersection(*first, *second), *second]),
            (false, false) => None,
        };
        let projected = clipped
            .map(|points| points.map(project_individual_point).to_vec())
            .unwrap_or_default();
        return (!individually_projected_face_is_outside_playfield(&projected))
            .then_some(projected)
            .unwrap_or_default();
    }
    let mut clipped = Vec::with_capacity(input.len() + 2);
    for index in 0..input.len() {
        let first = input[index];
        let second = input[(index + 1) % input.len()];
        let first_front = first[2] >= MIN_FRONT_DEPTH;
        let second_front = second[2] >= MIN_FRONT_DEPTH;
        match (first_front, second_front) {
            (true, true) => clipped.push(first),
            (true, false) => {
                clipped.push(first);
                clipped.push(near_plane_intersection(second, first));
            }
            (false, true) => clipped.push(near_plane_intersection(first, second)),
            (false, false) => {}
        }
    }
    if clipped.len() < 3 {
        return Vec::new();
    }
    let projected = clipped
        .into_iter()
        .map(project_individual_point)
        .collect::<Vec<_>>();
    (!individually_projected_face_is_outside_playfield(&projected))
        .then_some(projected)
        .unwrap_or_default()
}

/// The independent projection path performs a whole-face outcode test before
/// 2D clipping. Its minimum X/Y comparisons include the boundary, while its
/// maximum comparisons do not. Preserve that authored asymmetry so a
/// camera-plane-clipped face cannot leave a pixel on a rejected minimum edge.
fn individually_projected_face_is_outside_playfield(points: &[ProjectedPoint]) -> bool {
    points.is_empty()
        || points.iter().all(|point| point.x <= PLAYFIELD_LEFT)
        || points.iter().all(|point| point.x > PLAYFIELD_RIGHT)
        || points.iter().all(|point| point.y <= PLAYFIELD_TOP)
        || points.iter().all(|point| point.y > PLAYFIELD_BOTTOM)
}

/// Near-clipped and exploding faces are projected independently after their
/// per-face geometry changes. Their authored path divides unsigned magnitudes
/// and restores signs afterward, so negative components truncate toward zero
/// instead of using the ordinary reciprocal-table rounding.
fn project_individual_point(point: [i16; 3]) -> ProjectedPoint {
    let original_depth = point[2];
    let behind = original_depth < 0;
    let depth = if original_depth == 0 {
        1
    } else {
        original_depth.unsigned_abs()
    };
    let magnitude_x = point[0].unsigned_abs();
    let magnitude_y = point[1].unsigned_abs();
    let (projected_x, projected_y) = if magnitude_x >= magnitude_y {
        project_magnitudes(magnitude_x, magnitude_y, depth)
    } else {
        let (major, minor) = project_magnitudes(magnitude_y, magnitude_x, depth);
        (minor, major)
    };
    let restore_sign = |magnitude: u16, negative: bool| {
        let value = magnitude as i16;
        if negative {
            value.wrapping_neg()
        } else {
            value
        }
    };
    let projected_x = restore_sign(projected_x, (point[0] < 0) ^ behind);
    let projected_y = restore_sign(projected_y, (point[1] < 0) ^ behind);

    ProjectedPoint {
        x: projected_x
            .wrapping_add(PROJECTION_CENTER_X)
            .wrapping_add(PLAYFIELD_LEFT),
        y: projected_y
            .wrapping_add(PROJECTION_CENTER_Y)
            .wrapping_add(PLAYFIELD_TOP),
        depth: original_depth,
    }
}

fn project_magnitudes(major: u16, minor: u16, depth: u16) -> (u16, u16) {
    let projected_major = u32::from(major) * INDIVIDUAL_PROJECTION_SCALE / u32::from(depth);
    if projected_major <= INDIVIDUAL_PROJECTION_LIMIT {
        return (
            projected_major as u16,
            (u32::from(minor) * INDIVIDUAL_PROJECTION_SCALE / u32::from(depth)) as u16,
        );
    }
    (
        INDIVIDUAL_PROJECTION_LIMIT as u16,
        (u32::from(minor) * INDIVIDUAL_PROJECTION_LIMIT / u32::from(major.max(1))) as u16,
    )
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
    let ab_x = b.x.wrapping_sub(a.x);
    let ab_y = b.y.wrapping_sub(a.y);
    let ac_x = c.x.wrapping_sub(a.x);
    let ac_y = c.y.wrapping_sub(a.y);
    let behind_count = [a, b, c]
        .into_iter()
        .filter(|point| point.depth < MIN_FRONT_DEPTH)
        .count();
    let visible_winding = if shape_is_fully_inside_playfield(points) {
        source_winding_high_byte(ab_x, ab_y, ac_x, ac_y) < 0
    } else {
        i32::from(ab_x) * i32::from(ac_y) - i32::from(ab_y) * i32::from(ac_x) < 0
    };
    let visible = visible_winding ^ (behind_count & 1 != 0);
    // Projected source coordinates increase downward. The source-visible
    // winding is therefore negative here, opposite the renderer's Y-up clip
    // coordinates. Before near clipping, an odd number of behind-camera
    // visibility vertices reverses that winding decision.
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
        point.depth >= MIN_FRONT_DEPTH
            && (PLAYFIELD_LEFT..PLAYFIELD_RIGHT).contains(&point.x)
            && (PLAYFIELD_TOP..PLAYFIELD_BOTTOM).contains(&point.y)
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
        assert_eq!(
            rotate_reflected_shape_pair(matrix, [127, 127, 127], 0)[0],
            [122, 122, 122],
        );
    }

    #[test]
    fn reflected_scaled_byte_points_share_the_first_x_contribution() {
        let matrix = zxy_matrix_q15(0, 128, 0);
        assert_eq!(
            rotate_reflected_shape_pair(matrix, [-160, 0, -160], 3),
            [[159, 0, 159], [-159, 0, 159]],
        );
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
    fn exact_right_outcode_boundary_keeps_full_precision_visibility() {
        let points = [
            ProjectedPoint {
                x: PLAYFIELD_RIGHT - 17,
                y: 114,
                depth: 100,
            },
            ProjectedPoint {
                x: PLAYFIELD_RIGHT,
                y: 114,
                depth: 100,
            },
            ProjectedPoint {
                x: PLAYFIELD_RIGHT,
                y: 88,
                depth: 100,
            },
        ];
        assert!(!shape_is_fully_inside_playfield(&points));
        assert!(face_is_visible(&points, [0, 1, 2]));
    }

    #[test]
    fn training_base_face_crossing_the_camera_flips_visibility() {
        let points = [
            project_point([115, 27, -44]),
            project_point([115, 27, 316]),
            project_point([75, -13, 296]),
        ];

        assert!(!shape_is_fully_inside_playfield(&points));
        assert!(!face_is_visible(&points, [0, 1, 2]));
    }

    #[test]
    fn title_demo_light_is_quantized_before_face_shading() {
        let (matrix, _) = source_object_transform(
            SourcePose {
                world_position: [20, 20, 1_261],
                rotation: [239, 96, 14],
                view_position: [0, 0, 1_021],
                view_rotation: [0; 3],
            },
            false,
        );
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
    fn corneria_launch_boost_uses_the_retail_close_range_scale() {
        let sprite = project_scaled_sprite(
            SourcePose {
                world_position: [40, -80, 9_278],
                rotation: [0; 3],
                view_position: [0, -45, 9_137],
                view_rotation: [64_928, 0, 0],
            },
            20,
            1,
            251,
        )
        .expect("visible launch boost sprite");
        // The retail sprite origin is local [156, -15]; typed source-raster
        // coordinates include the 16-pixel playfield origin.
        assert_eq!(sprite.top_left, [172, 1]);
        assert_eq!(sprite.size, 56);
    }

    #[test]
    fn corneria_edge_building_projects_like_the_retail_capture() {
        let building = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == 61)
            .expect("compiled Corneria building");
        let projected = project_shape(
            building.vertices,
            building.reflected_pair_starts,
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
            building.reflected_pair_starts,
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
            building.reflected_pair_starts,
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
            projected
                .points
                .iter()
                .map(|point| point.x)
                .collect::<Vec<_>>(),
            [239, 222, 239, 222, 247, 229, 246, 228, 232, 245, 231, 245]
        );
        assert!(!shape_is_outside_playfield(&projected.points));
    }

    #[test]
    fn corneria_corridor_segment_matches_complete_retail_projection() {
        let corridor = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == crate::shape_data::SHAPE_EXT_OP_1)
            .expect("compiled Corneria corridor segment");
        let pose = SourcePose {
            world_position: [0, 0, 2_301],
            rotation: [0; 3],
            view_position: [-700, -1_199, 3_037],
            view_rotation: [57_024, 25_488, 0],
        };
        let projected = project_shape(corridor.vertices, corridor.reflected_pair_starts, 3, pose);

        let retail_matrix = [
            [-25_103, -15_337, 14_423],
            [0, 22_445, 23_868],
            [-21_055, 18_284, -17_196],
        ];
        assert_eq!(
            zxy_matrix_q15_fine(
                pose.view_rotation[0],
                pose.view_rotation[1],
                pose.view_rotation[2],
            ),
            retail_matrix,
        );
        assert_eq!(source_object_transform(pose, false).0, retail_matrix);
        assert_eq!(projected.view_position, [-65, 82, 1_567]);
        assert_eq!(
            [projected.view_points[7], projected.view_points[11]],
            [[492, -11, 1_655], [320, -462, 1_386]],
        );
        assert_eq!(
            projected
                .points
                .iter()
                .map(|point| [point.x, point.y])
                .collect::<Vec<_>>(),
            [
                [127, 68],
                [202, 106],
                [212, 72],
                [127, 35],
                [188, 28],
                [153, 15],
                [123, 68],
                [204, 110],
                [122, 35],
                [214, 76],
                [151, 13],
                [187, 26],
                [123, 70],
                [198, 110],
                [122, 37],
                [208, 76],
                [183, 32],
                [148, 18],
                [160, 153],
                [81, 101],
                [165, 120],
                [75, 67],
                [96, 50],
                [133, 70],
                [76, 101],
                [161, 158],
                [69, 67],
                [167, 125],
                [92, 48],
                [130, 68],
                [155, 158],
                [76, 105],
                [159, 125],
                [68, 71],
                [126, 74],
                [89, 54],
                [93, 192],
                [1, 113],
                [48, 133],
                [11, 102],
            ],
        );
    }

    #[test]
    fn corneria_camera_plane_line_uses_individual_reprojection() {
        let corridor = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == crate::shape_data::SHAPE_EXT_OP_1)
            .expect("compiled Corneria corridor segment");
        let projected = project_shape(
            corridor.vertices,
            corridor.reflected_pair_starts,
            3,
            SourcePose {
                world_position: [0, 0, 5_421],
                rotation: [0; 3],
                view_position: [-348, -715, 5_649],
                view_rotation: [56_496, 6_752, 0],
            },
        );

        assert_eq!(
            [projected.view_points[4], projected.view_points[38]],
            [[607, 475, -100], [28, -108, 395]],
        );
        assert_eq!(
            project_near_clipped_face(&projected.view_points, &[4, 38]),
            [
                ProjectedPoint {
                    x: 16_511,
                    y: 12_039,
                    depth: 0,
                },
                ProjectedPoint {
                    x: 146,
                    y: 43,
                    depth: 395,
                },
            ],
        );
    }

    #[test]
    fn corneria_camera_plane_line_rejects_the_retail_minimum_edge() {
        let corridor = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == crate::shape_data::SHAPE_EXT_OP_0)
            .expect("compiled Corneria corridor frame");
        let projected = project_shape(
            corridor.vertices,
            corridor.reflected_pair_starts,
            3,
            SourcePose {
                world_position: [0, 0, 7_761],
                rotation: [0; 3],
                view_position: [-13, -139, 7_528],
                view_rotation: [61_456, 352, 0],
            },
        );
        assert_eq!(
            [projected.view_points[8], projected.view_points[0]],
            [[-116, 39, 263], [-100, 221, -181]],
        );
        let intersection = near_plane_intersection(
            projected.view_points[0],
            projected.view_points[8],
        );
        let individually_projected = [
            project_individual_point(projected.view_points[8]),
            project_individual_point(intersection),
        ];
        assert_eq!(
            individually_projected,
            [
                ProjectedPoint {
                    x: 16,
                    y: 149,
                    depth: 263,
                },
                ProjectedPoint {
                    x: -11_961,
                    y: 16_495,
                    depth: 0,
                },
            ],
        );
        assert!(individually_projected_face_is_outside_playfield(
            &individually_projected,
        ));
        assert!(project_near_clipped_face(&projected.view_points, &[8, 0]).is_empty());
    }

    #[test]
    fn corneria_player_shadow_matches_the_retail_projection() {
        let arwing = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == 2)
            .expect("compiled player Arwing");
        let projected = project_shadow_shape(
            arwing.vertices,
            arwing.reflected_pair_starts,
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
    fn exploded_training_shadow_uses_individual_projection_rounding() {
        const EXPLODING_FACE_INDEX: usize = 2;
        const EXPLOSION_STATE: u8 = 1;

        let debris = crate::shape_data::SHAPE_DATA
            .iter()
            .find(|entry| entry.shape_id == 465)
            .expect("compiled Training debris");
        let face = &debris.faces[EXPLODING_FACE_INDEX];
        let projected = project_exploded_shadow_face(
            debris.vertices,
            debris.reflected_pair_starts,
            &face.vertex_indices[..usize::from(face.num_verts)],
            face.normal,
            0,
            EXPLOSION_STATE,
            SourcePose {
                world_position: [-7, 0, 9_713],
                rotation: [84, 179, 0],
                view_position: [5, -28, 9_526],
                view_rotation: [0; 3],
            },
        );

        assert_eq!(projected.view_position, [-24, 27, 212]);
        assert_eq!(
            projected
                .points
                .iter()
                .map(|point| (point.x, point.y))
                .collect::<Vec<_>>(),
            [(77, 145), (99, 141), (110, 145)]
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
            demo.reflected_pair_starts,
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
            &[],
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
