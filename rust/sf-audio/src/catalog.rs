//! Stable Star Fox sound-catalog identifiers used by native playback.

pub const SND_INIT: u8 = 0;
pub const SND_INTRO: u8 = 1;
pub const SND_TITLE: u8 = 2;
pub const SND_OPS: u8 = 3;
pub const SND_TRAINING: u8 = 4;
pub const SND_MAP: u8 = 5;
pub const SND_CONTINUE: u8 = 6;
pub const SND_BHOLE: u8 = 7;
pub const SND_10: u8 = 8;
pub const SND_11: u8 = 9;
pub const SND_12: u8 = 10;
pub const SND_13: u8 = 11;
pub const SND_13B: u8 = 12;
pub const SND_14: u8 = 13;
pub const SND_15: u8 = 14;
pub const SND_16: u8 = 15;
pub const SND_20: u8 = 16;
pub const SND_21: u8 = 17;
pub const SND_22: u8 = 18;
pub const SND_23: u8 = 19;
pub const SND_24: u8 = 20;
pub const SND_25: u8 = 21;
pub const SND_26: u8 = 22;
pub const SND_30: u8 = 23;
pub const SND_31: u8 = 24;
pub const SND_32: u8 = 25;
pub const SND_33: u8 = 26;
pub const SND_34: u8 = 27;
pub const SND_35: u8 = 28;
pub const SND_36: u8 = 29;
pub const SND_37: u8 = 30;
pub const SND_ENDSEQ: u8 = 31;
pub const SND_STAFF: u8 = 32;
pub const SND_GAMEOVER: u8 = 33;
pub const SND_SPECIAL: u8 = 34;
pub const SND_TRACK_COUNT: u8 = 35;

/// Driver command that stops the current song.
pub const MUSIC_STOP: u8 = 0;
/// Driver command that starts the catalog's level-clear music.
pub const MUSIC_ALL_CLEAR: u8 = 0xF0;
/// Driver command that fades the current song to silence.
pub const MUSIC_FADE_OUT: u8 = 0xF1;

/// Gameplay catalogs whose source programs accept the shared music cues.
pub const GAMEPLAY_TRACKS: std::ops::RangeInclusive<u8> = SND_10..=SND_37;
/// Named cue set used by level and strategy code while a gameplay catalog is loaded.
pub const GAMEPLAY_MUSIC_CUES: [u8; 9] = [2, 4, 5, 6, 7, 8, 14, 17, MUSIC_ALL_CLEAR];

#[rustfmt::skip]
const TRACK_START_CUE: [u8; SND_TRACK_COUNT as usize] = [
    0, 18, 18, 18, 3, 1, 10, 3,
    16, 3, 3, 9, 3, 3, 3, 3,
    16, 3, 3, 3, 3, 3, 3, 16,
    3, 3, 3, 3, 3, 3, 3, 3,
    18, 12, 3,
];

/// Initial music cue for a loaded sound catalog.
pub fn track_start_cue(track: u8) -> u8 {
    TRACK_START_CUE
        .get(track as usize)
        .copied()
        .unwrap_or(track)
}
