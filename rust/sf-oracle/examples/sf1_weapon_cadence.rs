//! Oracle-only cadence census for the Corneria weapon scenario.

mod support;

use sf_oracle::{
    load_retail_rom, RetailMachine, RETAIL_DOSTRATS, RETAIL_FRAMERATE, RETAIL_GAMEFRAME,
};

const WORK_RAM: u32 = 0x7E_0000;
const VIDEO_FRAMES_PER_FRONT_END_TICK: u32 = 3;
const FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF: u32 = 890;
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD: u32 = 240;
const CORNERIA_AUDIO_UPLOAD_FRAME: u16 = 186;
const DEFAULT_LAST_INPUT_INDEPENDENT_FRAME: u16 = 318;
const LAST_FRAME_ENV: &str = "SF1_CADENCE_LAST_FRAME";
const OUTPUT_FIRST_FRAME_ENV: &str = "SF1_CADENCE_OUTPUT_FIRST_FRAME";
const MAX_HANDOFF_BOUNDARIES: usize = 4;

fn main() {
    let last_frame = std::env::var(LAST_FRAME_ENV)
        .ok()
        .map(|value| {
            value
                .parse::<u16>()
                .unwrap_or_else(|error| panic!("invalid {LAST_FRAME_ENV}={value:?}: {error}"))
        })
        .unwrap_or(DEFAULT_LAST_INPUT_INDEPENDENT_FRAME);
    let output_first_frame = std::env::var(OUTPUT_FIRST_FRAME_ENV)
        .ok()
        .map(|value| {
            value.parse::<u16>().unwrap_or_else(|error| {
                panic!("invalid {OUTPUT_FIRST_FRAME_ENV}={value:?}: {error}")
            })
        })
        .unwrap_or(0);
    assert!(
        output_first_frame <= last_frame,
        "{OUTPUT_FIRST_FRAME_ENV} must not exceed {LAST_FRAME_ENV}"
    );
    let rom = load_retail_rom().expect("Star Fox retail ROM is required");
    let mut retail = RetailMachine::new(rom);
    for tick in 0..FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF {
        retail
            .tick_video_frames(support::weapon_input(tick), VIDEO_FRAMES_PER_FRONT_END_TICK)
            .expect("retail front-end cadence");
    }
    assert!(
        retail
            .tick_until_cpu_execution(
                support::weapon_input(FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF),
                RETAIL_DOSTRATS,
                MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
            )
            .expect("initial Corneria cadence boundary"),
        "retail did not reach the first Corneria cadence boundary"
    );
    let mut handoff_input_tick = FRONT_END_TICKS_BEFORE_CORNERIA_HANDOFF;
    for _ in 0..MAX_HANDOFF_BOUNDARIES {
        if retail.peek16(WORK_RAM | RETAIL_GAMEFRAME) == 0 {
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
                .expect("Corneria frame-zero boundary"),
            "retail did not reach Corneria frame zero"
        );
    }
    assert_eq!(
        retail.peek16(WORK_RAM | RETAIL_GAMEFRAME),
        0,
        "cadence extraction must begin at Corneria frame zero"
    );

    let mut updates = 0u32;
    let mut cadence_by_frame = Vec::new();
    let mut presentation_refreshes_by_frame = Vec::new();
    for expected_game_frame in 0..=last_frame {
        let rate = retail.peek8(WORK_RAM | RETAIL_FRAMERATE);
        let game_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        assert_eq!(game_frame, expected_game_frame);
        cadence_by_frame.push((game_frame, rate));
        let max_video_frames = if game_frame == CORNERIA_AUDIO_UPLOAD_FRAME {
            MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD
        } else {
            MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE
        };
        let first_video_frame = retail.video_frame();
        assert!(
            retail
                .tick_until_cpu_execution(0, RETAIL_DOSTRATS, max_video_frames,)
                .expect("next Corneria cadence boundary"),
            "retail did not reach cadence boundary after game frame {game_frame}"
        );
        presentation_refreshes_by_frame.push((
            game_frame,
            retail.video_frame().saturating_sub(first_video_frame),
        ));
        updates += 1;
    }
    println!(
        "cadence_values={:?}",
        cadence_by_frame
            .iter()
            .filter(|(game_frame, _)| *game_frame >= output_first_frame)
            .map(|(_, rate)| *rate)
            .collect::<Vec<_>>()
    );
    println!(
        "presentation_refresh_values={:?}",
        presentation_refreshes_by_frame
            .iter()
            .filter(|(game_frame, _)| *game_frame >= output_first_frame)
            .map(|(_, refreshes)| *refreshes)
            .collect::<Vec<_>>()
    );
    println!("sf1_weapon_cadence certified_updates={updates} first_divergence=none");
}
