//! Typed flight-stage announcement state shared by the game and renderer.

/// Source presentation refreshes spanned by one fixed 20 Hz game update.
pub const PRESENTATION_FRAMES_PER_GAMEPLAY_TICK: u8 = 3;
/// Counter loaded by the source `setstage` and player-launch strategies.
pub const STAGE_BANNER_INITIAL_TICKS: u8 = 50;
/// `do_stage` state visible in the completed source sprite lane at the game
/// boundary sampled by the port. OAM completion is the middle refresh of the
/// three-refresh gameplay interval.
pub const STAGE_BANNER_COMPLETION_PHASE: u8 = 1;

/// Message selected when a stage announcement begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageBannerKind {
    Stage(u16),
    Training,
}

/// Source presentation frames remaining at the start of a 20 Hz game interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageBannerState {
    pub kind: StageBannerKind,
    pub ticks_remaining: u8,
}

impl StageBannerState {
    pub const fn is_visible(self) -> bool {
        self.is_visible_at_phase(STAGE_BANNER_COMPLETION_PHASE)
    }

    pub const fn is_visible_at_phase(self, presentation_phase: u8) -> bool {
        let remaining = self
            .ticks_remaining
            .saturating_sub(presentation_phase.saturating_add(1));
        remaining != 0 && remaining & 7 >= 3
    }
}

/// Source presentation frames remaining and gameplay blink phase for the
/// launch `SCRAMBLE` warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrambleBannerState {
    pub ticks_remaining: u8,
    pub game_frame: u16,
}

impl ScrambleBannerState {
    pub const fn is_visible(self) -> bool {
        self.is_visible_at_phase(0)
    }

    pub const fn is_visible_at_phase(self, presentation_phase: u8) -> bool {
        self.ticks_remaining > presentation_phase && self.game_frame & 7 < 3
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ScrambleBannerState, StageBannerKind, StageBannerState, STAGE_BANNER_COMPLETION_PHASE,
        STAGE_BANNER_INITIAL_TICKS,
    };

    #[test]
    fn source_blink_window_is_five_ticks_on_and_three_ticks_off() {
        for ticks_remaining in 1..=STAGE_BANNER_INITIAL_TICKS {
            let state = StageBannerState {
                kind: StageBannerKind::Training,
                ticks_remaining,
            };
            let remaining_after_source_decrement =
                ticks_remaining.saturating_sub(STAGE_BANNER_COMPLETION_PHASE.saturating_add(1));
            assert_eq!(
                state.is_visible(),
                remaining_after_source_decrement != 0 && remaining_after_source_decrement & 7 >= 3
            );
        }
    }

    #[test]
    fn completed_sprite_lane_matches_the_source_first_blink_cycle() {
        let expected = [
            false, true, true, true, true, true, false, false, false, true, true,
        ];
        for (elapsed, expected_visible) in expected.into_iter().enumerate() {
            let state = StageBannerState {
                kind: StageBannerKind::Stage(0),
                ticks_remaining: STAGE_BANNER_INITIAL_TICKS - elapsed as u8,
            };
            assert_eq!(
                state.is_visible_at_phase(STAGE_BANNER_COMPLETION_PHASE),
                expected_visible,
                "elapsed sprite lanes: {elapsed}"
            );
        }
    }

    #[test]
    fn presentation_phase_consumes_one_source_frame_per_refresh() {
        let state = StageBannerState {
            kind: StageBannerKind::Training,
            ticks_remaining: 6,
        };
        assert!(state.is_visible_at_phase(0));
        assert!(state.is_visible_at_phase(1));
        assert!(state.is_visible_at_phase(2));

        let ending = StageBannerState {
            kind: StageBannerKind::Training,
            ticks_remaining: 2,
        };
        assert!(!ending.is_visible_at_phase(0));
        assert!(!ending.is_visible_at_phase(1));
        assert!(!ending.is_visible_at_phase(2));
    }

    #[test]
    fn scramble_warning_blinks_three_ticks_on_and_five_ticks_off() {
        for game_frame in 0..16 {
            let state = ScrambleBannerState {
                ticks_remaining: 3,
                game_frame,
            };
            assert_eq!(state.is_visible(), game_frame & 7 < 3);
        }
        assert!(!ScrambleBannerState {
            ticks_remaining: 0,
            game_frame: 0,
        }
        .is_visible());

        let ending = ScrambleBannerState {
            ticks_remaining: 2,
            game_frame: 0,
        };
        assert!(ending.is_visible_at_phase(0));
        assert!(ending.is_visible_at_phase(1));
        assert!(!ending.is_visible_at_phase(2));
    }
}
