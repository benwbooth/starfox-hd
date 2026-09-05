//! Differential verification of the complete, unmodified retail scene
//! dispatcher and palette routines. Machine state is confined to this oracle.

use sf2_game::intro_camera::OpeningCameraCue;
use sf2_game::intro_controller::{
    IntroColor, IntroPaletteEffectState, OpeningEventTiming, OpeningSceneAction,
    OpeningSceneController, OpeningScenePalette, INTRO_PALETTE_COLORS, OPENING_SCENE_ACTIONS,
};
use sf_oracle::{call, Entry, SnesBus};

const DATA: u32 = 0x7E0000;
const ACTOR: u16 = 0x033F;
const AUX: u16 = 0x0140;
const AUX_DATA: u32 = DATA + AUX as u32;
const DISPATCH: u32 = 0x0DBCCF;
const SCRIPT: u32 = 0x0DBEDF;
const SCRIPT_POINTER: u32 = AUX_DATA + 0x6C13;
const ELAPSED: u32 = AUX_DATA + 0x6C16;
const SINCE_CUT: u32 = AUX_DATA + 0x6C1A;
const EFFECTS: u32 = AUX_DATA + 0x6BE9;
const CUE: u32 = DATA + 0x1D72;
const TRANSITION: u32 = DATA + 0x1B96;
const REFRESH: u32 = DATA + 0x1E58;
const LIVE_COLORS: u32 = DATA + 0xEFE5;
const SAVED_COLORS: u32 = DATA + 0xF2E5;
const SENTINEL: u8 = 0x85;
const RESTORING: u8 = 0x10;
const PERSISTENT: u8 = 0x20;
const HIGHLIGHTED: u8 = 0x40;
const REFRESH_BIT: u8 = 0x80;
const TRANSITION_BIT: u16 = 0x10;
const FLASH: u32 = 0x07EC9A;
const RESTORE: u32 = 0x07EEB2;
const COLOR_DOMAIN: usize = 1 << 15;

fn bus() -> SnesBus {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rom = std::fs::read(root.join("Star Fox 2 (USA, Europe).sfc"))
        .expect("controller tests require the user-owned retail SF2 ROM");
    let mut bus = SnesBus::new(rom);
    bus.write16(DATA + u32::from(ACTOR) + 0x2B, AUX);
    bus.write16(SCRIPT_POINTER, SCRIPT as u16);
    bus.write8(SCRIPT_POINTER + 2, (SCRIPT >> 16) as u8);
    bus.write8(CUE, 1);
    bus.write16(TRANSITION, u16::from(SENTINEL));
    bus
}

fn invoke(bus: &mut SnesBus, target: u32) {
    call(
        bus,
        target,
        &Entry {
            x: ACTOR,
            y: AUX,
            dbr: 0x7E,
            p: 0x20,
            ..Default::default()
        },
    );
}

fn write_palette(bus: &mut SnesBus, palette: &OpeningScenePalette) {
    for (index, (live, saved)) in palette.colors.iter().zip(&palette.saved_colors).enumerate() {
        bus.write16(LIVE_COLORS + index as u32 * 2, live.bgr555());
        bus.write16(SAVED_COLORS + index as u32 * 2, saved.bgr555());
    }
    bus.write8(EFFECTS, effect_bits(palette.effects));
}

fn effect_bits(effects: IntroPaletteEffectState) -> u8 {
    SENTINEL
        | if effects.restoring { RESTORING } else { 0 }
        | if effects.persistent_highlight {
            PERSISTENT
        } else {
            0
        }
        | if effects.highlighted { HIGHLIGHTED } else { 0 }
}

fn assert_palette(bus: &SnesBus, palette: &OpeningScenePalette, context: usize) {
    for (index, (live, saved)) in palette.colors.iter().zip(&palette.saved_colors).enumerate() {
        assert_eq!(
            live.bgr555(),
            bus.read16(LIVE_COLORS + index as u32 * 2),
            "live color {index}, case {context}"
        );
        assert_eq!(
            saved.bgr555(),
            bus.read16(SAVED_COLORS + index as u32 * 2),
            "saved color {index}, case {context}"
        );
    }
    assert_eq!(
        bus.read8(EFFECTS),
        effect_bits(palette.effects),
        "effects, case {context}"
    );
    assert_eq!(
        bus.read8(REFRESH),
        1 | if palette.refresh_requested {
            REFRESH_BIT
        } else {
            0
        },
        "refresh, case {context}"
    );
}

fn cue_value(cue: OpeningCameraCue) -> u8 {
    match cue {
        OpeningCameraCue::Opening => 1,
        OpeningCameraCue::FirstCut => 2,
        OpeningCameraCue::SecondCut => 3,
        OpeningCameraCue::ThirdCut => 4,
        OpeningCameraCue::FourthCut => 5,
        OpeningCameraCue::FinalCut => 6,
    }
}

#[test]
fn native_actions_match_authored_records_in_order() {
    let bus = bus();
    let mut cursor = SCRIPT;
    for expected in OPENING_SCENE_ACTIONS {
        let condition = bus.read8(cursor);
        let start = bus.read16(cursor + 1);
        let (timing, service) = match condition {
            0 => (OpeningEventTiming::At(start), bus.read16(cursor + 3)),
            3 => (
                OpeningEventTiming::Interval {
                    start,
                    end: bus.read16(cursor + 3),
                },
                bus.read16(cursor + 5),
            ),
            other => panic!("unreviewed timing condition {other}"),
        };
        let action = match service {
            0xCF2A => OpeningSceneAction::PrepareLogoPalette,
            0xCF73 => OpeningSceneAction::RestorePalette { steps: 2 },
            0xCF6C => OpeningSceneAction::RestorePalette { steps: 1 },
            0xC82F => OpeningSceneAction::AdvanceCameraCue,
            0xCA18 => OpeningSceneAction::RequestNextScene,
            0xCF89 => OpeningSceneAction::FlashPalette,
            other => panic!("unreviewed scene service {other}"),
        };
        assert_eq!(expected.timing, timing);
        assert_eq!(expected.action, action);
        cursor += if condition == 3 { 7 } else { 5 };
    }
    assert_eq!(bus.read16(cursor), u16::MAX);
}

#[test]
fn complete_scene_matches_original_dispatcher_for_all_effect_policies() {
    for pattern in 0..3 {
        for policy in 0..8 {
            let mut bus = bus();
            let colors = std::array::from_fn(|index| {
                IntroColor::from_bgr555(match pattern {
                    0 => (index * 257 % COLOR_DOMAIN) as u16,
                    1 => (COLOR_DOMAIN - 1 - index * 127) as u16,
                    _ => 0,
                })
            });
            let mut palette = OpeningScenePalette::new(colors);
            palette.saved_colors = std::array::from_fn(|index| {
                IntroColor::from_bgr555((index * 179 % COLOR_DOMAIN) as u16)
            });
            palette.effects = IntroPaletteEffectState {
                restoring: policy & 1 != 0,
                persistent_highlight: policy & 2 != 0,
                highlighted: policy & 4 != 0,
            };
            write_palette(&mut bus, &palette);
            let mut controller = OpeningSceneController::default();
            for update in 0..460 {
                bus.write8(REFRESH, 1);
                palette.refresh_requested = false;
                invoke(&mut bus, DISPATCH);
                controller.tick(&mut palette);
                assert_eq!(
                    controller.elapsed_updates(),
                    bus.read16(ELAPSED),
                    "update {update}"
                );
                assert_eq!(
                    controller.updates_since_cut(),
                    bus.read16(SINCE_CUT),
                    "update {update}"
                );
                assert_eq!(
                    cue_value(controller.cue()),
                    bus.read8(CUE),
                    "update {update}"
                );
                assert_eq!(
                    bus.read16(TRANSITION),
                    u16::from(SENTINEL)
                        | if controller.transition_requested {
                            TRANSITION_BIT
                        } else {
                            0
                        }
                );
                assert_palette(&bus, &palette, update);
            }
        }
    }
}

#[test]
fn elapsed_time_saturates_without_replaying_the_scene() {
    let mut bus = bus();
    let mut palette = OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]);
    let mut controller = OpeningSceneController::default();
    for update in 0..=usize::from(u16::MAX) + 2 {
        invoke(&mut bus, DISPATCH);
        controller.tick(&mut palette);
        assert_eq!(
            controller.elapsed_updates(),
            bus.read16(ELAPSED),
            "update {update}"
        );
        assert_eq!(
            controller.updates_since_cut(),
            bus.read16(SINCE_CUT),
            "update {update}"
        );
        assert_eq!(
            cue_value(controller.cue()),
            bus.read8(CUE),
            "update {update}"
        );
    }
    assert_eq!(controller.elapsed_updates(), u16::MAX);
}

#[test]
fn every_artwork_color_matches_original_flash_including_channel_clamps() {
    const AFFECTED_COLORS: usize = 60;
    let mut bus = bus();
    for base in (0..COLOR_DOMAIN).step_by(AFFECTED_COLORS) {
        let mut palette =
            OpeningScenePalette::new([IntroColor::new(31, 31, 31); INTRO_PALETTE_COLORS]);
        let mut next = base;
        for index in 64..INTRO_PALETTE_COLORS {
            if index % 16 != 0 {
                palette.colors[index] = IntroColor::from_bgr555((next % COLOR_DOMAIN) as u16);
                next += 1;
            }
        }
        write_palette(&mut bus, &palette);
        bus.write8(REFRESH, 1);
        invoke(&mut bus, FLASH);
        palette.flash();
        assert_palette(&bus, &palette, base);
    }
}

#[test]
fn restoration_matches_all_channel_pairs_and_completion_policies() {
    let mut bus = bus();
    for channel in 0..3 {
        for base in (0..1024).step_by(64) {
            let mut palette =
                OpeningScenePalette::new([IntroColor::default(); INTRO_PALETTE_COLORS]);
            for index in 0..64 {
                let pair = base + index;
                palette.colors[index + 64] =
                    IntroColor::from_bgr555(((pair % 32) << (channel * 5)) as u16);
                palette.saved_colors[index + 64] =
                    IntroColor::from_bgr555(((pair / 32) << (channel * 5)) as u16);
            }
            palette.effects = IntroPaletteEffectState {
                restoring: true,
                persistent_highlight: false,
                highlighted: true,
            };
            write_palette(&mut bus, &palette);
            for step in 0..33 {
                bus.write8(REFRESH, 1);
                palette.refresh_requested = false;
                invoke(&mut bus, RESTORE);
                palette.restore_step();
                assert_palette(&bus, &palette, step);
            }
        }
    }
}
