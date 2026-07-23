//! Native Star Fox ending-recap artwork.
//!
//! The original uncompressed development assets are embedded as immutable
//! graphics input, then converted once into modern RGBA textures. Game state
//! remains semantic and flat; no source-machine memory model is involved.

use crate::renderer::EndingReplayBackdrop;

pub const PANEL_LEFT: i32 = 112;
pub const PANEL_TOP: i32 = 16;
pub const PANEL_WIDTH: usize = 128;
pub const PANEL_HEIGHT: usize = 136;
pub const GLYPH_SIZE: usize = 8;
pub const GLYPH_COUNT: usize = 48;
pub const GLYPH_ATLAS_WIDTH: usize = GLYPH_SIZE * GLYPH_COUNT;
pub const GLYPH_ATLAS_HEIGHT: usize = GLYPH_SIZE;

const TILE_PIXELS: usize = GLYPH_SIZE * GLYPH_SIZE;
const FOUR_BIT_TILE_BYTES: usize = 32;
const TWO_BIT_TILE_BYTES: usize = 16;
const TILE_INDEX_MASK: u16 = 0x03ff;
const PALETTE_INDEX_MASK: u16 = 0x0007;
const HORIZONTAL_FLIP: u16 = 0x4000;
const VERTICAL_FLIP: u16 = 0x8000;
const TILEMAP_WIDTH_TILES: usize = 32;
const PANEL_SOURCE_TOP: usize = 41;

const RECAP_TILES: &[u8] = include_bytes!("../../../reference/ultrastarfox/SF/DATA/E-TEST2.CGX");
const RECAP_TILEMAP: &[u8] = include_bytes!("../../../reference/ultrastarfox/SF/DATA/E-TEST2.SCR");
const ENDING_GLYPHS: &[u8] = include_bytes!("../../../reference/ultrastarfox/SF/DATA/E-TEST.CGX");
const RISING_PALETTE: &[u8] =
    include_bytes!("../../../reference/ultrastarfox/SF/DATA/COL/E-TEST.COL");
const SPLIT_PALETTE: &[u8] =
    include_bytes!("../../../reference/ultrastarfox/SF/DATA/COL/E-TEST0.COL");

/// Source `etesttrans`, indexed by printable ASCII minus space. Hexadecimal is
/// retained because these are packed tile identities from the artwork.
const GLYPH_TRANSLATION: [u8; 60] = [
    0x00, 0x1c, 0x00, 0x2c, 0x2b, 0x2a, 0x00, 0x00, 0x00, 0x00, 0x29, 0x00, 0x00, 0x27, 0x28, 0x00,
    0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
    0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
];

#[inline]
fn expand_five_bits(value: u16) -> u8 {
    let value = value as u8;
    (value << 3) | (value >> 2)
}

fn palette_color(palette: &[u8], index: usize) -> [u8; 3] {
    let offset = index * 2;
    let packed = u16::from_le_bytes([palette[offset], palette[offset + 1]]);
    [
        expand_five_bits(packed & 0x001f),
        expand_five_bits((packed >> 5) & 0x001f),
        expand_five_bits((packed >> 10) & 0x001f),
    ]
}

fn decode_four_bit_tiles() -> Vec<[u8; TILE_PIXELS]> {
    RECAP_TILES
        .chunks_exact(FOUR_BIT_TILE_BYTES)
        .map(|tile| {
            let mut pixels = [0; TILE_PIXELS];
            for row in 0..GLYPH_SIZE {
                let low = tile[row * 2];
                let high = tile[row * 2 + 1];
                let upper_low = tile[TWO_BIT_TILE_BYTES + row * 2];
                let upper_high = tile[TWO_BIT_TILE_BYTES + row * 2 + 1];
                for column in 0..GLYPH_SIZE {
                    let bit = 7 - column;
                    pixels[row * GLYPH_SIZE + column] = ((low >> bit) & 1)
                        | (((high >> bit) & 1) << 1)
                        | (((upper_low >> bit) & 1) << 2)
                        | (((upper_high >> bit) & 1) << 3);
                }
            }
            pixels
        })
        .collect()
}

/// Decode the exact 128 by 136 recap panel seen in the independent retail
/// oracle. The source crop is the settled right-hand panel after its slide-in.
pub fn decode_panel(backdrop: EndingReplayBackdrop) -> Vec<u8> {
    let palette = match backdrop {
        EndingReplayBackdrop::RisingGradient => RISING_PALETTE,
        EndingReplayBackdrop::SplitGradient => SPLIT_PALETTE,
    };
    let tiles = decode_four_bit_tiles();
    let mut rgba = vec![0; PANEL_WIDTH * PANEL_HEIGHT * 4];

    for destination_row in 0..PANEL_HEIGHT {
        let source_y = PANEL_SOURCE_TOP + destination_row;
        for destination_column in 0..PANEL_WIDTH {
            let source_x = PANEL_LEFT as usize + destination_column;
            let entry_offset =
                ((source_y / GLYPH_SIZE) * TILEMAP_WIDTH_TILES + source_x / GLYPH_SIZE) * 2;
            let entry =
                u16::from_le_bytes([RECAP_TILEMAP[entry_offset], RECAP_TILEMAP[entry_offset + 1]]);
            let tile_index = usize::from(entry & TILE_INDEX_MASK);
            let mut tile_row = source_y % GLYPH_SIZE;
            let mut tile_column = source_x % GLYPH_SIZE;
            if entry & VERTICAL_FLIP != 0 {
                tile_row = GLYPH_SIZE - 1 - tile_row;
            }
            if entry & HORIZONTAL_FLIP != 0 {
                tile_column = GLYPH_SIZE - 1 - tile_column;
            }
            let color_index = usize::from(tiles[tile_index][tile_row * GLYPH_SIZE + tile_column]);
            let palette_row = usize::from((entry >> 10) & PALETTE_INDEX_MASK);
            let color = palette_color(palette, palette_row * 16 + color_index);
            let output = (destination_row * PANEL_WIDTH + destination_column) * 4;
            rgba[output..output + 3].copy_from_slice(&color);
            rgba[output + 3] = u8::MAX;
        }
    }

    rgba
}

/// Decode the original two-bit recap alphabet into a single-row RGBA atlas.
pub fn decode_glyph_atlas() -> Vec<u8> {
    let mut rgba = vec![0; GLYPH_ATLAS_WIDTH * GLYPH_ATLAS_HEIGHT * 4];
    for glyph in 0..GLYPH_COUNT {
        let source = &ENDING_GLYPHS[glyph * TWO_BIT_TILE_BYTES..(glyph + 1) * TWO_BIT_TILE_BYTES];
        for row in 0..GLYPH_SIZE {
            let low = source[row * 2];
            let high = source[row * 2 + 1];
            for column in 0..GLYPH_SIZE {
                let bit = 7 - column;
                let color_index = usize::from(((low >> bit) & 1) | (((high >> bit) & 1) << 1));
                if color_index == 0 {
                    continue;
                }
                let color = palette_color(RISING_PALETTE, color_index);
                let output = (row * GLYPH_ATLAS_WIDTH + glyph * GLYPH_SIZE + column) * 4;
                rgba[output..output + 3].copy_from_slice(&color);
                rgba[output + 3] = u8::MAX;
            }
        }
    }
    rgba
}

pub fn glyph_index(character: u8) -> usize {
    const FIRST_CHARACTER: u8 = b' ';
    let index = character.saturating_sub(FIRST_CHARACTER) as usize;
    usize::from(GLYPH_TRANSLATION.get(index).copied().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_symbols_use_the_source_tiles() {
        assert_eq!(glyph_index(b'A'), 2);
        assert_eq!(glyph_index(b'Z'), 27);
        assert_eq!(glyph_index(b'0'), 29);
        assert_eq!(glyph_index(b'#'), 44);
        assert_eq!(glyph_index(b'$'), 43);
        assert_eq!(glyph_index(b'%'), 42);
    }

    #[test]
    fn decoded_panels_have_the_oracle_geometry() {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x00000100000001b3;
        // Hashes are over the exact RGBA crops independently captured from
        // retail at x=112..239, y=16..151 after removing the emulator border.
        const EXPECTED: [(EndingReplayBackdrop, u64); 2] = [
            (EndingReplayBackdrop::RisingGradient, 0x73be2801b8a1d725),
            (EndingReplayBackdrop::SplitGradient, 0x0bd675580371cb25),
        ];

        for (backdrop, expected_hash) in EXPECTED {
            let panel = decode_panel(backdrop);
            assert_eq!(panel.len(), PANEL_WIDTH * PANEL_HEIGHT * 4);
            assert!(panel.chunks_exact(4).all(|pixel| pixel[3] == u8::MAX));
            let actual_hash = panel.iter().fold(FNV_OFFSET, |hash, &byte| {
                (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
            });
            assert_eq!(actual_hash, expected_hash);
        }
    }
}
