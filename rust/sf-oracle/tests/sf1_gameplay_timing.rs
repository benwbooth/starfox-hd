//! Strict whole-machine proof for typed native Corneria neutral-run timing.

#[path = "../examples/support/mod.rs"]
mod support;

use sf_game::gameplay_timing::timing_for_update;
use sf_map::catalog::map_id;
use sf_oracle::{
    load_retail_rom, RetailMachine, RETAIL_DOSTRATS, RETAIL_FRAMERATE, RETAIL_GAMEFRAME,
};

const WORK_RAM: u32 = 0x7E_0000;
const VIDEO_FRAMES_PER_FRONT_END_TICK: u32 = 3;
const FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF: u32 = 890;
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD: u32 = 240;
const CORNERIA_AUDIO_UPLOAD_FRAME: u16 = 186;
const EXPECTED_FIRST_GAME_FRAME: u16 = 0;
const EXPECTED_LAST_GAME_FRAME: u16 = 820;
const MAX_HANDOFF_BOUNDARIES: usize = 4;

#[test]
fn typed_corneria_neutral_timing_matches_retail_refresh_boundaries() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("Corneria timing proof skipped: Star Fox retail ROM not found");
        return;
    };
    let mut retail = RetailMachine::new(rom);
    for tick in 0..FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF {
        retail
            .tick_video_frames(support::weapon_input(tick), VIDEO_FRAMES_PER_FRONT_END_TICK)
            .expect("retail front-end timing");
    }
    assert!(
        retail
            .tick_until_cpu_execution(
                support::weapon_input(FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF),
                RETAIL_DOSTRATS,
                MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
            )
            .expect("initial Corneria timing boundary"),
        "retail did not reach the first Corneria timing boundary"
    );
    let mut handoff_input_tick = FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF;
    for _ in 0..MAX_HANDOFF_BOUNDARIES {
        if retail.peek16(WORK_RAM | RETAIL_GAMEFRAME) == EXPECTED_FIRST_GAME_FRAME {
            break;
        }
        handoff_input_tick = handoff_input_tick.saturating_add(1);
        assert!(
            retail
                .tick_until_cpu_execution(
                    support::weapon_input(handoff_input_tick),
                    RETAIL_DOSTRATS,
                    MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
                )
                .expect("Corneria frame-zero timing boundary"),
            "retail did not reach Corneria frame zero"
        );
    }
    assert_eq!(
        retail.peek16(WORK_RAM | RETAIL_GAMEFRAME),
        EXPECTED_FIRST_GAME_FRAME,
        "timing proof must begin at Corneria frame zero"
    );

    let mut certified_frames = 0usize;
    for expected_game_frame in EXPECTED_FIRST_GAME_FRAME..=EXPECTED_LAST_GAME_FRAME {
        let game_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        assert_eq!(game_frame, expected_game_frame);
        let expected = timing_for_update(map_id::M1_1, game_frame);
        assert_eq!(
            expected.motion_refreshes,
            retail.peek8(WORK_RAM | RETAIL_FRAMERATE),
            "Corneria player-motion timing at game frame {game_frame}"
        );

        let max_video_frames = if game_frame == CORNERIA_AUDIO_UPLOAD_FRAME {
            MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD
        } else {
            MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE
        };
        let first_video_frame = retail.video_frame();
        assert!(
            retail
                .tick_until_cpu_execution(0, RETAIL_DOSTRATS, max_video_frames,)
                .expect("next Corneria timing boundary"),
            "retail did not reach the boundary after game frame {game_frame}"
        );
        let elapsed_refreshes = retail.video_frame().saturating_sub(first_video_frame);
        assert_eq!(
            u64::from(expected.presentation_refreshes),
            elapsed_refreshes,
            "Corneria presentation timing at game frame {game_frame}"
        );
        certified_frames += 1;
    }

    assert_eq!(
        retail.peek16(WORK_RAM | RETAIL_GAMEFRAME).wrapping_sub(1),
        EXPECTED_LAST_GAME_FRAME
    );
    assert_eq!(
        certified_frames,
        usize::from(EXPECTED_LAST_GAME_FRAME - EXPECTED_FIRST_GAME_FRAME + 1)
    );
}
