//! The Star Fox 2 map screen's sprite layer, drawn from the ported map's
//! sprite list (its OAM buffer) and palette.
//!
//! The sprite tiles are the map's character set as retail holds it in
//! VRAM (OBSEL 03: base word 6000, 8x8 and 16x16 sprites), captured from a
//! campaign map frame. The crafts' tiles (characters 00..03 and 10..13)
//! are streamed per frame for their headings, from the frame sheet and its
//! table (`$04:D68B`) captured from the same frame.

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 224;
/// OAM: 128 four-byte entries, then the 32-byte high table.
pub const OAM_BYTES: usize = 0x220;
/// CGRAM: 256 BGR555 colours.
pub const CGRAM_BYTES: usize = 0x200;
/// OAM, CGRAM, then the ship's and the wingmate's frame set and sector.
pub const LAYER_BYTES: usize = OAM_BYTES + CGRAM_BYTES + 4;

const TILES: &[u8] = include_bytes!("../assets/sf2_map_obj_tiles.bin");
/// The frame table (16 sectors for each of three frame sets), then the
/// frame sheet: a frame's top two tiles at 32 bytes per table step, its
/// bottom two a sheet row (0x200 bytes) below.
const CRAFT: &[u8] = include_bytes!("../assets/sf2_map_craft.bin");
const CRAFT_TABLE: usize = 48;
const SHEET_ROW: usize = 0x200;
const TILE_ROW: usize = 0x200;
/// OBSEL 03 with name select 0: the second table follows at word 7000.
const NAME_SELECT: usize = 0x2000;
const SMALL: usize = 8;
const LARGE: usize = 16;
const OBJ_PALETTES: usize = 128;

/// Which sprites to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Every sprite (the oracle's comparison).
    All,
    /// The map area only: not the bottom panel's sprites (Y 0xB7 and
    /// below).
    MapArea,
}

/// The characters with each craft's frame streamed in: the ship's at
/// characters 00/01 and 10/11, the wingmate's at 02/03 and 12/13.
/// `crafts` holds each craft's frame set and sector.
pub fn characters(crafts: Option<[u8; 4]>) -> Vec<u8> {
    let mut tiles = TILES.to_vec();
    let Some(crafts) = crafts else {
        return tiles;
    };
    let (table, sheet) = CRAFT.split_at(CRAFT_TABLE);
    for (slot, frame) in crafts.chunks_exact(2).enumerate() {
        let index = usize::from(frame[0]) * 16 + usize::from(frame[1] & 0x0F);
        let Some(&entry) = table.get(index) else {
            continue;
        };
        let source = usize::from(entry) * 32;
        for row in 0..2 {
            let from = source + row * SHEET_ROW;
            let to = slot * 64 + row * TILE_ROW;
            if let Some(bytes) = sheet.get(from..from + 64) {
                tiles[to..to + 64].copy_from_slice(bytes);
            }
        }
    }
    tiles
}

/// The sprite layer as RGBA, transparent where no sprite covers a pixel.
pub fn rasterize(oam: &[u8], cgram: &[u8], tiles: &[u8], layer: Layer) -> Vec<u8> {
    let mut rgba = vec![0u8; WIDTH * HEIGHT * 4];
    if oam.len() < OAM_BYTES || cgram.len() < CGRAM_BYTES || tiles.len() < TILES.len() {
        return rgba;
    }
    // Lower-numbered sprites are drawn over higher-numbered ones; within a
    // pixel a higher priority wins.
    let mut priority_at = vec![None::<u8>; WIDTH * HEIGHT];
    for sprite in (0..128).rev() {
        let entry = &oam[sprite * 4..sprite * 4 + 4];
        let high = oam[0x200 + sprite / 4] >> ((sprite & 3) * 2);
        let size = if high & 2 != 0 { LARGE } else { SMALL };
        let mut x = i32::from(entry[0]) | (i32::from(high & 1) << 8);
        if x >= 256 {
            x -= 512;
        }
        let y = i32::from(entry[1].wrapping_add(1));
        let tile = usize::from(entry[2]);
        let attr = entry[3];
        if layer == Layer::MapArea && entry[1] >= 0xB7 {
            continue;
        }
        let palette = usize::from((attr >> 1) & 7);
        let priority = (attr >> 4) & 3;
        for py in 0..size {
            let mut screen_y = y + py as i32;
            if screen_y >= 256 {
                screen_y -= 256;
            }
            if !(0..HEIGHT as i32).contains(&screen_y) {
                continue;
            }
            let ty = if attr & 0x80 != 0 { size - 1 - py } else { py };
            for px in 0..size {
                let screen_x = x + px as i32;
                if !(0..WIDTH as i32).contains(&screen_x) {
                    continue;
                }
                let tx = if attr & 0x40 != 0 { size - 1 - px } else { px };
                let mut character = tile + tx / 8 + (ty / 8) * 16;
                let mut base = 0;
                if attr & 1 != 0 {
                    character &= 0xFF;
                    base = NAME_SELECT;
                }
                let value = pixel(tiles, base, character, tx % 8, ty % 8);
                if value == 0 {
                    continue;
                }
                let at = screen_y as usize * WIDTH + screen_x as usize;
                if priority_at[at].is_some_and(|old| priority < old) {
                    continue;
                }
                priority_at[at] = Some(priority);
                let index = OBJ_PALETTES + palette * 16 + usize::from(value);
                rgba[at * 4..at * 4 + 4].copy_from_slice(&color(cgram, index));
            }
        }
    }
    rgba
}

/// A 4bpp character's pixel; characters wrap within the 16K-word space.
fn pixel(tiles: &[u8], base: usize, character: usize, x: usize, y: usize) -> u8 {
    let address = (base + character * 32) % 0x4000 + y * 2;
    let byte = |offset: usize| tiles.get((address + offset) % 0x4000).copied().unwrap_or(0);
    let bit = 7 - x;
    let plane = |b: u8| (b >> bit) & 1;
    plane(byte(0)) | plane(byte(1)) << 1 | plane(byte(16)) << 2 | plane(byte(17)) << 3
}

fn color(cgram: &[u8], index: usize) -> [u8; 4] {
    let raw = u16::from_le_bytes([cgram[index * 2], cgram[index * 2 + 1]]);
    let expand = |five: u16| ((five << 3) | (five >> 2)) as u8;
    [expand(raw & 31), expand((raw >> 5) & 31), expand((raw >> 10) & 31), 255]
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../assets/sf2_map_obj_fixture.bin");
    /// The captured frame's crafts: Fox's set in sector 0E, Falco's in 0D.
    const CAPTURED_CRAFTS: [u8; 4] = [0, 0x0E, 1, 0x0D];

    fn fnv1a(bytes: &[u8]) -> u32 {
        bytes.iter().fold(0x811C_9DC5, |hash, &byte| (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193))
    }

    /// The retail sprite layer of the captured campaign map frame (the
    /// oracle PPU's OBJ snapshot), from the same frame's OAM and CGRAM.
    #[test]
    fn sprite_layer_matches_the_retail_capture() {
        let (oam, cgram) = FIXTURE.split_at(OAM_BYTES);
        let rgba = rasterize(oam, cgram, &characters(None), Layer::All);
        assert_eq!(fnv1a(&rgba), 0x0720_4E18);
    }

    /// The captured frame's crafts (Fox's frame set, ship sector 0E and
    /// wingmate sector from the same frame) stream the same tiles.
    #[test]
    fn streamed_craft_frames_match_the_capture() {
        let streamed = characters(Some(CAPTURED_CRAFTS));
        assert_eq!(&streamed[..0x80], &TILES[..0x80]);
        assert_eq!(&streamed[0x200..0x280], &TILES[0x200..0x280]);
    }

    #[test]
    fn the_map_area_leaves_out_the_panel() {
        let (oam, cgram) = FIXTURE.split_at(OAM_BYTES);
        let all = rasterize(oam, cgram, &characters(None), Layer::All);
        let map = rasterize(oam, cgram, &characters(None), Layer::MapArea);
        let drawn = |rgba: &[u8], y0: usize, y1: usize| (y0 * WIDTH..y1 * WIDTH).filter(|&p| rgba[p * 4 + 3] != 0).count();
        assert_eq!(drawn(&map, 184, HEIGHT), 0);
        assert!(drawn(&map, 0, 184) > 0);
        assert_eq!(drawn(&all, 0, 184), drawn(&map, 0, 184));
        assert!(drawn(&all, 184, HEIGHT) > 0);
    }
}
