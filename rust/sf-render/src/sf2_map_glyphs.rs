//! Dynamic strategic-map digits recovered from the retail HUD tiles.
//!
//! The mission and strategic-map screens use the same digit masks with
//! different palettes. This module recolors the decoded masks into the map
//! palette so the shipping UI can render typed campaign values over the
//! oracle-derived background.

pub const WIDTH: usize = 200;
pub const HEIGHT: usize = 16;
pub const GLYPH_SIZE: i32 = 8;
pub const ITEM_DIGIT_HEIGHT: i32 = 16;
pub const SCORE_DIGITS_LEFT: i32 = 0;
pub const TIME_DIGITS_LEFT: i32 = 80;
pub const ITEM_DIGITS_LEFT: i32 = 160;
pub const POST_CARRIER_SCORE_ZERO_LEFT: i32 = 192;

const CHANNELS_PER_PIXEL: usize = 4;
const DIGIT_COUNT: usize = 10;
const ITEM_DIGIT_COUNT: usize = 4;
const SOURCE_SHADOW: [u8; CHANNELS_PER_PIXEL] = [33, 33, 33, 255];
const TRANSPARENT: [u8; CHANNELS_PER_PIXEL] = [0, 0, 0, 0];
const MAP_SHADOW: [u8; CHANNELS_PER_PIXEL] = [24, 24, 24, 255];
const MAP_WHITE: [u8; CHANNELS_PER_PIXEL] = [255, 255, 255, 255];
const MAP_CLOCK_WHITE: [u8; CHANNELS_PER_PIXEL] = [247, 255, 255, 255];
const MAP_SCORE_BRIGHT: [u8; CHANNELS_PER_PIXEL] = [140, 255, 140, 255];
const MAP_SCORE_DARK: [u8; CHANNELS_PER_PIXEL] = [24, 214, 24, 255];
const MAP_TIME_BRIGHT: [u8; CHANNELS_PER_PIXEL] = [255, 214, 107, 255];
const MAP_TIME_DARK: [u8; CHANNELS_PER_PIXEL] = [231, 173, 66, 255];
const MAP_ITEM: [u8; CHANNELS_PER_PIXEL] = [231, 173, 66, 255];
const MAP_PANEL_BLUE: [u8; CHANNELS_PER_PIXEL] = [24, 24, 90, 255];

#[derive(Clone, Copy)]
enum DigitPalette {
    Score,
    Time,
    Item,
}

fn recolor(source: [u8; CHANNELS_PER_PIXEL], row: usize, palette: DigitPalette) -> [u8; 4] {
    if source[3] == 0 {
        return TRANSPARENT;
    }
    if source == SOURCE_SHADOW {
        return MAP_SHADOW;
    }
    match palette {
        DigitPalette::Score if row == 1 => MAP_WHITE,
        DigitPalette::Score if row <= 4 => MAP_SCORE_BRIGHT,
        DigitPalette::Score => MAP_SCORE_DARK,
        DigitPalette::Time if row == 1 => MAP_CLOCK_WHITE,
        DigitPalette::Time if row <= 4 => MAP_TIME_BRIGHT,
        DigitPalette::Time => MAP_TIME_DARK,
        DigitPalette::Item if row == 2 => MAP_CLOCK_WHITE,
        DigitPalette::Item => MAP_ITEM,
    }
}

#[allow(clippy::too_many_arguments)]
fn copy_glyph(
    destination: &mut [u8],
    source: &[u8],
    source_left: usize,
    destination_left: usize,
    width: usize,
    height: usize,
    palette: DigitPalette,
) {
    for row in 0..height {
        for column in 0..width {
            let source_offset =
                (row * crate::sf2_hud_glyphs::WIDTH + source_left + column) * CHANNELS_PER_PIXEL;
            let destination_offset = (row * WIDTH + destination_left + column) * CHANNELS_PER_PIXEL;
            let source_pixel = source[source_offset..source_offset + CHANNELS_PER_PIXEL]
                .try_into()
                .expect("HUD glyph pixels are RGBA");
            destination[destination_offset..destination_offset + CHANNELS_PER_PIXEL]
                .copy_from_slice(&recolor(source_pixel, row, palette));
        }
    }
}

fn set_pixel(rgba: &mut [u8], left: usize, row: usize, color: [u8; CHANNELS_PER_PIXEL]) {
    let offset = (row * WIDTH + left) * CHANNELS_PER_PIXEL;
    rgba[offset..offset + CHANNELS_PER_PIXEL].copy_from_slice(&color);
}

pub fn decode_rgba() -> Vec<u8> {
    let source = crate::sf2_hud_glyphs::decode_rgba();
    let mut rgba = vec![0; WIDTH * HEIGHT * CHANNELS_PER_PIXEL];
    let glyph_size = crate::sf2_hud_glyphs::GLYPH_SIZE as usize;

    for digit in 0..DIGIT_COUNT {
        copy_glyph(
            &mut rgba,
            &source,
            crate::sf2_hud_glyphs::SCORE_DIGITS_LEFT as usize + digit * glyph_size,
            SCORE_DIGITS_LEFT as usize + digit * glyph_size,
            glyph_size,
            glyph_size,
            DigitPalette::Score,
        );
        copy_glyph(
            &mut rgba,
            &source,
            crate::sf2_hud_glyphs::CLOCK_DIGITS_LEFT as usize + digit * glyph_size,
            TIME_DIGITS_LEFT as usize + digit * glyph_size,
            glyph_size,
            glyph_size,
            DigitPalette::Time,
        );
    }
    for digit in 0..ITEM_DIGIT_COUNT {
        copy_glyph(
            &mut rgba,
            &source,
            crate::sf2_hud_glyphs::ITEM_DIGITS_LEFT as usize + digit * glyph_size,
            ITEM_DIGITS_LEFT as usize + digit * glyph_size,
            glyph_size,
            ITEM_DIGIT_HEIGHT as usize,
            DigitPalette::Item,
        );
    }
    // The strategic-screen `3` uses a narrower seam where its two 8x8 OBJ
    // tiles meet than the in-flight item counter. These two rows are recovered
    // directly from `sf2_post_sortie_14640.ppm`.
    let item_three_left = ITEM_DIGITS_LEFT as usize + 3 * glyph_size;
    for row in 7..=8 {
        set_pixel(&mut rgba, item_three_left + 6, row, MAP_SHADOW);
        set_pixel(&mut rgba, item_three_left + 7, row, TRANSPARENT);
    }
    // The map clock's `9` leaves the leading pixel of its center row open.
    // The in-flight clock mask has a shadow pixel there instead.
    let time_nine_left = TIME_DIGITS_LEFT as usize + 9 * glyph_size;
    set_pixel(&mut rgba, time_nine_left, 4, TRANSPARENT);
    rgba
}

pub fn decode_post_interception_rgba() -> Vec<u8> {
    let mut rgba = decode_rgba();
    // Once the strategic clock reaches the post-interception phase, the
    // retail map uses the open-top variant of `5`. The ordinary flight HUD
    // mask differs by one bright pixel; keep the phase-specific art semantic
    // and confined to this generated-atlas bridge.
    let time_five_left = TIME_DIGITS_LEFT as usize + 5 * GLYPH_SIZE as usize;
    set_pixel(&mut rgba, time_five_left + 2, 2, MAP_SHADOW);
    rgba
}

pub fn decode_post_fighter_intercept_rgba() -> Vec<u8> {
    let mut rgba = decode_post_interception_rgba();
    // The fourth return uses the alternate strategic-map `3` mask. Keep the
    // three changed seam pixels in this semantic phase variant.
    let score_three_left = SCORE_DIGITS_LEFT as usize + 3 * GLYPH_SIZE as usize;
    set_pixel(&mut rgba, score_three_left + 1, 3, MAP_SHADOW);
    set_pixel(&mut rgba, score_three_left + 2, 3, MAP_SCORE_BRIGHT);
    set_pixel(&mut rgba, score_three_left + 1, 4, MAP_SHADOW);

    // The clock changes from 055 to 051 during this return. The certified
    // frame contains the retail scanline transition: the upper four rows are
    // still the old glyph while the lower four rows are already `1`.
    let time_one_left = TIME_DIGITS_LEFT as usize + GLYPH_SIZE as usize;
    for (row, mask) in ["ssssssss", "sWWWWWWs", "sssssBBs", "..sBBsss"]
        .into_iter()
        .enumerate()
    {
        for (column, symbol) in mask.chars().enumerate() {
            let color = match symbol {
                '.' => TRANSPARENT,
                's' => MAP_SHADOW,
                'W' => MAP_CLOCK_WHITE,
                'B' => MAP_TIME_BRIGHT,
                _ => unreachable!(),
            };
            set_pixel(&mut rgba, time_one_left + column, row, color);
        }
    }
    rgba
}

pub fn decode_post_pigma_rgba() -> Vec<u8> {
    let mut rgba = decode_post_interception_rgba();
    // Pigma's return retains the alternate strategic-map score `3`, but the
    // clock is stable at 061 rather than changing during the scanline.
    let score_three_left = SCORE_DIGITS_LEFT as usize + 3 * GLYPH_SIZE as usize;
    set_pixel(&mut rgba, score_three_left + 1, 3, MAP_SHADOW);
    set_pixel(&mut rgba, score_three_left + 2, 3, MAP_SCORE_BRIGHT);
    set_pixel(&mut rgba, score_three_left + 1, 4, MAP_SHADOW);

    // The strategic item counter's `2` is a distinct two-tile mask. These
    // seam and lower-stem pixels are recovered from the certified return.
    let item_two_left = ITEM_DIGITS_LEFT as usize + 2 * GLYPH_SIZE as usize;
    set_pixel(&mut rgba, item_two_left + 1, 6, TRANSPARENT);
    set_pixel(&mut rgba, item_two_left + 6, 9, TRANSPARENT);
    set_pixel(&mut rgba, item_two_left, 13, MAP_SHADOW);
    set_pixel(&mut rgba, item_two_left + 1, 13, MAP_ITEM);
    set_pixel(&mut rgba, item_two_left, 14, MAP_SHADOW);
    set_pixel(&mut rgba, item_two_left + 1, 14, MAP_SHADOW);
    rgba
}

pub fn decode_post_eladard_rgba() -> Vec<u8> {
    let mut rgba = decode_post_pigma_rgba();
    // The stable Eladard return uses the map-specific lower tail of `2` and
    // open-top `5`. These pixels come from the certified 02251 score frame.
    let score_two_left = SCORE_DIGITS_LEFT as usize + 2 * GLYPH_SIZE as usize;
    set_pixel(&mut rgba, score_two_left + 7, 6, TRANSPARENT);
    let score_five_left = SCORE_DIGITS_LEFT as usize + 5 * GLYPH_SIZE as usize;
    set_pixel(&mut rgba, score_five_left + 2, 2, MAP_SHADOW);
    rgba
}

pub fn decode_post_carrier_rgba() -> Vec<u8> {
    let mut rgba = decode_post_eladard_rgba();

    // The stable carrier return uses the alternate map-clock `7` mask.
    let time_seven_left = TIME_DIGITS_LEFT as usize + 7 * GLYPH_SIZE as usize;
    set_pixel(&mut rgba, time_seven_left + 3, 3, MAP_TIME_BRIGHT);
    set_pixel(&mut rgba, time_seven_left + 5, 3, MAP_SHADOW);
    set_pixel(&mut rgba, time_seven_left + 6, 4, MAP_PANEL_BLUE);

    // Its rolling score counter also exposes the strategic-map `0` variant.
    // Panel-blue pixels are opaque because the counter region is cleared
    // before the glyph is composited.
    let score_zero_left = POST_CARRIER_SCORE_ZERO_LEFT as usize;
    for row in 0..GLYPH_SIZE as usize {
        for column in 0..GLYPH_SIZE as usize {
            let source = (row * WIDTH + SCORE_DIGITS_LEFT as usize + column) * CHANNELS_PER_PIXEL;
            let destination = (row * WIDTH + score_zero_left + column) * CHANNELS_PER_PIXEL;
            let pixel: [u8; CHANNELS_PER_PIXEL] = rgba[source..source + CHANNELS_PER_PIXEL]
                .try_into()
                .expect("map glyph pixels are RGBA");
            rgba[destination..destination + CHANNELS_PER_PIXEL].copy_from_slice(&pixel);
        }
    }
    for (column, color) in [(0, MAP_SHADOW), (7, MAP_SHADOW)] {
        set_pixel(&mut rgba, score_zero_left + column, 0, color);
    }
    for (column, color) in [
        (1, MAP_WHITE),
        (3, MAP_SHADOW),
        (4, MAP_SHADOW),
        (6, MAP_WHITE),
    ] {
        set_pixel(&mut rgba, score_zero_left + column, 1, color);
    }
    for (column, color) in [(1, MAP_SHADOW), (3, MAP_SCORE_DARK), (4, MAP_SCORE_DARK)] {
        set_pixel(&mut rgba, score_zero_left + column, 5, color);
    }
    for (column, color) in [
        (0, MAP_PANEL_BLUE),
        (2, MAP_SHADOW),
        (3, MAP_SHADOW),
        (4, MAP_SHADOW),
        (6, MAP_SCORE_DARK),
    ] {
        set_pixel(&mut rgba, score_zero_left + column, 6, color);
    }
    for (column, color) in [
        (1, MAP_PANEL_BLUE),
        (2, MAP_PANEL_BLUE),
        (3, MAP_PANEL_BLUE),
        (7, MAP_SHADOW),
    ] {
        set_pixel(&mut rgba, score_zero_left + column, 7, color);
    }
    rgba
}

pub fn decode_post_mirage_rgba() -> Vec<u8> {
    let mut rgba = decode_post_carrier_rgba();
    // The stable post-Mirage score `03903` uses the open-center map `9`.
    // This single transparent pixel is recovered from the certified return
    // frame and leaves the panel-blue counter background visible.
    let score_nine_left = SCORE_DIGITS_LEFT as usize + 9 * GLYPH_SIZE as usize;
    set_pixel(&mut rgba, score_nine_left, 4, TRANSPARENT);
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_glyph(
        atlas: &[u8],
        atlas_left: usize,
        expected_rows: &[&str],
        colors: &[(char, [u8; CHANNELS_PER_PIXEL])],
    ) {
        for (row, expected) in expected_rows.iter().enumerate() {
            assert_eq!(expected.len(), GLYPH_SIZE as usize);
            for (column, symbol) in expected.chars().enumerate() {
                let source = (row * WIDTH + atlas_left + column) * CHANNELS_PER_PIXEL;
                let expected_color = colors
                    .iter()
                    .find_map(|(candidate, color)| (*candidate == symbol).then_some(*color))
                    .expect("every oracle mask symbol has a color");
                assert_eq!(
                    &atlas[source..source + CHANNELS_PER_PIXEL],
                    expected_color,
                    "glyph at atlas x={atlas_left}, row={row}, column={column}"
                );
            }
        }
    }

    #[test]
    fn dynamic_glyphs_match_the_certified_first_return_values() {
        let atlas = decode_rgba();
        let time_colors = [
            ('.', TRANSPARENT),
            ('s', MAP_SHADOW),
            ('W', MAP_CLOCK_WHITE),
            ('B', MAP_TIME_BRIGHT),
            ('D', MAP_TIME_DARK),
        ];
        assert_glyph(
            &atlas,
            TIME_DIGITS_LEFT as usize,
            &[
                ".ssssss.", "ssWWWWss", "sBBssBBs", "sBBssBBs", "sBBssBBs", "sDDssDDs", "ssDDDDss",
                ".ssssss.",
            ],
            &time_colors,
        );
        assert_glyph(
            &atlas,
            TIME_DIGITS_LEFT as usize + GLYPH_SIZE as usize,
            &[
                ".sssss..", ".sWWWs..", ".ssBBs..", "..sBBs..", "..sBBs..", "..sDDs..", "..sDDs..",
                "..ssss..",
            ],
            &time_colors,
        );
        assert_glyph(
            &atlas,
            TIME_DIGITS_LEFT as usize + 2 * GLYPH_SIZE as usize,
            &[
                "sssssss.", "sWWWWWss", "sssssBBs", "ssBBBBss", "sBBssss.", "sDDsssss", "sDDDDDDs",
                "ssssssss",
            ],
            &time_colors,
        );

        assert_glyph(
            &atlas,
            SCORE_DIGITS_LEFT as usize,
            &[
                ".ssssss.", "sswwwwss", "sGGssGGs", "sGGssGGs", "sGGssGGs", "sggssggs", "ssggggss",
                ".ssssss.",
            ],
            &[
                ('.', TRANSPARENT),
                ('s', MAP_SHADOW),
                ('w', MAP_WHITE),
                ('G', MAP_SCORE_BRIGHT),
                ('g', MAP_SCORE_DARK),
            ],
        );
        assert_glyph(
            &atlas,
            ITEM_DIGITS_LEFT as usize + 3 * GLYPH_SIZE as usize,
            &[
                "........", "ssssss..", "sWWWWWs.", "sDDDDDDs", "sssssDDs", "....sDDs", ".ssssDDs",
                ".sDDDDs.", ".sDDDDs.", ".ssssDDs", "....sDDs", "sssssDDs", "sDDDDDDs", "sDDDDDs.",
                "ssssss..", "........",
            ],
            &[
                ('.', TRANSPARENT),
                ('s', MAP_SHADOW),
                ('W', MAP_CLOCK_WHITE),
                ('D', MAP_ITEM),
            ],
        );
    }
}
