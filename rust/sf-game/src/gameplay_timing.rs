//! Typed gameplay timing extracted from complete retail-machine observations.
//!
//! The source game advances one logical update after a variable number of
//! 60 Hz display refreshes. The native port keeps this as authored scene data:
//! no machine memory, processor state, or original program execution is
//! reachable here.

use sf_map::catalog::map_id;

/// Source-authored baseline: one gameplay update every four 60 Hz refreshes.
pub const BASELINE_GAMEPLAY_REFRESHES: u8 = 4;
const CORNERIA_NEUTRAL_FIRST_MEASURED_FRAME: u16 = 0;
const CORNERIA_NEUTRAL_LAST_MEASURED_FRAME: u16 = 820;
const CORNERIA_NEUTRAL_MEASURED_FRAMES: usize =
    (CORNERIA_NEUTRAL_LAST_MEASURED_FRAME - CORNERIA_NEUTRAL_FIRST_MEASURED_FRAME + 1) as usize;

/// Elapsed display refreshes represented by player X/Y motion on each
/// certified neutral-input Corneria update.
const CORNERIA_NEUTRAL_MOTION_REFRESHES: [u8; CORNERIA_NEUTRAL_MEASURED_FRAMES] = [
    3, 4, 2, 5, 4, 6, 8, 8, 7, 7, 8, 8, 8, 8, 8, 7, 7, 9, 9, 7, 9, 9, 8, 7, 7, 7, 7, 9, 7, 9, 7, 7,
    8, 8, 7, 9, 7, 8, 7, 7, 7, 9, 7, 8, 7, 7, 6, 8, 7, 8, 6, 6, 6, 7, 8, 6, 6, 6, 8, 6, 6, 7, 6, 8,
    6, 6, 9, 6, 8, 7, 7, 7, 7, 9, 8, 7, 6, 7, 9, 9, 7, 9, 9, 8, 9, 8, 8, 8, 7, 8, 8, 8, 9, 9, 8, 8,
    7, 7, 6, 6, 8, 7, 7, 8, 8, 7, 6, 8, 8, 6, 8, 7, 7, 8, 8, 7, 7, 8, 8, 7, 6, 7, 8, 7, 7, 7, 6, 7,
    8, 5, 7, 5, 7, 7, 6, 6, 6, 6, 6, 6, 6, 6, 6, 7, 5, 7, 6, 6, 5, 5, 7, 5, 7, 7, 6, 6, 6, 6, 7, 6,
    6, 5, 5, 5, 6, 5, 6, 5, 7, 5, 5, 6, 6, 6, 6, 5, 5, 5, 4, 6, 4, 6, 5, 5, 4, 4, 4, 11, 5, 4, 5,
    5, 7, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 5, 6, 4, 6, 5, 6, 6, 6, 5, 5, 5, 5, 6, 6, 6, 6, 6,
    5, 6, 6, 4, 6, 7, 5, 7, 5, 5, 5, 5, 6, 6, 6, 6, 6, 5, 6, 6, 5, 6, 6, 7, 5, 6, 7, 7, 5, 6, 5, 6,
    6, 6, 4, 4, 4, 6, 6, 6, 5, 5, 6, 5, 6, 6, 5, 5, 5, 3, 3, 5, 5, 3, 4, 5, 3, 3, 4, 5, 5, 5, 4, 4,
    5, 6, 6, 5, 5, 6, 6, 5, 4, 4, 6, 4, 6, 6, 4, 6, 5, 6, 5, 6, 6, 4, 6, 4, 5, 6, 4, 5, 5, 6, 6, 6,
    4, 5, 4, 4, 6, 4, 4, 4, 6, 6, 6, 6, 6, 5, 6, 6, 5, 6, 6, 6, 5, 4, 4, 4, 4, 6, 6, 6, 6, 5, 5, 4,
    5, 6, 6, 6, 5, 6, 6, 6, 6, 6, 4, 6, 6, 6, 4, 4, 6, 6, 6, 6, 5, 5, 5, 4, 4, 6, 6, 6, 6, 6, 4, 4,
    5, 6, 4, 6, 6, 5, 5, 4, 6, 6, 6, 5, 4, 5, 5, 6, 5, 6, 6, 5, 6, 6, 5, 5, 6, 6, 5, 6, 6, 6, 6, 4,
    6, 7, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 4, 6, 6, 5, 6, 6, 7, 7, 6, 6, 4, 6, 5, 4, 6, 5, 5, 5, 5, 6,
    6, 6, 6, 5, 6, 6, 5, 6, 4, 6, 5, 6, 6, 5, 6, 6, 4, 6, 4, 5, 5, 6, 5, 6, 5, 5, 6, 6, 4, 4, 4, 4,
    6, 6, 4, 4, 4, 4, 6, 5, 6, 6, 6, 6, 6, 7, 7, 6, 6, 4, 6, 5, 5, 6, 6, 6, 6, 6, 6, 4, 4, 6, 5, 4,
    5, 6, 5, 5, 6, 6, 5, 6, 6, 6, 6, 6, 6, 6, 6, 4, 5, 6, 5, 4, 6, 6, 4, 4, 6, 6, 5, 6, 6, 6, 6, 6,
    5, 6, 4, 6, 6, 4, 6, 6, 4, 4, 4, 5, 5, 4, 6, 5, 5, 4, 4, 4, 6, 6, 6, 5, 6, 5, 6, 4, 4, 4, 4, 4,
    4, 6, 6, 6, 5, 7, 6, 7, 6, 6, 6, 5, 5, 7, 7, 7, 6, 7, 6, 6, 6, 7, 7, 5, 7, 6, 7, 5, 6, 4, 6, 5,
    6, 5, 7, 7, 5, 6, 6, 6, 4, 5, 6, 6, 6, 6, 5, 4, 4, 5, 5, 4, 6, 4, 7, 6, 5, 5, 6, 6, 5, 6, 6, 6,
    5, 5, 4, 4, 4, 3, 3, 5, 3, 5, 5, 5, 4, 4, 5, 3, 5, 4, 6, 5, 5, 6, 6, 5, 6, 4, 6, 6, 6, 5, 6, 4,
    6, 6, 5, 6, 4, 6, 4, 5, 5, 6, 4, 6, 5, 7, 5, 7, 5, 5, 7, 7, 6, 5, 6, 6, 6, 6, 6, 5, 6, 4, 6, 6,
    7, 6, 7, 4, 6, 5, 4, 6, 4, 3, 5, 5, 6, 4, 5, 5, 5, 4, 6, 4, 6, 5, 4, 5, 5, 4, 6, 6, 5, 5, 5, 4,
    5, 3, 5, 5, 4, 3, 5, 5, 4, 4, 4, 4, 4, 4, 5, 4, 5, 4, 5, 5, 4, 5, 6, 4, 6, 5, 6, 6, 6, 4, 6, 6,
    5, 6, 6, 6, 4, 7, 7, 7, 4, 7, 7, 5, 4, 5, 6, 6, 5, 6, 7, 7, 7, 7, 7, 6, 8, 5, 7, 6, 7, 7, 7, 7,
    6, 8, 8, 6, 8, 7, 8, 6, 7, 7, 7, 6, 6, 6, 6, 4, 6, 6, 6, 6, 6, 6,
];

/// Visible duration of each certified neutral-input Corneria update. This
/// intentionally differs from motion timing at the source audio-transfer
/// boundary, where the completed picture remains visible without moving the
/// player for the entire wait.
const CORNERIA_NEUTRAL_PRESENTATION_REFRESHES: [u8; CORNERIA_NEUTRAL_MEASURED_FRAMES] = [
    2, 5, 4, 3, 8, 8, 6, 9, 7, 8, 8, 8, 8, 6, 7, 9, 9, 7, 9, 9, 8, 7, 5, 7, 9, 8, 6, 10, 6, 8, 9,
    8, 7, 8, 6, 10, 7, 6, 8, 8, 8, 8, 7, 6, 6, 9, 7, 8, 7, 5, 6, 8, 7, 6, 6, 7, 7, 6, 7, 6, 6, 8,
    5, 7, 8, 6, 9, 7, 5, 7, 8, 9, 9, 7, 5, 7, 9, 9, 7, 9, 9, 8, 9, 8, 7, 8, 7, 8, 10, 7, 9, 9, 9,
    8, 6, 7, 6, 7, 8, 6, 6, 10, 8, 7, 5, 9, 8, 6, 8, 7, 6, 9, 8, 6, 8, 8, 8, 6, 6, 8, 8, 7, 7, 6,
    6, 9, 7, 6, 7, 5, 7, 7, 5, 7, 5, 5, 6, 6, 7, 6, 5, 9, 6, 6, 7, 5, 6, 5, 6, 4, 8, 7, 5, 6, 6, 6,
    8, 6, 6, 4, 6, 5, 7, 4, 7, 5, 6, 4, 5, 8, 6, 6, 6, 4, 4, 5, 5, 7, 4, 6, 4, 4, 5, 6, 3, 6, 88,
    2, 7, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 7, 6, 5, 4, 6, 6, 5, 6, 6, 6, 5, 5, 5, 5, 6, 6,
    6, 6, 6, 5, 6, 4, 6, 6, 4, 8, 4, 5, 5, 5, 8, 6, 6, 6, 6, 6, 5, 6, 6, 5, 6, 6, 4, 8, 6, 7, 4, 8,
    4, 8, 7, 7, 4, 6, 5, 7, 6, 6, 5, 6, 6, 6, 5, 6, 6, 6, 5, 3, 3, 5, 5, 3, 4, 6, 3, 3, 5, 4, 5, 5,
    5, 2, 7, 6, 6, 6, 5, 5, 6, 6, 3, 5, 7, 4, 6, 6, 4, 6, 6, 5, 6, 5, 6, 3, 7, 4, 4, 7, 4, 6, 5, 5,
    6, 6, 4, 6, 3, 4, 7, 4, 4, 4, 8, 6, 6, 6, 6, 6, 5, 6, 6, 5, 6, 6, 6, 3, 5, 5, 6, 6, 6, 6, 6, 6,
    5, 4, 4, 8, 6, 6, 4, 8, 6, 6, 6, 6, 4, 8, 6, 6, 4, 5, 6, 6, 6, 6, 6, 4, 6, 3, 5, 7, 6, 6, 6, 6,
    4, 4, 6, 5, 3, 7, 6, 6, 5, 3, 6, 6, 6, 6, 3, 6, 5, 5, 6, 5, 6, 6, 5, 6, 6, 5, 5, 6, 6, 5, 6, 6,
    6, 3, 7, 6, 4, 5, 6, 5, 5, 4, 5, 5, 5, 5, 5, 7, 6, 6, 5, 6, 6, 7, 7, 6, 3, 7, 4, 4, 7, 3, 5, 5,
    5, 8, 6, 6, 6, 6, 5, 6, 6, 5, 3, 7, 6, 5, 6, 4, 7, 6, 4, 5, 6, 5, 5, 7, 4, 8, 4, 6, 7, 6, 4, 5,
    5, 5, 7, 6, 4, 5, 5, 5, 8, 5, 6, 4, 8, 6, 6, 6, 7, 5, 8, 4, 7, 7, 5, 5, 6, 6, 6, 6, 6, 4, 4, 8,
    6, 3, 6, 6, 4, 7, 5, 6, 6, 5, 6, 6, 6, 6, 6, 6, 6, 4, 6, 5, 6, 3, 6, 6, 4, 4, 6, 6, 4, 7, 6, 6,
    6, 6, 6, 5, 3, 7, 6, 4, 6, 6, 4, 4, 6, 6, 5, 3, 6, 6, 3, 6, 3, 5, 6, 6, 6, 4, 7, 6, 5, 4, 6, 4,
    5, 5, 5, 8, 6, 6, 4, 8, 7, 7, 7, 6, 6, 6, 4, 7, 7, 7, 7, 6, 7, 6, 5, 7, 7, 5, 7, 7, 6, 4, 8, 3,
    7, 6, 5, 3, 8, 7, 5, 7, 6, 6, 4, 6, 5, 6, 6, 6, 6, 3, 4, 6, 5, 3, 6, 4, 6, 7, 6, 5, 5, 6, 6, 5,
    6, 6, 6, 5, 3, 3, 4, 4, 4, 6, 3, 5, 5, 5, 5, 4, 4, 3, 5, 5, 4, 6, 5, 5, 6, 6, 5, 4, 6, 6, 6, 6,
    5, 4, 6, 6, 6, 5, 4, 6, 3, 7, 5, 5, 3, 7, 3, 8, 5, 7, 4, 6, 7, 7, 7, 4, 7, 6, 4, 8, 6, 5, 6, 3,
    7, 6, 6, 5, 8, 4, 7, 6, 3, 6, 3, 4, 6, 5, 5, 3, 7, 5, 5, 2, 7, 4, 6, 4, 5, 6, 5, 4, 5, 6, 6, 5,
    5, 2, 7, 3, 5, 5, 3, 3, 6, 5, 5, 3, 3, 3, 4, 4, 5, 5, 6, 2, 7, 5, 3, 6, 5, 4, 6, 6, 5, 6, 6, 4,
    6, 6, 6, 5, 6, 6, 3, 7, 7, 7, 5, 6, 7, 4, 5, 5, 7, 6, 6, 5, 6, 7, 7, 7, 7, 7, 6, 5, 8, 7, 6, 7,
    7, 7, 5, 8, 8, 6, 8, 6, 9, 8, 6, 7, 4, 7, 9, 6, 6, 4, 6, 6, 6, 6, 6, 6, 6, 5,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GameplayTickTiming {
    pub motion_refreshes: u8,
    pub presentation_refreshes: u8,
}

impl GameplayTickTiming {
    const fn baseline() -> Self {
        Self {
            motion_refreshes: BASELINE_GAMEPLAY_REFRESHES,
            presentation_refreshes: BASELINE_GAMEPLAY_REFRESHES,
        }
    }
}

pub fn timing_for_update(map: u32, game_frame: u16) -> GameplayTickTiming {
    if map != map_id::M1_1 {
        return GameplayTickTiming::baseline();
    }
    let Some(index) = game_frame
        .checked_sub(CORNERIA_NEUTRAL_FIRST_MEASURED_FRAME)
        .map(usize::from)
        .filter(|index| *index < CORNERIA_NEUTRAL_MEASURED_FRAMES)
    else {
        return GameplayTickTiming::baseline();
    };
    GameplayTickTiming {
        motion_refreshes: CORNERIA_NEUTRAL_MOTION_REFRESHES[index],
        presentation_refreshes: CORNERIA_NEUTRAL_PRESENTATION_REFRESHES[index],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corneria_audio_transfer_keeps_motion_and_visible_waits_independent() {
        const AUDIO_TRANSFER_FRAME: u16 = 186;
        const AUDIO_TRANSFER_MOTION_REFRESHES: u8 = 4;
        const AUDIO_TRANSFER_PRESENTATION_REFRESHES: u8 = 88;

        assert_eq!(
            timing_for_update(map_id::M1_1, AUDIO_TRANSFER_FRAME),
            GameplayTickTiming {
                motion_refreshes: AUDIO_TRANSFER_MOTION_REFRESHES,
                presentation_refreshes: AUDIO_TRANSFER_PRESENTATION_REFRESHES,
            }
        );
    }

    #[test]
    fn unmeasured_frames_and_other_maps_use_the_authored_baseline() {
        const FIRST_UNMEASURED_FRAME: u16 = CORNERIA_NEUTRAL_LAST_MEASURED_FRAME + 1;

        assert_eq!(
            timing_for_update(map_id::M1_1, FIRST_UNMEASURED_FRAME),
            GameplayTickTiming::baseline()
        );
        assert_eq!(
            timing_for_update(map_id::TRAINING, CORNERIA_NEUTRAL_FIRST_MEASURED_FRAME),
            GameplayTickTiming::baseline()
        );
    }
}
