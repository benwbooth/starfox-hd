//! Native Star Fox 2 campaign-loss Results presentation.
//!
//! The generated asset contains ordinary 256x224 retail image deltas. Source
//! extraction lives in `tools/sf2/generate_results_presentation.py`.

use crate::sf2_game_over::{apply_brightness, Brightness};

const ASSET: &[u8] = include_bytes!("../assets/sf2_results.bin");
const ASSET_MAGIC: &[u8; 5] = b"SFRS1";

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub const REVEAL_FRAME_COUNT: usize = 971;
pub const REVEAL_LOOP_FIRST_FRAME: usize = 843;
pub const OPENING_FRAME_COUNT: usize = 5;
pub const CHOICE_FRAME_COUNT: usize = 9;
pub const CHOICE_LOOP_FRAME_COUNT: usize = 8;
pub const LEAVING_FRAME_COUNT: usize = 2;
const TRACK_COUNT: usize = 6;
const CHANNELS_PER_PIXEL: usize = 4;
const ASSET_HEADER_FIELDS: usize = 10;
const BYTES_PER_HEADER_FIELD: usize = 2;
const BYTES_PER_PALETTE_COLOR: usize = 3;
const BYTES_PER_CHANGE: usize = 4;
const RETAIL_FRAMES_PER_PRESENTATION_FRAME: u32 = 4;
const CHOICE_ENTRY_RETAIL_FRAME: u32 = 20;
const CHOICE_LOOP_FIRST_RETAIL_FRAME: u32 = 24;
#[cfg(test)]
const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
#[cfg(test)]
const FNV_PRIME: u32 = 0x01000193;
#[cfg(test)]
const ASSET_FNV1A: u32 = 0x3FC91D01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    Reveal,
    Opening,
    RetryChoice,
    TitleChoice,
    RetryLeaving,
    TitleLeaving,
}

impl Track {
    const fn index(self) -> usize {
        match self {
            Self::Reveal => 0,
            Self::Opening => 1,
            Self::RetryChoice => 2,
            Self::TitleChoice => 3,
            Self::RetryLeaving => 4,
            Self::TitleLeaving => 5,
        }
    }

    const fn frame_count(self) -> usize {
        match self {
            Self::Reveal => REVEAL_FRAME_COUNT,
            Self::Opening => OPENING_FRAME_COUNT,
            Self::RetryChoice | Self::TitleChoice => CHOICE_FRAME_COUNT,
            Self::RetryLeaving | Self::TitleLeaving => LEAVING_FRAME_COUNT,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PixelChange {
    offset: u16,
    palette_index: u16,
}

#[derive(Debug)]
pub struct Presentation {
    palette: Vec<[u8; CHANNELS_PER_PIXEL]>,
    tracks: [Vec<Vec<PixelChange>>; TRACK_COUNT],
    black_index: u16,
    current_track: Option<Track>,
    current_frame: Option<usize>,
    indices: Vec<u16>,
    reveal_loop_indices: Option<Vec<u16>>,
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
        assert_eq!(usize::from(header[3]), REVEAL_LOOP_FIRST_FRAME);
        assert_eq!(usize::from(header[4]), OPENING_FRAME_COUNT);
        assert_eq!(usize::from(header[5]), CHOICE_FRAME_COUNT);
        assert_eq!(usize::from(header[6]), LEAVING_FRAME_COUNT);
        assert_eq!(usize::from(header[7]), TRACK_COUNT);
        let palette_count = usize::from(header[8]);
        let black_index = header[9];

        let mut palette = Vec::with_capacity(palette_count);
        for _ in 0..palette_count {
            let end = cursor + BYTES_PER_PALETTE_COLOR;
            let color = ASSET.get(cursor..end).expect("complete Results palette");
            palette.push([color[0], color[1], color[2], u8::MAX]);
            cursor = end;
        }
        assert_eq!(palette[usize::from(black_index)], [0, 0, 0, u8::MAX]);

        let tracks = [
            decode_track(ASSET, &mut cursor, REVEAL_FRAME_COUNT),
            decode_track(ASSET, &mut cursor, OPENING_FRAME_COUNT),
            decode_track(ASSET, &mut cursor, CHOICE_FRAME_COUNT),
            decode_track(ASSET, &mut cursor, CHOICE_FRAME_COUNT),
            decode_track(ASSET, &mut cursor, LEAVING_FRAME_COUNT),
            decode_track(ASSET, &mut cursor, LEAVING_FRAME_COUNT),
        ];
        assert_eq!(cursor, ASSET.len());

        Self {
            palette,
            tracks,
            black_index,
            current_track: None,
            current_frame: None,
            indices: vec![black_index; WIDTH * HEIGHT],
            reveal_loop_indices: None,
        }
    }

    pub fn frame_rgba(
        &mut self,
        track: Track,
        frame_index: usize,
        brightness: Brightness,
    ) -> Vec<u8> {
        self.position(track, frame_index);
        let mut rgba = Vec::with_capacity(WIDTH * HEIGHT * CHANNELS_PER_PIXEL);
        for &index in &self.indices {
            rgba.extend_from_slice(&self.palette[usize::from(index)]);
        }
        apply_brightness(&mut rgba, brightness);
        rgba
    }

    fn position(&mut self, track: Track, frame_index: usize) {
        assert!(frame_index < track.frame_count());
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
        if track == Track::Reveal
            && frame_index == REVEAL_LOOP_FIRST_FRAME
            && self.reveal_loop_indices.is_some()
        {
            self.indices
                .copy_from_slice(self.reveal_loop_indices.as_deref().unwrap_or_default());
            self.current_track = Some(track);
            self.current_frame = Some(frame_index);
            return;
        }

        self.indices.fill(self.black_index);
        self.current_track = Some(track);
        self.current_frame = None;
        for index in 0..=frame_index {
            self.apply_frame(track, index);
        }
    }

    fn apply_frame(&mut self, track: Track, frame_index: usize) {
        for change in &self.tracks[track.index()][frame_index] {
            self.indices[usize::from(change.offset)] = change.palette_index;
        }
        self.current_track = Some(track);
        self.current_frame = Some(frame_index);
        if track == Track::Reveal && frame_index == REVEAL_LOOP_FIRST_FRAME {
            self.reveal_loop_indices = Some(self.indices.clone());
        }
    }
}

pub fn reveal_frame_at_retail_frame(elapsed_retail_frames: u32) -> usize {
    let frame = usize::try_from(elapsed_retail_frames / RETAIL_FRAMES_PER_PRESENTATION_FRAME)
        .unwrap_or(usize::MAX);
    if frame < REVEAL_FRAME_COUNT {
        frame
    } else {
        REVEAL_LOOP_FIRST_FRAME
            + (frame - REVEAL_LOOP_FIRST_FRAME) % (REVEAL_FRAME_COUNT - REVEAL_LOOP_FIRST_FRAME)
    }
}

pub fn opening_frame_at_retail_frame(elapsed_retail_frames: u32) -> usize {
    usize::try_from(elapsed_retail_frames / RETAIL_FRAMES_PER_PRESENTATION_FRAME)
        .unwrap_or(usize::MAX)
        .min(OPENING_FRAME_COUNT - 1)
}

pub fn choice_frame_at_retail_frame(elapsed_retail_frames: u32) -> usize {
    if elapsed_retail_frames <= CHOICE_ENTRY_RETAIL_FRAME {
        return 0;
    }
    let loop_frame = elapsed_retail_frames.saturating_sub(CHOICE_LOOP_FIRST_RETAIL_FRAME)
        / RETAIL_FRAMES_PER_PRESENTATION_FRAME;
    1 + usize::try_from(loop_frame).unwrap_or_default() % CHOICE_LOOP_FRAME_COUNT
}

pub fn leaving_frame_at_retail_frame(elapsed_retail_frames: u32) -> usize {
    usize::from(elapsed_retail_frames != 0)
}

fn read_u16(data: &[u8], cursor: &mut usize) -> u16 {
    let bytes: [u8; BYTES_PER_HEADER_FIELD] = data[*cursor..*cursor + BYTES_PER_HEADER_FIELD]
        .try_into()
        .expect("complete Results asset field");
    *cursor += BYTES_PER_HEADER_FIELD;
    u16::from_le_bytes(bytes)
}

fn decode_track(data: &[u8], cursor: &mut usize, frame_count: usize) -> Vec<Vec<PixelChange>> {
    (0..frame_count)
        .map(|_| {
            let change_count = usize::from(read_u16(data, cursor));
            let bytes = change_count * BYTES_PER_CHANGE;
            assert!(*cursor + bytes <= data.len());
            (0..change_count)
                .map(|_| PixelChange {
                    offset: read_u16(data, cursor),
                    palette_index: read_u16(data, cursor),
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
        assert_eq!(presentation.tracks[Track::Reveal.index()].len(), 971);
        assert_eq!(presentation.tracks[Track::Opening.index()].len(), 5);
        assert_eq!(presentation.tracks[Track::RetryChoice.index()].len(), 9);
        assert_eq!(presentation.tracks[Track::TitleChoice.index()].len(), 9);
    }

    #[test]
    fn retail_cadence_selects_reveal_opening_choice_and_idle_frames() {
        assert_eq!(reveal_frame_at_retail_frame(0), 0);
        assert_eq!(reveal_frame_at_retail_frame(652), 163);
        assert_eq!(reveal_frame_at_retail_frame(3_880), 970);
        assert_eq!(reveal_frame_at_retail_frame(3_884), 843);
        assert_eq!(opening_frame_at_retail_frame(0), 0);
        assert_eq!(opening_frame_at_retail_frame(16), 4);
        assert_eq!(choice_frame_at_retail_frame(20), 0);
        assert_eq!(choice_frame_at_retail_frame(24), 1);
        assert_eq!(choice_frame_at_retail_frame(56), 1);
        assert_eq!(leaving_frame_at_retail_frame(0), 0);
        assert_eq!(leaving_frame_at_retail_frame(4), 1);
    }

    #[test]
    fn certified_frames_match_the_cropped_retail_captures() {
        let mut presentation = Presentation::decode();
        for (track, frame, expected_hash) in [
            (Track::Reveal, 0, 0xA0379DC5),
            (Track::Reveal, 107, 0xB44667B9),
            (Track::Reveal, 163, 0x0281362F),
            (Track::Reveal, 843, 0xB6678DD1),
            (Track::Reveal, 970, 0xF8E4D9C8),
            (Track::Opening, 0, 0x2C013838),
            (Track::Opening, 4, 0xC822F4CC),
            (Track::RetryChoice, 0, 0xB6D4CF91),
            (Track::RetryChoice, 1, 0xB6D4CF91),
            (Track::RetryChoice, 8, 0xB8A4FBD1),
            (Track::TitleChoice, 1, 0x2AD27B41),
            (Track::TitleChoice, 8, 0x81193E99),
            (Track::RetryLeaving, 0, 0xB6D4CF91),
            (Track::RetryLeaving, 1, 0x793EA790),
            (Track::TitleLeaving, 0, 0x46A4EC91),
            (Track::TitleLeaving, 1, 0xE8BED150),
        ] {
            let rgba = presentation.frame_rgba(track, frame, Brightness::Full);
            assert_eq!(fnv1a(rgba), expected_hash);
        }
    }

    #[test]
    fn leaving_brightness_matches_each_retail_destination() {
        let mut presentation = Presentation::decode();
        let retry_thirteen =
            presentation.frame_rgba(Track::RetryLeaving, 1, Brightness::ThirteenFifteenths);
        assert_eq!(fnv1a(retry_thirteen), 0x5CFFFE51);
        let retry_seven =
            presentation.frame_rgba(Track::RetryLeaving, 1, Brightness::SevenFifteenths);
        assert_eq!(fnv1a(retry_seven), 0x5EA7D298);
        let title_nine =
            presentation.frame_rgba(Track::TitleLeaving, 1, Brightness::NineFifteenths);
        assert_eq!(fnv1a(title_nine), 0x136E6D99);
    }
}
