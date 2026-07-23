//! Native Star Fox 2 title, menu, and records presentation.
//!
//! The generated asset contains ordinary 256x224 retail image deltas selected
//! through typed title-page, menu-item, difficulty, and audio-output values.

const ASSET: &[u8] = include_bytes!("../assets/sf2_title.bin");
const ASSET_MAGIC: &[u8; 5] = b"SFTL2";

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub const TITLE_FRAME_COUNT: usize = 150;
pub const RECORDS_FRAME_COUNT: usize = 200;
const TRACK_COUNT: usize = 8;
const RECORDS_LOOP_FIRST_FRAME: usize = 24;
const CHANNELS_PER_PIXEL: usize = 4;
const ASSET_HEADER_FIELDS: usize = 7;
const BYTES_PER_FIELD: usize = 2;
const BYTES_PER_PALETTE_COLOR: usize = 3;
#[cfg(test)]
const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
#[cfg(test)]
const FNV_PRIME: u32 = 0x01000193;
#[cfg(test)]
const ASSET_FNV1A: u32 = 0x16E4521C;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    Mission,
    RecordsMenu,
    Stereo,
    Mono,
    Normal,
    Hard,
    Expert,
    RecordsScreen,
}

impl Track {
    const fn index(self) -> usize {
        match self {
            Self::Mission => 0,
            Self::RecordsMenu => 1,
            Self::Stereo => 2,
            Self::Mono => 3,
            Self::Normal => 4,
            Self::Hard => 5,
            Self::Expert => 6,
            Self::RecordsScreen => 7,
        }
    }

    const fn frame_count(self) -> usize {
        match self {
            Self::RecordsScreen => RECORDS_FRAME_COUNT,
            _ => TITLE_FRAME_COUNT,
        }
    }
}

#[derive(Clone, Debug)]
struct PixelRun {
    first_offset: u16,
    palette_indices: Vec<u16>,
}

#[derive(Debug)]
pub struct Presentation {
    palette: Vec<[u8; CHANNELS_PER_PIXEL]>,
    tracks: Vec<Vec<Vec<PixelRun>>>,
    black_index: u16,
    current_track: Option<usize>,
    current_frame: Option<usize>,
    indices: Vec<u16>,
}

impl Presentation {
    pub fn decode() -> Self {
        assert!(ASSET.starts_with(ASSET_MAGIC));
        let mut cursor = ASSET_MAGIC.len();
        let header: [u16; ASSET_HEADER_FIELDS] =
            std::array::from_fn(|_| read_u16(ASSET, &mut cursor));
        assert_eq!(usize::from(header[0]), WIDTH);
        assert_eq!(usize::from(header[1]), HEIGHT);
        assert_eq!(usize::from(header[2]), TITLE_FRAME_COUNT);
        assert_eq!(usize::from(header[3]), RECORDS_FRAME_COUNT);
        assert_eq!(usize::from(header[4]), TRACK_COUNT);
        let palette_count = usize::from(header[5]);
        let black_index = header[6];

        let mut palette = Vec::with_capacity(palette_count);
        for _ in 0..palette_count {
            let end = cursor + BYTES_PER_PALETTE_COLOR;
            let color = ASSET.get(cursor..end).expect("complete SF2 title palette");
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

    pub fn frame_rgba(&mut self, track: Track, frame_index: usize) -> Vec<u8> {
        assert!(frame_index < track.frame_count());
        self.position(track.index(), frame_index);
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

pub fn frame_at_tick(track: Track, mode_tick: u32) -> usize {
    let frame = usize::try_from(mode_tick).unwrap_or(usize::MAX);
    match track {
        Track::RecordsScreen if frame >= RECORDS_FRAME_COUNT => {
            RECORDS_LOOP_FIRST_FRAME
                + (frame - RECORDS_LOOP_FIRST_FRAME)
                    % (RECORDS_FRAME_COUNT - RECORDS_LOOP_FIRST_FRAME)
        }
        _ => frame % track.frame_count(),
    }
}

const fn frame_count_for_track(track: usize) -> usize {
    if track + 1 == TRACK_COUNT {
        RECORDS_FRAME_COUNT
    } else {
        TITLE_FRAME_COUNT
    }
}

fn read_u16(data: &[u8], cursor: &mut usize) -> u16 {
    let bytes: [u8; BYTES_PER_FIELD] = data[*cursor..*cursor + BYTES_PER_FIELD]
        .try_into()
        .expect("complete SF2 title asset field");
    *cursor += BYTES_PER_FIELD;
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
                    let palette_indices = (0..length).map(|_| read_u16(data, cursor)).collect();
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
        assert_eq!(presentation.palette.len(), 445);
        assert_eq!(presentation.tracks.len(), TRACK_COUNT);
        assert_eq!(presentation.tracks[0].len(), TITLE_FRAME_COUNT);
        assert_eq!(presentation.tracks[7].len(), RECORDS_FRAME_COUNT);
    }

    #[test]
    fn retail_cadence_loops_titles_and_preserves_the_records_transition() {
        assert_eq!(frame_at_tick(Track::Mission, 0), 0);
        assert_eq!(frame_at_tick(Track::Mission, 149), 149);
        assert_eq!(frame_at_tick(Track::Mission, 150), 0);
        assert_eq!(frame_at_tick(Track::RecordsScreen, 199), 199);
        assert_eq!(frame_at_tick(Track::RecordsScreen, 200), 24);
        assert_eq!(frame_at_tick(Track::RecordsScreen, 375), 199);
        assert_eq!(frame_at_tick(Track::RecordsScreen, 376), 24);
    }

    #[test]
    fn certified_frames_match_cropped_retail_captures() {
        let mut presentation = Presentation::decode();
        for (track, frame, expected_hash) in [
            (Track::Mission, 0, 0xAED6AA21),
            (Track::Mission, 149, 0xECA7A20C),
            (Track::RecordsMenu, 0, 0x87ADDC8A),
            (Track::Stereo, 0, 0x26AAB505),
            (Track::Mono, 0, 0x29C0DD0D),
            (Track::Normal, 0, 0x8441A4A1),
            (Track::Hard, 0, 0xA0008C5A),
            (Track::Expert, 0, 0x3F3C6E0F),
            (Track::RecordsScreen, 0, 0x09B50842),
            (Track::RecordsScreen, 24, 0x319476EB),
            (Track::RecordsScreen, 199, 0x33F2E601),
        ] {
            assert_eq!(fnv1a(presentation.frame_rgba(track, frame)), expected_hash);
        }
    }
}
