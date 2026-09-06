//! Source-derived render work used to build the opening's concurrent scheduler.
//!
//! Work counts are not elapsed master clocks. Cache state, pending memory work
//! and bus grants must still be accounted for before scheduling an actor pass.

/// Inputs to the rectangular clear in the source display job. `base` is the
/// caller's bitmap pointer; the clear routine itself receives it as an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitmapClearLayout {
    pub base: u16,
    pub last_line: u16,
    pub width: u16,
    pub pitch: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapClearWorkError {
    /// The source reloads its width every row. A clear that overwrites that
    /// parameter cannot be represented by an invariant rectangular workload.
    OverwritesWidth,
}

/// Exact work of a non-self-modifying source bitmap clear, including its
/// post-tested, wrapping counters. Construction does not allocate a framebuffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitmapClearWork {
    layout: BitmapClearLayout,
    rows: u32,
    words_per_row: u32,
}

impl BitmapClearWork {
    pub fn new(layout: BitmapClearLayout) -> Result<Self, BitmapClearWorkError> {
        fn divide_two(value: u16) -> u16 {
            // Source DIV2 is arithmetic shift, except that -1 becomes zero.
            if value == u16::MAX {
                0
            } else {
                ((value as i16) >> 1) as u16
            }
        }
        fn loop_count(value: u16) -> u32 {
            if value == 0 {
                65_536
            } else {
                u32::from(value)
            }
        }
        let mut rows = layout.last_line.wrapping_add(1);
        for _ in 0..3 {
            rows = divide_two(rows);
        }
        let work = Self {
            layout,
            rows: loop_count(rows),
            words_per_row: loop_count(layout.width.wrapping_mul(2)),
        };
        // STW pairs A with A XOR 1, including odd starting pointers. Within a
        // row those pairs cover a circular contiguous interval of RAM bytes.
        // Width is reread at every row, so reject even a final-row alias rather
        // than silently treating a mutable source input as a constant.
        for row in 0..work.rows {
            let start = work.row_address(row).unwrap() & !1;
            let distance = u32::from(0x24C2_u16.wrapping_sub(start));
            if distance < work.words_per_row * 2 {
                return Err(BitmapClearWorkError::OverwritesWidth);
            }
        }
        Ok(work)
    }

    pub const fn rows(self) -> u32 {
        self.rows
    }

    pub const fn words_per_row(self) -> u32 {
        self.words_per_row
    }

    pub fn row_address(self, row: u32) -> Option<u16> {
        (row < self.rows).then(|| {
            self.layout
                .base
                .wrapping_add((row as u16).wrapping_mul(self.layout.pitch.wrapping_mul(4)))
        })
    }

    pub fn final_address(self) -> u16 {
        self.layout
            .base
            .wrapping_add((self.rows as u16).wrapping_mul(self.layout.pitch.wrapping_mul(4)))
    }

    pub const fn word_stores(self) -> u64 {
        self.rows as u64 * self.words_per_row as u64
    }

    /// Includes entry, row setup, loops and return delay; excludes caller and
    /// the instruction at the return target. This is deliberately not cycles.
    pub const fn source_instructions(self) -> u64 {
        23 + self.rows as u64 * (11 + 4 * self.words_per_row as u64)
    }

    /// Entry reads last-line, width and pitch; each row reloads width.
    pub const fn word_loads(self) -> u64 {
        3 + self.rows as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_work_depends_on_geometry_and_preserves_pitch() {
        let work = BitmapClearWork::new(BitmapClearLayout {
            base: 0x6C00,
            last_line: 191,
            width: 224,
            pitch: 256,
        })
        .unwrap();
        assert_eq!(work.rows(), 24);
        assert_eq!(work.words_per_row(), 448);
        assert_eq!(work.word_stores(), 10_752);
        assert_eq!(work.word_loads(), 27);
        assert_eq!(work.source_instructions(), 43_295);
        assert_eq!(work.row_address(1), Some(0x7000));
        assert_eq!(work.row_address(24), None);
        assert_eq!(work.final_address(), 0xCC00);
    }

    #[test]
    fn zero_counter_is_not_an_empty_clear_and_aliasing_is_explicit() {
        let layout = BitmapClearLayout {
            base: 0x6000,
            last_line: 0,
            width: 1,
            pitch: 0,
        };
        let work = BitmapClearWork::new(layout).unwrap();
        assert_eq!(work.rows(), 65_536);
        assert_eq!(work.word_stores(), 131_072);
        for width in [0, 0x8000] {
            assert_eq!(
                BitmapClearWork::new(BitmapClearLayout { width, ..layout }),
                Err(BitmapClearWorkError::OverwritesWidth)
            );
        }
        assert_eq!(
            BitmapClearWork::new(BitmapClearLayout {
                base: 0x24C3,
                ..layout
            }),
            Err(BitmapClearWorkError::OverwritesWidth)
        );
    }
}
