//! Original CPU shape/color frame selection, before GSU draw submission.
use sf2_game::intro_scene::OpeningAnimationFrame;
use sf_oracle::{call, Entry, SnesBus};

#[test]
fn independent_animation_controls_match_original_for_every_clock_and_selector() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    let mut bus = SnesBus::new(rom.clone());
    for offset in 0..0x7E00 {
        bus.write8(0x7F0000 + offset as u32, rom[0x10000 + offset]);
    }
    // Return immediately after both frame stores. All selection instructions
    // ($7F:1406..141B), branches and clock reads remain original bytes.
    bus.write8(0x7F141E, 0x6B);
    const OBJECT: u16 = 0x03BD;
    const RECORD: u16 = 0xB273;
    const WRAM: u32 = 0x7E0000;
    let selector = |raw: u8| {
        if raw & 128 == 0 {
            OpeningAnimationFrame::SceneClock
        } else {
            OpeningAnimationFrame::Authored(raw & 127)
        }
    };
    for clock in u8::MIN..=u8::MAX {
        bus.write8(WRAM + 0xC4, clock);
        for raw in u8::MIN..=u8::MAX {
            // Rotation covers both selector bytes completely while exercising
            // all four combinations of automatic and actor-authored clocks.
            let shape = raw;
            let color = raw.rotate_left(1);
            bus.write8(WRAM + u32::from(OBJECT) + 0x1CCB, shape);
            bus.write8(WRAM + u32::from(OBJECT) + 0x1CCA, color);
            for field in 0..0x26 {
                bus.write8(WRAM + u32::from(RECORD) + field, 0xA5);
            }
            call(
                &mut bus,
                0x7F1406,
                &Entry {
                    x: RECORD,
                    y: OBJECT,
                    dbr: 0x7E,
                    p: 0x20,
                    ..Default::default()
                },
            );
            for field in 0..0x26 {
                let expected = match field {
                    0x19 => selector(shape).resolve(clock),
                    0x1A => selector(color).resolve(clock),
                    _ => 0xA5,
                };
                assert_eq!(
                    bus.read8(WRAM + u32::from(RECORD) + field),
                    expected,
                    "clock={clock} shape={shape} color={color} field={field:X}"
                );
            }
        }
    }
}
