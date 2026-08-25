//! Typed flight-stage announcement state shared by the game and renderer.

/// Message selected when a stage announcement begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageBannerKind {
    Stage(u16),
    Training,
}

/// Remaining source gameplay ticks and the message presented during them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageBannerState {
    pub kind: StageBannerKind,
    pub ticks_remaining: u8,
}

impl StageBannerState {
    pub const fn is_visible(self) -> bool {
        self.ticks_remaining & 7 >= 3
    }
}

#[cfg(test)]
mod tests {
    use super::{StageBannerKind, StageBannerState};

    #[test]
    fn source_blink_window_is_five_ticks_on_and_three_ticks_off() {
        for ticks_remaining in 1..=50 {
            let state = StageBannerState {
                kind: StageBannerKind::Training,
                ticks_remaining,
            };
            assert_eq!(state.is_visible(), ticks_remaining & 7 >= 3);
        }
    }
}
