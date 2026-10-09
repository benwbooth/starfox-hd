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
use sf2_game::strategic_exit::ExitInputs;
use sf2_game::strategic_screen::MapPad;
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
        launch: launch(s),
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

/// The parts of two debug renderings that differ, for failure messages.
fn differences<T: std::fmt::Debug>(native: &T, retail: &T) -> String {
    let (a, b) = (format!("{native:?}"), format!("{retail:?}"));
    let a: Vec<&str> = a.split(", ").collect();
    let b: Vec<&str> = b.split(", ").collect();
    a.iter().zip(&b).filter(|(x, y)| x != y).map(|(x, y)| format!("native {x} retail {y}")).collect::<Vec<_>>().join("; ")
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
    assert!(native.screen == expected.screen, "{context}: screen {}", differences(&native.screen, &expected.screen));
    assert!(native.links == expected.links, "{context}: links {}", differences(&native.links, &expected.links));
    assert_eq!(native.upload, expected.upload, "{context}: upload");
    assert!(native.sprites == expected.sprites, "{context}: sprites {}", differences(&native.sprites, &expected.sprites));
    assert_eq!(native.sprite_inputs, expected.sprite_inputs, "{context}: sprite inputs");
    assert_eq!(native.hud, expected.hud, "{context}: hud");
    assert_eq!(native.hud_inputs, expected.hud_inputs, "{context}: hud inputs");
    assert_eq!(native.radio, expected.radio, "{context}: radio");
    assert_eq!(native.message_box, expected.message_box, "{context}: message box");
    assert!(native.launch == expected.launch, "{context}: launch {}", differences(&native.launch, &expected.launch));
    assert!(native.terrain == expected.terrain, "{context}: terrain");
}

/// Each map entry of a campaign driven by `schedule`, up to `frames`.
fn compare_entries(schedule: &dyn Fn(u64) -> u16, frames: u64) -> u32 {
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
    entries
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
    let entries = compare_entries(&schedule, frames);
    eprintln!("map entry matched {entries} entries");
    assert!(entries >= 3);
}

#[test]
fn map_entry_back_from_the_star_wolf_stage_matches_retail() {
    // The Star Wolf route; Start through the stage's end and its results.
    let schedule = |frame: u64| match frame {
        600..4600 if (frame - 600) % 200 < 6 => START,
        4660..4701 => RIGHT,
        4701..4721 => UP,
        4721..4727 => B,
        _ if frame > 9000 && frame % 64 < 4 => START,
        _ => 0,
    };
    let entries = compare_entries(&schedule, 12_000);
    assert!(entries >= 3, "the return from the stage was not reached");
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
            // Bit 2 (back from a stage) waits on the sound processor in the
            // original; the retail return from the Star Wolf stage covers it.
            let results = rng.pick(&[0x0080u16, 0x0001, 0x0000, 0x0040, 0x0081, 0x0101]);
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

const EXIT_DISPATCH: u32 = 0x04AEE1;
const EPILOGUE_RETURN: u32 = 0x04B1BF;

/// Each exit of a campaign driven by `schedule`, up to `frames`: retail's
/// state at the exit's dispatch and when the map program returns, with the
/// sprite passes its fade ran.
fn compare_exits(schedule: &dyn Fn(u64) -> u16, frames: u64) -> u32 {
    let mut m = RetailMachine::new(rom());
    let mut exits = 0;
    while m.video_frame() < frames {
        let pad = schedule(m.video_frame());
        if !m.tick_until_cpu_execution(pad, EXIT_DISPATCH, 1).unwrap() || m.peek8(0x7E1B74) == 0 {
            continue;
        }
        let before = Snapshot::take(&m);
        let mut passes = 0u16;
        loop {
            let pad = schedule(m.video_frame());
            match m.tick_until_cpu_execution_any(pad, &[SPRITE_PASS, EPILOGUE_RETURN], 1).unwrap() {
                Some(SPRITE_PASS) => passes += 1,
                Some(_) => break,
                None => assert!(m.video_frame() < frames + 2000, "exit {exits}: the map never left"),
            }
        }
        let after = Snapshot::take(&m);
        let context = format!("exit {exits} (1B74 {:02X})", before.byte(0x1B74));
        let mut native = visit(&before);
        let inputs = ExitInputs {
            dark_after: passes,
            pad: MapPad { held: after.word(0x1C1F), pressed: after.word(0x1C21) },
            generator_tail: before.byte(0x00E4),
        };
        let mut output = ScreenOutput::default();
        native.leave(inputs, &mut output).unwrap_or_else(|error| panic!("{context}: {error:x?}"));
        compare(&native, &visit(&after), &context);
        assert_eq!(output.cues, queued_cues(&before, &after), "{context}: cues");
        eprintln!("{context}: matched after {passes} fade passes");
        exits += 1;
    }
    exits
}

#[test]
fn map_exits_match_retail_through_a_campaign_driven_by_taps() {
    let frames: u64 = std::env::var("SF2_EXIT_FRAMES").map(|v| v.parse().unwrap()).unwrap_or(25_000);
    let schedule = |frame: u64| match frame % 32 {
        0..=3 => START,
        8..=9 => A,
        16..=19 => B,
        24..=27 if frame % 256 < 128 => RIGHT,
        24..=27 => DOWN,
        _ => 0,
    };
    let exits = compare_exits(&schedule, frames);
    eprintln!("map exits matched {exits}");
    assert!(exits >= 2);
}

#[test]
fn map_exit_for_the_star_wolf_interception_matches_retail() {
    // The Star Wolf oracle's route: Leon intercepts the ship.
    let schedule = |frame: u64| match frame {
        600..4600 if (frame - 600) % 200 < 6 => START,
        4660..4701 => RIGHT,
        4701..4721 => UP,
        4721..4727 => B,
        _ => 0,
    };
    let exits = compare_exits(&schedule, 7_000);
    assert!(exits >= 2, "the interception's exit was not reached");
}

/// The exits by 1B74 (`$04:AEE4`).
const EXITS: [u32; 5] = [0x04AF56, 0x04AF9A, 0x04AF74, 0x04AF1B, 0x04AEF0];

/// Run the original exit for 1B74 on the oracle bus, stopped where it
/// fades (`$04:B121` and `$04:B14D` return).
fn run_exit(s: &Snapshot) -> Snapshot {
    let mut rom = rom();
    for address in [0xB121usize, 0xB14D] {
        rom[0x20000 + address - 0x8000] = 0x6B;
    }
    let runtime = rom[0x10000..0x17E00].to_vec();
    let mut bus = sf_oracle::SnesBus::new(rom);
    for (offset, byte) in runtime.into_iter().enumerate() {
        bus.write8(0x7F0000 + offset as u32, byte);
    }
    for (offset, &byte) in s.low.iter().enumerate() {
        bus.write8(0x7E0000 + offset as u32, byte);
    }
    let entry = sf_oracle::Entry { p: 0x20, dbr: 0x7E, ..Default::default() };
    let exit = sf_oracle::call(&mut bus, EXITS[usize::from(s.byte(0x1B74)) - 1], &entry);
    assert!(exit.returned, "the original exit did not return");
    Snapshot { low: (0..0x10000u32).map(|a| bus.read8(0x7E0000 + a)).collect(), terrain: s.terrain.clone(), gsu: s.gsu.clone() }
}

fn list(s: &Snapshot, head: u16) -> Vec<u16> {
    let mut out = Vec::new();
    let mut pointer = s.word(head);
    while pointer != 0 && out.len() < 32 {
        out.push(pointer);
        pointer = s.word(pointer);
    }
    out
}

fn toggle(s: &mut Snapshot, address: u16, bit: u16) {
    let value = s.word(address) ^ bit;
    set_word(s, address, value);
}

#[test]
fn map_exits_match_the_original_on_mutated_retail_states() {
    let schedule = |frame: u64| match frame {
        600..4600 if (frame - 600) % 200 < 6 => START,
        4660..4701 => RIGHT,
        4701..4721 => UP,
        _ if frame >= 4721 && frame % 64 < 4 => B,
        _ => 0,
    };
    let mut m = RetailMachine::new(rom());
    let mut snapshots = Vec::new();
    while snapshots.len() < 6 && m.video_frame() < 9_000 {
        if m.tick_until_cpu_execution(schedule(m.video_frame()), PROGRAM, 1).unwrap() && m.video_frame() > 4_000 {
            snapshots.push(Snapshot::take(&m));
            m.tick_video_frames(0, 150).unwrap();
        }
    }
    assert!(snapshots.len() >= 3);
    let cases: u32 = std::env::var("SF2_EXIT_FUZZ").map(|v| v.parse().unwrap()).unwrap_or(120);
    let mut rng = Lcg(0x04AE_E4AA);
    let (mut compared, mut faulted) = (0, std::collections::BTreeMap::new());
    for (index, snapshot) in snapshots.iter().enumerate() {
        for case in 0..cases {
            let mut state = Snapshot { low: snapshot.low.clone(), terrain: snapshot.terrain.clone(), gsu: snapshot.gsu.clone() };
            let places = list(&state, 0xDB67);
            let units = list(&state, 0xE0A3);
            state.low[0x1B74] = 1 + rng.below(5) as u8;
            state.low[0x1B75] = 0;
            if rng.below(4) == 0 {
                toggle(&mut state, 0x1B86, 0x0020);
            }
            toggle(&mut state, 0x1B88, if rng.below(2) == 0 { 0x0004 } else { 0 });
            set_word(&mut state, 0xD7F2, rng.below(3) as u16);
            state.low[0x00E4] = rng.below(0x100) as u8;
            for k in 0..6u16 {
                state.low[usize::from(0xD794 + k)] = if rng.below(2) == 0 { 0 } else { rng.below(0x40) as u8 };
            }
            if !places.is_empty() {
                set_word(&mut state, 0xDA6B, rng.pick(&places));
                let place = rng.pick(&places);
                set_word(&mut state, 0xDB07, place);
                set_word(&mut state, place + 0x04, rng.below(0x0B) as u16);
                set_word(&mut state, place + 0x0A, rng.below(0x100) as u16);
                toggle(&mut state, place + 0x1E, 0x0002);
            }
            if !units.is_empty() {
                let unit = rng.pick(&units);
                set_word(&mut state, 0xE097, unit);
                let flags = rng.pick(&[0x8000u16, 0x0040, 0x0080, 0x0800, 0x0008, 0x0000]);
                let old = state.word(unit + 0x2E);
                set_word(&mut state, unit + 0x2E, (old & !0x88C8) | flags);
                toggle(&mut state, unit + 0x30, 0x0002);
                set_word(&mut state, unit + 0x3A, rng.below(16) as u16);
                if flags == 0x0040 && !places.is_empty() {
                    set_word(&mut state, unit + 0x12, rng.pick(&places));
                }
            }
            toggle(&mut state, 0xE087, if rng.below(2) == 0 { 0x0400 } else { 0 });
            state.low[0x1BB6] = 0;
            state.low[0x1BA8] = 0;
            let mut native = visit(&state);
            let mut inputs = ExitInputs { generator_tail: state.byte(0x00E4), ..Default::default() };
            let mut output = ScreenOutput::default();
            if let Err(error) = native.exit_words(&mut inputs, &mut output) {
                *faulted.entry(format!("{error:x?}")).or_insert(0) += 1;
                continue;
            }
            let after = run_exit(&state);
            let context = format!("snapshot {index} case {case} (exit {})", state.byte(0x1B74));
            compare(&native, &visit(&after), &context);
            assert_eq!(inputs.generator_tail, after.byte(0x00E4), "{context}: generator tail");
            assert_eq!(output.cues, queued_cues(&state, &after), "{context}: cues");
            compared += 1;
        }
    }
    eprintln!("compared {compared} mutated exits; native faults {faulted:?}");
    assert!(compared > 0);
}

/// Run the original stage results (`$04:B2F2`) on the oracle bus.
fn run_results(s: &Snapshot) -> Snapshot {
    let rom = rom();
    let runtime = rom[0x10000..0x17E00].to_vec();
    let mut bus = sf_oracle::SnesBus::new(rom);
    for (offset, byte) in runtime.into_iter().enumerate() {
        bus.write8(0x7F0000 + offset as u32, byte);
    }
    for (offset, &byte) in s.low.iter().enumerate() {
        bus.write8(0x7E0000 + offset as u32, byte);
    }
    let entry = sf_oracle::Entry { p: 0x20, dbr: 0x7E, ..Default::default() };
    let exit = sf_oracle::call_near(&mut bus, 0x04B2F2, &entry);
    assert!(exit.returned, "the original stage results did not return");
    Snapshot { low: (0..0x10000u32).map(|a| bus.read8(0x7E0000 + a)).collect(), terrain: s.terrain.clone(), gsu: s.gsu.clone() }
}

#[test]
fn stage_results_match_the_original_on_mutated_retail_states() {
    let schedule = |frame: u64| match frame {
        600..4600 if (frame - 600) % 200 < 6 => START,
        4660..4701 => RIGHT,
        4701..4721 => UP,
        _ if frame >= 4721 && frame % 64 < 4 => B,
        _ => 0,
    };
    let mut m = RetailMachine::new(rom());
    let mut snapshots = Vec::new();
    while snapshots.len() < 5 && m.video_frame() < 8_000 {
        if m.tick_until_cpu_execution(schedule(m.video_frame()), PROGRAM, 1).unwrap() && m.video_frame() > 4_000 {
            snapshots.push(Snapshot::take(&m));
            m.tick_video_frames(0, 150).unwrap();
        }
    }
    assert!(snapshots.len() >= 3);
    let cases: u32 = std::env::var("SF2_RESULTS_FUZZ").map(|v| v.parse().unwrap()).unwrap_or(150);
    let mut rng = Lcg(0x04B2_F2AA);
    let (mut compared, mut faulted) = (0, std::collections::BTreeMap::new());
    for (index, snapshot) in snapshots.iter().enumerate() {
        for case in 0..cases {
            let mut state = Snapshot { low: snapshot.low.clone(), terrain: snapshot.terrain.clone(), gsu: snapshot.gsu.clone() };
            let places = list(&state, 0xDB67);
            let units = list(&state, 0xE0A3);
            let mut events = 0x0010u16;
            for bit in [0x0002u16, 0x0004, 0x0800, 0x0200, 0x4000, 0x0020, 0x2000] {
                if rng.below(3) == 0 {
                    events |= bit;
                }
            }
            set_word(&mut state, 0x1B88, events);
            let mut campaign = state.word(0x1B8A) & !0x00B2;
            for bit in [0x0002u16, 0x0020, 0x0080, 0x0010] {
                if rng.below(4) == 0 {
                    campaign |= bit;
                }
            }
            set_word(&mut state, 0x1B8A, campaign);
            set_word(&mut state, 0xDA7D, rng.pick(&[0u16, 2, 4, 6, 8, 0x0A]));
            set_word(&mut state, 0xD79D, rng.pick(&[0u16, 1, 0xFFFE, 5]));
            set_word(&mut state, 0xD7F4, rng.below(8) as u16);
            set_word(&mut state, 0x1BB5, rng.below(11) as u16);
            set_word(&mut state, 0x1BA5, rng.below(0x40) as u16);
            state.low[0xD7A1] = rng.below(0x100) as u8;
            set_word(&mut state, 0xD79F, rng.next() as u16);
            set_word(&mut state, 0x1BA7, rng.below(16) as u16);
            set_word(&mut state, 0xD7F6, rng.next() as u16);
            set_word(&mut state, 0xDA29, rng.pick(&[1u16, 2, 5]));
            toggle(&mut state, 0xE087, rng.pick(&[0x0008u16, 0x1000, 0x0000]));
            set_word(&mut state, 0xE093, rng.below(2) as u16);
            if !places.is_empty() {
                let place = rng.pick(&places);
                set_word(&mut state, 0xDB07, place);
                toggle(&mut state, place + 0x1C, rng.pick(&[0x0004u16, 0x0080, 0x0800, 0x0000]));
                set_word(&mut state, place + 0x36, rng.below(3) as u16);
            }
            if !units.is_empty() {
                let unit = rng.pick(&units);
                set_word(&mut state, 0xE097, unit);
                let flags = rng.pick(&[0x0800u16, 0x0020, 0x0080, 0x0008, 0x0000]);
                let old = state.word(unit + 0x2E);
                set_word(&mut state, unit + 0x2E, (old & !0x08A8) | flags | 0x2002);
                if !places.is_empty() {
                    set_word(&mut state, unit + 0x12, rng.pick(&places));
                }
            }
            let mut native = visit(&state);
            let mut output = ScreenOutput::default();
            if let Err(error) = native.stage_results(&mut output) {
                *faulted.entry(format!("{error:x?}")).or_insert(0) += 1;
                continue;
            }
            let after = run_results(&state);
            let context = format!("snapshot {index} case {case} (DA7D {:X})", state.word(0xDA7D));
            compare(&native, &visit(&after), &context);
            assert_eq!(output.cues, queued_cues(&state, &after), "{context}: cues");
            compared += 1;
        }
    }
    eprintln!("compared {compared} mutated stage results; native faults {faulted:?}");
    assert!(compared > 0);
}
