//! Original scene-band and full-screen fade services, including the frame
//! wrapper's independently scheduled blank hold. No source code is replaced.

use sf2_game::scene_display::{DisplayBand, FadeRequest, Intensity, SceneDisplay};
use sf_oracle::{call, call_near, Entry, SnesBus};

const REQUESTS: [FadeRequest; 7] = [
    FadeRequest::Idle,
    FadeRequest::In,
    FadeRequest::InFast,
    FadeRequest::InPaced,
    FadeRequest::Out,
    FadeRequest::OutFast,
    FadeRequest::OutAlternating,
];

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

fn published(band: DisplayBand) -> u8 {
    band.intensity.value() | if band.blanked { 0x80 } else { 0 }
}

fn original() -> SnesBus {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned retail SF2 ROM");
    let runtime = rom[0x10000..0x17E00].to_vec();
    let mut source = SnesBus::new(rom);
    for (offset, byte) in runtime.into_iter().enumerate() {
        source.write8(0x7F0000 + offset as u32, byte);
    }
    source
}

fn state() -> SceneDisplay {
    SceneDisplay {
        request: FadeRequest::Idle,
        progress: Intensity::DARK,
        bands: [
            DisplayBand {
                blanked: false,
                intensity: Intensity::new(3),
            },
            DisplayBand {
                blanked: true,
                intensity: Intensity::new(7),
            },
            DisplayBand {
                blanked: false,
                intensity: Intensity::new(12),
            },
        ],
        blank_hold: 255,
        interval_remaining: 0,
        interval_reload: 0,
    }
}

fn seed(source: &mut SnesBus, state: SceneDisplay, odd: bool) -> Entry {
    source.write8(0xC4, u8::from(odd));
    source.write8(0x18BB, state.blank_hold);
    source.write8(0x1C59, state.interval_remaining);
    source.write8(0x1C5A, state.interval_reload);
    for (index, band) in state.bands.into_iter().enumerate() {
        source.write8(0x7F007C + 2 * index as u32, published(band));
    }
    Entry {
        // The bootstrap supplies F2/F3 and F4/F5 from these two parameters.
        x: u16::from(request_byte(state.request)) << 8,
        y: u16::from(state.progress.value()),
        p: 0x20,
        ..Default::default()
    }
}

fn compare(source: &SnesBus, native: SceneDisplay) {
    assert_eq!(source.read8(0xF3), request_byte(native.request));
    assert_eq!(source.read8(0xF4), native.progress.value());
    assert_eq!(source.read8(0x18BB), native.blank_hold);
    assert_eq!(source.read8(0x1C59), native.interval_remaining);
    assert_eq!(source.read8(0x1C5A), native.interval_reload);
    for (index, band) in native.bands.into_iter().enumerate() {
        assert_eq!(source.read8(0x7F007C + 2 * index as u32), published(band));
    }
    assert_eq!(
        native.ready_for_scene_load(),
        source.read8(0xF4) == 0 && source.read8(0x7F007C) == 0x80
    );
}

fn compare_scene_visit(source: &mut SnesBus, mut native: SceneDisplay, odd: bool) {
    let entry = seed(source, native, odd);
    assert!(call_near(source, 0x7F0E79, &entry).returned);
    native.visit_scene_fade(odd);
    compare(source, native);
}

#[test]
fn scene_band_fades_match_all_levels_holds_parities_and_paced_counter_pairs() {
    let mut source = original();
    for request in REQUESTS {
        for progress in 0..=15 {
            for odd in [false, true] {
                for blank_hold in [0, 1, 10, 255] {
                    for interval_remaining in [0, 1, 2, 255] {
                        for interval_reload in [0, 1, 3, 255] {
                            compare_scene_visit(
                                &mut source,
                                SceneDisplay {
                                    request,
                                    progress: Intensity::new(progress),
                                    blank_hold,
                                    interval_remaining,
                                    interval_reload,
                                    ..state()
                                },
                                odd,
                            );
                        }
                    }
                }
            }
        }
    }
    for progress in [0, 14, 15] {
        for interval_remaining in 0..=255 {
            for interval_reload in 0..=255 {
                compare_scene_visit(
                    &mut source,
                    SceneDisplay {
                        request: FadeRequest::InPaced,
                        progress: Intensity::new(progress),
                        interval_remaining,
                        interval_reload,
                        ..state()
                    },
                    false,
                );
            }
        }
    }
}

#[test]
fn full_screen_fades_keep_requests_and_band_publications_at_both_endpoints() {
    let mut source = original();
    for request in REQUESTS {
        for progress in 0..=15 {
            for blank_hold in [0, 255] {
                let mut native = SceneDisplay {
                    request,
                    progress: Intensity::new(progress),
                    blank_hold,
                    interval_remaining: 7,
                    interval_reload: 19,
                    ..state()
                };
                let entry = seed(&mut source, native, false);
                let returned = call_near(&mut source, 0x7F0FFF, &entry);
                assert!(returned.returned);
                let output = native.visit_full_screen_fade();
                compare(&source, native);
                // The original returns the same byte it just wrote to the
                // whole-screen output. This harness does not model a PPU.
                assert_eq!(returned.a as u8, published(output));
            }
        }
    }
}

#[test]
fn frame_hold_entry_reset_and_explicit_publication_preserve_other_owners() {
    let mut source = original();
    for blank_hold in 0..=255 {
        let mut native = SceneDisplay {
            request: FadeRequest::OutFast,
            progress: Intensity::new(11),
            blank_hold,
            interval_remaining: 91,
            interval_reload: 71,
            ..state()
        };
        let entry = seed(&mut source, native, false);
        assert!(call(&mut source, 0x03DD81, &entry).returned);
        native.advance_blank_hold();
        compare(&source, native);
        let entry = seed(&mut source, native, false);
        assert!(call(&mut source, 0x03DD6F, &entry).returned);
        native.begin_entry_fade();
        compare(&source, native);
    }
    for blanked in [false, true] {
        for intensity in 0..=15 {
            let band = DisplayBand {
                blanked,
                intensity: Intensity::new(intensity),
            };
            let mut native = state();
            let mut entry = seed(&mut source, native, false);
            entry.a = u16::from(published(band));
            assert!(call(&mut source, 0x038784, &entry).returned);
            native.publish_all_bands(band);
            compare(&source, native);
        }
    }
}
