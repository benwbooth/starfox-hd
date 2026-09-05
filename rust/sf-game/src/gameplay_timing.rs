//! Typed gameplay timing extracted from complete retail-machine observations.
//!
//! The source game advances one logical update after a variable number of
//! 60 Hz display refreshes. The native port keeps this as authored scene data:
//! no machine memory, processor state, or original program execution is
//! reachable here.

use sf_map::catalog::map_id;

/// Source-authored baseline: one gameplay update every four 60 Hz refreshes.
pub const BASELINE_GAMEPLAY_REFRESHES: u8 = 4;
/// The recorded 103-refresh interval is the checkpoint-restart boundary.
/// It is valid only when the live game is actually in its death/restart path.
pub const CORNERIA_CHECKPOINT_RESTART_FRAME: u16 = 943;
const CORNERIA_NEUTRAL_FIRST_MEASURED_FRAME: u16 = 0;
const CORNERIA_NEUTRAL_LAST_MEASURED_FRAME: u16 = 982;
const CORNERIA_NEUTRAL_MEASURED_FRAMES: usize =
    (CORNERIA_NEUTRAL_LAST_MEASURED_FRAME - CORNERIA_NEUTRAL_FIRST_MEASURED_FRAME + 1) as usize;

/// Elapsed display refreshes represented by player X/Y motion on each
/// certified neutral-input Corneria update.
const CORNERIA_NEUTRAL_MOTION_REFRESHES: [u8; CORNERIA_NEUTRAL_MEASURED_FRAMES] = [
    2, 3, 3, 3, 4, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 6, 7, 7, 8, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 8, 7, 7, 7, 6, 7, 7, 7, 7, 6, 6, 6, 7, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 8, 6, 7, 7, 8, 7, 7, 8, 8, 6, 7, 8, 8, 7, 7, 7, 7, 8, 7, 8, 8, 8, 7, 8, 7, 9, 7, 8, 7, 7,
    7, 7, 6, 7, 8, 7, 7, 7, 6, 7, 6, 7, 6, 6, 7, 7, 7, 7, 7, 8, 7, 7, 7, 7, 6, 7, 7, 7, 7, 7, 6, 7,
    7, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 5, 7, 5, 5, 5, 5, 6, 5, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 5, 5, 5, 5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 5, 5, 5, 4, 5, 4, 4, 5, 5, 4, 4, 4, 10, 4, 4, 4,
    4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 4, 4, 4, 5, 4, 5, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    4, 5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5,
    5, 5, 4, 4, 4, 5, 5, 6, 5, 5, 4, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 4, 4, 3, 4, 3, 3, 4, 4, 4, 3, 4,
    4, 5, 4, 6, 4, 5, 4, 4, 4, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 5, 5, 5, 4, 5, 5, 5, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 4, 4, 5, 4,
    5, 5, 6, 5, 5, 5, 5, 5, 4, 4, 6, 4, 4, 5, 4, 4, 5, 4, 5, 5, 5, 5, 5, 5, 5, 5, 4, 5, 5, 4, 4, 4,
    4, 5, 4, 5, 5, 4, 5, 4, 4, 4, 4, 4, 6, 4, 5, 4, 5, 5, 4, 5, 4, 5, 4, 5, 4, 4, 5, 4, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 4, 5, 4, 5, 5, 5, 6, 6, 5, 4, 6, 4, 4, 5, 4, 5, 5, 5, 5, 5,
    6, 5, 5, 5, 5, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 4, 5, 5, 4, 4, 6, 6, 5, 6, 5, 6, 4, 5, 4, 4, 4, 4,
    5, 5, 5, 4, 4, 4, 4, 5, 6, 5, 6, 5, 5, 6, 6, 6, 6, 4, 5, 5, 4, 5, 4, 4, 5, 4, 5, 4, 4, 4, 4, 4,
    4, 4, 4, 5, 5, 4, 4, 5, 4, 5, 4, 5, 4, 4, 4, 5, 4, 5, 5, 6, 4, 5, 4, 5, 4, 5, 6, 6, 5, 4, 5, 4,
    4, 4, 4, 4, 5, 4, 5, 5, 5, 5, 5, 5, 5, 4, 5, 4, 5, 4, 5, 4, 5, 5, 5, 5, 5, 5, 5, 4, 6, 4, 4, 4,
    4, 4, 4, 5, 5, 6, 5, 5, 5, 4, 4, 4, 4, 5, 5, 6, 6, 6, 6, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 5,
    5, 5, 6, 5, 5, 6, 4, 5, 4, 4, 5, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4, 5, 4, 5, 4, 4, 4, 4, 4, 5, 5,
    4, 4, 4, 4, 5, 4, 3, 4, 3, 3, 3, 3, 3, 3, 3, 3, 4, 3, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 4, 5, 5, 6, 5, 5, 5, 6, 5, 5, 5, 5, 4, 5, 5, 5, 4, 5, 5, 5,
    5, 5, 4, 5, 5, 4, 4, 4, 4, 4, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 3, 4, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 5, 4, 5, 4, 5, 5, 5, 4, 5, 6, 4, 4, 5, 4, 4, 4, 5, 6, 5, 6, 6, 6, 5, 5, 5, 6, 5, 6, 5, 7, 5,
    6, 6, 6, 7, 6, 6, 6, 6, 5, 5, 5, 5, 6, 5, 5, 4, 4, 4, 4, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 5, 4, 5, 4, 4, 4, 4, 5, 5, 5, 5, 6, 4, 5, 5, 6, 4, 4, 6, 5, 5, 4, 5, 5, 4, 5, 5, 6, 4, 4,
    4, 5, 5, 5, 5, 4, 5, 4, 5, 4, 4, 4, 5, 5, 4, 5, 5, 5, 4, 6, 6, 5, 4, 6, 5, 5, 6, 6, 5, 6, 6, 7,
    6, 6, 5, 5, 4, 5, 4, 4, 5, 6, 5, 4, 4, 6, 5, 6, 5, 6, 6, 6, 6, 5, 6, 6, 5, 5, 5, 5, 4, 6, 5, 4,
    5, 5, 5, 5, 4, 5, 4, 5, 5, 5, 5, 6, 4, 4, 4, 5, 4, 8, 3, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 3, 5, 3, 4, 3,
];

/// Visible duration of each certified neutral-input Corneria update. This
/// intentionally differs from motion timing at the source audio-transfer
/// boundary, where the completed picture remains visible without moving the
/// player for the entire wait.
const CORNERIA_NEUTRAL_PRESENTATION_REFRESHES: [u8; CORNERIA_NEUTRAL_MEASURED_FRAMES] = [
    3, 3, 3, 3, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 6, 7, 6, 8, 7, 8, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 8, 7, 7, 7, 6, 7, 7, 7, 7, 6, 6, 6, 7, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 8, 6, 7, 7, 8, 7, 7, 8, 8, 6, 7, 8, 8, 7, 7, 7, 7, 8, 8, 9, 8, 8, 7, 8, 7, 9, 7, 8, 8, 6, 7,
    7, 6, 8, 7, 7, 7, 7, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 8, 7, 7, 7, 7, 6, 7, 7, 7, 7, 7, 7, 6, 7,
    5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 5, 5, 5, 5, 6, 5, 7, 6, 6, 6, 6, 6, 6, 6, 6,
    5, 5, 5, 5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 5, 5, 5, 4, 6, 4, 4, 5, 5, 4, 4, 4, 87, 4, 4, 4, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 4, 4, 4, 5, 4, 5, 4, 5, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    4, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 4, 4, 4, 4, 3, 3, 3, 3, 3, 4, 5, 3, 4, 3, 4, 4, 4, 4, 4, 4, 4,
    5, 4, 6, 5, 4, 5, 5, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 5, 5, 5, 4, 5, 6, 4, 5, 5, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 5, 4, 5, 5, 5,
    5, 6, 6, 5, 5, 5, 5, 5, 5, 6, 5, 5, 4, 5, 4, 5, 4, 5, 5, 5, 5, 5, 5, 5, 5, 4, 5, 5, 4, 4, 4, 4,
    5, 4, 5, 5, 4, 5, 4, 4, 4, 4, 4, 6, 4, 5, 5, 4, 5, 5, 5, 4, 5, 4, 5, 4, 4, 5, 4, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 6, 5, 5, 5, 5, 5, 5, 4, 4, 5, 5, 5, 6, 6, 5, 5, 5, 4, 6, 4, 5, 4, 5, 5, 5, 5, 6,
    5, 5, 5, 5, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 4, 5, 5, 5, 6, 6, 5, 6, 5, 6, 5, 5, 5, 5, 5, 4, 5,
    5, 5, 5, 5, 5, 5, 5, 6, 5, 6, 6, 5, 6, 6, 6, 6, 5, 4, 5, 4, 5, 4, 5, 4, 5, 5, 4, 6, 5, 5, 5, 5,
    5, 5, 4, 5, 5, 4, 5, 4, 5, 4, 5, 4, 4, 4, 5, 4, 5, 5, 6, 4, 5, 4, 5, 4, 5, 6, 6, 5, 4, 5, 4, 4,
    4, 5, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 4, 6, 3, 5, 4, 5, 4, 5, 5, 5, 5, 5, 5, 5, 6, 6, 5, 5, 5, 5,
    5, 5, 5, 5, 6, 5, 6, 5, 5, 5, 5, 5, 5, 5, 6, 6, 6, 6, 5, 6, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 4, 5,
    5, 6, 5, 5, 6, 4, 5, 4, 5, 5, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4, 5, 5, 4, 4, 4, 4, 4, 4, 5, 5, 4,
    4, 5, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 3, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 4, 5, 5, 6, 5, 5, 5, 6, 5, 5, 5, 5, 5, 4, 5, 5, 5, 4, 6, 5, 5,
    5, 4, 5, 5, 4, 4, 4, 4, 4, 4, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 3, 4, 3, 4, 4, 4, 4, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    5, 4, 5, 4, 5, 5, 5, 4, 5, 6, 4, 4, 5, 4, 4, 4, 5, 6, 5, 6, 6, 6, 5, 5, 5, 6, 5, 6, 5, 7, 5, 6,
    7, 5, 7, 6, 6, 6, 6, 5, 5, 5, 5, 6, 5, 5, 4, 4, 4, 4, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 5, 4, 5, 4, 4, 4, 4, 5, 5, 5, 5, 6, 5, 4, 5, 6, 4, 4, 6, 5, 5, 4, 6, 4, 5, 4, 5, 6, 4, 4, 4,
    5, 5, 5, 5, 4, 5, 4, 5, 4, 5, 4, 5, 5, 4, 5, 5, 5, 4, 6, 6, 5, 4, 6, 6, 4, 6, 6, 5, 6, 6, 7, 6,
    6, 5, 5, 4, 5, 4, 5, 4, 6, 5, 4, 5, 5, 5, 7, 5, 5, 6, 6, 6, 5, 6, 7, 5, 5, 4, 5, 7, 4, 4, 5, 4,
    6, 4, 5, 5, 4, 5, 4, 5, 5, 8, 3, 5, 4, 5, 5, 4, 103, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 3, 5, 3, 4, 3, 3,
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

/// Apply the recorded restart-only presentation interval only when the live
/// state is entering a checkpoint restart. The oracle arrays remain unchanged;
/// this separates their neutral recording from other input paths that are
/// alive at the same game-frame number.
pub fn timing_for_update_with_restart_context(
    map: u32,
    game_frame: u16,
    restart_pending: bool,
) -> GameplayTickTiming {
    let timing = timing_for_update(map, game_frame);
    if map == map_id::M1_1 && game_frame == CORNERIA_CHECKPOINT_RESTART_FRAME && !restart_pending {
        GameplayTickTiming {
            presentation_refreshes: timing.motion_refreshes,
            ..timing
        }
    } else {
        timing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corneria_audio_transfer_keeps_motion_and_visible_waits_independent() {
        const AUDIO_TRANSFER_FRAME: u16 = 186;
        const AUDIO_TRANSFER_MOTION_REFRESHES: u8 = 4;
        const AUDIO_TRANSFER_PRESENTATION_REFRESHES: u8 = 87;

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
