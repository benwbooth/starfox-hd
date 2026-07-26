//! Exact source-tile decoder for CONT.ASM's TRAINING/GAME selection panel.

use sf_core::sf1_controls::BriefingChoice;

use crate::bg2d::{cgram_color, decode_4bpp_tile};

const SOURCE_TILES: &[u8] = include_bytes!("../../../reference/ultrastarfox/SF/DATA/OBJ-3.CGX");
const SOURCE_PALETTES: &[u8] =
    include_bytes!("../../../reference/ultrastarfox/SF/DATA/COL/BG2-E.COL");

const TILE_SIZE: usize = 8;
const BYTES_PER_TILE: usize = 32;
const RGBA_CHANNELS: usize = 4;
#[cfg(test)]
const ALPHA_CHANNEL: usize = 3;
const OPAQUE_ALPHA: u8 = u8::MAX;
const PANEL_COLUMNS: usize = 10;
const PANEL_ROWS: usize = 6;
const SOURCE_SPRITE_PALETTE_START: usize = 128;
const TRANSPARENT_COLOR: u8 = 0;
const CORNER_TILE: usize = 0;
const EDGE_TILE: usize = 1;
const SIDE_TILE: usize = 2;
const EMPTY_SELECTION_TILE: usize = 3;
const FIRST_TEXT_TILE: usize = 4;
const SELECTION_TILE: usize = 32;
const TRAINING_SELECTION_FIRST_ROW: usize = 1;
const TRAINING_SELECTION_LAST_ROW: usize = 2;
const GAME_SELECTION_FIRST_ROW: usize = 3;
const GAME_SELECTION_LAST_ROW: usize = 4;
const TEXT_COLUMNS: usize = 7;

pub const WIDTH: usize = PANEL_COLUMNS * TILE_SIZE;
pub const HEIGHT: usize = PANEL_ROWS * TILE_SIZE;
pub const SCREEN_LEFT: i32 = 168;
pub const SCREEN_TOP: i32 = 151;
/// CONT.ASM's authored 3D projection center inside the controller diagram.
pub const VANISH_X: f32 = 64.0;
pub const VANISH_Y: f32 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tile {
    index: usize,
    horizontal_flip: bool,
    vertical_flip: bool,
}

const fn tile(index: usize, horizontal_flip: bool, vertical_flip: bool) -> Tile {
    Tile {
        index,
        horizontal_flip,
        vertical_flip,
    }
}

const fn panel_tile(choice: BriefingChoice, row: usize, column: usize) -> Tile {
    if row == 0 {
        return match column {
            0 => tile(CORNER_TILE, false, false),
            column if column == PANEL_COLUMNS - 1 => tile(CORNER_TILE, true, false),
            _ => tile(EDGE_TILE, false, false),
        };
    }
    if row == PANEL_ROWS - 1 {
        return match column {
            0 => tile(CORNER_TILE, false, true),
            column if column == PANEL_COLUMNS - 1 => tile(CORNER_TILE, true, true),
            _ => tile(EDGE_TILE, false, true),
        };
    }
    if column == 0 {
        return tile(SIDE_TILE, false, false);
    }
    if column == PANEL_COLUMNS - 1 {
        return tile(SIDE_TILE, true, false);
    }
    if column == 1 {
        let selected = match choice {
            BriefingChoice::Training => {
                row >= TRAINING_SELECTION_FIRST_ROW && row <= TRAINING_SELECTION_LAST_ROW
            }
            BriefingChoice::Game => {
                row >= GAME_SELECTION_FIRST_ROW && row <= GAME_SELECTION_LAST_ROW
            }
        };
        return if selected {
            tile(
                SELECTION_TILE,
                false,
                row == TRAINING_SELECTION_LAST_ROW || row == GAME_SELECTION_LAST_ROW,
            )
        } else {
            tile(EMPTY_SELECTION_TILE, false, false)
        };
    }

    let text_index = FIRST_TEXT_TILE + (row - 1) * TEXT_COLUMNS + (column - 2);
    tile(text_index, false, false)
}

/// Decode the exact US/EU source sprite panel into top-down RGBA.
pub fn decode_selection(choice: BriefingChoice) -> Vec<u8> {
    let mut rgba = vec![0; WIDTH * HEIGHT * RGBA_CHANNELS];
    for row in 0..PANEL_ROWS {
        for column in 0..PANEL_COLUMNS {
            let spec = panel_tile(choice, row, column);
            let source =
                &SOURCE_TILES[spec.index * BYTES_PER_TILE..(spec.index + 1) * BYTES_PER_TILE];
            let mut pixels = [0; TILE_SIZE * TILE_SIZE];
            decode_4bpp_tile(source, &mut pixels);

            for y in 0..TILE_SIZE {
                for x in 0..TILE_SIZE {
                    let source_x = if spec.horizontal_flip {
                        TILE_SIZE - 1 - x
                    } else {
                        x
                    };
                    let source_y = if spec.vertical_flip {
                        TILE_SIZE - 1 - y
                    } else {
                        y
                    };
                    let color_index = pixels[source_y * TILE_SIZE + source_x];
                    let output =
                        ((row * TILE_SIZE + y) * WIDTH + column * TILE_SIZE + x) * RGBA_CHANNELS;
                    if color_index == TRANSPARENT_COLOR {
                        rgba[output..output + RGBA_CHANNELS].copy_from_slice(&[0; RGBA_CHANNELS]);
                    } else {
                        let color = cgram_color(
                            SOURCE_PALETTES,
                            SOURCE_SPRITE_PALETTE_START + usize::from(color_index),
                        );
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
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_selection_tables_decode_to_the_authored_sprite_rectangle() {
        let training = decode_selection(BriefingChoice::Training);
        let game = decode_selection(BriefingChoice::Game);
        assert_eq!(training.len(), WIDTH * HEIGHT * RGBA_CHANNELS);
        assert_eq!(game.len(), WIDTH * HEIGHT * RGBA_CHANNELS);
        assert_ne!(training, game);
        assert!(training
            .chunks_exact(RGBA_CHANNELS)
            .all(|pixel| pixel[ALPHA_CHANNEL] == OPAQUE_ALPHA));
    }

    #[test]
    fn source_selection_marker_moves_between_the_two_authored_rows() {
        assert_eq!(
            panel_tile(BriefingChoice::Training, TRAINING_SELECTION_FIRST_ROW, 1,),
            tile(SELECTION_TILE, false, false)
        );
        assert_eq!(
            panel_tile(BriefingChoice::Training, TRAINING_SELECTION_LAST_ROW, 1,),
            tile(SELECTION_TILE, false, true)
        );
        assert_eq!(
            panel_tile(BriefingChoice::Game, GAME_SELECTION_FIRST_ROW, 1),
            tile(SELECTION_TILE, false, false)
        );
        assert_eq!(
            panel_tile(BriefingChoice::Game, GAME_SELECTION_LAST_ROW, 1),
            tile(SELECTION_TILE, false, true)
        );
    }
}
