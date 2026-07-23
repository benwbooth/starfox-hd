//! Semantic Eladard-return palette for the strategic-map damage digits.

pub const WIDTH: usize = crate::sf2_map_damage_warning_glyphs::WIDTH;
pub const HEIGHT: usize = crate::sf2_map_damage_warning_glyphs::HEIGHT;
pub const DIGIT_WIDTH: i32 = crate::sf2_map_damage_warning_glyphs::DIGIT_WIDTH;
pub const DIGIT_HEIGHT: i32 = crate::sf2_map_damage_warning_glyphs::DIGIT_HEIGHT;

const CHANNELS_PER_PIXEL: usize = 4;
const WARNING_HIGHLIGHT: [u8; CHANNELS_PER_PIXEL] = [255, 255, 173, 255];
const WARNING_BRIGHT: [u8; CHANNELS_PER_PIXEL] = [255, 231, 90, 255];
const WARNING_DARK: [u8; CHANNELS_PER_PIXEL] = [255, 189, 90, 255];
const ELADARD_HIGHLIGHT: [u8; CHANNELS_PER_PIXEL] = [255, 148, 148, 255];
const ELADARD_BRIGHT: [u8; CHANNELS_PER_PIXEL] = [255, 57, 57, 255];
const ELADARD_DARK: [u8; CHANNELS_PER_PIXEL] = [181, 57, 57, 255];

pub fn decode_rgba() -> Vec<u8> {
    let mut rgba = crate::sf2_map_damage_warning_glyphs::decode_rgba();
    for pixel in rgba.chunks_exact_mut(CHANNELS_PER_PIXEL) {
        let source: [u8; CHANNELS_PER_PIXEL] =
            pixel.try_into().expect("damage atlas pixels are RGBA");
        let replacement = match source {
            WARNING_HIGHLIGHT => Some(ELADARD_HIGHLIGHT),
            WARNING_BRIGHT => Some(ELADARD_BRIGHT),
            WARNING_DARK => Some(ELADARD_DARK),
            _ => None,
        };
        if let Some(color) = replacement {
            pixel.copy_from_slice(&color);
        }
    }
    rgba
}
