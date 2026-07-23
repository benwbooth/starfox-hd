//! Capture high-level SF2 scene transitions from the retail oracle.
//!
//! This intentionally reads source-machine storage. Its output is evidence
//! for typed native scene data; the shipping game never depends on it.

use sf_oracle::RetailMachine;
use std::path::Path;

const DEFAULT_VIDEO_FRAMES: u32 = 900;
const GAME_TICK_VIDEO_FRAMES: u32 = 3;
const TITLE_CRAFT_SHAPE: u16 = 50_776;
const DRAW_RECORD_SIZE: u32 = 38;
const DRAW_RECORD_START: u32 = 0x7E_B273;
const DRAW_COUNT: u32 = 0x7E_18C6;
const CAMERA_X: u32 = 0x7E_00C7;
const CAMERA_Y: u32 = 0x7E_00C9;
const CAMERA_Z: u32 = 0x7E_00CB;
const CAMERA_PITCH: u32 = 0x7E_00CD;
const CAMERA_YAW: u32 = 0x7E_00CE;
const CAMERA_ROLL: u32 = 0x7E_00CF;
const GLOBAL_FRAME: u32 = 0x7E_00C4;

fn main() {
    let frame_limit = std::env::args()
        .nth(1)
        .map(|value| value.parse::<u32>().expect("frame limit must be decimal"))
        .unwrap_or(DEFAULT_VIDEO_FRAMES);
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace has a repository parent");
    let rom_path = repository.join("Star Fox 2 (USA, Europe).sfc");
    let rom = std::fs::read(&rom_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", rom_path.display()));
    let mut machine = RetailMachine::new(rom);
    let mut previous = None;

    println!(
        "video sample global_frame draw craft nonblack camera_position camera_rotation craft_poses"
    );
    for video_frame in 1..=frame_limit {
        machine
            .tick_video_frames(0, 1)
            .unwrap_or_else(|error| panic!("retail machine failed: {error}"));
        if video_frame % GAME_TICK_VIDEO_FRAMES != 0 {
            continue;
        }

        let word = |address: u32| {
            u16::from(machine.peek8(address)) | (u16::from(machine.peek8(address + 1)) << 8)
        };
        let draw_count = word(DRAW_COUNT);
        let mut craft_poses = Vec::new();
        for index in 0..u32::from(draw_count) {
            let base = DRAW_RECORD_START + index * DRAW_RECORD_SIZE;
            if word(base + 8) == TITLE_CRAFT_SHAPE {
                craft_poses.push((
                    [
                        word(base + 32) as i16,
                        word(base + 34) as i16,
                        word(base + 36) as i16,
                    ],
                    [
                        machine.peek8(base + 4),
                        machine.peek8(base + 5),
                        machine.peek8(base + 6),
                    ],
                ));
            }
        }
        let nonblack = machine
            .ppu_frame()
            .rgba
            .chunks_exact(4)
            .filter(|pixel| pixel[..3] != [0, 0, 0])
            .count();
        let signature = (draw_count, craft_poses.clone(), nonblack);
        if previous.as_ref() != Some(&signature) || video_frame % 60 == 0 {
            println!(
                "{video_frame} {} {} {draw_count} {} {nonblack} {:?} {:?} {:?}",
                video_frame / GAME_TICK_VIDEO_FRAMES,
                machine.peek8(GLOBAL_FRAME),
                craft_poses.len(),
                [
                    word(CAMERA_X) as i16,
                    word(CAMERA_Y) as i16,
                    word(CAMERA_Z) as i16,
                ],
                [
                    machine.peek8(CAMERA_PITCH),
                    machine.peek8(CAMERA_YAW),
                    machine.peek8(CAMERA_ROLL),
                ],
                craft_poses,
            );
            previous = Some(signature);
        }
    }
}
