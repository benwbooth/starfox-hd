//! Native Star Fox 2 strategic-map opening presentation.
//!
//! The generated asset is an ordinary 256x224 retail image-delta track. It
//! is selected by typed opening-overview state and contains no game memory.

const ASSET: &[u8] = include_bytes!("../assets/sf2_opening_overview.bin");
const ASSET_MAGIC: &[u8; 5] = b"SFOV2";

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub const FRAME_COUNT: usize = 940;
const CHANNELS_PER_PIXEL: usize = 4;
const ASSET_HEADER_FIELDS: usize = 5;
const BYTES_PER_FIELD: usize = 2;
const BYTES_PER_PALETTE_COLOR: usize = 3;
#[cfg(test)]
const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
#[cfg(test)]
const FNV_PRIME: u32 = 0x01000193;
#[cfg(test)]
const ASSET_FNV1A: u32 = 0xAB6EA6EF;

#[derive(Clone, Debug)]
struct PixelRun {
    first_offset: u16,
    palette_indices: Vec<u16>,
}

#[derive(Debug)]
pub struct Presentation {
    palette: Vec<[u8; CHANNELS_PER_PIXEL]>,
    frames: Vec<Vec<PixelRun>>,
    black_index: u16,
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
        let palette_count = usize::from(header[3]);
        let black_index = header[4];

        let mut palette = Vec::with_capacity(palette_count);
        for _ in 0..palette_count {
            let end = cursor + BYTES_PER_PALETTE_COLOR;
            let color = ASSET
                .get(cursor..end)
                .expect("complete SF2 opening-overview palette");
            palette.push([color[0], color[1], color[2], u8::MAX]);
            cursor = end;
        }
        assert_eq!(palette[usize::from(black_index)], [0, 0, 0, u8::MAX]);

        let frames = (0..FRAME_COUNT)
            .map(|_| {
                let run_count = usize::from(read_u16(ASSET, &mut cursor));
                (0..run_count)
                    .map(|_| {
                        let first_offset = read_u16(ASSET, &mut cursor);
                        let length = usize::from(read_u16(ASSET, &mut cursor));
                        let palette_indices =
                            (0..length).map(|_| read_u16(ASSET, &mut cursor)).collect();
                        PixelRun {
                            first_offset,
                            palette_indices,
                        }
                    })
                    .collect()
            })
            .collect();
        assert_eq!(cursor, ASSET.len());

        Self {
            palette,
            frames,
            black_index,
            current_frame: None,
            indices: vec![black_index; WIDTH * HEIGHT],
        }
    }

    pub fn frame_rgba(&mut self, frame_index: usize) -> Vec<u8> {
        assert!(frame_index < FRAME_COUNT);
        self.position(frame_index);
        let mut rgba = Vec::with_capacity(WIDTH * HEIGHT * CHANNELS_PER_PIXEL);
        for &index in &self.indices {
            rgba.extend_from_slice(&self.palette[usize::from(index)]);
        }
        rgba
    }

    fn position(&mut self, frame_index: usize) {
        if self.current_frame == Some(frame_index) {
            return;
        }
        if self
            .current_frame
            .is_some_and(|current| current + 1 == frame_index)
        {
            self.apply_frame(frame_index);
            return;
        }

        self.indices.fill(self.black_index);
        self.current_frame = None;
        for index in 0..=frame_index {
            self.apply_frame(index);
        }
    }

    fn apply_frame(&mut self, frame_index: usize) {
        for run in &self.frames[frame_index] {
            let first = usize::from(run.first_offset);
            let end = first + run.palette_indices.len();
            self.indices[first..end].copy_from_slice(&run.palette_indices);
        }
        self.current_frame = Some(frame_index);
    }
}

pub fn frame_at_tick(presentation_tick: u16) -> usize {
    usize::from(presentation_tick).min(FRAME_COUNT - 1)
}

fn read_u16(data: &[u8], cursor: &mut usize) -> u16 {
    let bytes: [u8; BYTES_PER_FIELD] = data[*cursor..*cursor + BYTES_PER_FIELD]
        .try_into()
        .expect("complete SF2 opening-overview asset field");
    *cursor += BYTES_PER_FIELD;
    u16::from_le_bytes(bytes)
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
        assert_eq!(presentation.palette.len(), 927);
        assert_eq!(presentation.frames.len(), FRAME_COUNT);
    }

    #[test]
    fn frame_selection_clamps_at_the_black_pilot_selection_handoff() {
        assert_eq!(frame_at_tick(0), 0);
        assert_eq!(frame_at_tick(939), 939);
        assert_eq!(frame_at_tick(940), 939);
    }

    #[test]
    fn certified_frames_match_cropped_retail_captures() {
        let mut presentation = Presentation::decode();
        for (frame, expected_hash) in [
            (0, 0xA0379DC5),
            (131, 0x79B79C01),
            (294, 0xEA45EFB7),
            (502, 0x7B4FBD58),
            (664, 0x8D2FA372),
            (800, 0xFE0618DD),
            (939, 0xA0379DC5),
        ] {
            assert_eq!(fnv1a(presentation.frame_rgba(frame)), expected_hash);
        }
    }
}
