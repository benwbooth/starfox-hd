//! The real ENDSEQ intro exit, entered using controller input only.
//! Compare its transfer loop with the shipping shell; boot wall-clock and
//! intro actor scheduling are separate, still-open timing contracts.

#[path = "../examples/support/mod.rs"]
mod support;

use sf_core::pad;
use sf_game::shell::{GameState, Shell};
use sf_oracle::{
    load_retail_rom, RetailMachine, SnesBus, RETAIL_FADE, RETAIL_FADEDIR, RETAIL_GAMEFRAME,
    RETAIL_XINIDISP1,
};
use sha2::{Digest, Sha256};
use w65c816::{AddressType, Signals, System, CPU};

const WORK_RAM: u32 = 0x7E_0000;
const INTRO_EXIT_TRANSFER: u32 = 0x1F_F586;
const INTRO_EXIT_TRANSFER_DONE: u32 = 0x1F_F58E;
const INTRO_EXIT_RETURN: u32 = 0x1F_F595;
const MAX_BOOT_VIDEO_FRAMES: u32 = 600;
const MAX_TRANSFER_VIDEO_FRAMES: u32 = 12;
const INTRO_EXIT_GATE: u32 = 0x1F_F568;
const INTRO_REPEAT: u32 = 0x1F_F55E;
const INTRO_EXIT_REQUEST: u32 = 0x1FFC;
const CONTROLLER_HIGH: u32 = 0x1202;
const CONTROLLER_LOW: u32 = 0x1204;

struct OriginalGate {
    bus: SnesBus,
    reset: bool,
}

impl System for OriginalGate {
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
fn original_intro_gate_and_shipping_shell_use_the_low_frame_byte_before_both_exit_requests() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("skip: no retail Star Fox 1 ROM");
        return;
    };
    assert_eq!(
        format!("{:x}", Sha256::digest(&rom)),
        "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548"
    );
    let mut original = OriginalGate {
        bus: SnesBus::new(rom),
        reset: false,
    };
    // Bootstrap into the unchanged ENDSEQ prefix. Stop only at its two real
    // successors, before either transfer is called; no source code is patched.
    for (offset, byte) in [
        0x18,
        0xFB,
        0xE2,
        0x20,
        0xC2,
        0x10,
        0x5C,
        INTRO_EXIT_GATE as u8,
        (INTRO_EXIT_GATE >> 8) as u8,
        (INTRO_EXIT_GATE >> 16) as u8,
    ]
    .into_iter()
    .enumerate()
    {
        original.bus.write8(0x200 + offset as u32, byte);
    }
    for high in [0, 1, 127, 255] {
        for low in 0..=u8::MAX {
            let frame = u16::from_le_bytes([low, high]);
            for held in [0, 1, 0x0080, 0x0100, 0x8000, 0xFFFF] {
                for scripted in [0, 1, 0x80] {
                    original.bus.write16(WORK_RAM | RETAIL_GAMEFRAME, frame);
                    original
                        .bus
                        .write8(WORK_RAM | CONTROLLER_HIGH, (held >> 8) as u8);
                    original.bus.write8(WORK_RAM | CONTROLLER_LOW, held as u8);
                    original.bus.write8(WORK_RAM | INTRO_EXIT_REQUEST, scripted);
                    original.bus.write8(WORK_RAM | RETAIL_FADE, 7);
                    original.bus.write8(WORK_RAM | RETAIL_FADEDIR, 1);
                    original.reset = true;
                    let mut cpu = CPU::new();
                    let mut successor = None;
                    for _ in 0..200 {
                        cpu.cycle(&mut original);
                        if cpu.tcu() == 0 {
                            let address =
                                (u32::from(cpu.pbr()) << 16) | u32::from(cpu.pc().wrapping_sub(1));
                            if [INTRO_REPEAT, INTRO_EXIT_TRANSFER].contains(&address) {
                                successor = Some(address);
                                break;
                            }
                        }
                    }
                    let source_exits = successor.expect("intro gate must reach a successor")
                        == INTRO_EXIT_TRANSFER;
                    let mut native = Shell::new();
                    while native.state() == GameState::Boot {
                        native.tick(0);
                    }
                    native.tick(0); // complete real presentation-map initialization
                    native.game.vars.gameframe = frame.wrapping_sub(1);
                    native.game.vars.strategy.intro_exit_requested = scripted != 0;
                    native.tick(held);
                    assert_eq!(native.game.vars.gameframe, frame);
                    assert_eq!(native.state(), GameState::AttractIntro);
                    assert_eq!(
                        native.game.vars.strategy.fade_direction == -2,
                        source_exits,
                        "frame={frame} held={held} scripted={scripted}"
                    );
                    assert_eq!(
                        original.bus.read8(WORK_RAM | RETAIL_FADE),
                        if source_exits { 11 } else { 7 }
                    );
                    assert_eq!(
                        original.bus.read8(WORK_RAM | RETAIL_FADEDIR),
                        if source_exits { (-2i8) as u8 } else { 1 }
                    );
                    assert_eq!(original.bus.read16(WORK_RAM | RETAIL_GAMEFRAME), frame);
                    assert_eq!(original.bus.read8(WORK_RAM | INTRO_EXIT_REQUEST), scripted);
                }
            }
        }
    }
}

#[test]
fn original_intro_exit_and_shipping_shell_publish_six_transfers_then_handoff() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("skip: no retail Star Fox 1 ROM");
        return;
    };
    assert_eq!(
        format!("{:x}", Sha256::digest(&rom)),
        "82e39dfbb3e4fe5c28044e80878392070c618b298dd5a267e5ea53c8f72cc548"
    );
    let mut original = RetailMachine::new(rom);
    assert!(original
        .tick_until_cpu_execution(pad::A, INTRO_EXIT_TRANSFER, MAX_BOOT_VIDEO_FRAMES)
        .unwrap());
    let mut native = support::configured_shell();
    for _ in 0..MAX_BOOT_VIDEO_FRAMES {
        native.tick(pad::A);
        if native.game.vars.strategy.fade_direction == -2 {
            break;
        }
    }
    assert_eq!(native.state(), GameState::AttractIntro);
    assert_eq!(native.game.vars.strategy.fade_direction, -2);
    assert_eq!(original.peek8(WORK_RAM | RETAIL_FADE), 11);
    assert_eq!(original.peek8(WORK_RAM | RETAIL_FADEDIR) as i8, -2);
    assert_eq!(
        native.game.vars.gameframe,
        original.peek16(WORK_RAM | RETAIL_GAMEFRAME)
    );
    let display = original.peek8(RETAIL_XINIDISP1);
    assert_eq!(native.frame().display_brightness, display & 15);
    assert_eq!(native.frame().display_forced_blank, display & 0x80 != 0);

    for expected_level in [9, 7, 5, 3, 1, 0] {
        assert!(original
            .tick_until_cpu_execution(0, INTRO_EXIT_TRANSFER_DONE, MAX_TRANSFER_VIDEO_FRAMES)
            .unwrap());
        native.tick(0);
        assert_eq!(
            native.game.vars.gameframe,
            original.peek16(WORK_RAM | RETAIL_GAMEFRAME)
        );
        assert_eq!(original.peek8(WORK_RAM | RETAIL_FADE), expected_level);
        let display = original.peek8(RETAIL_XINIDISP1);
        assert_eq!(native.frame().display_brightness, display & 15);
        assert_eq!(native.frame().display_forced_blank, display & 0x80 != 0);
        assert_eq!(
            native.game.vars.strategy.fade_direction as u8,
            original.peek8(WORK_RAM | RETAIL_FADEDIR),
        );
        let next = original
            .tick_until_cpu_execution_any(
                0,
                &[INTRO_EXIT_TRANSFER, INTRO_EXIT_RETURN],
                MAX_TRANSFER_VIDEO_FRAMES,
            )
            .unwrap()
            .expect("intro exit must repeat or return");
        if expected_level == 0 {
            assert_eq!(next, INTRO_EXIT_RETURN);
            assert_eq!(native.state(), GameState::Title);
        } else {
            assert_eq!(next, INTRO_EXIT_TRANSFER);
            assert_eq!(native.state(), GameState::AttractIntro);
        }
    }
}
