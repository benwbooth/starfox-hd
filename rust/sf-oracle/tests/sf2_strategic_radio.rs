//! The strategic map's radio (`$0B:9F87`) against the original routine on
//! the oracle bus, from retail states with the radio's inputs mutated. Each
//! chain feeds the original's result into the next call, so the scripts
//! run on through their pages, waits and answers.

#[path = "support/sf2_strategic_snapshot.rs"]
mod strategic_snapshot;
#[path = "support/sf2_map_snapshot.rs"]
mod map_snapshot;

use map_snapshot::*;
use sf2_game::strategic_radio::Radio;
use sf2_game::strategic_screen::{ScreenError, ScreenOutput};
use sf2_game::strategic_sim::{PLACE_CAPACITY, UNIT_CAPACITY};
use sf_oracle::RetailMachine;
use strategic_snapshot::{Snapshot, GSU_BYTES, TERRAIN};

const RADIO: u32 = 0x0B9F87;
const GSU_RAM: u32 = 0x700000;

fn compare(before: &Snapshot, after: &Snapshot, context: &str) -> Result<(), ScreenError> {
    let mut radio = radio(before);
    let mut message_box = message_box(before);
    let mut native_sprites = sprites(before);
    let mut native_screen = screen(before);
    let mut native_director = director(before);
    let mut map = before.map();
    let mut native_links = links(before);
    let mut output = ScreenOutput::default();
    Radio {
        radio: &mut radio,
        message_box: &mut message_box,
        sprites: &mut native_sprites,
        screen: &mut native_screen,
        director: &mut native_director,
        map: &mut map,
        links: &mut native_links,
        inputs: radio_inputs(before),
        output: &mut output,
    }
    .run()?;
    let expected_map = after.map();
    for index in 0..PLACE_CAPACITY {
        assert_eq!(map.places[index], expected_map.places[index], "{context}: place {index}");
    }
    for index in 0..UNIT_CAPACITY {
        assert_eq!(map.units[index], expected_map.units[index], "{context}: unit {index}");
    }
    assert_eq!(map.globals, expected_map.globals, "{context}: globals");
    assert_eq!(radio, map_snapshot::radio(after), "{context}: radio");
    assert_eq!(message_box, map_snapshot::message_box(after), "{context}: message box");
    assert_eq!(native_sprites, sprites(after), "{context}: sprites");
    assert_eq!(native_screen, screen(after), "{context}: screen");
    assert_eq!(native_director, director(after), "{context}: director");
    assert_eq!(native_links, links(after), "{context}: links");
    assert_eq!(output.cues, queued_cues(before, after), "{context}: cues");
    Ok(())
}

/// Run the original radio on the oracle bus from this state.
fn run_original(s: &Snapshot) -> Snapshot {
    let rom = rom();
    let runtime = rom[0x10000..0x17E00].to_vec();
    let mut bus = sf_oracle::SnesBus::new(rom);
    for (offset, byte) in runtime.into_iter().enumerate() {
        bus.write8(0x7F0000 + offset as u32, byte);
    }
    for (offset, &byte) in s.low.iter().enumerate() {
        bus.write8(0x7E0000 + offset as u32, byte);
    }
    for (offset, &byte) in s.terrain.iter().enumerate() {
        bus.write8(TERRAIN + offset as u32, byte);
    }
    for (offset, &byte) in s.gsu.iter().enumerate() {
        bus.write8(GSU_RAM + offset as u32, byte);
    }
    let exit = sf_oracle::call(&mut bus, RADIO, &sf_oracle::Entry::default());
    assert!(exit.returned, "the original radio did not return");
    Snapshot {
        low: (0..0x10000u32).map(|a| bus.read8(0x7E0000 + a)).collect(),
        terrain: s.terrain.clone(),
        gsu: (0..GSU_BYTES as u32).map(|a| bus.read8(GSU_RAM + a)).collect(),
    }
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

fn set_word(s: &mut Snapshot, address: u16, value: u16) {
    s.low[usize::from(address)] = value as u8;
    s.low[usize::from(address.wrapping_add(1))] = (value >> 8) as u8;
}

fn set_gsu_word(s: &mut Snapshot, address: usize, value: u16) {
    s.gsu[address] = value as u8;
    s.gsu[address + 1] = (value >> 8) as u8;
}

fn toggle(s: &mut Snapshot, address: u16, bit: u16) {
    let value = s.word(address) ^ bit;
    set_word(s, address, value);
}

fn mutate(s: &mut Snapshot, rng: &mut Lcg) {
    for _ in 0..rng.below(4) {
        match rng.below(16) {
            0 => toggle(s, 0x1B88, 0x0100),
            1 => set_gsu_word(s, 0x37C, rng.pick(&[0u16, 0, 0xFFFF])),
            2 => set_word(s, 0xF594, rng.pick(&[0u16, 0, 0xA642, 0xA609])),
            3 => set_word(s, 0xF582, rng.pick(&[0u16, 0xA609, 0xA630, 0xA642, 0xA6BB, 0xA6CA, 0xA6E3, 0xA6EA, 0xA712, 0xA732, 0xA786])),
            4 => toggle(s, 0xF55C, 1 << rng.below(15)),
            5 => set_word(s, 0x1E84, rng.pick(&[0u16, 0x6E, 0x73, 0xD1, 0x57])),
            6 => toggle(s, 0xDB33, 1 << rng.below(12)),
            7 => {
                toggle(s, 0xDB2D, 0x0001);
                set_word(s, 0xDB2F, rng.below(12) as u16);
                set_word(s, 0xDB31, rng.pick(&[0u16, 0x0F, 0x03]));
            }
            8 => toggle(s, 0xDA8D, 0x0040),
            9 => set_word(s, 0x1292, rng.pick(&[0u16, 0, 0x0100, 0x0800, 0x8000, 0x0080])),
            10 => toggle(s, 0x1C21, rng.pick(&[0x1000u16, 0x8000, 0x0080])),
            11 => set_word(s, 0xF56A, rng.pick(&[0u16, 0x4000, 0x8000, 0xC000])),
            12 => {
                let message = rng.pick(&[0u16, 0x5D, 0x6E, 0x73, 0xFF, 0xFFFF]);
                let end = if rng.below(3) == 0 { 0x8000 } else { 0 };
                set_gsu_word(s, 0x388, message | end);
            }
            13 => set_word(s, 0xF584, rng.below(0x40) as u16),
            14 => set_word(s, 0xDA0D, rng.pick(&[0u16, 0x60, 0x62])),
            _ => {
                set_word(s, 0xF59A, rng.pick(&[0xFFFFu16, 0, 3, 9]));
                set_gsu_word(s, 0x392, rng.below(7) as u16);
                set_word(s, 0xF578, rng.pick(&[0u16, 0x5D, 0x73]));
                set_word(s, 0xF576, rng.below(2) as u16);
            }
        }
    }
}

#[test]
fn map_radio_matches_the_original_on_mutated_retail_states() {
    let mut m = RetailMachine::new(rom());
    navigate_to_map(&mut m);
    let mut snapshots = Vec::new();
    for call in 0..1200u32 {
        let pad = match call % 16 {
            0 => START,
            8 => B,
            _ => 0,
        };
        if !m.tick_until_cpu_execution(pad, RADIO, 600).unwrap() {
            break;
        }
        if call % 100 == 0 {
            snapshots.push(Snapshot::take(&m));
        }
    }
    assert!(snapshots.len() > 5);
    let chains: u32 = std::env::var("SF2_RADIO_FUZZ").map(|v| v.parse().unwrap()).unwrap_or(40);
    let mut rng = Lcg(0x0B9F_87AA);
    let (mut compared, mut faulted) = (0, std::collections::BTreeMap::new());
    let mut scripts = std::collections::BTreeSet::new();
    for (index, snapshot) in snapshots.iter().enumerate() {
        for chain in 0..chains {
            let mut state = Snapshot { low: snapshot.low.clone(), terrain: snapshot.terrain.clone(), gsu: snapshot.gsu.clone() };
            for link in 0..24 {
                mutate(&mut state, &mut rng);
                let after = run_original(&state);
                let context = format!("snapshot {index} chain {chain} call {link}");
                if let Err(error) = compare(&state, &after, &context) {
                    *faulted.entry(format!("{error:x?}")).or_insert(0) += 1;
                    break;
                }
                scripts.insert(after.word(0xF582));
                compared += 1;
                state = after;
            }
        }
    }
    eprintln!("compared {compared} mutated radio calls; scripts {scripts:04X?}; native faults {faulted:?}");
    assert!(compared > 0);
}
