//! Print observable Star Fox 2 state changes while the retail machine boots.
//! This is an oracle archaeology tool; none of these storage locations are
//! consumed by the native port.

use sf_oracle::RetailMachine;
use std::path::Path;

const DEFAULT_FRAMES: u32 = 720;
const START_BUTTON: u16 = 1 << 12;
const CONFIRM_BUTTON: u16 = 1 << 7;

fn main() {
    let frame_limit = std::env::args()
        .nth(1)
        .map(|value| value.parse::<u32>().expect("frame limit must be decimal"))
        .unwrap_or(DEFAULT_FRAMES);
    let input_mode = std::env::args().nth(2).unwrap_or_default();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace has a repository parent");
    let rom_path = repository.join("Star Fox 2 (USA, Europe).sfc");
    let rom = std::fs::read(&rom_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", rom_path.display()));
    let mut machine = RetailMachine::new(rom);
    let mut previous = None;

    println!("video_frame input mode global_frame active_object draw_count nonblack");
    for video_frame in 1..=frame_limit {
        let input = match input_mode.as_str() {
            "autostart" if video_frame >= 45 && video_frame % 120 < 2 => START_BUTTON,
            "advance" if video_frame >= 45 && video_frame % 60 < 2 => {
                if video_frame % 120 < 60 {
                    START_BUTTON
                } else {
                    CONFIRM_BUTTON
                }
            }
            _ => 0,
        };
        machine
            .tick_video_frames(input, 1)
            .unwrap_or_else(|error| panic!("retail machine failed: {error}"));
        let state = (
            machine.peek8(0x7E_1911),
            machine.peek8(0x7E_00C4),
            u16::from(machine.peek8(0x7E_12A8)) | (u16::from(machine.peek8(0x7E_12A9)) << 8),
            u16::from(machine.peek8(0x7E_18C6)) | (u16::from(machine.peek8(0x7E_18C7)) << 8),
        );
        if previous != Some(state) || input != 0 {
            let nonblack = machine
                .ppu_frame()
                .rgba
                .chunks_exact(4)
                .filter(|pixel| pixel[..3] != [0, 0, 0])
                .count();
            println!(
                "{video_frame} {input:04X} {:02X} {:02X} {:04X} {} {nonblack}",
                state.0, state.1, state.2, state.3
            );
            previous = Some(state);
        }
    }

    let word = |address: u32| {
        u16::from(machine.peek8(address)) | (u16::from(machine.peek8(address + 1)) << 8)
    };
    let camera = [
        word(0x7E_00C7),
        word(0x7E_00C9),
        word(0x7E_00CB),
        u16::from(machine.peek8(0x7E_00CD)),
        u16::from(machine.peek8(0x7E_00CE)),
        u16::from(machine.peek8(0x7E_00CF)),
    ];
    let first_draw: Vec<_> = (0..38)
        .map(|offset| machine.peek8(0x7E_B273 + offset))
        .collect();
    println!("final_camera={camera:04X?}");
    println!("final_first_draw={first_draw:02X?}");
    let draw_count = usize::from(word(0x7E_18C6));
    for index in 0..draw_count {
        let base = 0x7E_B273 + (index * 38) as u32;
        println!(
            "draw={index} shape={:04X} position=({},{},{}) rotation=({},{},{}) material={:04X}",
            word(base + 8),
            word(base + 32) as i16,
            word(base + 34) as i16,
            word(base + 36) as i16,
            machine.peek8(base + 4),
            machine.peek8(base + 5),
            machine.peek8(base + 6),
            word(base + 22),
        );
    }
}
