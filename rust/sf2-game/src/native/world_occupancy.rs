//! World X/Z occupancy, ported from `$0D:DA7A..DBA9`.
//!
//! Rows are gameplay bitsets, not a source-machine memory region. Marker
//! coverage retains the writer's unusual half-row boundary rule. A marker
//! that would write outside the occupancy plane is rejected before mutation;
//! reproducing those writes would corrupt unrelated source state, not wrap
//! the marker inside this world. Map construction must propagate that error.

use super::Vector3;

pub const CELLS_PER_AXIS: usize = 128;
pub const CELL_SIZE: u16 = 512;
const HALF_ROW_CELLS: usize = CELLS_PER_AXIS / 2;
const COORDINATE_SHIFT: u32 = CELL_SIZE.trailing_zeros();
const COORDINATE_MASK: u16 = CELLS_PER_AXIS as u16 - 1;
const ZERO_WIDTH_ITERATIONS: usize = 256;

/// Authored world rectangle. X/Z and extents retain their unsigned word
/// representation: negative world coordinates wrap through the 128-cell map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldRectangle {
    pub x: i16,
    pub z: i16,
    pub width: u16,
    pub depth: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccupancyChange {
    Mark,
    Erase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkerOutsideWorld;

impl std::fmt::Display for MarkerOutsideWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("source marker coverage leaves the world occupancy plane")
    }
}

impl std::error::Error for MarkerOutsideWorld {}

/// Validated marker coverage. Construction has no side effects and rejects
/// out-of-plane source writes; applying or erasing a valid marker cannot fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerCoverage {
    rows: [u128; CELLS_PER_AXIS],
}

impl MarkerCoverage {
    pub fn from_rectangle(rectangle: WorldRectangle) -> Result<Self, MarkerOutsideWorld> {
        let first_column = cell(rectangle.x);
        let first_row = cell(rectangle.z);
        let width = extent(rectangle.width);
        let width = if width == 0 {
            ZERO_WIDTH_ITERATIONS
        } else {
            width
        };
        let depth = extent(rectangle.depth);
        // The source word decrement runs 65,536 rows for a zero extent.
        // All 128 starting rows repeat identically, and set/erase is
        // idempotent, so one cycle has exactly the same coverage and errors.
        let depth = if depth == 0 { CELLS_PER_AXIS } else { depth };
        let mut coverage = Self {
            rows: [0; CELLS_PER_AXIS],
        };
        for row_offset in 0..depth {
            let mut row = (first_row + row_offset) % CELLS_PER_AXIS;
            let mut column = first_column;
            for step in 0..width {
                coverage.rows[row] |= 1_u128 << column;
                // Source advances its cursor even after the final bit, but
                // does not dereference that cursor again on this row.
                if step + 1 == width {
                    break;
                }
                column += 1;
                if column == HALF_ROW_CELLS {
                    row = row.checked_sub(1).ok_or(MarkerOutsideWorld)?;
                } else if column == CELLS_PER_AXIS {
                    column = 0;
                }
            }
        }
        Ok(coverage)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldOccupancy {
    rows: [u128; CELLS_PER_AXIS],
}

impl Default for WorldOccupancy {
    fn default() -> Self {
        Self {
            rows: [0; CELLS_PER_AXIS],
        }
    }
}

impl WorldOccupancy {
    pub fn apply(&mut self, marker: &MarkerCoverage, change: OccupancyChange) {
        for (row, coverage) in self.rows.iter_mut().zip(marker.rows) {
            match change {
                OccupancyChange::Mark => *row |= coverage,
                OccupancyChange::Erase => *row &= !coverage,
            }
        }
    }

    /// Raw occupancy query (`$0D:DB71`). Height has no part in this test.
    pub fn contains(&self, position: Vector3) -> bool {
        self.rows[cell(position.z)] & (1_u128 << cell(position.x)) != 0
    }

    /// Path condition `$7F:B73E` uses the selected player's exemption, not
    /// the projectile's flags. Keep the selection at the owner call site.
    pub fn blocks_path(&self, position: Vector3, selected_player_exempt: bool) -> bool {
        !selected_player_exempt && self.contains(position)
    }
}

fn cell(coordinate: i16) -> usize {
    usize::from(((coordinate as u16) >> COORDINATE_SHIFT) & COORDINATE_MASK)
}

fn extent(world_extent: u16) -> usize {
    usize::from((world_extent.wrapping_add(CELL_SIZE - 1) >> COORDINATE_SHIFT) & COORDINATE_MASK)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle(x: u16, z: u16, width: u16, depth: u16) -> WorldRectangle {
        WorldRectangle {
            x: x as i16,
            z: z as i16,
            width,
            depth,
        }
    }

    fn position(x: usize, z: usize) -> Vector3 {
        Vector3 {
            x: (x as u16 * CELL_SIZE) as i16,
            y: 0,
            z: (z as u16 * CELL_SIZE) as i16,
        }
    }

    #[test]
    fn quantization_uses_wrapped_words_and_rounds_extents_up() {
        let marker = MarkerCoverage::from_rectangle(rectangle(513, 1025, 513, 1)).unwrap();
        let mut world = WorldOccupancy::default();
        world.apply(&marker, OccupancyChange::Mark);
        assert!(world.contains(position(1, 2)));
        assert!(world.contains(position(2, 2)));
        assert!(!world.contains(position(3, 2)));
        assert!(!world.contains(position(1, 3)));
        let negative = MarkerCoverage::from_rectangle(rectangle(65535, 65535, 1, 1)).unwrap();
        world.apply(&negative, OccupancyChange::Mark);
        assert!(world.contains(Vector3 {
            x: -1,
            y: i16::MIN,
            z: -1
        }));
    }

    #[test]
    fn half_row_boundary_moves_backward_but_end_of_row_returns_to_column_zero() {
        let mut world = WorldOccupancy::default();
        let marker = MarkerCoverage::from_rectangle(rectangle(
            63 * CELL_SIZE,
            2 * CELL_SIZE,
            2 * CELL_SIZE,
            1,
        ))
        .unwrap();
        world.apply(&marker, OccupancyChange::Mark);
        assert!(world.contains(position(63, 2)));
        assert!(world.contains(position(64, 1)));
        assert!(!world.contains(position(64, 2)));
        let marker = MarkerCoverage::from_rectangle(rectangle(
            127 * CELL_SIZE,
            4 * CELL_SIZE,
            2 * CELL_SIZE,
            1,
        ))
        .unwrap();
        world.apply(&marker, OccupancyChange::Mark);
        assert!(world.contains(position(127, 4)));
        assert!(world.contains(position(0, 4)));
        assert!(!world.contains(position(0, 5)));
    }

    #[test]
    fn row_starts_wrap_at_the_end_of_the_world() {
        let marker =
            MarkerCoverage::from_rectangle(rectangle(0, 127 * CELL_SIZE, 1, 2 * CELL_SIZE))
                .unwrap();
        assert_eq!(marker.rows[127], 1);
        assert_eq!(marker.rows[0], 1);
        assert_eq!(
            marker.rows.iter().map(|row| row.count_ones()).sum::<u32>(),
            2
        );
    }

    #[test]
    fn out_of_plane_writes_are_errors_not_wrapped_cells() {
        let marker = rectangle(63 * CELL_SIZE, 0, 2 * CELL_SIZE, 1);
        assert_eq!(
            MarkerCoverage::from_rectangle(marker),
            Err(MarkerOutsideWorld)
        );
        // Moving past the boundary after the last write is harmless.
        assert!(MarkerCoverage::from_rectangle(WorldRectangle { width: 1, ..marker }).is_ok());
    }

    #[test]
    fn zero_width_runs_256_bits_and_zero_depth_covers_a_complete_row_cycle() {
        let marker = MarkerCoverage::from_rectangle(rectangle(0, 4 * CELL_SIZE, 0, 1)).unwrap();
        assert_eq!(marker.rows[4], (1_u128 << HALF_ROW_CELLS) - 1);
        assert_eq!(marker.rows[3], u128::MAX);
        assert_eq!(marker.rows[2], u128::MAX << HALF_ROW_CELLS);
        let marker = MarkerCoverage::from_rectangle(rectangle(0, 0, 1, 0)).unwrap();
        assert!(marker.rows.into_iter().all(|row| row == 1));
        // Extent rounding wraps before the shift, so 65535 also means zero.
        let wrapped = MarkerCoverage::from_rectangle(rectangle(0, 0, 1, u16::MAX)).unwrap();
        assert_eq!(marker, wrapped);
    }

    #[test]
    fn erase_is_not_reference_counted_and_selected_exemption_is_respected() {
        let mut world = WorldOccupancy::default();
        let marker = MarkerCoverage::from_rectangle(rectangle(0, 0, 1, 1)).unwrap();
        world.apply(&marker, OccupancyChange::Mark);
        world.apply(&marker, OccupancyChange::Mark);
        assert!(world.blocks_path(Vector3::default(), false));
        assert!(!world.blocks_path(Vector3::default(), true));
        world.apply(&marker, OccupancyChange::Erase);
        assert!(!world.contains(Vector3::default()));
    }
}
