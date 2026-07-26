//! Exact source-tile decoder for the General Pepper map briefing portraits.

use crate::bg2d::{cgram_color, decode_4bpp_tile};

const DOG_TILES: &[u8] = include_bytes!("../../../reference/ultrastarfox/SF/DATA/DOG.CGX");
const DOG_TILEMAP: &[u8] = include_bytes!("../../../reference/ultrastarfox/SF/DATA/DOG.SCR");
const MAP_TILES: &[u8] = include_bytes!("../../../reference/ultrastarfox/SF/DATA/MAP.CGX");
const MAP_PALETTES: &[u8] = include_bytes!("../../../reference/ultrastarfox/SF/DATA/COL/MAP_C.COL");

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;

const TILE_SIZE: usize = 8;
const BYTES_PER_TILE: usize = 32;
const TILEMAP_COLUMNS: usize = 32;
const SOURCE_PALETTE: usize = 6;
const COLORS_PER_PALETTE: usize = 16;
const BRIEFING_TILE_BASE: usize = 386;
const PADDING_TILE: usize = BRIEFING_TILE_BASE - 1;
const PEPPER_LEFT_TILE: usize = 2;
const PEPPER_TOP_TILE: usize = 10;
const PEPPER_TILE_COLUMNS: usize = 6;
const PEPPER_TILE_ROWS: usize = 10;
// `MAP.CGX` itself is uploaded 32 VRAM tiles after `p_bg2_cgx`; the source
// `copyfox` address names that VRAM tile. Within the extracted file the Fox
// portrait therefore starts at tile zero.
const FOX_SOURCE_FIRST_TILE: usize = 0;
const FOX_LEFT_TILE: usize = 26;
const FOX_TOP_TILE: usize = 16;
const FOX_TILE_COLUMNS: usize = 4;
const FOX_TILE_ROWS: usize = 5;
const FOX_TILE_COUNT: usize = FOX_TILE_COLUMNS * FOX_TILE_ROWS;
const HORIZONTAL_FLIP_FLAG: u16 = 1 << 14;
const VERTICAL_FLIP_FLAG: u16 = 1 << 15;
const TILE_INDEX_MASK: u16 = 1023;
const RGBA_CHANNELS: usize = 4;
const OPAQUE_ALPHA: u8 = u8::MAX;

/// Decode the portrait-only part of DOG.SCR. The source's dynamic bitmap
/// tiles are intentionally transparent here; the renderer supplies the
/// selected planet and type-on text from typed presentation state.
pub fn decode_portraits() -> Vec<u8> {
    let mut rgba = vec![0; WIDTH * HEIGHT * RGBA_CHANNELS];
    let mut decoded = [0; TILE_SIZE * TILE_SIZE];
    for tile_y in PEPPER_TOP_TILE..PEPPER_TOP_TILE + PEPPER_TILE_ROWS {
        for tile_x in PEPPER_LEFT_TILE..PEPPER_LEFT_TILE + PEPPER_TILE_COLUMNS {
            let map_offset = (tile_y * TILEMAP_COLUMNS + tile_x) * 2;
            let map_word =
                u16::from_le_bytes([DOG_TILEMAP[map_offset], DOG_TILEMAP[map_offset + 1]]);
            let tile_index = usize::from(map_word & TILE_INDEX_MASK);
            if tile_index < PADDING_TILE {
                continue;
            }

            if tile_index == PADDING_TILE {
                decoded.fill(0);
            } else {
                let source_index = tile_index - BRIEFING_TILE_BASE;
                let source_start = source_index * BYTES_PER_TILE;
                decode_4bpp_tile(
                    &DOG_TILES[source_start..source_start + BYTES_PER_TILE],
                    &mut decoded,
                );
            }

            for pixel_y in 0..TILE_SIZE {
                for pixel_x in 0..TILE_SIZE {
                    let source_x = if map_word & HORIZONTAL_FLIP_FLAG != 0 {
                        TILE_SIZE - 1 - pixel_x
                    } else {
                        pixel_x
                    };
                    let source_y = if map_word & VERTICAL_FLIP_FLAG != 0 {
                        TILE_SIZE - 1 - pixel_y
                    } else {
                        pixel_y
                    };
                    let color_index = usize::from(decoded[source_y * TILE_SIZE + source_x]);
                    let color = cgram_color(
                        MAP_PALETTES,
                        SOURCE_PALETTE * COLORS_PER_PALETTE + color_index,
                    );
                    let output_x = tile_x * TILE_SIZE + pixel_x;
                    let output_y = tile_y * TILE_SIZE + pixel_y;
                    let output = (output_y * WIDTH + output_x) * RGBA_CHANNELS;
                    rgba[output..output + RGBA_CHANNELS].copy_from_slice(&[
                        color[0],
                        color[1],
                        color[2],
                        OPAQUE_ALPHA,
                    ]);
                }
            }
        }
    }

    // `copyfox` copies the first twenty extracted MAP.CGX tiles into the
    // briefing character area. Their source order is four columns of five
    // tiles, which produces the 32 by 40 Fox portrait at the right.
    for tile_index in 0..FOX_TILE_COUNT {
        let source_index = FOX_SOURCE_FIRST_TILE + tile_index;
        let source_start = source_index * BYTES_PER_TILE;
        decode_4bpp_tile(
            &MAP_TILES[source_start..source_start + BYTES_PER_TILE],
            &mut decoded,
        );
        let tile_x = FOX_LEFT_TILE + tile_index / FOX_TILE_ROWS;
        let tile_y = FOX_TOP_TILE + tile_index % FOX_TILE_ROWS;
        for pixel_y in 0..TILE_SIZE {
            for pixel_x in 0..TILE_SIZE {
                let color_index = usize::from(decoded[pixel_y * TILE_SIZE + pixel_x]);
                let color = cgram_color(
                    MAP_PALETTES,
                    SOURCE_PALETTE * COLORS_PER_PALETTE + color_index,
                );
                let output_x = tile_x * TILE_SIZE + pixel_x;
                let output_y = tile_y * TILE_SIZE + pixel_y;
                let output = (output_y * WIDTH + output_x) * RGBA_CHANNELS;
                rgba[output..output + RGBA_CHANNELS].copy_from_slice(&[
                    color[0],
                    color[1],
                    color[2],
                    OPAQUE_ALPHA,
                ]);
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_portraits_decode_only_the_authored_tilemap_regions() {
        let rgba = decode_portraits();
        assert_eq!(rgba.len(), WIDTH * HEIGHT * RGBA_CHANNELS);
        let opaque_pixels = rgba
            .chunks_exact(RGBA_CHANNELS)
            .filter(|pixel| pixel[3] == OPAQUE_ALPHA)
            .count();
        assert_eq!(
            opaque_pixels,
            (PEPPER_TILE_COLUMNS * PEPPER_TILE_ROWS + FOX_TILE_COUNT) * TILE_SIZE * TILE_SIZE
        );
    }

    #[test]
    fn copied_fox_tiles_form_the_authored_four_by_five_portrait() {
        let rgba = decode_portraits();
        let fox_region_has_color =
            (FOX_TOP_TILE * TILE_SIZE..(FOX_TOP_TILE + FOX_TILE_ROWS) * TILE_SIZE).any(|y| {
                (FOX_LEFT_TILE * TILE_SIZE..(FOX_LEFT_TILE + FOX_TILE_COLUMNS) * TILE_SIZE).any(
                    |x| {
                        let pixel = &rgba[(y * WIDTH + x) * RGBA_CHANNELS..][..RGBA_CHANNELS];
                        pixel[3] == OPAQUE_ALPHA && pixel[..3] != [0, 0, 0]
                    },
                )
            });
        assert!(fox_region_has_color);
    }
}
