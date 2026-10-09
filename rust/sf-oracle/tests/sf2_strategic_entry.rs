//! The map program's entry (`$04:DCBC`) against the retail machine: retail's
//! state as the entry starts is decoded into a native `MapVisit`, which runs
//! the entry, and is compared with retail's state as the first program frame
//! begins. A tapped campaign enters the tutorial map, returns to it, loses
//! its planet and starts again, so every entry path it reaches is compared.

#[path = "support/sf2_strategic_snapshot.rs"]
mod strategic_snapshot;
#[path = "support/sf2_map_snapshot.rs"]
mod map_snapshot;

use map_snapshot::*;
use sf2_game::strategic_entry::EntryInputs;
use sf2_game::strategic_screen::ScreenOutput;
use sf2_game::strategic_sim::{PLACE_CAPACITY, UNIT_CAPACITY};
use sf2_game::strategic_visit::MapVisit;
use sf_oracle::RetailMachine;
use strategic_snapshot::Snapshot;

const ENTRY: u32 = 0x04DCBC;

fn visit(s: &Snapshot) -> MapVisit {
    MapVisit {
        screen: screen(s),
        director: director(s),
        map: s.map(),
        links: links(s),
        sprites: sprites(s),
        sprite_inputs: sprite_inputs(s),
        hud: hud(s),
        hud_inputs: hud_inputs(s),
        radio: radio(s),
        message_box: message_box(s),
        radio_inputs: radio_inputs(s),
        pad: pad(s),
        timers: sf2_game::strategic_visit::FrameTimers {
            ticks: s.byte(0xEFD0),
            countdowns: [s.byte(0xEFCD), s.byte(0xEFCE), s.byte(0xEFCF)],
            long_countdown: s.word(0xEFD1),
        },
        rng: sf2_game::RandomState::new([s.byte(0x00E0), s.byte(0x00E1), s.byte(0x00E2), s.byte(0x00E3)]),
        upload: s.word(0xDA77),
        terrain: s.terrain.clone(),
    }
}

fn compare(native: &MapVisit, expected: &MapVisit, context: &str) {
    for index in 0..PLACE_CAPACITY {
        assert_eq!(native.map.places[index], expected.map.places[index], "{context}: place {index}");
    }
    for index in 0..UNIT_CAPACITY {
        assert_eq!(native.map.units[index], expected.map.units[index], "{context}: unit {index}");
    }
    assert_eq!(native.map.globals, expected.map.globals, "{context}: globals");
    assert_eq!(
        (native.map.place_head, native.map.place_free, native.map.unit_head, native.map.unit_free),
        (expected.map.place_head, expected.map.place_free, expected.map.unit_head, expected.map.unit_free),
        "{context}: list heads"
    );
    assert_eq!(native.timers, expected.timers, "{context}: timers");
    assert_eq!(native.rng.bytes(), expected.rng.bytes(), "{context}: generator");
    assert_eq!(native.director, expected.director, "{context}: director");
    assert_eq!(native.screen, expected.screen, "{context}: screen");
    assert_eq!(native.links, expected.links, "{context}: links");
    assert_eq!(native.upload, expected.upload, "{context}: upload");
    assert_eq!(native.sprites, expected.sprites, "{context}: sprites");
    assert_eq!(native.sprite_inputs, expected.sprite_inputs, "{context}: sprite inputs");
    assert_eq!(native.hud, expected.hud, "{context}: hud");
    assert_eq!(native.hud_inputs, expected.hud_inputs, "{context}: hud inputs");
    assert_eq!(native.radio, expected.radio, "{context}: radio");
    assert_eq!(native.message_box, expected.message_box, "{context}: message box");
    assert!(native.terrain == expected.terrain, "{context}: terrain");
}

#[test]
fn map_entry_matches_retail_through_a_campaign_driven_by_taps() {
    let frames: u64 = std::env::var("SF2_ENTRY_FRAMES").map(|v| v.parse().unwrap()).unwrap_or(40_000);
    let schedule = |frame: u64| match frame % 32 {
        0..=3 => START,
        8..=9 => A,
        16..=19 => B,
        24..=27 if frame % 256 < 128 => RIGHT,
        24..=27 => DOWN,
        _ => 0,
    };
    let mut m = RetailMachine::new(rom());
    let mut entries = 0;
    while m.video_frame() < frames {
        let mut reached = false;
        for _ in 0..4000 {
            let pad = schedule(m.video_frame());
            if m.tick_until_cpu_execution(pad, ENTRY, 1).unwrap() {
                reached = true;
                break;
            }
            if m.video_frame() >= frames {
                break;
            }
        }
        if !reached {
            continue;
        }
        let before = Snapshot::take(&m);
        let mut held = 0;
        let mut started = false;
        for _ in 0..600 {
            held = schedule(m.video_frame());
            if m.tick_until_cpu_execution(held, PROGRAM, 1).unwrap() {
                started = true;
                break;
            }
        }
        assert!(started, "entry {entries}: no program frame");
        let _ = held;
        let after = Snapshot::take(&m);
        let context = format!("entry {entries} (1B86 {:04X})", before.word(0x1B86));
        let mut native = visit(&before);
        let inputs = EntryInputs {
            seed: before.word(0x1C02),
            tallies: [before.byte(0xD7E1), before.byte(0xD7E2), before.byte(0xD7E3)],
            generator_tail: before.byte(0x00E4),
        };
        native.enter(inputs, &mut ScreenOutput::default()).unwrap_or_else(|error| panic!("{context}: {error:x?}"));
        compare(&native, &visit(&after), &context);
        eprintln!("{context}: matched");
        entries += 1;
    }
    eprintln!("map entry matched {entries} entries");
    assert!(entries >= 3);
}

/// Run the original campaign setup on the oracle bus from this state.
fn run_setup(s: &Snapshot) -> Snapshot {
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
        bus.write8(strategic_snapshot::TERRAIN + offset as u32, byte);
    }
    let entry = sf_oracle::Entry { p: 0x20, dbr: 0x7E, ..Default::default() };
    let exit = sf_oracle::call(&mut bus, 0x04DEA9, &entry);
    assert!(exit.returned, "the original campaign setup did not return");
    Snapshot { low: (0..0x10000u32).map(|a| bus.read8(0x7E0000 + a)).collect(), terrain: s.terrain.clone(), gsu: s.gsu.clone() }
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

#[test]
fn campaign_setup_matches_the_original_on_mutated_retail_states() {
    let mut m = RetailMachine::new(rom());
    let schedule = |frame: u64| match frame % 32 {
        0..=3 => START,
        16..=19 => B,
        _ => 0,
    };
    let mut snapshots = Vec::new();
    while snapshots.len() < 3 && m.video_frame() < 20_000 {
        if m.tick_until_cpu_execution(schedule(m.video_frame()), ENTRY, 1).unwrap() {
            snapshots.push(Snapshot::take(&m));
            m.tick_video_frames(0, 30).unwrap();
        }
    }
    assert!(snapshots.len() >= 2);
    let cases: u32 = std::env::var("SF2_SETUP_FUZZ").map(|v| v.parse().unwrap()).unwrap_or(150);
    let mut rng = Lcg(0x04DE_A9AA);
    let (mut compared, mut faulted) = (0, std::collections::BTreeMap::new());
    for (index, snapshot) in snapshots.iter().enumerate() {
        for case in 0..cases {
            let mut state = Snapshot { low: snapshot.low.clone(), terrain: snapshot.terrain.clone(), gsu: snapshot.gsu.clone() };
            let results = rng.pick(&[0x0080u16, 0x0001, 0x0000, 0x0040, 0x0081, 0x0101, 0x0002]);
            set_word(&mut state, 0x1B86, results | (rng.below(2) as u16) << 8);
            set_word(&mut state, 0xD7F2, rng.below(3) as u16);
            set_word(&mut state, 0x1BA3, 2 * rng.below(3) as u16);
            set_word(&mut state, 0x1C02, rng.next() as u16);
            set_word(&mut state, 0x1C00, rng.next() as u16);
            for address in [0xD7E1u16, 0xD7E2, 0xD7E3, 0x00E0, 0x00E1, 0x00E2, 0x00E3, 0x00E4, 0xD9B6, 0xD9B7, 0xD9B8, 0xD9B9] {
                state.low[usize::from(address)] = rng.below(0x100) as u8;
            }
            for address in [0xD7E1u16, 0xD7E2, 0xD7E3] {
                if rng.below(2) == 0 {
                    state.low[usize::from(address)] = rng.below(0x18) as u8;
                }
            }
            let mut native = visit(&state);
            let mut inputs = EntryInputs {
                seed: state.word(0x1C02),
                tallies: [state.byte(0xD7E1), state.byte(0xD7E2), state.byte(0xD7E3)],
                generator_tail: state.byte(0x00E4),
            };
            if let Err(error) = native.set_up_campaign(&mut inputs) {
                *faulted.entry(format!("{error:x?}")).or_insert(0) += 1;
                continue;
            }
            let after = run_setup(&state);
            let context = format!("snapshot {index} case {case} (1B86 {:04X})", state.word(0x1B86));
            compare(&native, &visit(&after), &context);
            assert_eq!(inputs.generator_tail, after.byte(0x00E4), "{context}: generator tail");
            compared += 1;
        }
    }
    eprintln!("compared {compared} mutated campaign setups; native faults {faulted:?}");
    assert!(compared > 0);
}
