//! Native Star Fox 2 boot and title-attract presentation.
//!
//! This generated asset contains ordinary 256x224 retail image-delta tracks.
//! Typed native intro state selects the unattended attract presentation or
//! the short response to accepting Start; no machine state is present.

const ASSET: &[u8] = include_bytes!("../assets/sf2_intro.bin");
const ASSET_MAGIC: &[u8; 5] = b"SFIN2";

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub const FRAME_COUNT: usize = 1_140;
pub const TITLE_RESPONSE_FRAME_COUNT: usize = 5;
pub const LOOP_START_FRAME: usize = 18;
pub const LOOP_FRAME_COUNT: usize = FRAME_COUNT - LOOP_START_FRAME;
const CHANNELS_PER_PIXEL: usize = 4;
const ASSET_HEADER_FIELDS: usize = 6;
const BYTES_PER_FIELD: usize = 2;
const BYTES_PER_PALETTE_COLOR: usize = 3;
#[cfg(test)]
const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
#[cfg(test)]
const FNV_PRIME: u32 = 0x01000193;
#[cfg(test)]
const ASSET_FNV1A: u32 = 0xF227BDB8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    Attract,
    TitleResponse,
}

impl Track {
    const fn frame_count(self) -> usize {
        match self {
            Self::Attract => FRAME_COUNT,
            Self::TitleResponse => TITLE_RESPONSE_FRAME_COUNT,
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
    frames: Vec<Vec<PixelRun>>,
    title_response_frames: Vec<Vec<PixelRun>>,
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
        assert_eq!(usize::from(header[3]), TITLE_RESPONSE_FRAME_COUNT);
        let palette_count = usize::from(header[4]);
        let black_index = header[5];

        let mut palette = Vec::with_capacity(palette_count);
        for _ in 0..palette_count {
            let end = cursor + BYTES_PER_PALETTE_COLOR;
            let color = ASSET.get(cursor..end).expect("complete SF2 intro palette");
            palette.push([color[0], color[1], color[2], u8::MAX]);
            cursor = end;
        }
        assert_eq!(palette[usize::from(black_index)], [0, 0, 0, u8::MAX]);

        let frames = decode_frames(ASSET, &mut cursor, FRAME_COUNT);
        let title_response_frames = decode_frames(ASSET, &mut cursor, TITLE_RESPONSE_FRAME_COUNT);
        assert_eq!(cursor, ASSET.len());

        Self {
            palette,
            frames,
            title_response_frames,
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
            Track::Attract => &self.frames,
            Track::TitleResponse => &self.title_response_frames,
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

pub fn frame_at_tick(presentation_tick: u16) -> usize {
    let tick = usize::from(presentation_tick);
    if tick < FRAME_COUNT {
        tick
    } else {
        LOOP_START_FRAME + (tick - LOOP_START_FRAME) % LOOP_FRAME_COUNT
    }
}

pub fn title_response_frame(countdown: u8) -> usize {
    TITLE_RESPONSE_FRAME_COUNT
        .saturating_sub(usize::from(countdown))
        .min(TITLE_RESPONSE_FRAME_COUNT - 1)
}

fn read_u16(data: &[u8], cursor: &mut usize) -> u16 {
    let bytes: [u8; BYTES_PER_FIELD] = data[*cursor..*cursor + BYTES_PER_FIELD]
        .try_into()
        .expect("complete SF2 intro asset field");
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
        assert_eq!(presentation.palette.len(), 1_965);
        assert_eq!(presentation.frames.len(), FRAME_COUNT);
        assert_eq!(
            presentation.title_response_frames.len(),
            TITLE_RESPONSE_FRAME_COUNT
        );
    }

    #[test]
    fn retail_attract_cycle_and_title_response_select_their_frames() {
        assert_eq!(frame_at_tick(0), 0);
        assert_eq!(frame_at_tick(1_139), 1_139);
        assert_eq!(frame_at_tick(1_140), LOOP_START_FRAME);
        assert_eq!(frame_at_tick(2_261), 1_139);
        assert_eq!(frame_at_tick(2_262), LOOP_START_FRAME);
        assert_eq!(title_response_frame(5), 0);
        assert_eq!(title_response_frame(1), 4);
    }

    #[test]
    fn certified_frames_match_cropped_retail_captures() {
        let mut presentation = Presentation::decode();
        for (frame, expected_hash) in [
            (0, 0xA0379DC5),
            (44, 0xD00001CE),
            (65, 0x017ADCBB),
            (155, 0x65B929AE),
            (679, 0xFD4B341D),
            (779, 0xC86738A8),
            (899, 0x7F3AE8D3),
            (1_139, 0x11C68A2D),
        ] {
            assert_eq!(
                fnv1a(presentation.frame_rgba(Track::Attract, frame)),
                expected_hash
            );
        }
        for (frame, expected_hash) in [
            (0, 0x9DB30AF6),
            (1, 0x81839FB9),
            (2, 0x50424658),
            (3, 0xEFE45FEB),
            (4, 0xCA82D68E),
        ] {
            assert_eq!(
                fnv1a(presentation.frame_rgba(Track::TitleResponse, frame)),
                expected_hash
            );
        }
    }
}
