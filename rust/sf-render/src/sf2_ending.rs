//! Native Star Fox 2 staff-roll and end-screen presentation.
//!
//! This generated asset contains ordinary 256x224 retail image-delta tracks.
//! Typed native ending state selects the uninterrupted presentation or the
//! short fade after accepting Start; no machine state is present.

const ASSET: &[u8] = include_bytes!("../assets/sf2_ending.bin");
const ASSET_MAGIC: &[u8; 5] = b"SFEN2";

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub const FRAME_COUNT: usize = 3_001;
pub const START_RESPONSE_FRAME_COUNT: usize = 8;
const CHANNELS_PER_PIXEL: usize = 4;
const ASSET_HEADER_FIELDS: usize = 6;
const BYTES_PER_FIELD: usize = 2;
const BYTES_PER_PALETTE_COLOR: usize = 3;
const RETAIL_FRAMES_PER_NATIVE_TICK: u16 = 4;
#[cfg(test)]
const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
#[cfg(test)]
const FNV_PRIME: u32 = 0x01000193;
#[cfg(test)]
const ASSET_FNV1A: u32 = 0x51DBFAB0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    StaffRoll,
    StartResponse,
}

impl Track {
    const fn frame_count(self) -> usize {
        match self {
            Self::StaffRoll => FRAME_COUNT,
            Self::StartResponse => START_RESPONSE_FRAME_COUNT,
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
    staff_roll_frames: Vec<Vec<PixelRun>>,
    start_response_frames: Vec<Vec<PixelRun>>,
    black_index: u16,
    current_track: Option<Track>,
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
        assert_eq!(usize::from(header[2]), FRAME_COUNT);
        assert_eq!(usize::from(header[3]), START_RESPONSE_FRAME_COUNT);
        let palette_count = usize::from(header[4]);
        let black_index = header[5];

        let mut palette = Vec::with_capacity(palette_count);
        for _ in 0..palette_count {
            let end = cursor + BYTES_PER_PALETTE_COLOR;
            let color = ASSET.get(cursor..end).expect("complete SF2 ending palette");
            palette.push([color[0], color[1], color[2], u8::MAX]);
            cursor = end;
        }
        assert_eq!(palette[usize::from(black_index)], [0, 0, 0, u8::MAX]);

        let staff_roll_frames = decode_frames(ASSET, &mut cursor, FRAME_COUNT);
        let start_response_frames = decode_frames(ASSET, &mut cursor, START_RESPONSE_FRAME_COUNT);
        assert_eq!(cursor, ASSET.len());

        Self {
            palette,
            staff_roll_frames,
            start_response_frames,
            black_index,
            current_track: None,
            current_frame: None,
            indices: vec![black_index; WIDTH * HEIGHT],
        }
    }

    pub fn frame_rgba(&mut self, track: Track, frame_index: usize) -> Vec<u8> {
        assert!(frame_index < track.frame_count());
        self.position(track, frame_index);
        let mut rgba = Vec::with_capacity(WIDTH * HEIGHT * CHANNELS_PER_PIXEL);
        for &index in &self.indices {
            rgba.extend_from_slice(&self.palette[usize::from(index)]);
        }
        rgba
    }

    fn position(&mut self, track: Track, frame_index: usize) {
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

    fn apply_frame(&mut self, track: Track, frame_index: usize) {
        let frames = match track {
            Track::StaffRoll => &self.staff_roll_frames,
            Track::StartResponse => &self.start_response_frames,
        };
        for run in &frames[frame_index] {
            let first = usize::from(run.first_offset);
            let end = first + run.palette_indices.len();
            self.indices[first..end].copy_from_slice(&run.palette_indices);
        }
        self.current_track = Some(track);
        self.current_frame = Some(frame_index);
    }
}

pub fn staff_roll_frame(presentation_tick: u32) -> usize {
    usize::try_from(presentation_tick)
        .unwrap_or(usize::MAX)
        .min(FRAME_COUNT - 1)
}

pub fn start_response_frame(elapsed_retail_frames: u16) -> usize {
    usize::from(elapsed_retail_frames / RETAIL_FRAMES_PER_NATIVE_TICK)
        .min(START_RESPONSE_FRAME_COUNT - 1)
}

fn read_u16(data: &[u8], cursor: &mut usize) -> u16 {
    let bytes: [u8; BYTES_PER_FIELD] = data[*cursor..*cursor + BYTES_PER_FIELD]
        .try_into()
        .expect("complete SF2 ending asset field");
    *cursor += BYTES_PER_FIELD;
    u16::from_le_bytes(bytes)
}

fn decode_frames(data: &[u8], cursor: &mut usize, frame_count: usize) -> Vec<Vec<PixelRun>> {
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
        assert_eq!(presentation.palette.len(), 267);
        assert_eq!(presentation.staff_roll_frames.len(), FRAME_COUNT);
        assert_eq!(
            presentation.start_response_frames.len(),
            START_RESPONSE_FRAME_COUNT
        );
    }

    #[test]
    fn frame_selection_clamps_the_unattended_end_screen() {
        assert_eq!(staff_roll_frame(0), 0);
        assert_eq!(staff_roll_frame(3_000), 3_000);
        assert_eq!(staff_roll_frame(3_001), 3_000);
        assert_eq!(staff_roll_frame(u32::MAX), 3_000);
        assert_eq!(start_response_frame(0), 0);
        assert_eq!(start_response_frame(28), 7);
        assert_eq!(start_response_frame(u16::MAX), 7);
    }

    #[test]
    fn certified_frames_match_cropped_retail_captures() {
        let mut presentation = Presentation::decode();
        for (frame, expected_hash) in [
            (0, 0xA0379DC5),
            (100, 0x02B1CE46),
            (500, 0xFF7403D7),
            (1_000, 0x999031B8),
            (1_500, 0xDBEDB110),
            (2_000, 0x222276D6),
            (2_500, 0x361987F6),
            (3_000, 0x52645755),
        ] {
            assert_eq!(
                fnv1a(presentation.frame_rgba(Track::StaffRoll, frame)),
                expected_hash
            );
        }
        for (frame, expected_hash) in [
            (0, 0x827762FB),
            (1, 0xC728F348),
            (2, 0x8999B7FF),
            (3, 0xBB265B91),
            (4, 0xC2BEF98A),
            (5, 0xB948AC52),
            (6, 0xB227A48D),
            (7, 0x6B2164DD),
        ] {
            assert_eq!(
                fnv1a(presentation.frame_rgba(Track::StartResponse, frame)),
                expected_hash
            );
        }
    }
}
