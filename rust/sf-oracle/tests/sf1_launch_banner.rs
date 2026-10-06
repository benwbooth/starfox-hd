//! Execute the unchanged retail launch-warning decision and countdown.
//! The reconstructed SPRITES source has a different blink branch, so the
//! pinned cartridge is the authority for this externally visible behavior.

use sf_core::stage_banner::ScrambleBannerState;
use sf_oracle::{load_retail_rom, SnesBus, RETAIL_GAMEFRAME, RETAIL_SCRAMBLE_COUNT};
use sha2::{Digest, Sha256};
use w65c816::{AddressType, Signals, System, CPU};

const WRAM: u32 = 0x7E_0000;
const WARNING_ENTRY: u32 = 0x03_DD34;
const FORMAT_WARNING: u32 = 0x03_DD4C;
const RETURN_WITHOUT_WARNING: u32 = 0x03_DD7E;

struct Source {
    bus: SnesBus,
    reset: bool,
}

impl System for Source {
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

impl Source {
    fn new() -> Self {
        let rom = load_retail_rom().expect("retail Rev 2 ROM is required");
        assert_eq!(
            format!("{:x}", Sha256::digest(&rom)),
            "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548"
        );
        let mut bus = SnesBus::new(rom);
        // Bootstrap only: native mode, word indexes, then the original entry.
        for (offset, byte) in [
            0x18,
            0xFB,
            0xC2,
            0x30,
            0x5C,
            WARNING_ENTRY as u8,
            (WARNING_ENTRY >> 8) as u8,
            (WARNING_ENTRY >> 16) as u8,
        ]
        .into_iter()
        .enumerate()
        {
            bus.write8(0x200 + offset as u32, byte);
        }
        Self { bus, reset: false }
    }

    fn compare(&mut self, count: u8, frame: u16) {
        self.bus.write8(WRAM | RETAIL_SCRAMBLE_COUNT, count);
        self.bus.write16(WRAM | RETAIL_GAMEFRAME, frame);
        self.reset = true;
        let mut cpu = CPU::new();
        for _ in 0..200 {
            cpu.cycle(self);
            if cpu.tcu() != 0 {
                continue;
            }
            let boundary = u32::from(cpu.pbr()) << 16 | u32::from(cpu.pc().wrapping_sub(1));
            if ![FORMAT_WARNING, RETURN_WITHOUT_WARNING].contains(&boundary) {
                continue;
            }
            assert_eq!(
                ScrambleBannerState {
                    ticks_remaining: count,
                    game_frame: frame
                }
                .is_visible(),
                boundary == FORMAT_WARNING,
                "count={count} frame={frame}",
            );
            assert_eq!(
                self.bus.read8(WRAM | RETAIL_SCRAMBLE_COUNT),
                count.saturating_sub(1)
            );
            assert_eq!(self.bus.read16(WRAM | RETAIL_GAMEFRAME), frame);
            return;
        }
        panic!("original warning decision did not return: count={count} frame={frame}");
    }
}

#[test]
fn warning_visibility_and_counter_match_every_byte_and_frame_word() {
    let mut source = Source::new();
    for count in 0..=u8::MAX {
        for phase in 0..16 {
            source.compare(count, phase);
        }
    }
    for frame in 0..=u16::MAX {
        source.compare(50, frame);
    }
}
