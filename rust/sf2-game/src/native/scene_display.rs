//! Source-owned scene fades and their separately scheduled blank hold.
//!
//! The scene-band and full-screen update paths intentionally differ. The
//! frame/display owner chooses which service to visit; neither advances an
//! invented display clock or consumes the other service's publication.

const FULL_INTENSITY: u8 = 15;
const BLANK_HOLD_FINISHED: u8 = u8::MAX;
const INITIAL_FADE_HOLD: u8 = 2;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Intensity(u8);

impl Intensity {
    pub const DARK: Self = Self(0);
    pub const FULL: Self = Self(FULL_INTENSITY);

    pub const fn new(value: u8) -> Self {
        assert!(value <= FULL_INTENSITY);
        Self(value)
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

/// Forced blanking is independent of the retained four-bit intensity. In
/// particular, blank-at-full and blank-at-zero do not satisfy the same map wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayBand {
    pub blanked: bool,
    pub intensity: Intensity,
}

impl DisplayBand {
    pub const BLANK_DARK: Self = Self {
        blanked: true,
        intensity: Intensity::DARK,
    };
    pub const BLANK_FULL: Self = Self {
        blanked: true,
        intensity: Intensity::FULL,
    };
}

/// Requests installed by the authored map/scene services. The paced and
/// alternating modes have special meanings in scene-band updates; the
/// full-screen service interprets those requests as signed three-level steps.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FadeRequest {
    #[default]
    Idle,
    In,
    InFast,
    InPaced,
    Out,
    OutFast,
    OutAlternating,
}

impl FadeRequest {
    fn full_screen_step(self) -> i16 {
        match self {
            Self::Idle => 0,
            Self::In => 1,
            Self::InFast => 2,
            Self::InPaced => 3,
            Self::Out => -1,
            Self::OutFast => -2,
            Self::OutAlternating => -3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneDisplay {
    pub request: FadeRequest,
    pub progress: Intensity,
    /// The three source scanline bands keep independent published values.
    pub bands: [DisplayBand; 3],
    /// Scene-frame countdown; 255 means inactive, while zero is the last hold.
    /// This is not consumed by either fade service.
    pub blank_hold: u8,
    pub interval_remaining: u8,
    pub interval_reload: u8,
}

impl SceneDisplay {
    /// Map wait ($03:99BE): both publications are required. Finishing the
    /// scene-band fade-out alone publishes blank-at-full, not this state.
    pub fn ready_for_scene_load(&self) -> bool {
        self.progress == Intensity::DARK && self.bands[0] == DisplayBand::BLANK_DARK
    }

    /// Scene-frame wrapper ($03:DD81), independently of display interrupts.
    pub fn advance_blank_hold(&mut self) {
        if self.blank_hold != BLANK_HOLD_FINISHED {
            self.blank_hold = self.blank_hold.wrapping_sub(1);
        }
    }

    /// Common entry helper ($03:DD6F) does not publish new band intensities.
    pub fn begin_entry_fade(&mut self) {
        self.request = FadeRequest::InFast;
        self.progress = Intensity::DARK;
        self.blank_hold = INITIAL_FADE_HOLD;
    }

    /// Explicit all-band publication ($03:8784). Request, progress and hold
    /// remain the responsibility of its caller.
    pub fn publish_all_bands(&mut self, band: DisplayBand) {
        self.bands.fill(band);
    }

    /// Banded scene service ($7F:0E79). The alternating fade uses the strategy
    /// clock's low bit, not a newly generated presentation-frame parity.
    pub fn visit_scene_fade(&mut self, odd_strategy_clock: bool) {
        if self.blank_hold != BLANK_HOLD_FINISHED {
            self.bands[..2].fill(DisplayBand::BLANK_FULL);
            return;
        }
        let step = match self.request {
            FadeRequest::Idle => return,
            FadeRequest::OutAlternating if odd_strategy_clock => return,
            FadeRequest::Out | FadeRequest::OutAlternating => -1,
            FadeRequest::OutFast => -2,
            FadeRequest::In => 1,
            FadeRequest::InFast => 2,
            FadeRequest::InPaced => {
                self.interval_remaining = self.interval_remaining.wrapping_sub(1);
                if self.interval_remaining != 0 {
                    return;
                }
                self.interval_remaining = self.interval_reload;
                1
            }
        };
        let old = i16::from(self.progress.value());
        if step < 0 && old + step <= 0 {
            self.request = FadeRequest::Idle;
            self.progress = Intensity::DARK;
            self.publish_all_bands(DisplayBand::BLANK_FULL);
            return;
        }
        // Fade-in clears the request only on a visit beginning at full,
        // not on the visit which first reaches full intensity.
        if old == i16::from(FULL_INTENSITY) && step > 0 {
            self.request = FadeRequest::Idle;
        }
        self.progress = Intensity::new((old + step).clamp(0, i16::from(FULL_INTENSITY)) as u8);
        self.publish_all_bands(DisplayBand {
            blanked: false,
            intensity: self.progress,
        });
    }

    /// Full-screen service ($7F:0FFF). It neither changes the band records
    /// nor clears the request at either endpoint. The returned publication is
    /// the whole-screen output, to be consumed by the display owner.
    pub fn visit_full_screen_fade(&mut self) -> DisplayBand {
        let next = i16::from(self.progress.value()) + self.request.full_screen_step();
        self.progress = Intensity::new(next.clamp(0, i16::from(FULL_INTENSITY)) as u8);
        DisplayBand {
            blanked: false,
            intensity: self.progress,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display() -> SceneDisplay {
        SceneDisplay {
            request: FadeRequest::Idle,
            progress: Intensity::DARK,
            bands: [DisplayBand::BLANK_DARK; 3],
            blank_hold: BLANK_HOLD_FINISHED,
            interval_remaining: 0,
            interval_reload: 0,
        }
    }

    #[test]
    fn blank_hold_is_frame_owned_and_leaves_the_third_band_and_fade_untouched() {
        for hold in 0..=u8::MAX {
            let mut value = display();
            value.request = FadeRequest::InFast;
            value.blank_hold = hold;
            value.visit_scene_fade(false);
            assert_eq!(value.blank_hold, hold);
            if hold != BLANK_HOLD_FINISHED {
                assert_eq!(
                    value.bands,
                    [
                        DisplayBand::BLANK_FULL,
                        DisplayBand::BLANK_FULL,
                        DisplayBand::BLANK_DARK
                    ]
                );
                assert_eq!(value.progress, Intensity::DARK);
            }
            value.advance_blank_hold();
            assert_eq!(
                value.blank_hold,
                if hold == BLANK_HOLD_FINISHED {
                    hold
                } else {
                    hold.wrapping_sub(1)
                }
            );
        }
    }

    #[test]
    fn fade_in_keeps_request_for_one_full_intensity_visit() {
        let mut value = display();
        value.request = FadeRequest::InFast;
        value.progress = Intensity::new(14);
        value.visit_scene_fade(false);
        assert_eq!(value.progress, Intensity::FULL);
        assert_eq!(value.request, FadeRequest::InFast);
        value.visit_scene_fade(false);
        assert_eq!(value.request, FadeRequest::Idle);
    }

    #[test]
    fn completing_fade_out_does_not_manufacture_map_readiness() {
        let mut value = display();
        value.request = FadeRequest::OutFast;
        value.progress = Intensity::new(2);
        value.visit_scene_fade(false);
        assert_eq!(value.request, FadeRequest::Idle);
        assert_eq!(value.progress, Intensity::DARK);
        assert_eq!(value.bands, [DisplayBand::BLANK_FULL; 3]);
        assert!(!value.ready_for_scene_load());
        value.publish_all_bands(DisplayBand::BLANK_DARK);
        assert!(value.ready_for_scene_load());
    }

    #[test]
    fn paced_zero_counter_wraps_and_reloads_only_at_its_boundary() {
        let mut value = display();
        value.request = FadeRequest::InPaced;
        value.interval_reload = 3;
        for _ in 0..255 {
            value.visit_scene_fade(false);
        }
        assert_eq!(value.progress, Intensity::DARK);
        value.visit_scene_fade(false);
        assert_eq!(value.progress.value(), 1);
        assert_eq!(value.interval_remaining, 3);
        for _ in 0..3 {
            value.visit_scene_fade(false);
        }
        assert_eq!(value.progress.value(), 2);
    }

    #[test]
    fn alternating_fade_uses_strategy_parity_without_advancing_it() {
        let mut value = display();
        value.request = FadeRequest::OutAlternating;
        value.progress = Intensity::FULL;
        let previous = value;
        for _ in 0..10 {
            value.visit_scene_fade(true);
        }
        assert_eq!(value, previous);
        value.visit_scene_fade(false);
        assert_eq!(value.progress.value(), 14);
    }

    #[test]
    fn full_screen_update_keeps_request_bands_hold_and_interval() {
        let mut value = display();
        value.request = FadeRequest::InPaced;
        value.progress = Intensity::new(14);
        value.blank_hold = 10;
        value.interval_remaining = 27;
        let previous = value;
        assert_eq!(
            value.visit_full_screen_fade(),
            DisplayBand {
                blanked: false,
                intensity: Intensity::FULL
            }
        );
        assert_eq!(
            value,
            SceneDisplay {
                progress: Intensity::FULL,
                ..previous
            }
        );
    }

    #[test]
    fn entry_fade_resets_only_its_three_owned_fields() {
        let mut value = display();
        value.progress = Intensity::FULL;
        value.interval_reload = 7;
        let previous = value;
        value.begin_entry_fade();
        assert_eq!(
            value,
            SceneDisplay {
                request: FadeRequest::InFast,
                progress: Intensity::DARK,
                blank_hold: 2,
                ..previous
            }
        );
    }
}
