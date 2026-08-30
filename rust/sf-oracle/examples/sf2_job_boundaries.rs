//! Print retail SF2 Super FX job completion frames for Mesen comparison.

use sf_oracle::RetailMachine;
use std::path::Path;

fn main() {
    let frames = std::env::args()
        .nth(1)
        .map(|value| value.parse::<u32>().expect("frames must be an integer"))
        .unwrap_or(220);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root");
    let rom_path = root.join("Star Fox 2 (USA, Europe).sfc");
    let rom = std::fs::read(&rom_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", rom_path.display()));
    let mut machine = RetailMachine::new(rom);
    let mut last_sequence = 0;

    println!(
        "sequence frame pbr pc steps entry_master_clock exit_master_clock hit_limit program ram rom multiply pixel"
    );
    for frame in 1..=frames {
        machine
            .tick_video_frames(0, 1)
            .unwrap_or_else(|error| panic!("retail machine failed: {error}"));
        for event in machine.gsu_recent_runs() {
            if event.sequence > last_sequence {
                println!(
                    "{} {} {:02X} {:04X} {} {} {} {} {} {} {} {} {}",
                    event.sequence,
                    frame,
                    event.pbr,
                    event.pc,
                    event.steps,
                    event.entry_master_clock,
                    event.exit_master_clock,
                    event.hit_limit,
                    event.timing_breakdown[0],
                    event.timing_breakdown[1],
                    event.timing_breakdown[2],
                    event.timing_breakdown[3],
                    event.timing_breakdown[4],
                );
                last_sequence = event.sequence;
            }
        }
    }
}
