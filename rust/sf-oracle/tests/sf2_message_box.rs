//! The map's message box step in a Super FX picture (`$0B:8234`) against the
//! original GSU job on the oracle bus, from retail GSU states with the box's
//! words mutated across every message.

#[path = "support/sf2_strategic_snapshot.rs"]
mod strategic_snapshot;
#[path = "support/sf2_map_snapshot.rs"]
mod map_snapshot;

use map_snapshot::*;
use sf2_game::strategic_radio::{self, MessageBox};
use sf_oracle::RetailMachine;

const RADIO: u32 = 0x0B9F87;
const GSU_RAM: u32 = 0x700000;

struct GsuState {
    ram: Vec<u8>,
    screen_base: u8,
    screen_mode: u8,
}

fn word(ram: &[u8], address: usize) -> u16 {
    u16::from_le_bytes([ram[address], ram[address + 1]])
}

fn set_word(ram: &mut [u8], address: usize, value: u16) {
    ram[address..address + 2].copy_from_slice(&value.to_le_bytes());
}

fn message_box_of(ram: &[u8]) -> MessageBox {
    let w = |a| word(ram, a);
    MessageBox {
        busy: w(0x37C),
        size: w(0x36C),
        style: w(0x378),
        open: w(0x37A),
        left_pilot: w(0x382),
        right_pilot: w(0x386),
        message: w(0x388),
        progress: w(0x38A),
        duration: w(0x390),
        remaining: w(0x380),
        row: w(0x38E),
        portrait: w(0x392),
        height: w(0x356),
    }
}

/// Run the original job on the oracle bus from this GSU state.
fn run_original(state: &GsuState) -> Vec<u8> {
    let mut bus = sf_oracle::SnesBus::new(rom());
    bus.enable_gsu();
    for (offset, &byte) in state.ram.iter().enumerate() {
        bus.write8(GSU_RAM + offset as u32, byte);
    }
    bus.write8(0x003038, state.screen_base);
    // The GSU owns the ROM and RAM buses while the job runs.
    bus.write8(0x00303A, state.screen_mode | 0x18);
    bus.write8(0x003034, 0x0B);
    bus.write8(0x00301E, 0x34);
    bus.write8(0x00301F, 0x82);
    for _ in 0..10_000 {
        if !bus.gsu_ref().is_some_and(|gsu| gsu.is_running()) {
            break;
        }
        bus.tick_gsu(100_000);
    }
    assert!(!bus.gsu_ref().unwrap().is_running(), "the original message box job did not stop");
    (0..0x10000u32).map(|a| bus.read8(GSU_RAM + a)).collect()
}

struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        self.0 >> 8
    }
    fn below(&mut self, limit: u32) -> u32 {
        self.next() % limit
    }
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u32) as usize]
    }
}

fn mutate(ram: &mut [u8], rng: &mut Lcg, message: u16) {
    let any = [rng.below(0x90) as u16, rng.below(0x100) as u16, rng.below(0x100) as u16, rng.below(0x80) as u16];
    set_word(ram, 0x388, message);
    set_word(ram, 0x38A, rng.pick(&[0u16, 0, 1, 0x10, any[0]]));
    set_word(ram, 0x390, rng.pick(&[0u16, 0x21, 0x7F, any[1]]));
    set_word(ram, 0x380, rng.pick(&[0u16, 1, any[2]]));
    set_word(ram, 0x37A, rng.pick(&[0u16, 1, 0xFFFF]));
    set_word(ram, 0x378, rng.below(8) as u16);
    if rng.below(4) == 0 {
        set_word(ram, 0x36C, rng.pick(&[0u16, 0x64, 0x7F, any[3]]));
        set_word(ram, 0x37C, rng.pick(&[0u16, 0xFFFF]));
        set_word(ram, 0x392, rng.below(7) as u16);
    }
}

#[test]
fn map_message_box_matches_the_original_gsu_job_for_every_message() {
    let mut m = RetailMachine::new(rom());
    navigate_to_map(&mut m);
    let mut states = Vec::new();
    for call in 0..600u32 {
        let pad = if call % 16 == 0 { START } else { 0 };
        if !m.tick_until_cpu_execution(pad, RADIO, 600).unwrap() {
            break;
        }
        if call % 150 == 0 {
            let (screen_base, screen_mode, ..) = m.gsu_screen_state().expect("the map runs the Super FX");
            states.push(GsuState { ram: (0..0x10000).map(|a| m.peek_gsu_ram(a)).collect(), screen_base, screen_mode });
        }
    }
    assert!(states.len() >= 3);
    let rounds: u32 = std::env::var("SF2_MESSAGE_BOX_ROUNDS").map(|v| v.parse().unwrap()).unwrap_or(3);
    let mut rng = Lcg(0x0B82_34AA);
    let mut compared = 0;
    let mut heights = std::collections::BTreeSet::new();
    for round in 0..rounds {
        for (index, state) in states.iter().enumerate() {
            let messages = (0..216u16).chain([0x00FF, 0xFFFF, 0x8005, 0x0000]);
            for message in messages {
                let mut case = GsuState { ram: state.ram.clone(), ..*state };
                mutate(&mut case.ram, &mut rng, message);
                let mut native = message_box_of(&case.ram);
                strategic_radio::picture(&mut native).unwrap_or_else(|error| {
                    panic!("round {round} state {index} message {message:04X}: native faulted: {error:x?}")
                });
                let after = run_original(&case);
                let expected = message_box_of(&after);
                assert_eq!(native, expected, "round {round} state {index} message {message:04X}");
                heights.insert(expected.height);
                compared += 1;
            }
        }
    }
    eprintln!("compared {compared} message box steps; heights {heights:?}");
}
