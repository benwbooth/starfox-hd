//! Original cinematic exit ownership, including input and fade barriers.

use sf2_game::cinematic_exit::{
    CinematicExit, CinematicExitPolicy, CinematicExitVisit, CinematicSignals, CinematicSkipPolicy,
    SceneExitFade, OPENING_INPUT_HOLD,
};
use sf2_game::intro_scene::OpeningScene;
use sf2_game::scene_display::{DisplayBand, FadeRequest, Intensity, SceneDisplay};
use sf2_game::{AudioState, Button, Buttons, SoundEvent};
use sf_oracle::{call, call_near, Entry, RetailMachine, SnesBus};

fn rom() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM")
}

fn reach(source: &mut RetailMachine, address: u32) {
    assert!(source.tick_until_cpu_execution(0, address, 2400).unwrap());
}

fn original() -> SnesBus {
    let rom = rom();
    let runtime = rom[0x10000..0x17E00].to_vec();
    let mut source = SnesBus::new(rom);
    for (offset, byte) in runtime.into_iter().enumerate() {
        source.write8(0x7F0000 + offset as u32, byte);
    }
    source
}

fn display() -> SceneDisplay {
    SceneDisplay {
        request: FadeRequest::InFast,
        progress: Intensity::FULL,
        bands: [DisplayBand {
            blanked: false,
            intensity: Intensity::FULL,
        }; 3],
        blank_hold: 255,
        interval_remaining: 7,
        interval_reload: 19,
    }
}

fn request_byte(request: FadeRequest) -> u8 {
    match request {
        FadeRequest::Idle => 0,
        FadeRequest::In => 1,
        FadeRequest::InFast => 2,
        FadeRequest::InPaced => 3,
        FadeRequest::Out => 255,
        FadeRequest::OutFast => 254,
        FadeRequest::OutAlternating => 253,
    }
}

fn fade_bits(fade: SceneExitFade) -> u16 {
    u16::from(fade.active) * 0x20 | u16::from(fade.audio_requested) * 0x10
}

fn signal_bits(signals: CinematicSignals) -> u16 {
    u16::from(signals.exit_requested) * 0x10 | u16::from(signals.skip_ready) * 0x20
}

fn seed(source: &mut SnesBus, display: SceneDisplay, fade: SceneExitFade) -> Entry {
    source.write16(0x1C67, 0x4283 | fade_bits(fade));
    source.write16(0x1BBE, fade.delay);
    source.write8(0x1CD9, 0x55);
    source.write8(0x1CDA, 0x47);
    source.write8(0x1CD3, 0xA8);
    source.write8(0x18BB, display.blank_hold);
    source.write8(0x1C59, display.interval_remaining);
    source.write8(0x1C5A, display.interval_reload);
    for (index, band) in display.bands.into_iter().enumerate() {
        source.write8(
            0x7F007C + index as u32 * 2,
            band.intensity.value() | if band.blanked { 0x80 } else { 0 },
        );
    }
    Entry {
        x: u16::from(request_byte(display.request)) << 8,
        y: u16::from(display.progress.value()),
        p: 0x20,
        ..Default::default()
    }
}

fn compare_fade(source: &SnesBus, display: SceneDisplay, fade: SceneExitFade, audio_request: bool) {
    assert_eq!(source.read16(0x1C67), 0x4283 | fade_bits(fade));
    assert_eq!(source.read16(0x1BBE), fade.delay);
    assert_eq!(source.read8(0xF3), request_byte(display.request));
    assert_eq!(source.read8(0xF4), display.progress.value());
    assert_eq!(source.read8(0x1CD9), if audio_request { 0 } else { 0x55 });
    assert_eq!(source.read8(0x1CDA), if audio_request { 9 } else { 0x47 });
    assert_eq!(source.read8(0x1CD3), 0xA8 | u8::from(audio_request));
    assert_eq!(source.read8(0x18BB), display.blank_hold);
    assert_eq!(source.read8(0x1C59), display.interval_remaining);
    assert_eq!(source.read8(0x1C5A), display.interval_reload);
    for (index, band) in display.bands.into_iter().enumerate() {
        assert_eq!(
            source.read8(0x7F007C + index as u32 * 2),
            band.intensity.value() | if band.blanked { 0x80 } else { 0 }
        );
    }
}

#[test]
fn shared_exit_fade_matches_every_word_delay_and_audio_latch() {
    let mut source = original();
    for active in [false, true] {
        for audio_requested in [false, true] {
            for delay in 0..=u16::MAX {
                let mut fade = SceneExitFade {
                    active,
                    audio_requested,
                    delay,
                };
                let mut display = display();
                // Alternate dark/visible outcomes, plus the complete level sweep below.
                if delay & 1 == 0 {
                    display.progress = Intensity::DARK;
                }
                let entry = seed(&mut source, display, fade);
                assert!(call(&mut source, 0x03E0FC, &entry).returned);
                let event = fade.visit(&mut display);
                compare_fade(&source, display, fade, event.request_audio_exit);
            }
        }
    }
    for progress in 0..=15 {
        for delay in [0, 1, 2, 0x7FFF, 0x8000, 0x8001, 0xFFFF] {
            let mut fade = SceneExitFade {
                active: true,
                audio_requested: false,
                delay,
            };
            let mut display = SceneDisplay {
                progress: Intensity::new(progress),
                ..display()
            };
            let entry = seed(&mut source, display, fade);
            assert!(call(&mut source, 0x03E0FC, &entry).returned);
            let event = fade.visit(&mut display);
            compare_fade(&source, display, fade, event.request_audio_exit);
        }
    }
}

#[test]
fn cinematic_policies_inputs_holds_signals_and_side_effects_match_original() {
    let mut source = original();
    for profile in 0..11u16 {
        let policy = CinematicExitPolicy {
            initial_intensity: Intensity::new(source.read8(0x03C282 + u32::from(profile))),
            skip: match profile {
                5 | 10 => CinematicSkipPolicy::Disabled,
                7 => CinematicSkipPolicy::StartWhenReady,
                _ => CinematicSkipPolicy::StartOrB,
            },
        };
        if profile == 1 {
            assert_eq!(policy, CinematicExitPolicy::OPENING);
        }
        for signal in 0..4 {
            for input_hold in [0, 1, 10, u16::MAX] {
                for fade_state in 0..4 {
                    for pressed in [
                        0,
                        Button::Start as u16,
                        Button::B as u16,
                        Button::A as u16,
                        0x9000,
                        0x6FFF,
                    ] {
                        for alternate in [false, true] {
                            let mut exit = CinematicExit::new(input_hold);
                            exit.fade.active = fade_state & 1 != 0;
                            exit.fade.audio_requested = fade_state & 2 != 0;
                            let mut signals = CinematicSignals {
                                exit_requested: signal & 1 != 0,
                                skip_ready: signal & 2 != 0,
                            };
                            let initial_signals = signals;
                            let mut display = display();
                            if pressed == 0 {
                                display.progress = Intensity::DARK;
                            }
                            let entry = seed(&mut source, display, exit.fade);
                            source.write16(0x1B76, profile * 4);
                            source.write16(0x1B96, 0xABCF | signal_bits(signals));
                            source.write16(0x1B84, 0x4283 | if alternate { 8 } else { 0 });
                            source.write16(0x1B6A, 0x6532);
                            source.write16(0x1B70, 0xAC01);
                            source.write16(0x1C6E, input_hold);
                            source.write16(0x1296, pressed);
                            source.write16(0x1D16, 30);
                            source.write16(0x1D18, 8);
                            for index in 0..16 {
                                source.write16(0x1CF6 + index * 2, 0xBEEF);
                            }
                            assert!(call_near(&mut source, 0x03C1DC, &entry).returned);
                            let mut audio = AudioState::default();
                            let event = exit.visit(
                                policy,
                                &mut signals,
                                Buttons::from_bits(pressed),
                                alternate,
                                &mut display,
                                &mut audio,
                            );
                            compare_fade(&source, display, exit.fade, event.request_audio_exit);
                            assert_eq!(source.read16(0x1C6E), exit.input_hold);
                            assert_eq!(
                                source.read16(0x1B96),
                                if event.completed {
                                    0
                                } else {
                                    0xABCF | signal_bits(signals)
                                }
                            );
                            assert_eq!(
                                source.read16(0x1B70),
                                if event.completed { 0 } else { 0xAC01 }
                            );
                            assert_eq!(
                                source.read16(0x1B6A),
                                if event.select_alternate_destination {
                                    13
                                } else {
                                    0x6532
                                }
                            );
                            assert_eq!(source.read16(0x1B76), profile * 4);
                            assert_eq!(
                                source.read16(0x1B84),
                                0x4283 | if alternate { 8 } else { 0 }
                            );
                            assert_eq!(source.read16(0x1296), pressed);
                            assert_eq!(source.read16(0x1D18), 8);
                            let events = audio.take_events();
                            let queued = match events[0] {
                                Some(SoundEvent::Authored(cue)) => {
                                    assert!(!initial_signals.exit_requested);
                                    assert_eq!(cue.parameter(), 0);
                                    u16::from(cue.id)
                                }
                                None => 0xBEEF,
                                other => panic!("unexpected skip cue {other:?}"),
                            };
                            assert!(events[1..].iter().all(Option::is_none));
                            assert_eq!(
                                source.read16(0x1D16),
                                if events[0].is_some() { 0 } else { 30 }
                            );
                            assert_eq!(source.read16(0x1CF6 + 30), queued);
                            for index in 0..15 {
                                assert_eq!(source.read16(0x1CF6 + index * 2), 0xBEEF);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn original_boot_exit_matches_native_opening_handoff_with_observed_display_visits() {
    let mut source = RetailMachine::new(rom());
    reach(&mut source, 0x0DBCCF);
    reach(&mut source, 0x03C1DC);
    assert_eq!(source.peek16(0x7E1C6E), OPENING_INPUT_HOLD);
    assert_eq!(source.peek16(0x7E1B76), 4);
    let mut native = OpeningScene::default();
    let mut display = SceneDisplay {
        request: FadeRequest::Idle,
        ..display()
    };
    let mut audio = AudioState::default();
    source.watch_cpu_execution(&[0x03C1DC, 0x0DBCCF]);
    reach(&mut source, 0x0DCA18);
    reach(&mut source, 0x03C1DC);
    let visits = source.take_cpu_execution_watch_hits();
    // The initially pending entry was fetched before watching began; the
    // final watched entry has not executed. Those two boundary visits cancel.
    let previous_outer_visits = visits.iter().filter(|&&pc| pc == 0x03C1DC).count();
    assert_eq!(previous_outer_visits, 441);
    for _ in 0..previous_outer_visits {
        native.tick().unwrap();
        assert_eq!(
            native.visit_exit(Buttons::default(), &mut display, &mut audio),
            CinematicExitVisit::default()
        );
    }
    native.tick().unwrap();
    assert!(native.controller().transition_requested);
    assert_eq!(native.exit().input_hold, source.peek16(0x7E1C6E));
    assert_eq!(source.peek8(0xF3), request_byte(display.request));
    assert_eq!(source.peek8(0xF4), display.progress.value());
    assert_eq!(source.peek8(0x7E18BB), display.blank_hold);
    // The first body instruction distinguishes repeated display visits in
    // this execution-watch API, which suppresses consecutive identical hits.
    source.watch_cpu_execution(&[0x7F0E79, 0x7F0E7C]);
    source.take_cpu_execution_watch_hits();
    for visit in 0..12 {
        let event = native.visit_exit(Buttons::default(), &mut display, &mut audio);
        reach(&mut source, 0x03C27E);
        assert_eq!(source.peek8(0xF3), request_byte(display.request));
        assert_eq!(source.peek8(0xF4), display.progress.value());
        assert_eq!(
            source.peek16(0x7E1C67) & 0x30,
            fade_bits(native.exit().fade)
        );
        assert_eq!(
            source.peek16(0x7E1B96) & 0x10 != 0,
            native.controller().transition_requested
        );
        assert_eq!(source.peek16(0x7E1B70) == 0, event.completed);
        assert_eq!(event.request_audio_exit, visit == 0);
        if event.completed {
            assert!(native.exit().completed());
            assert_eq!(source.peek16(0x7E1B96), 0);
            assert_eq!(visit, 4);
            return;
        }
        reach(&mut source, 0x03C1DC);
        for service in source.take_cpu_execution_watch_hits() {
            if service == 0x7F0E79 {
                display.visit_scene_fade(false); // OutFast does not consult parity.
            } else {
                assert_eq!(service, 0x7F0E7C);
            }
        }
    }
    panic!("opening exit did not complete");
}
