//! Cinematic outer-loop exit and the shared scene-exit fade.
//!
//! `$03:C1DC` runs after the ordinary frame, separately from actor updates
//! and display services. A button edge only requests an exit; the following
//! visit starts it. `$03:E0FC` requests a fade but never advances intensity.

use super::path_control::PlayerTarget;
use super::path_sound::AuthoredCue;
use super::scene_display::{FadeRequest, Intensity, SceneDisplay};
use super::{AudioState, Button, Buttons, SoundEvent};

const SKIP_CUE: u8 = 14;
const READY_START_CUE: u8 = 66;
const OPENING_EXIT_INTENSITY: u8 = 8;
/// `$03:BDDD` calls the authored UI entry `$0B:8C33`, whose second
/// inline parameter is the ten-visit input hold returned to its caller.
pub const OPENING_INPUT_HOLD: u16 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CinematicSkipPolicy {
    Disabled,
    StartOrB,
    StartWhenReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CinematicExitPolicy {
    pub initial_intensity: Intensity,
    pub skip: CinematicSkipPolicy,
}

impl CinematicExitPolicy {
    pub const OPENING: Self = Self {
        initial_intensity: Intensity::new(OPENING_EXIT_INTENSITY),
        skip: CinematicSkipPolicy::StartOrB,
    };
}

/// The two cinematic signals used here are shared with the scene controller.
/// Completion also tells the outer owner to clear its remaining scene signals.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CinematicSignals {
    pub exit_requested: bool,
    pub skip_ready: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExitFadeVisit {
    /// Request the source's scene-exit audio control and silence its two
    /// continuous channels. This is an output request, not an acknowledgement
    /// or a claim that native PCM playback has performed the transition.
    pub request_audio_exit: bool,
    pub completed: bool,
}

/// Shared transition state (`$03:E0FC`), independent of any particular scene.
/// The audio latch belongs to the outer transition, not the display service.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SceneExitFade {
    pub active: bool,
    pub audio_requested: bool,
    pub delay: u16,
}

impl SceneExitFade {
    pub fn visit(&mut self, display: &mut SceneDisplay) -> ExitFadeVisit {
        let mut event = ExitFadeVisit::default();
        if !self.active {
            return event;
        }
        if !self.audio_requested {
            self.audio_requested = true;
            event.request_audio_exit = true;
        }
        // The source decrements at word width, then clamps a negative result.
        // In particular 32768 becomes 32767; this is not saturating_sub.
        self.delay = self.delay.wrapping_sub(1);
        if (self.delay as i16) < 0 {
            self.delay = 0;
        }
        if self.delay != 0 {
            return event;
        }
        if display.progress != Intensity::DARK {
            display.request = FadeRequest::OutFast;
            return event;
        }
        self.active = false;
        self.audio_requested = false;
        event.completed = true;
        event
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CinematicExitVisit {
    pub request_audio_exit: bool,
    /// Sampled once when the fade begins. The outer scene router owns the
    /// alternate destination, not this cinematic or its display service.
    pub select_alternate_destination: bool,
    /// Clear all outer scene signals and leave its active frame loop.
    pub completed: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CinematicExit {
    pub input_hold: u16,
    pub fade: SceneExitFade,
    completed: bool,
}

impl CinematicExit {
    pub const fn new(input_hold: u16) -> Self {
        Self {
            input_hold,
            fade: SceneExitFade {
                active: false,
                audio_requested: false,
                delay: 0,
            },
            completed: false,
        }
    }

    pub const fn completed(&self) -> bool {
        self.completed
    }

    pub fn visit(
        &mut self,
        policy: CinematicExitPolicy,
        signals: &mut CinematicSignals,
        pressed: Buttons,
        alternate_destination: bool,
        display: &mut SceneDisplay,
        audio: &mut AudioState,
    ) -> CinematicExitVisit {
        let mut event = CinematicExitVisit::default();
        // The outer source loop no longer dispatches this scene after exit.
        if self.completed {
            return event;
        }
        if !self.fade.active {
            if self.input_hold != 0 {
                self.input_hold -= 1;
                return event;
            }
            if !signals.exit_requested {
                let cue = match policy.skip {
                    CinematicSkipPolicy::Disabled => None,
                    CinematicSkipPolicy::StartOrB
                        if pressed.contains(Button::Start) || pressed.contains(Button::B) =>
                    {
                        Some(SKIP_CUE)
                    }
                    CinematicSkipPolicy::StartWhenReady
                        if signals.skip_ready && pressed.contains(Button::Start) =>
                    {
                        Some(READY_START_CUE)
                    }
                    _ => None,
                };
                if let Some(cue) = cue {
                    signals.exit_requested = true;
                    audio.queue(SoundEvent::Authored(AuthoredCue::new(
                        cue,
                        0,
                        PlayerTarget::Primary,
                    )));
                }
                return event;
            }
            self.fade.active = true;
            display.progress = policy.initial_intensity;
            event.select_alternate_destination = alternate_destination;
        }
        let fade = self.fade.visit(display);
        event.request_audio_exit = fade.request_audio_exit;
        event.completed = fade.completed;
        if fade.completed {
            *signals = CinematicSignals::default();
            self.completed = true;
        }
        event
    }
}

#[cfg(test)]
mod tests {
    use super::super::scene_display::DisplayBand;
    use super::*;

    fn display() -> SceneDisplay {
        SceneDisplay {
            request: FadeRequest::Idle,
            progress: Intensity::FULL,
            bands: [DisplayBand::BLANK_FULL; 3],
            blank_hold: 255,
            interval_remaining: 7,
            interval_reload: 9,
        }
    }

    #[test]
    fn button_request_fade_and_completion_are_separate_visits() {
        let mut exit = CinematicExit::new(1);
        let mut signals = CinematicSignals::default();
        let mut display = display();
        let initial_display = display;
        let mut audio = AudioState::default();
        let pressed = Buttons::from_bits(Button::Start as u16);
        let visit = |exit: &mut CinematicExit,
                     signals: &mut CinematicSignals,
                     display: &mut SceneDisplay,
                     audio: &mut AudioState| {
            exit.visit(
                CinematicExitPolicy::OPENING,
                signals,
                pressed,
                true,
                display,
                audio,
            )
        };
        assert_eq!(
            visit(&mut exit, &mut signals, &mut display, &mut audio),
            CinematicExitVisit::default()
        );
        assert!(!signals.exit_requested);
        visit(&mut exit, &mut signals, &mut display, &mut audio);
        assert!(signals.exit_requested);
        assert_eq!(display, initial_display);
        assert!(!exit.fade.active);
        assert_eq!(
            audio.take_events()[0],
            Some(SoundEvent::Authored(AuthoredCue::new(
                SKIP_CUE,
                0,
                PlayerTarget::Primary
            )))
        );
        assert_eq!(
            visit(&mut exit, &mut signals, &mut display, &mut audio),
            CinematicExitVisit {
                request_audio_exit: true,
                select_alternate_destination: true,
                completed: false,
            }
        );
        assert_eq!(display.progress.value(), OPENING_EXIT_INTENSITY);
        assert_eq!(display.request, FadeRequest::OutFast);
        for _ in 0..20 {
            assert_eq!(
                visit(&mut exit, &mut signals, &mut display, &mut audio),
                CinematicExitVisit::default()
            );
        }
        assert!(!exit.completed());
        for _ in 0..4 {
            display.visit_scene_fade(false);
        }
        assert!(visit(&mut exit, &mut signals, &mut display, &mut audio).completed);
        assert!(!signals.exit_requested);
        assert!(exit.completed());
        let completed = (exit, display, audio.clone());
        assert_eq!(
            visit(&mut exit, &mut signals, &mut display, &mut audio),
            CinematicExitVisit::default()
        );
        assert_eq!((exit, display, audio), completed);
    }

    #[test]
    fn shared_fade_preserves_display_until_delay_finishes_and_never_advances_it() {
        for delay in 0..=u16::MAX {
            let mut fade = SceneExitFade {
                active: true,
                audio_requested: false,
                delay,
            };
            let mut display = display();
            let before = display;
            let event = fade.visit(&mut display);
            let decremented = delay.wrapping_sub(1) as i16;
            assert_eq!(fade.delay, decremented.max(0) as u16);
            assert!(event.request_audio_exit);
            assert!(!event.completed);
            assert_eq!(display.progress, before.progress);
            assert_eq!(display.bands, before.bands);
            assert_eq!(display.blank_hold, before.blank_hold);
            assert_eq!(
                display.request,
                if fade.delay == 0 {
                    FadeRequest::OutFast
                } else {
                    before.request
                }
            );
        }
    }

    #[test]
    fn skip_policies_use_button_edges_but_never_block_an_existing_exit_request() {
        for policy in [
            CinematicSkipPolicy::Disabled,
            CinematicSkipPolicy::StartOrB,
            CinematicSkipPolicy::StartWhenReady,
        ] {
            for ready in [false, true] {
                for button in [Button::Start, Button::B, Button::A, Button::Select] {
                    let mut exit = CinematicExit::default();
                    let mut signals = CinematicSignals {
                        skip_ready: ready,
                        ..Default::default()
                    };
                    let mut display = display();
                    let mut audio = AudioState::default();
                    let policy = CinematicExitPolicy {
                        skip: policy,
                        ..CinematicExitPolicy::OPENING
                    };
                    let mut input = super::super::InputState::default();
                    input.sample(Buttons::from_bits(button as u16));
                    let first = exit.visit(
                        policy,
                        &mut signals,
                        input.pressed,
                        false,
                        &mut display,
                        &mut audio,
                    );
                    assert_eq!(first, CinematicExitVisit::default());
                    let accepted = match policy.skip {
                        CinematicSkipPolicy::Disabled => false,
                        CinematicSkipPolicy::StartOrB => {
                            matches!(button, Button::Start | Button::B)
                        }
                        CinematicSkipPolicy::StartWhenReady => ready && button == Button::Start,
                    };
                    assert_eq!(signals.exit_requested, accepted);
                    assert_eq!(
                        audio.take_events().iter().flatten().count(),
                        usize::from(accepted)
                    );
                    // A held button has no new edge. A controller-authored
                    // request still starts the transition for every policy.
                    input.sample(Buttons::from_bits(button as u16));
                    signals.exit_requested = true;
                    let second = exit.visit(
                        policy,
                        &mut signals,
                        input.pressed,
                        false,
                        &mut display,
                        &mut audio,
                    );
                    assert!(second.request_audio_exit);
                    assert!(audio.take_events().iter().all(Option::is_none));
                    assert_eq!(display.request, FadeRequest::OutFast);
                }
            }
        }
    }

    #[test]
    fn active_fade_bypasses_input_hold_without_consuming_it() {
        let mut exit = CinematicExit::new(123);
        exit.fade.active = true;
        exit.fade.audio_requested = true;
        exit.fade.delay = 2;
        let mut signals = CinematicSignals {
            exit_requested: false,
            skip_ready: true,
        };
        let mut display = display();
        display.progress = Intensity::DARK;
        let before = display;
        let mut audio = AudioState::default();
        let first = exit.visit(
            CinematicExitPolicy::OPENING,
            &mut signals,
            Buttons::default(),
            true,
            &mut display,
            &mut audio,
        );
        assert_eq!(first, CinematicExitVisit::default());
        assert_eq!(display, before);
        assert_eq!(exit.input_hold, 123);
        assert!(signals.skip_ready);
        let last = exit.visit(
            CinematicExitPolicy::OPENING,
            &mut signals,
            Buttons::default(),
            true,
            &mut display,
            &mut audio,
        );
        assert_eq!(
            last,
            CinematicExitVisit {
                completed: true,
                ..Default::default()
            }
        );
        assert_eq!(signals, CinematicSignals::default());
        assert_eq!(display, before);
        assert_eq!(exit.input_hold, 123);
    }
}
