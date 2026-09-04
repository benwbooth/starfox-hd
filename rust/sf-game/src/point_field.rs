//! Native typed implementation of the source background point fields.

use sf_core::point_field::{PointFieldMode, PointIdentity, PointPixel};
use sf_core::snes_trig::{gsu_fmult_q15, matrix_rotate_q15};

const DUST_POINT_COUNT: usize = 120;
const DUST_RANDOM_SEED: u16 = 0x19F8;
const DUST_XY_RANGE: i16 = 2_048;
const DUST_Z_RANGE: i16 = 2_560;
const DUST_RESPAWN_Z_NEAR: u16 = 512;
const DUST_RESPAWN_SHIFT: u32 = 5;
const DUST_XY_EXTRA_SHIFT: u32 = 1;
const DUST_NEAR_DOUBLE_PIXEL_Z: i16 = 1_024;
const PROJECTION_NEAR_Z: i16 = 256;
const PROJECTION_MAX_Z: i16 = 12_288;
const PROJECTION_NUMERATOR: i32 = 32_767 * 256;
const PROJECTION_CENTER_X: i16 = 112;
const PROJECTION_CENTER_Y: i16 = 96;
const PROJECTION_WIDTH: i16 = 224;
const PROJECTION_HEIGHT: i16 = 192;
const GROUND_GRID_POINTS_PER_AXIS: usize = 15;
const GROUND_GRID_WORLD_SPACING: i16 = 256;
const GROUND_GRID_HALF_WIDTH: i16 =
    GROUND_GRID_WORLD_SPACING * GROUND_GRID_POINTS_PER_AXIS as i16 / 2;
const GROUND_GRID_MATRIX_SHIFT: u32 = 7;
const GROUND_GRID_DOUBLE_PIXEL_Z: i16 = 512;
const GROUND_GRID_COLOR: u8 = 14;

const STAR_COLORS: [u8; 64] = [
    14, 14, 13, 12, 11, 10, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 14, 14, 13, 12, 11, 10, 9, 9, 9, 9, 9, 9,
    9, 9, 9, 9, 8, 8, 8, 7, 7, 7, 6, 6, 6, 5, 5, 5, 5, 5, 5, 5, 4, 4, 4, 3, 3, 3, 2, 2, 2, 1, 1, 1,
    1, 1, 1, 1,
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DustPoint {
    x: i16,
    y: i16,
    z: i16,
}

/// Flat game-owned dust state. The point array is ordinary native data, not
/// an addressable image of the source machine.
#[derive(Debug, Clone)]
pub struct PointField {
    dust: [DustPoint; DUST_POINT_COUNT],
    dust_generations: [u64; DUST_POINT_COUNT],
    random_state: u16,
    pixels: Vec<PointPixel>,
}

impl Default for PointField {
    fn default() -> Self {
        Self::new()
    }
}

impl PointField {
    pub fn new() -> Self {
        let mut random_state = DUST_RANDOM_SEED;
        let mut carry = false;
        let mut dust = [DustPoint::default(); DUST_POINT_COUNT];
        for point in &mut dust {
            point.x = next_random(&mut random_state, &mut carry) as i16;
            point.y = next_random(&mut random_state, &mut carry) as i16;
            point.z = next_random(&mut random_state, &mut carry) as i16;
        }
        Self {
            dust,
            dust_generations: [0; DUST_POINT_COUNT],
            // The source initializer publishes its seed before filling the
            // points; the local initialization stream is intentionally not
            // retained as the respawn stream.
            random_state: DUST_RANDOM_SEED,
            pixels: Vec::with_capacity(DUST_POINT_COUNT * 2),
        }
    }

    pub fn pixels(&self) -> &[PointPixel] {
        &self.pixels
    }

    pub fn update(&mut self, mode: PointFieldMode, view_position: [i16; 3], matrix: [[i16; 3]; 3]) {
        self.pixels.clear();
        if mode == PointFieldMode::GroundGrid {
            self.project_ground_grid(view_position, matrix);
            return;
        }
        if !matches!(
            mode,
            PointFieldMode::SpaceDust | PointFieldMode::Snow | PointFieldMode::Pollen
        ) {
            return;
        }

        let mut rotated = [DustPoint::default(); DUST_POINT_COUNT];
        for (index, (point, output)) in self.dust.iter_mut().zip(&mut rotated).enumerate() {
            let mut relative = relative_point(*point, view_position);
            let mut transformed = matrix_rotate_q15(matrix, relative.x, relative.y, relative.z);
            if !within_dust_volume(relative) || transformed.2 < 0 {
                *point = respawn_point(&mut self.random_state, view_position, matrix);
                self.dust_generations[index] = self.dust_generations[index].wrapping_add(1);
                relative = relative_point(*point, view_position);
                transformed = matrix_rotate_q15(matrix, relative.x, relative.y, relative.z);
            }
            *output = DustPoint {
                x: transformed.0,
                y: transformed.1,
                z: transformed.2,
            };
        }

        for (index, point) in rotated.into_iter().enumerate() {
            self.project_dust_point(mode, index, point);
        }
    }

    /// Source `mshowgrid_l` / `mshowgrid`: build the 15-by-15 planetary
    /// ground grid directly from the typed camera transform.  The source
    /// prepares one rotated corner plus rotated world-axis increments, then
    /// projects each point through the same fixed-point reciprocal path used
    /// for dust.  No source-machine storage is represented here.
    fn project_ground_grid(&mut self, view_position: [i16; 3], matrix: [[i16; 3]; 3]) {
        let grid_corner = |position: i16| {
            let within_cell = (position as u16 & (GROUND_GRID_WORLD_SPACING as u16 - 1))
                ^ (GROUND_GRID_WORLD_SPACING as u16 - 1);
            (within_cell as i16).wrapping_sub(GROUND_GRID_HALF_WIDTH)
        };
        let start = matrix_rotate_q15(
            matrix,
            grid_corner(view_position[0]),
            view_position[1].wrapping_neg(),
            grid_corner(view_position[2]),
        );
        let axis_step = |axis: usize| {
            [
                matrix[axis][0] >> GROUND_GRID_MATRIX_SHIFT,
                matrix[axis][1] >> GROUND_GRID_MATRIX_SHIFT,
                matrix[axis][2] >> GROUND_GRID_MATRIX_SHIFT,
            ]
        };
        let x_step = axis_step(0);
        let z_step = axis_step(2);

        let mut row_start = [start.0, start.1, start.2];
        // Identify world cells, not positions in the clipped output vector.
        // The grid recenters whenever the camera crosses a spacing boundary.
        let cell = |position: i16| position.div_euclid(GROUND_GRID_WORLD_SPACING) as u8;
        for row in 0..GROUND_GRID_POINTS_PER_AXIS {
            let mut point = row_start;
            for column in 0..GROUND_GRID_POINTS_PER_AXIS {
                self.project_ground_point(
                    point,
                    cell(view_position[0]).wrapping_add(column as u8),
                    cell(view_position[2]).wrapping_add(row as u8),
                );
                point[0] = point[0].wrapping_add(x_step[0]);
                point[1] = point[1].wrapping_add(x_step[1]);
                point[2] = point[2].wrapping_add(x_step[2]);
            }
            row_start[0] = row_start[0].wrapping_add(z_step[0]);
            row_start[1] = row_start[1].wrapping_add(z_step[1]);
            row_start[2] = row_start[2].wrapping_add(z_step[2]);
        }
    }

    fn project_ground_point(&mut self, point: [i16; 3], column: u8, row: u8) {
        if point[2] < PROJECTION_NEAR_Z {
            return;
        }
        let depth = point[2].min(PROJECTION_MAX_Z - 1) & !1;
        let factor = (PROJECTION_NUMERATOR / i32::from(depth)) as i16;
        let x = gsu_fmult_q15(point[0], factor).wrapping_add(PROJECTION_CENTER_X);
        let y = gsu_fmult_q15(point[1], factor).wrapping_add(PROJECTION_CENTER_Y);
        if !(0..PROJECTION_WIDTH).contains(&x) || !(0..PROJECTION_HEIGHT).contains(&y) {
            return;
        }
        self.pixels.push(PointPixel {
            x: x as u8,
            y: y as u8,
            palette_index: GROUND_GRID_COLOR,
            identity: PointIdentity::Ground {
                column,
                row,
                lower: false,
            },
        });
        if point[2] < GROUND_GRID_DOUBLE_PIXEL_Z && y + 1 < PROJECTION_HEIGHT {
            self.pixels.push(PointPixel {
                x: x as u8,
                y: (y + 1) as u8,
                palette_index: GROUND_GRID_COLOR,
                identity: PointIdentity::Ground {
                    column,
                    row,
                    lower: true,
                },
            });
        }
    }

    fn project_dust_point(&mut self, mode: PointFieldMode, index: usize, point: DustPoint) {
        if point.z < PROJECTION_NEAR_Z {
            return;
        }
        let depth = point.z.min(PROJECTION_MAX_Z - 1) & !1;
        let factor = (PROJECTION_NUMERATOR / i32::from(depth)) as i16;
        let x = gsu_fmult_q15(point.x, factor).wrapping_add(PROJECTION_CENTER_X);
        let y = gsu_fmult_q15(point.y, factor).wrapping_add(PROJECTION_CENTER_Y);
        if !(0..PROJECTION_WIDTH).contains(&x) || !(0..PROJECTION_HEIGHT).contains(&y) {
            return;
        }

        let remaining = DUST_POINT_COUNT - index;
        let depth_color = usize::from((point.z as u16 >> 8).min(15));
        let palette_index = match mode {
            PointFieldMode::SpaceDust => STAR_COLORS[depth_color + (remaining & 3) * 16],
            PointFieldMode::Snow => {
                if depth_color < 8 {
                    14
                } else {
                    8
                }
            }
            PointFieldMode::Pollen => 3,
            PointFieldMode::None | PointFieldMode::GroundGrid => return,
        };
        self.pixels.push(PointPixel {
            x: x as u8,
            y: y as u8,
            palette_index,
            identity: PointIdentity::Dust {
                slot: index as u8,
                generation: self.dust_generations[index],
                lower: false,
            },
        });
        if point.z < DUST_NEAR_DOUBLE_PIXEL_Z && y + 1 < PROJECTION_HEIGHT {
            // The source PLOT operation advances its horizontal coordinate.
            // Its following decrement restores the original column before
            // drawing the second pixel on the next row.
            self.pixels.push(PointPixel {
                x: x as u8,
                y: (y + 1) as u8,
                palette_index,
                identity: PointIdentity::Dust {
                    slot: index as u8,
                    generation: self.dust_generations[index],
                    lower: true,
                },
            });
        }
    }
}

fn relative_point(point: DustPoint, view: [i16; 3]) -> DustPoint {
    DustPoint {
        x: point.x.wrapping_sub(view[0]),
        y: point.y.wrapping_sub(view[1]),
        z: point.z.wrapping_sub(view[2]),
    }
}

fn within_dust_volume(point: DustPoint) -> bool {
    (-DUST_XY_RANGE..DUST_XY_RANGE).contains(&point.x)
        && (-DUST_XY_RANGE..DUST_XY_RANGE).contains(&point.y)
        && (-DUST_Z_RANGE..DUST_Z_RANGE).contains(&point.z)
}

fn respawn_point(random_state: &mut u16, view: [i16; 3], matrix: [[i16; 3]; 3]) -> DustPoint {
    // Rewinding the source point cursor immediately before its random macro
    // leaves carry set. Each following shift supplies carry to the next call.
    let mut carry = true;
    let random_x = next_random(random_state, &mut carry);
    let x = arithmetic_shift(
        random_x,
        DUST_RESPAWN_SHIFT + DUST_XY_EXTRA_SHIFT,
        &mut carry,
    );
    let random_y = next_random(random_state, &mut carry);
    let y = arithmetic_shift(
        random_y,
        DUST_RESPAWN_SHIFT + DUST_XY_EXTRA_SHIFT,
        &mut carry,
    );
    let random_z = next_random(random_state, &mut carry);
    let z = logical_shift(random_z, DUST_RESPAWN_SHIFT, &mut carry)
        .wrapping_add(DUST_RESPAWN_Z_NEAR) as i16;

    let dot = |row: usize| {
        gsu_fmult_q15(x, matrix[row][0])
            .wrapping_add(gsu_fmult_q15(y, matrix[row][1]))
            .wrapping_add(gsu_fmult_q15(z, matrix[row][2]))
    };
    DustPoint {
        x: dot(0).wrapping_add(view[0]),
        y: dot(1).wrapping_add(view[1]),
        z: dot(2).wrapping_add(view[2]),
    }
}

fn next_random(state: &mut u16, carry: &mut bool) -> u16 {
    let swapped = state.swap_bytes();
    let rotated = (swapped >> 1) | (u16::from(*carry) << 15);
    let (added, first_carry) = rotated.overflowing_add(*state);
    let (with_state, second_carry) = added.overflowing_add(*state);
    let (with_carry, carry_overflow) = with_state.overflowing_add(u16::from(first_carry));
    *carry = second_carry || carry_overflow;
    *state = with_carry.wrapping_add(1);
    *state
}

fn arithmetic_shift(value: u16, count: u32, carry: &mut bool) -> i16 {
    let mut shifted = value as i16;
    for _ in 0..count {
        *carry = shifted & 1 != 0;
        shifted >>= 1;
    }
    shifted
}

fn logical_shift(value: u16, count: u32, carry: &mut bool) -> u16 {
    let mut shifted = value;
    for _ in 0..count {
        *carry = shifted & 1 != 0;
        shifted >>= 1;
    }
    shifted
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::snes_trig::zxy_matrix_q15_fine;

    #[test]
    fn initialization_matches_retail_random_points() {
        let field = PointField::new();
        assert_eq!(
            field.dust[0],
            DustPoint {
                x: -20_483,
                y: -8_493,
                z: 10_135
            }
        );
        assert_eq!(
            field.dust[1],
            DustPoint {
                x: 6_850,
                y: 5_778,
                z: -2_512
            }
        );
    }

    #[test]
    fn opening_title_respawns_match_mesen_point_state() {
        let matrix = zxy_matrix_q15_fine(0, 0, 0);
        let mut field = PointField::new();
        field.update(PointFieldMode::SpaceDust, [0, 0, -76], matrix);
        assert_eq!(
            field.dust[0],
            DustPoint {
                x: 190,
                y: 379,
                z: 2_041
            }
        );
        assert_eq!(field.random_state, 0xDF7C);

        field.update(PointFieldMode::SpaceDust, [0, 0, 1_120], matrix);
        assert_eq!(field.random_state, 0xFC65);
        field.update(PointFieldMode::SpaceDust, [0, 0, 1_136], matrix);
        assert_eq!(field.random_state, 0x6FB6);
    }

    #[test]
    fn near_dust_pair_uses_one_source_column() {
        let mut field = PointField::new();
        field.pixels.clear();
        field.project_dust_point(
            PointFieldMode::SpaceDust,
            0,
            DustPoint { x: 0, y: 0, z: 512 },
        );
        assert_eq!(field.pixels.len(), 2);
        assert_eq!(field.pixels[0].x, field.pixels[1].x);
        assert_eq!(field.pixels[0].y + 1, field.pixels[1].y);
    }

    #[test]
    fn ground_grid_projects_typed_points_without_consuming_dust_state() {
        let matrix = zxy_matrix_q15_fine(0, 0, 0);
        let mut field = PointField::new();
        let random_state = field.random_state;
        field.update(PointFieldMode::GroundGrid, [0, -128, 0], matrix);
        assert!(!field.pixels.is_empty());
        assert!(field
            .pixels
            .iter()
            .all(|pixel| pixel.palette_index == GROUND_GRID_COLOR));
        assert_eq!(field.random_state, random_state);
    }

    #[test]
    fn ground_point_identity_survives_cell_recentering_and_clipping() {
        let matrix = zxy_matrix_q15_fine(0, 0, 0);
        let mut field = PointField::new();
        field.update(PointFieldMode::GroundGrid, [0, -128, 255], matrix);
        let previous = field.pixels.clone();
        field.update(PointFieldMode::GroundGrid, [0, -128, 256], matrix);
        let matched: Vec<_> = previous
            .iter()
            .filter_map(|previous| {
                field
                    .pixels
                    .iter()
                    .find(|current| current.identity == previous.identity)
                    .map(|current| (previous, current))
            })
            .collect();
        assert!(
            matched.len() > 1,
            "same world cells must survive recentering"
        );
        for (previous, current) in matched {
            assert!(previous.x.abs_diff(current.x) <= 1);
            assert!(previous.y.abs_diff(current.y) <= 1);
        }
    }

    #[test]
    fn point_identities_are_unique_and_dust_lifetimes_change_only_on_respawn() {
        let matrix = zxy_matrix_q15_fine(0, 0, 0);
        let mut field = PointField::new();
        for mode in [PointFieldMode::GroundGrid, PointFieldMode::SpaceDust] {
            field.update(mode, [0, -128, 0], matrix);
            let identities: std::collections::HashSet<_> =
                field.pixels.iter().map(|pixel| pixel.identity).collect();
            assert_eq!(identities.len(), field.pixels.len());
        }
        let generations = field.dust_generations;
        field.update(PointFieldMode::SpaceDust, [0, -128, 0], matrix);
        assert_eq!(field.dust_generations, generations);
        field.update(PointFieldMode::SpaceDust, [0, -128, 10_000], matrix);
        assert!(field
            .dust_generations
            .iter()
            .zip(generations)
            .all(|(current, previous)| *current > previous));
    }

    #[test]
    fn source_framebuffer_retains_top_point_and_near_trail() {
        let mut field = PointField::new();
        field.pixels.clear();
        field.project_dust_point(
            PointFieldMode::SpaceDust,
            0,
            DustPoint {
                x: 0,
                y: -192,
                z: 512,
            },
        );
        assert_eq!(field.pixels.len(), 2);
        assert_eq!(field.pixels[0].y, 0);
        assert_eq!(field.pixels[1].y, 1);

        field.pixels.clear();
        field.project_dust_point(
            PointFieldMode::SpaceDust,
            49,
            DustPoint {
                x: 392,
                y: -390,
                z: 1_045,
            },
        );
        assert_eq!(field.pixels.len(), 1);
        assert_eq!(field.pixels[0].y, 0);
    }
}
