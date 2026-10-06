//! Execute the unchanged retail display fade at every supported level,
//! direction byte, frame parity and representative prior display state.

use sf_core::display_fade::{DisplayFade, FULL_BRIGHTNESS};
use sf_oracle::{
    load_retail_rom, SnesBus, RETAIL_FADE, RETAIL_FADEDIR, RETAIL_GAMEFRAME, RETAIL_XINIDISP1,
};
use sha2::{Digest, Sha256};
use w65c816::{AddressType, Signals, System, CPU};

const WORK_RAM: u32 = 0x7E_0000;
const FADE_ENTRY: u32 = 0x02_8C18;
const FADE_RETURN: u32 = 0x02_8CA1;
const SLOW_SKIP_RETURN: u32 = 0x02_8C60;

struct Original {
    bus: SnesBus,
    reset: bool,
}

impl System for Original {
    fn read(&mut self, address: u32, kind: AddressType, signals: &Signals) -> u8 {
        self.bus.read(address, kind, signals)
    }
    fn write(&mut self, address: u32, value: u8, kind: AddressType, signals: &Signals) {
        self.bus.write(address, value, kind, signals);
    }
    fn res(&mut self) -> bool {
        std::mem::take(&mut self.reset)
    }
}

#[test]
fn original_display_fade_preserves_level_direction_and_publication() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("skip: no retail Star Fox 1 ROM");
        return;
    };
    assert_eq!(
        format!("{:x}", Sha256::digest(&rom)),
        "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548"
    );
    let mut source = Original {
        bus: SnesBus::new(rom),
        reset: false,
    };
    // Bootstrap only. Original instructions and their return paths remain intact.
    for (offset, byte) in [
        0x18,
        0xFB,
        0xE2,
        0x20,
        0xC2,
        0x10,
        0x5C,
        FADE_ENTRY as u8,
        (FADE_ENTRY >> 8) as u8,
        (FADE_ENTRY >> 16) as u8,
    ]
    .into_iter()
    .enumerate()
    {
        source.bus.write8(0x200 + offset as u32, byte);
    }

    for direction in i8::MIN..=i8::MAX {
        for level in 0..=FULL_BRIGHTNESS {
            for game_frame in [0, 1, 65_534, 65_535] {
                for prior_display in [0, 6, 15, 0x80] {
                    let mut fade = DisplayFade {
                        level,
                        brightness: prior_display & 15,
                        forced_blank: prior_display & 0x80 != 0,
                    };
                    let mut native_direction = direction;
                    fade.advance(&mut native_direction, game_frame);
                    source
                        .bus
                        .write8(WORK_RAM | RETAIL_FADEDIR, direction as u8);
                    source.bus.write8(WORK_RAM | RETAIL_FADE, level);
                    source.bus.write16(WORK_RAM | RETAIL_GAMEFRAME, game_frame);
                    for output in [RETAIL_XINIDISP1, RETAIL_XINIDISP1 + 2, RETAIL_XINIDISP1 + 4] {
                        source.bus.write8(output, prior_display);
                    }
                    source.reset = true;
                    let mut cpu = CPU::new();
                    let mut returned = false;
                    for _ in 0..300 {
                        cpu.cycle(&mut source);
                        if cpu.tcu() != 0 {
                            continue;
                        }
                        let pc = u32::from(cpu.pbr()) << 16 | u32::from(cpu.pc().wrapping_sub(1));
                        if [FADE_RETURN, SLOW_SKIP_RETURN].contains(&pc) {
                            returned = true;
                            break;
                        }
                    }
                    assert!(returned, "original fade did not return");
                    assert_eq!(
                        source.bus.read8(WORK_RAM | RETAIL_FADEDIR),
                        native_direction as u8,
                        "direction={direction} level={level} frame={game_frame}"
                    );
                    assert_eq!(source.bus.read8(WORK_RAM | RETAIL_FADE), fade.level);
                    for output in [RETAIL_XINIDISP1, RETAIL_XINIDISP1 + 2, RETAIL_XINIDISP1 + 4] {
                        assert_eq!(source.bus.read8(output), fade.brightness | if fade.forced_blank { 0x80 } else { 0 }, "display: direction={direction} level={level} frame={game_frame} prior={prior_display}");
                    }
                    assert_eq!(source.bus.read16(WORK_RAM | RETAIL_GAMEFRAME), game_frame);
                }
            }
        }
    }
}
