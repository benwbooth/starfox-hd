//! Typed screen-aperture transitions used by the native renderer.
//!
//! The retail game stores these effects as interpreter positions into
//! `circletab` and builds two scanline window edges with `MBUMWIPE.MC`.  The
//! port keeps the visible result instead: a semantic wipe kind, a presentation
//! frame, and the source scanline aperture.  No source-machine address space
//! or processor state crosses this boundary.

/// Width of the retail polygon playfield used by `MDATA.MC` wipe records.
pub const SOURCE_WIDTH: usize = 224;
/// Height of the retail polygon playfield used by `MDATA.MC` wipe records.
pub const SOURCE_HEIGHT: usize = 192;

const SCREEN_MIDDLE_X: i16 = 112;
const SCREEN_MIDDLE_Y: i16 = 95;
const SCREEN_BOTTOM_Y: i16 = 191;
const SCREEN_RIGHT_X: i16 = 223;
const STAR_FRAME_COUNT: u8 = 16;
const STAR_X_STEP: i16 = 7;
const STAR_Y_STEP: i16 = 6;
const HORIZONTAL_FRAME_COUNT: u8 = 12;
const HORIZONTAL_MIDDLE_Y: i16 = 96;
const HORIZONTAL_Y_STEP: i16 = 8;

/// Source-authored opening aperture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScreenWipeKind {
    /// Four diagonal rays expanding from the center (`mstarwipe`).
    #[default]
    StarReveal,
    /// A horizontal band expanding upward and downward (`mscramwipe`).
    HorizontalReveal,
}

impl ScreenWipeKind {
    /// Compact native map operands, not source `circletab` addresses. Two is
    /// reserved for the independently modeled smart-bomb color circle.
    pub const fn map_request(self) -> i16 {
        match self {
            Self::StarReveal => 1,
            Self::HorizontalReveal => 3,
        }
    }

    pub const fn from_map_request(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::StarReveal),
            3 => Some(Self::HorizontalReveal),
            _ => None,
        }
    }

    pub const fn frame_count(self) -> u8 {
        match self {
            Self::StarReveal => STAR_FRAME_COUNT,
            Self::HorizontalReveal => HORIZONTAL_FRAME_COUNT,
        }
    }
}

/// Source `do_circle_explosion` command state, distinct from the record
/// already rendered and from the `doingwipe` sprite/pause lock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ScreenWipeCommand {
    #[default]
    Idle,
    Record(ScreenWipeState),
    End,
}

/// Transfer-owned aperture lifecycle. A map request made during strategies
/// cannot replace the record selected before those strategies. Rendering the
/// final record, removing the aperture and releasing the sprite lock are
/// likewise three distinct source operations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenWipeSequence {
    command: ScreenWipeCommand,
    requested_kind: Option<ScreenWipeKind>,
    prepared_record: Option<ScreenWipeState>,
    blocks_sprites: bool,
}

impl ScreenWipeSequence {
    pub fn request(&mut self, kind: ScreenWipeKind) {
        self.requested_kind = Some(kind);
    }

    pub const fn blocks_sprites(self) -> bool {
        self.blocks_sprites
    }

    /// `do_circle_explosion`, before the following strategy visit. Return
    /// true only when `wipeend_do` clears the animation request. That routine
    /// does not clear `doingwipe`; the next idle visit owns that write.
    pub fn prepare_transfer(&mut self) -> bool {
        if let Some(kind) = self.requested_kind.take() {
            let mut record = ScreenWipeState::inactive();
            record.begin(kind);
            self.command = ScreenWipeCommand::Record(record);
        }
        self.prepared_record = None;
        match self.command {
            ScreenWipeCommand::Idle => {
                self.blocks_sprites = false;
                false
            }
            ScreenWipeCommand::Record(record) => {
                self.prepared_record = Some(record);
                false
            }
            ScreenWipeCommand::End => {
                self.command = ScreenWipeCommand::Idle;
                true
            }
        }
    }

    /// `do_window_wipe`, after drawing the current scene. Only the prepared
    /// record is consumed; a request alone does not yet produce an aperture.
    pub fn complete_scene(&mut self) -> ScreenWipeState {
        let Some(record) = self.prepared_record.take() else {
            return ScreenWipeState::inactive();
        };
        self.blocks_sprites = true;
        let mut next = record;
        self.command = if next.advance() {
            ScreenWipeCommand::Record(next)
        } else {
            ScreenWipeCommand::End
        };
        record
    }
}

/// Half-open visible interval on one source scanline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApertureSpan {
    pub left: u16,
    pub right_exclusive: u16,
}

/// One native wipe in progress. Inactive state has no meaningful frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenWipeState {
    pub kind: ScreenWipeKind,
    pub frame: u8,
    pub active: bool,
}

impl ScreenWipeState {
    pub const fn inactive() -> Self {
        Self {
            kind: ScreenWipeKind::StarReveal,
            frame: 0,
            active: false,
        }
    }

    pub fn begin(&mut self, kind: ScreenWipeKind) {
        self.kind = kind;
        self.frame = 0;
        self.active = true;
    }

    /// Advance to the next authored record. Returns whether the wipe remains
    /// active after the step.
    pub fn advance(&mut self) -> bool {
        if !self.active {
            return false;
        }
        if self.frame.saturating_add(1) >= self.kind.frame_count() {
            self.active = false;
            return false;
        }
        self.frame += 1;
        true
    }

    /// Visible intervals for all source scanlines in the current record.
    pub fn aperture_spans(self) -> [Option<ApertureSpan>; SOURCE_HEIGHT] {
        if !self.active {
            return [None; SOURCE_HEIGHT];
        }
        match self.kind {
            ScreenWipeKind::StarReveal => star_aperture(self.frame),
            ScreenWipeKind::HorizontalReveal => horizontal_aperture(self.frame),
        }
    }
}

fn horizontal_aperture(frame: u8) -> [Option<ApertureSpan>; SOURCE_HEIGHT] {
    let mut spans = [None; SOURCE_HEIGHT];
    let distance = i16::from(frame.min(HORIZONTAL_FRAME_COUNT - 1)) * HORIZONTAL_Y_STEP;
    let top = HORIZONTAL_MIDDLE_Y - distance;
    let bottom = HORIZONTAL_MIDDLE_Y + distance;
    for y in top..bottom {
        if (0..SOURCE_HEIGHT as i16).contains(&y) {
            spans[y as usize] = Some(ApertureSpan {
                left: 0,
                right_exclusive: SOURCE_WIDTH as u16,
            });
        }
    }
    spans
}

fn star_aperture(frame: u8) -> [Option<ApertureSpan>; SOURCE_HEIGHT] {
    let frame = i16::from(frame.min(STAR_FRAME_COUNT - 1));
    let left_x = SCREEN_MIDDLE_X - STAR_X_STEP * frame;
    let right_x = SCREEN_MIDDLE_X + STAR_X_STEP * frame;
    let top_y = SCREEN_MIDDLE_Y - STAR_Y_STEP * frame;
    let bottom_y = SCREEN_MIDDLE_Y + STAR_Y_STEP * frame;

    let mut left_edges = [SCREEN_MIDDLE_X; SOURCE_HEIGHT];
    let mut right_edges = [SCREEN_MIDDLE_X; SOURCE_HEIGHT];

    // MDATA.MC `mstarwipe`, in source record order. Multiple samples on one
    // scanline intentionally overwrite each other just as `mwindrawline`
    // stores successive horizontal-major samples into the same window cell.
    draw_source_line(&mut left_edges, SCREEN_MIDDLE_X, 0, left_x, top_y);
    draw_source_line(&mut right_edges, SCREEN_MIDDLE_X, 0, right_x, top_y);
    draw_source_line(&mut left_edges, 0, SCREEN_MIDDLE_Y, left_x, top_y);
    draw_source_line(
        &mut right_edges,
        SCREEN_RIGHT_X,
        SCREEN_MIDDLE_Y,
        right_x,
        top_y,
    );
    draw_source_line(&mut left_edges, 0, SCREEN_MIDDLE_Y, left_x, bottom_y);
    draw_source_line(
        &mut right_edges,
        SCREEN_RIGHT_X,
        SCREEN_MIDDLE_Y,
        right_x,
        bottom_y,
    );
    draw_source_line(
        &mut left_edges,
        SCREEN_MIDDLE_X,
        SCREEN_BOTTOM_Y,
        left_x,
        bottom_y,
    );
    draw_source_line(
        &mut right_edges,
        SCREEN_MIDDLE_X,
        SCREEN_BOTTOM_Y,
        right_x,
        bottom_y,
    );

    let mut spans = [None; SOURCE_HEIGHT];
    for y in 0..SOURCE_HEIGHT {
        let left = left_edges[y].min(right_edges[y]);
        let right = left_edges[y].max(right_edges[y]);
        if left < right {
            spans[y] = Some(ApertureSpan {
                left: left as u16,
                right_exclusive: (right + 1) as u16,
            });
        }
    }
    spans
}

/// Integer line walk used by `MBUMWIPE.MC`: start at half the difference
/// between the major- and minor-axis deltas, subtract the minor delta per
/// sample, and step the minor coordinate only when that accumulator becomes
/// negative.
fn draw_source_line(
    buffer: &mut [i16; SOURCE_HEIGHT],
    mut x: i16,
    mut y: i16,
    end_x: i16,
    end_y: i16,
) {
    let delta_x = (end_x - x).abs();
    let delta_y = (end_y - y).abs();
    let step_x = (end_x - x).signum();
    let step_y = (end_y - y).signum();

    if delta_x >= delta_y {
        let mut error = (delta_x - delta_y) / 2;
        for _ in 0..=delta_x {
            if (0..SOURCE_HEIGHT as i16).contains(&y) {
                buffer[y as usize] = x;
            }
            error -= delta_y;
            if error < 0 {
                error += delta_x.max(1);
                y += step_y;
            }
            x += step_x;
        }
    } else {
        let mut error = (delta_y - delta_x) / 2;
        for _ in 0..=delta_y {
            if (0..SOURCE_HEIGHT as i16).contains(&y) {
                buffer[y as usize] = x;
            }
            error -= delta_x;
            if error < 0 {
                error += delta_y.max(1);
                x += step_x;
            }
            y += step_y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_rendering_and_lock_cleanup_have_separate_source_boundaries() {
        for kind in [ScreenWipeKind::StarReveal, ScreenWipeKind::HorizontalReveal] {
            let mut sequence = ScreenWipeSequence::default();
            sequence.prepare_transfer();
            sequence.request(kind);
            assert!(
                !sequence.complete_scene().active,
                "map request cannot render immediately"
            );
            assert!(!sequence.blocks_sprites());
            for frame in 0..kind.frame_count() {
                assert!(!sequence.prepare_transfer());
                assert_eq!(sequence.blocks_sprites(), frame != 0);
                assert_eq!(
                    sequence.complete_scene(),
                    ScreenWipeState {
                        kind,
                        frame,
                        active: true
                    }
                );
                assert!(sequence.blocks_sprites());
            }
            assert!(sequence.prepare_transfer(), "wipeend clears the animation");
            assert!(sequence.blocks_sprites(), "wipeend retains the sprite lock");
            assert!(!sequence.complete_scene().active);
            assert!(!sequence.prepare_transfer());
            assert!(
                !sequence.blocks_sprites(),
                "the idle command releases sprites"
            );
            assert!(!sequence.complete_scene().active);
        }
    }

    #[test]
    fn a_new_request_rearms_only_at_the_next_transfer() {
        let mut sequence = ScreenWipeSequence::default();
        sequence.request(ScreenWipeKind::StarReveal);
        sequence.prepare_transfer();
        sequence.request(ScreenWipeKind::HorizontalReveal);
        assert_eq!(
            sequence.complete_scene(),
            ScreenWipeState {
                kind: ScreenWipeKind::StarReveal,
                frame: 0,
                active: true,
            }
        );
        sequence.prepare_transfer();
        assert_eq!(
            sequence.complete_scene(),
            ScreenWipeState {
                kind: ScreenWipeKind::HorizontalReveal,
                frame: 0,
                active: true,
            }
        );
        assert!(sequence.blocks_sprites());
    }

    #[test]
    fn star_records_begin_closed_and_end_at_source_authored_points() {
        let mut wipe = ScreenWipeState::inactive();
        wipe.begin(ScreenWipeKind::StarReveal);
        assert!(wipe.aperture_spans().iter().all(Option::is_none));

        assert!(wipe.advance());
        let second = wipe.aperture_spans();
        assert_eq!(second[0], None);
        assert_eq!(
            second[89],
            Some(ApertureSpan {
                left: 105,
                right_exclusive: 120
            })
        );
        assert_eq!(
            second[95],
            Some(ApertureSpan {
                left: 8,
                right_exclusive: 216
            })
        );
        assert_eq!(
            second[101],
            Some(ApertureSpan {
                left: 105,
                right_exclusive: 120
            })
        );
        assert_eq!(second[191], None);

        for _ in 1..STAR_FRAME_COUNT - 1 {
            assert!(wipe.advance());
        }
        assert_eq!(wipe.frame, STAR_FRAME_COUNT - 1);
        let final_record = wipe.aperture_spans();
        assert_eq!(
            final_record[5],
            Some(ApertureSpan {
                left: 7,
                right_exclusive: 218
            })
        );
        assert_eq!(
            final_record[95],
            Some(ApertureSpan {
                left: 0,
                right_exclusive: 224
            })
        );
        assert_eq!(
            final_record[185],
            Some(ApertureSpan {
                left: 7,
                right_exclusive: 218
            })
        );
        assert!(!wipe.advance());
        assert!(!wipe.active);
    }

    #[test]
    fn horizontal_records_expand_eight_lines_per_side() {
        let mut wipe = ScreenWipeState::inactive();
        wipe.begin(ScreenWipeKind::HorizontalReveal);
        assert!(wipe.aperture_spans().iter().all(Option::is_none));

        assert!(wipe.advance());
        let second = wipe.aperture_spans();
        assert_eq!(second.iter().filter(|span| span.is_some()).count(), 16);
        assert_eq!(second[87], None);
        assert_eq!(second[88].unwrap().right_exclusive, SOURCE_WIDTH as u16);
        assert_eq!(second[103].unwrap().right_exclusive, SOURCE_WIDTH as u16);
        assert_eq!(second[104], None);

        for _ in 1..HORIZONTAL_FRAME_COUNT - 1 {
            assert!(wipe.advance());
        }
        assert_eq!(
            wipe.aperture_spans()
                .iter()
                .filter(|span| span.is_some())
                .count(),
            176
        );
        assert!(!wipe.advance());
    }
}
