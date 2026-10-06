//! Source-owned aperture request, render and cleanup boundaries during launch.

#[path = "../examples/support/mod.rs"]
mod support;
#[path = "../examples/support/sf1_timing.rs"]
mod timing_entry;

use sf_core::screen_wipe::ScreenWipeKind;
use sf_oracle::{
    load_retail_rom, RetailMachine, RETAIL_DOSTRATS, RETAIL_FADEDIR, RETAIL_SCRAMBLE_COUNT,
    RETAIL_XINIDISP1,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const ROM_SHA256: &str = "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548";
const WORK_RAM: u32 = 0x7E_0000;
const BEFORE_SPRITES: u32 = 0x03_DB86;
const AFTER_SPRITES: u32 = 0x02_DA32;
const AFTER_WINDOW_WIPE: u32 = 0x02_DA76;
const MAX_VIDEO_FRAMES: u32 = 12;
const WIPE_REQUEST: u32 = 0x1FD0;
const WIPE_LOCK: u32 = 0x1FD1;
const WIPE_RECORD: u32 = 0x1FC9;
const CIRCLE_COMMAND: u32 = 0x16AB;
const STAY_BLACK: u32 = 0x1962;
const ONCE_WIPE: u32 = 0xF0B3;
const WINDOW_MODE: u32 = 0x14C1;
const BLACK_WINDOW: u32 = 0x1489;
const BLACK_WINDOW_MASK: u8 = 2;
const WINDOW_VALUE_OFFSET: u32 = 7;
const STAR_RECORD_BYTES: u16 = 42;
const LAUNCH_WARNING_DRAIN_UPDATES: u16 = 80;

fn boundary(retail: &mut RetailMachine, address: u32) {
    assert!(retail
        .tick_until_cpu_execution(0, address, MAX_VIDEO_FRAMES)
        .unwrap());
}

#[test]
fn retail_launch_map_owns_wipe_request_and_cleanup() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("skip: no retail Star Fox 1 ROM");
        return;
    };
    assert_eq!(format!("{:x}", Sha256::digest(&rom)), ROM_SHA256);
    let mut retail = RetailMachine::new(rom);
    let mut native = support::configured_shell();
    timing_entry::enter_first_corneria_update(&mut retail).unwrap();
    timing_entry::enter_native_corneria_update(&mut native).unwrap();

    assert_eq!(native.game.vars.circleanim, 0);
    assert!(!native.frame().screen_wipe.active);
    let mut first_record = None;
    let mut rendered_records = BTreeSet::new();
    let mut source_header_resumed = false;
    let mut observed_last_warning = false;

    for update in 1..=LAUNCH_WARNING_DRAIN_UPDATES {
        boundary(&mut retail, BEFORE_SPRITES);
        let has_record = retail.peek8(WORK_RAM | WIPE_REQUEST) != 0;
        let sprite_lock = retail.peek8(WORK_RAM | WIPE_LOCK) != 0;
        let count_before = retail.peek8(WORK_RAM | RETAIL_SCRAMBLE_COUNT);
        let record = retail.peek16(WORK_RAM | WIPE_RECORD);
        source_header_resumed |= retail.peek16(WORK_RAM | CIRCLE_COMMAND) != 0;
        boundary(&mut retail, AFTER_SPRITES);
        let count = retail.peek8(WORK_RAM | RETAIL_SCRAMBLE_COUNT);
        boundary(&mut retail, AFTER_WINDOW_WIPE);
        let next_record = retail.peek16(WORK_RAM | WIPE_RECORD);
        boundary(&mut retail, RETAIL_DOSTRATS);
        native.tick(0);
        let frame = native.frame();
        let original_display = retail.peek8(RETAIL_XINIDISP1);
        assert_eq!(
            frame.display_brightness,
            original_display & 15,
            "display brightness at update {update}"
        );
        assert_eq!(
            frame.display_forced_blank,
            original_display & 0x80 != 0,
            "display blank at update {update}"
        );
        assert_eq!(
            native.game.vars.strategy.fade_direction as u8,
            retail.peek8(WORK_RAM | RETAIL_FADEDIR),
            "fade direction at update {update}"
        );
        assert_eq!(native.game.vars.gameframe, update);
        assert_eq!(
            frame.screen_wipe.active, has_record,
            "aperture at update {update}"
        );
        if has_record {
            let base = *first_record.get_or_insert(record);
            let offset = record.checked_sub(base).expect("monotonic star records");
            assert_eq!(offset % STAR_RECORD_BYTES, 0);
            let source_frame = (offset / STAR_RECORD_BYTES) as u8;
            assert!(source_frame < ScreenWipeKind::StarReveal.frame_count());
            assert_eq!(frame.screen_wipe.kind, ScreenWipeKind::StarReveal);
            assert_eq!(
                frame.screen_wipe.frame, source_frame,
                "record at update {update}"
            );
            assert_eq!(
                next_record,
                if source_frame + 1 == ScreenWipeKind::StarReveal.frame_count() {
                    1
                } else {
                    record + STAR_RECORD_BYTES
                }
            );
            rendered_records.insert(source_frame);
        }
        assert_eq!(
            native.game.vars.strategy.wipe_active,
            retail.peek8(WORK_RAM | WIPE_LOCK),
            "next sprite lock at update {update}"
        );
        assert_eq!(
            native.game.vars.circleanim != 0,
            retail.peek16(WORK_RAM | CIRCLE_COMMAND) != 0,
            "animation cleanup at update {update}"
        );
        assert_eq!(
            native.game.vars.oncewipe,
            retail.peek8(WORK_RAM | ONCE_WIPE),
            "once-only flag at update {update}"
        );
        assert_eq!(
            native.game.vars.scramble_count, count,
            "countdown at update {update}"
        );
        let prepared_warning = !sprite_lock && count_before != 0;
        assert_eq!(
            frame.scramble_banner.is_some(),
            prepared_warning,
            "prepared warning at update {update}"
        );
        if let Some(warning) = frame.scramble_banner {
            assert_eq!(warning.ticks_remaining, count_before);
            assert_eq!(warning.game_frame, update);
            for phase in 0..MAX_VIDEO_FRAMES as u8 {
                assert_eq!(warning.is_visible_at_phase(phase), update & 7 > 3);
            }
            observed_last_warning |= count_before == 1 && count == 0 && warning.is_visible();
        }
        if source_header_resumed {
            let original_black_active =
                retail.peek8(WORK_RAM | WINDOW_MODE) & BLACK_WINDOW_MASK != 0;
            assert_eq!(
                frame.display_black_subtraction,
                if original_black_active {
                    retail.peek8(WORK_RAM | BLACK_WINDOW) & 31
                } else {
                    0
                },
                "prepared black window at update {update}"
            );
            let native_black = frame
                .windows
                .iter()
                .find(|window| window.mode == sf_game::windows::WINDOW_MODE_BLACK);
            assert_eq!(
                native_black.is_some(),
                original_black_active,
                "black window lifetime at update {update}"
            );
            if let Some(window) = native_black {
                assert_eq!(
                    window.wm_val,
                    retail.peek8(WORK_RAM | (BLACK_WINDOW + WINDOW_VALUE_OFFSET)),
                    "black window next intensity at update {update}"
                );
            }
            assert_eq!(
                native.game.vars.strategy.stay_black as u8,
                retail.peek8(WORK_RAM | STAY_BLACK),
                "black-window countdown at update {update}"
            );
        }
    }
    assert_eq!(
        rendered_records,
        (0..ScreenWipeKind::StarReveal.frame_count()).collect()
    );
    assert!(
        observed_last_warning,
        "last warning remains prepared after its counter reaches zero"
    );
    assert_eq!(native.game.vars.scramble_count, 0);
}
