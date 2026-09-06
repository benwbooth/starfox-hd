//! Isolated source-routine oracle for native render-work accounting. No frame
//! capture or observed actor partition is used as native scheduling input.

use sf2_game::intro_render_work::{BitmapClearLayout, BitmapClearWork};
use sf_oracle::gsu::Gsu;

#[test]
fn native_clear_work_matches_source_memory_and_instruction_count() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    // Return to an existing STOP following the caller, not a patched routine.
    assert_eq!(rom[0xD224], 0);
    for (base, last_line, width, pitch) in [
        (0x6C00, 191, 224, 256),
        (0x6C00, 191, 256, 256),
        (0x6000, 7, 1, 1),
        (0x6001, 15, 3, 7),
        (0xFFF9, 23, 3, 5),
        (0x6000, 23, 0x8001, 0xFFFF),
        (0x6000, 0, 1, 0),
        (0x6000, 0xFFFF, 1, 0),
        (0x6000, 0xFFFE, 1, 0),
        (0x6000, 0x8000, 1, 0),
    ] {
        let layout = BitmapClearLayout {
            base,
            last_line,
            width,
            pitch,
        };
        let work = BitmapClearWork::new(layout).unwrap();
        let mut source = Gsu::new(rom.clone());
        source.ram.fill(0xA5);
        for (address, value) in [(0x003A, last_line), (0x24C2, width), (0x24C4, pitch)] {
            source.ram[address..address + 2].copy_from_slice(&value.to_le_bytes());
        }
        let mut expected = source.ram.clone();
        for row in 0..work.rows() {
            let start = work.row_address(row).unwrap();
            for column in 0..work.words_per_row() {
                let address = start.wrapping_add((column as u16).wrapping_mul(2));
                expected[usize::from(address)] = 0;
                expected[usize::from(address ^ 1)] = 0;
            }
        }
        source.r[1] = base;
        source.r[11] = 0xD224;
        source.run_with_limit(1, 0xD226, 2_000_000);
        assert!(!source.last_run_hit_limit, "{layout:?}");
        assert_eq!(
            source.last_run_steps,
            work.source_instructions() + 1,
            "{layout:?}"
        );
        assert_eq!(source.r[1], work.final_address(), "{layout:?}");
        assert_eq!(source.r[2], 0, "row counter {layout:?}");
        assert_eq!(source.r[12], 0, "word counter {layout:?}");
        assert_eq!(source.ram, expected, "RAM footprint {layout:?}");
    }
}
