//! Native Star Fox 2 Andross briefing presentation.
//!
//! This generated asset is an ordinary 256x224 retail image-delta track.  It
//! is driven by the typed native `Briefing` mode and contains no machine state.

const ASSET: &[u8] = include_bytes!("../assets/sf2_briefing.bin");
const ASSET_MAGIC: &[u8; 5] = b"SFBR2";

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub const FRAME_COUNT: usize = 170;
const CHANNELS_PER_PIXEL: usize = 4;
const ASSET_HEADER_FIELDS: usize = 5;
const BYTES_PER_FIELD: usize = 2;
const BYTES_PER_PALETTE_COLOR: usize = 3;
#[cfg(test)]
const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
#[cfg(test)]
const FNV_PRIME: u32 = 0x01000193;
#[cfg(test)]
const ASSET_FNV1A: u32 = 0x447D945A;

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
                .expect("complete SF2 briefing palette");
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

pub fn frame_at_tick(mode_tick: u32) -> usize {
    usize::try_from(mode_tick)
        .unwrap_or(usize::MAX)
        .min(FRAME_COUNT - 1)
}

fn read_u16(data: &[u8], cursor: &mut usize) -> u16 {
    let bytes: [u8; BYTES_PER_FIELD] = data[*cursor..*cursor + BYTES_PER_FIELD]
        .try_into()
        .expect("complete SF2 briefing asset field");
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
        assert_eq!(presentation.palette.len(), 423);
        assert_eq!(presentation.frames.len(), FRAME_COUNT);
    }

    #[test]
    fn retail_cadence_clamps_at_the_black_handoff_frame() {
        assert_eq!(frame_at_tick(0), 0);
        assert_eq!(frame_at_tick(169), 169);
        assert_eq!(frame_at_tick(170), 169);
    }

    #[test]
    fn certified_frames_match_cropped_retail_captures() {
        let mut presentation = Presentation::decode();
        for (frame, expected_hash) in [
            (0, 0xA0379DC5),
            (12, 0xC3E864A9),
            (42, 0x78068920),
            (92, 0xAE0B2AE7),
            (142, 0x54847F08),
            (169, 0xA0379DC5),
        ] {
            assert_eq!(fnv1a(presentation.frame_rgba(frame)), expected_hash);
        }
    }
}
