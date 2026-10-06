//! Trace the opening's inherited display policy back to its source producers.

use sf_oracle::RetailMachine;

fn main() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .unwrap();
    let mut source = RetailMachine::new(rom);
    for boundary in [
        0x03BDA3, 0x03B40B, 0x03B6E5, 0x03AD59, 0x03AE38, 0x03AE4E, 0x0DBCCF, 0x039D5B, 0x03C80B,
    ] {
        assert!(
            source.tick_until_cpu_execution(0, boundary, 240).unwrap(),
            "did not reach {boundary:06X}; stopped {:06X}",
            source.pc()
        );
        print!("boundary={boundary:06X} frame={} ", source.video_frame());
        for address in [
            0x1AA5, 0x1AA6, 0x1A89, 0x1A8A, 0x1C52, 0x1C56, 0x1914, 0x1916, 0x1918, 0x002A, 0x002C,
            0x002E, 0x0030, 0x00D0, 0x00D2,
        ] {
            print!("{address:04X}={:04X} ", source.peek16(0x7E0000 + address));
        }
        println!();
    }
}
