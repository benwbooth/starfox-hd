//! Native Star Fox 2 Game Over presentation.
//!
//! The generated asset contains ordinary 256x224 retail image deltas and six
//! semantic pilot portraits. Oracle-only extraction lives in
//! `tools/sf2/generate_game_over_presentation.py`.

const ASSET: &[u8] = include_bytes!("../assets/sf2_game_over.bin");
const ASSET_MAGIC: &[u8; 5] = b"SFGO1";

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
pub const TAUNT_FRAME_COUNT: usize = 100;
pub const PROMPT_FRAME_COUNT: usize = 163;
pub const PROMPT_LOOP_FIRST_FRAME: usize = 19;
pub const PROMPT_LOOP_FRAME_COUNT: usize = 144;
pub const PILOT_COUNT: usize = 6;
const PORTRAIT_VARIANT_COUNT: usize = PILOT_COUNT + 1;
pub const PORTRAIT_LEFT: usize = 56;
pub const PORTRAIT_TOP: usize = 160;
pub const PORTRAIT_WIDTH: usize = 40;
pub const PORTRAIT_HEIGHT: usize = 48;
pub const PILOT_PORTRAIT_REVEAL_TICK: u32 = 97;
const CHANNELS_PER_PIXEL: usize = 4;
const FULL_BRIGHTNESS: u8 = 15;
const COLOR_COMPONENT_MAX: u16 = 31;
const ASSET_HEADER_FIELDS: usize = 12;
const BYTES_PER_HEADER_FIELD: usize = 2;
const BYTES_PER_PALETTE_COLOR: usize = 3;
const BYTES_PER_CHANGE: usize = 4;
#[cfg(test)]
const FNV_OFFSET_BASIS: u32 = 0x811C9DC5;
#[cfg(test)]
const FNV_PRIME: u32 = 0x01000193;
#[cfg(test)]
const ASSET_FNV1A: u32 = 0xF1AF67E9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    Taunt,
    PromptYes,
    PromptNo,
}

impl Track {
    const fn index(self) -> usize {
        match self {
            Self::Taunt => 0,
            Self::PromptYes => 1,
            Self::PromptNo => 2,
        }
    }

    const fn frame_count(self) -> usize {
        match self {
            Self::Taunt => TAUNT_FRAME_COUNT,
            Self::PromptYes | Self::PromptNo => PROMPT_FRAME_COUNT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Portrait {
    Fox,
    Falco,
    Peppy,
    Slippy,
    Miyu,
    Fay,
    None,
}

impl Portrait {
    const fn index(self) -> usize {
        match self {
            Self::Fox => 0,
            Self::Falco => 1,
            Self::Peppy => 2,
            Self::Slippy => 3,
            Self::Miyu => 4,
            Self::Fay => 5,
            Self::None => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brightness {
    Black,
    OneFifteenth,
    ThreeFifteenths,
    FiveFifteenths,
    SevenFifteenths,
    NineFifteenths,
    ElevenFifteenths,
    ThirteenFifteenths,
    Full,
}

impl Brightness {
    const fn level(self) -> u8 {
        match self {
            Self::Black => 0,
            Self::OneFifteenth => 1,
            Self::ThreeFifteenths => 3,
            Self::FiveFifteenths => 5,
            Self::SevenFifteenths => 7,
            Self::NineFifteenths => 9,
            Self::ElevenFifteenths => 11,
            Self::ThirteenFifteenths => 13,
            Self::Full => FULL_BRIGHTNESS,
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
    tracks: [Vec<Vec<PixelChange>>; 3],
    portraits: [Vec<u16>; PORTRAIT_VARIANT_COUNT],
    black_index: u16,
}

impl Presentation {
    pub fn decode() -> Self {
        assert!(ASSET.starts_with(ASSET_MAGIC));
        let mut cursor = ASSET_MAGIC.len();
        let header: [u16; ASSET_HEADER_FIELDS] =
            std::array::from_fn(|_| read_u16(ASSET, &mut cursor));
        assert_eq!(usize::from(header[0]), WIDTH);
        assert_eq!(usize::from(header[1]), HEIGHT);
        assert_eq!(usize::from(header[2]), TAUNT_FRAME_COUNT);
        assert_eq!(usize::from(header[3]), PROMPT_FRAME_COUNT);
        assert_eq!(usize::from(header[4]), PROMPT_LOOP_FIRST_FRAME);
        assert_eq!(usize::from(header[5]), PORTRAIT_VARIANT_COUNT);
        assert_eq!(usize::from(header[6]), PORTRAIT_LEFT);
        assert_eq!(usize::from(header[7]), PORTRAIT_TOP);
        assert_eq!(usize::from(header[8]), PORTRAIT_WIDTH);
        assert_eq!(usize::from(header[9]), PORTRAIT_HEIGHT);
        let palette_count = usize::from(header[10]);
        let black_index = header[11];

        let mut palette = Vec::with_capacity(palette_count);
        for _ in 0..palette_count {
            let end = cursor + BYTES_PER_PALETTE_COLOR;
            let color = ASSET.get(cursor..end).expect("complete Game Over palette");
            palette.push([color[0], color[1], color[2], u8::MAX]);
            cursor = end;
        }
        assert_eq!(palette[usize::from(black_index)], [0, 0, 0, u8::MAX]);

        let tracks = [
            decode_track(ASSET, &mut cursor, TAUNT_FRAME_COUNT),
            decode_track(ASSET, &mut cursor, PROMPT_FRAME_COUNT),
            decode_track(ASSET, &mut cursor, PROMPT_FRAME_COUNT),
        ];
        let portrait_pixels = PORTRAIT_WIDTH * PORTRAIT_HEIGHT;
        let portraits = std::array::from_fn(|_| {
            (0..portrait_pixels)
                .map(|_| read_u16(ASSET, &mut cursor))
                .collect()
        });
        assert_eq!(cursor, ASSET.len());

        Self {
            palette,
            tracks,
            portraits,
            black_index,
        }
    }

    pub fn frame_rgba(
        &self,
        track: Track,
        frame_index: usize,
        portrait: Option<Portrait>,
        brightness: Brightness,
    ) -> Vec<u8> {
        assert!(frame_index < track.frame_count());
        let mut indices = vec![self.black_index; WIDTH * HEIGHT];
        for changes in self.tracks[track.index()].iter().take(frame_index + 1) {
            for change in changes {
                indices[usize::from(change.offset)] = change.palette_index;
            }
        }
        if let Some(portrait) = portrait {
            self.blit_portrait(&mut indices, portrait);
        }

        let mut rgba = Vec::with_capacity(WIDTH * HEIGHT * CHANNELS_PER_PIXEL);
        for index in indices {
            let color = self.palette[usize::from(index)];
            rgba.extend_from_slice(&color);
        }
        apply_brightness(&mut rgba, brightness);
        rgba
    }

    fn blit_portrait(&self, indices: &mut [u16], portrait: Portrait) {
        let source = &self.portraits[portrait.index()];
        for row in 0..PORTRAIT_HEIGHT {
            let source_start = row * PORTRAIT_WIDTH;
            let destination_start = (PORTRAIT_TOP + row) * WIDTH + PORTRAIT_LEFT;
            indices[destination_start..destination_start + PORTRAIT_WIDTH]
                .copy_from_slice(&source[source_start..source_start + PORTRAIT_WIDTH]);
        }
    }
}

pub fn apply_brightness(rgba: &mut [u8], brightness: Brightness) {
    let level = brightness.level();
    for pixel in rgba.chunks_exact_mut(CHANNELS_PER_PIXEL) {
        pixel[0] = scaled_component(pixel[0], level);
        pixel[1] = scaled_component(pixel[1], level);
        pixel[2] = scaled_component(pixel[2], level);
    }
}

pub fn frame_at_mode_tick(mode_tick: u32, prompt_track: Track) -> (Track, usize) {
    debug_assert!(prompt_track != Track::Taunt);
    if mode_tick < TAUNT_FRAME_COUNT as u32 {
        return (Track::Taunt, mode_tick as usize);
    }
    let prompt_tick = mode_tick - TAUNT_FRAME_COUNT as u32;
    let frame = if prompt_tick < PROMPT_LOOP_FIRST_FRAME as u32 {
        prompt_tick as usize
    } else {
        PROMPT_LOOP_FIRST_FRAME
            + (prompt_tick as usize - PROMPT_LOOP_FIRST_FRAME) % PROMPT_LOOP_FRAME_COUNT
    };
    (prompt_track, frame)
}

fn scaled_component(component: u8, brightness: u8) -> u8 {
    let source = u16::from(component >> 3);
    let scaled = source * u16::from(brightness) / u16::from(FULL_BRIGHTNESS);
    debug_assert!(scaled <= COLOR_COMPONENT_MAX);
    let scaled = scaled as u8;
    (scaled << 3) | (scaled >> 2)
}

fn read_u16(data: &[u8], cursor: &mut usize) -> u16 {
    let bytes: [u8; BYTES_PER_HEADER_FIELD] = data[*cursor..*cursor + BYTES_PER_HEADER_FIELD]
        .try_into()
        .expect("complete Game Over asset field");
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
        assert_eq!(presentation.tracks[Track::Taunt.index()].len(), 100);
        assert_eq!(presentation.tracks[Track::PromptYes.index()].len(), 163);
        assert_eq!(presentation.tracks[Track::PromptNo.index()].len(), 163);
        assert_eq!(presentation.portraits.len(), PORTRAIT_VARIANT_COUNT);
    }

    #[test]
    fn certified_frames_match_the_cropped_retail_captures() {
        let presentation = Presentation::decode();
        for (track, frame, expected_hash) in [
            (Track::Taunt, 0, 0xA0379DC5),
            (Track::Taunt, 50, 0x270EE94A),
            (Track::Taunt, 99, 0xFEC41C59),
            (Track::PromptYes, 0, 0xE61C9921),
            (Track::PromptYes, 19, 0xB8E24DA6),
            (Track::PromptYes, 162, 0x7599DB04),
            (Track::PromptNo, 19, 0xE02CFB06),
            (Track::PromptNo, 162, 0x4980B798),
        ] {
            let rgba = presentation.frame_rgba(track, frame, None, Brightness::Full);
            assert_eq!(fnv1a(rgba), expected_hash);
        }
    }

    #[test]
    fn retail_prompt_cadence_has_a_nineteen_frame_reveal_and_144_frame_loop() {
        assert_eq!(frame_at_mode_tick(99, Track::PromptYes), (Track::Taunt, 99));
        assert_eq!(
            frame_at_mode_tick(100, Track::PromptYes),
            (Track::PromptYes, 0)
        );
        assert_eq!(
            frame_at_mode_tick(118, Track::PromptYes),
            (Track::PromptYes, 18)
        );
        assert_eq!(
            frame_at_mode_tick(119, Track::PromptYes),
            (Track::PromptYes, 19)
        );
        assert_eq!(
            frame_at_mode_tick(263, Track::PromptYes),
            (Track::PromptYes, 19)
        );
    }

    #[test]
    fn retail_brightness_quantizes_expanded_five_bit_color() {
        assert_eq!(scaled_component(u8::MAX, Brightness::Full.level()), u8::MAX);
        assert_eq!(
            scaled_component(u8::MAX, Brightness::ThirteenFifteenths.level()),
            214
        );
        assert_eq!(
            scaled_component(u8::MAX, Brightness::FiveFifteenths.level()),
            82
        );
        assert_eq!(scaled_component(u8::MAX, Brightness::Black.level()), 0);
    }
}
