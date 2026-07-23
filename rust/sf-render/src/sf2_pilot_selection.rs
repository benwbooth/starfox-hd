//! Native Star Fox 2 pilot-selection presentation.
//!
//! The generated asset contains ordinary 256x224 retail image deltas selected
//! through typed menu, pilot, control-style, and presentation-phase values.

const ASSET: &[u8] = include_bytes!("../assets/sf2_pilot_selection.bin");
const ASSET_MAGIC: &[u8; 5] = b"SFPS2";

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub const REVEAL_FRAME_COUNT: usize = 24;
pub const PRIMARY_FRAME_COUNT: usize = 20;
pub const WING_FRAME_COUNT: usize = 20;
pub const READY_FRAME_COUNT: usize = 43;
pub const LAUNCH_FRAME_COUNT: usize = 57;
const PILOT_COUNT: usize = 6;
const PRIMARY_VARIANT_COUNT: usize = 8;
const PAIR_TRACK_COUNT: usize = PILOT_COUNT * PILOT_COUNT;
const TRACK_COUNT: usize = 1 + PRIMARY_VARIANT_COUNT + PAIR_TRACK_COUNT * 3;
const PRIMARY_TRACK_FIRST: usize = 1;
const WING_TRACK_FIRST: usize = PRIMARY_TRACK_FIRST + PRIMARY_VARIANT_COUNT;
const READY_TRACK_FIRST: usize = WING_TRACK_FIRST + PAIR_TRACK_COUNT;
const LAUNCH_TRACK_FIRST: usize = READY_TRACK_FIRST + PAIR_TRACK_COUNT;
const IDLE_LOOP_FIRST_FRAME: usize = 12;
const IDLE_LOOP_FRAME_COUNT: usize = 8;
const CHANNELS_PER_PIXEL: usize = 4;
const ASSET_HEADER_FIELDS: usize = 12;
const BYTES_PER_HEADER_FIELD: usize = 2;
const BYTES_PER_PALETTE_COLOR: usize = 3;
#[cfg(test)]
const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
#[cfg(test)]
const FNV_PRIME: u32 = 0x01000193;
#[cfg(test)]
const ASSET_FNV1A: u32 = 0x42206A73;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pilot {
    Fox,
    Falco,
    Peppy,
    Slippy,
    Miyu,
    Fay,
}

impl Pilot {
    const fn index(self) -> usize {
        match self {
            Self::Fox => 0,
            Self::Falco => 1,
            Self::Peppy => 2,
            Self::Slippy => 3,
            Self::Miyu => 4,
            Self::Fay => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimaryView {
    Pilot(Pilot),
    ControlA,
    ControlB,
}

impl PrimaryView {
    const fn index(self) -> usize {
        match self {
            Self::Pilot(pilot) => pilot.index(),
            Self::ControlA => 6,
            Self::ControlB => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Reveal,
    Primary(PrimaryView),
    Wingmate { primary: Pilot, cursor: Pilot },
    Ready { primary: Pilot, wingmate: Pilot },
    Launch { primary: Pilot, wingmate: Pilot },
}

impl Screen {
    const fn track_index(self) -> usize {
        match self {
            Self::Reveal => 0,
            Self::Primary(view) => PRIMARY_TRACK_FIRST + view.index(),
            Self::Wingmate { primary, cursor } => {
                WING_TRACK_FIRST + primary.index() * PILOT_COUNT + cursor.index()
            }
            Self::Ready { primary, wingmate } => {
                READY_TRACK_FIRST + primary.index() * PILOT_COUNT + wingmate.index()
            }
            Self::Launch { primary, wingmate } => {
                LAUNCH_TRACK_FIRST + primary.index() * PILOT_COUNT + wingmate.index()
            }
        }
    }

    const fn frame_count(self) -> usize {
        match self {
            Self::Reveal => REVEAL_FRAME_COUNT,
            Self::Primary(_) => PRIMARY_FRAME_COUNT,
            Self::Wingmate { .. } => WING_FRAME_COUNT,
            Self::Ready { .. } => READY_FRAME_COUNT,
            Self::Launch { .. } => LAUNCH_FRAME_COUNT,
        }
    }
}

#[derive(Clone, Debug)]
struct PixelRun {
    first_offset: u16,
    palette_indices: Vec<u8>,
}

#[derive(Debug)]
pub struct Presentation {
    palette: Vec<[u8; CHANNELS_PER_PIXEL]>,
    tracks: Vec<Vec<Vec<PixelRun>>>,
    black_index: u8,
    current_track: Option<usize>,
    current_frame: Option<usize>,
    indices: Vec<u8>,
}

impl Presentation {
    pub fn decode() -> Self {
        assert!(ASSET.starts_with(ASSET_MAGIC));
        let mut cursor = ASSET_MAGIC.len();
        let header: [u16; ASSET_HEADER_FIELDS] =
            std::array::from_fn(|_| read_u16(ASSET, &mut cursor));
        assert_eq!(usize::from(header[0]), WIDTH);
        assert_eq!(usize::from(header[1]), HEIGHT);
        assert_eq!(usize::from(header[2]), REVEAL_FRAME_COUNT);
        assert_eq!(usize::from(header[3]), PRIMARY_FRAME_COUNT);
        assert_eq!(usize::from(header[4]), WING_FRAME_COUNT);
        assert_eq!(usize::from(header[5]), READY_FRAME_COUNT);
        assert_eq!(usize::from(header[6]), LAUNCH_FRAME_COUNT);
        assert_eq!(usize::from(header[7]), PILOT_COUNT);
        assert_eq!(usize::from(header[8]), PRIMARY_VARIANT_COUNT);
        assert_eq!(usize::from(header[9]), TRACK_COUNT);
        let palette_count = usize::from(header[10]);
        assert!(palette_count <= usize::from(u8::MAX) + 1);
        let black_index = u8::try_from(header[11]).expect("pilot palette index fits in a byte");

        let mut palette = Vec::with_capacity(palette_count);
        for _ in 0..palette_count {
            let end = cursor + BYTES_PER_PALETTE_COLOR;
            let color = ASSET
                .get(cursor..end)
                .expect("complete pilot-selection palette");
            palette.push([color[0], color[1], color[2], u8::MAX]);
            cursor = end;
        }
        assert_eq!(palette[usize::from(black_index)], [0, 0, 0, u8::MAX]);

        let tracks = (0..TRACK_COUNT)
            .map(|track| decode_track(ASSET, &mut cursor, frame_count_for_track(track)))
            .collect();
        assert_eq!(cursor, ASSET.len());

        Self {
            palette,
            tracks,
            black_index,
            current_track: None,
            current_frame: None,
            indices: vec![black_index; WIDTH * HEIGHT],
        }
    }

    pub fn frame_rgba(&mut self, screen: Screen, frame_index: usize) -> Vec<u8> {
        assert!(frame_index < screen.frame_count());
        self.position(screen.track_index(), frame_index);
        let mut rgba = Vec::with_capacity(WIDTH * HEIGHT * CHANNELS_PER_PIXEL);
        for &index in &self.indices {
            rgba.extend_from_slice(&self.palette[usize::from(index)]);
        }
        rgba
    }

    fn position(&mut self, track: usize, frame_index: usize) {
        if self.current_track == Some(track) && self.current_frame == Some(frame_index) {
            return;
        }
        if self.current_track == Some(track)
            && self
                .current_frame
                .is_some_and(|current| current + 1 == frame_index)
        {
            self.apply_frame(track, frame_index);
            return;
        }

        self.indices.fill(self.black_index);
        self.current_track = Some(track);
        self.current_frame = None;
        for index in 0..=frame_index {
            self.apply_frame(track, index);
        }
    }

    fn apply_frame(&mut self, track: usize, frame_index: usize) {
        for run in &self.tracks[track][frame_index] {
            let first = usize::from(run.first_offset);
            let end = first + run.palette_indices.len();
            self.indices[first..end].copy_from_slice(&run.palette_indices);
        }
        self.current_track = Some(track);
        self.current_frame = Some(frame_index);
    }
}

pub fn frame_at_tick(screen: Screen, mode_tick: u32) -> usize {
    let frame = usize::try_from(mode_tick).unwrap_or(usize::MAX);
    match screen {
        Screen::Primary(_) | Screen::Wingmate { .. } if frame >= IDLE_LOOP_FIRST_FRAME => {
            IDLE_LOOP_FIRST_FRAME + (frame - IDLE_LOOP_FIRST_FRAME) % IDLE_LOOP_FRAME_COUNT
        }
        _ => frame.min(screen.frame_count() - 1),
    }
}

const fn frame_count_for_track(track: usize) -> usize {
    if track == 0 {
        REVEAL_FRAME_COUNT
    } else if track < WING_TRACK_FIRST {
        PRIMARY_FRAME_COUNT
    } else if track < READY_TRACK_FIRST {
        WING_FRAME_COUNT
    } else if track < LAUNCH_TRACK_FIRST {
        READY_FRAME_COUNT
    } else {
        LAUNCH_FRAME_COUNT
    }
}

fn read_u16(data: &[u8], cursor: &mut usize) -> u16 {
    let bytes: [u8; BYTES_PER_HEADER_FIELD] = data[*cursor..*cursor + BYTES_PER_HEADER_FIELD]
        .try_into()
        .expect("complete pilot-selection asset field");
    *cursor += BYTES_PER_HEADER_FIELD;
    u16::from_le_bytes(bytes)
}

fn decode_track(data: &[u8], cursor: &mut usize, frame_count: usize) -> Vec<Vec<PixelRun>> {
    (0..frame_count)
        .map(|_| {
            let run_count = usize::from(read_u16(data, cursor));
            (0..run_count)
                .map(|_| {
                    let first_offset = read_u16(data, cursor);
                    let length = usize::from(read_u16(data, cursor));
                    let end = *cursor + length;
                    let palette_indices = data
                        .get(*cursor..end)
                        .expect("complete pilot-selection pixel run")
                        .to_vec();
                    *cursor = end;
                    PixelRun {
                        first_offset,
                        palette_indices,
                    }
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fnv1a(bytes: impl IntoIterator<Item = u8>) -> u32 {
        bytes.into_iter().fold(FNV_OFFSET_BASIS, |value, byte| {
            (value ^ u32::from(byte)).wrapping_mul(FNV_PRIME)
        })
    }

    #[test]
    fn generated_asset_is_complete_and_current() {
        assert_eq!(fnv1a(ASSET.iter().copied()), ASSET_FNV1A);
        let presentation = Presentation::decode();
        assert_eq!(presentation.palette.len(), 179);
        assert_eq!(presentation.tracks.len(), TRACK_COUNT);
        assert_eq!(presentation.tracks[0].len(), REVEAL_FRAME_COUNT);
        assert_eq!(
            presentation.tracks[LAUNCH_TRACK_FIRST].len(),
            LAUNCH_FRAME_COUNT
        );
    }

    #[test]
    fn retail_cadence_clamps_transitions_and_loops_interactive_views() {
        let reveal = Screen::Reveal;
        assert_eq!(frame_at_tick(reveal, 0), 0);
        assert_eq!(frame_at_tick(reveal, 23), 23);
        assert_eq!(frame_at_tick(reveal, 24), 23);
        let fox = Screen::Primary(PrimaryView::Pilot(Pilot::Fox));
        assert_eq!(frame_at_tick(fox, 11), 11);
        assert_eq!(frame_at_tick(fox, 12), 12);
        assert_eq!(frame_at_tick(fox, 20), 12);
        assert_eq!(frame_at_tick(fox, 27), 19);
        assert_eq!(frame_at_tick(fox, 28), 12);
    }

    #[test]
    fn certified_frames_match_cropped_retail_captures() {
        let mut presentation = Presentation::decode();
        for (screen, frame, expected_hash) in [
            (Screen::Reveal, 0, 0xA0379DC5),
            (Screen::Reveal, 23, 0x18250AFF),
            (
                Screen::Primary(PrimaryView::Pilot(Pilot::Fox)),
                0,
                0x1E51B129,
            ),
            (
                Screen::Primary(PrimaryView::Pilot(Pilot::Fox)),
                19,
                0xEFAC4801,
            ),
            (
                Screen::Primary(PrimaryView::Pilot(Pilot::Falco)),
                0,
                0x52585CE5,
            ),
            (
                Screen::Primary(PrimaryView::Pilot(Pilot::Falco)),
                19,
                0x058C62B5,
            ),
            (Screen::Primary(PrimaryView::ControlA), 0, 0xD5753679),
            (Screen::Primary(PrimaryView::ControlA), 19, 0x257D9C65),
            (Screen::Primary(PrimaryView::ControlB), 0, 0x7EA42BA5),
            (Screen::Primary(PrimaryView::ControlB), 19, 0x169FDAA5),
            (
                Screen::Wingmate {
                    primary: Pilot::Fox,
                    cursor: Pilot::Slippy,
                },
                0,
                0xEE6C2BE5,
            ),
            (
                Screen::Wingmate {
                    primary: Pilot::Fox,
                    cursor: Pilot::Slippy,
                },
                19,
                0x4FE95A9C,
            ),
            (
                Screen::Ready {
                    primary: Pilot::Fox,
                    wingmate: Pilot::Slippy,
                },
                0,
                0x4FE95A9C,
            ),
            (
                Screen::Ready {
                    primary: Pilot::Fox,
                    wingmate: Pilot::Slippy,
                },
                42,
                0x97B52289,
            ),
            (
                Screen::Launch {
                    primary: Pilot::Fox,
                    wingmate: Pilot::Slippy,
                },
                0,
                0x9A0B5891,
            ),
            (
                Screen::Launch {
                    primary: Pilot::Fox,
                    wingmate: Pilot::Slippy,
                },
                56,
                0x2B75BA25,
            ),
            (
                Screen::Ready {
                    primary: Pilot::Falco,
                    wingmate: Pilot::Miyu,
                },
                0,
                0xC62724C8,
            ),
            (
                Screen::Ready {
                    primary: Pilot::Falco,
                    wingmate: Pilot::Miyu,
                },
                42,
                0xE8BCD089,
            ),
            (
                Screen::Launch {
                    primary: Pilot::Falco,
                    wingmate: Pilot::Miyu,
                },
                0,
                0x34E64A91,
            ),
            (
                Screen::Launch {
                    primary: Pilot::Falco,
                    wingmate: Pilot::Miyu,
                },
                56,
                0xB19F2BD8,
            ),
        ] {
            assert_eq!(fnv1a(presentation.frame_rgba(screen, frame)), expected_hash);
        }
    }
}
