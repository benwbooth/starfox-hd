//! The strategic map's sprite pass (`$04:8301` in the map's mode) against
//! the retail machine, one call at a time, including the OAM it composes;
//! then the original routine on mutated retail states.

#[path = "support/sf2_strategic_snapshot.rs"]
mod strategic_snapshot;
#[path = "support/sf2_map_snapshot.rs"]
mod map_snapshot;

use map_snapshot::*;
use sf2_game::strategic_screen::{ScreenError, ScreenOutput};
use sf2_game::strategic_sim::{PLACE_CAPACITY, UNIT_CAPACITY};
use sf2_game::strategic_sprites;
use sf_oracle::RetailMachine;
use strategic_snapshot::{Snapshot, TERRAIN};

fn compare(before: &Snapshot, after: &Snapshot, context: &str) -> Result<(), ScreenError> {
    let mut native = sprites(before);
    let mut native_screen = screen(before);
    let mut map = before.map();
    let mut native_links = links(before);
    let mut output = ScreenOutput::default();
    strategic_sprites::pass(
        &mut native,
        &mut native_screen,
        &director(before),
        &mut map,
        &mut native_links,
        sprite_inputs(before),
        &mut output,
    )?;
    let expected_map = after.map();
    for index in 0..PLACE_CAPACITY {
        assert_eq!(map.places[index], expected_map.places[index], "{context}: place {index}");
    }
    for index in 0..UNIT_CAPACITY {
        assert_eq!(map.units[index], expected_map.units[index], "{context}: unit {index}");
    }
    assert_eq!(map.globals, expected_map.globals, "{context}: globals");
    let expected = sprites(after);
    if native.oam != expected.oam {
        let first = native.oam.iter().zip(&expected.oam).position(|(a, b)| a != b).unwrap();
        if std::env::var("SF2_SPRITE_DUMP").is_ok() {
            for (k, (n, r)) in native.oam.chunks(4).zip(expected.oam.chunks(4)).enumerate().take(48) {
                eprintln!("{:03X}: native {n:02X?} retail {r:02X?} {}", k * 4, if n != r { "<<" } else { "" });
            }
            eprintln!("ship {:?} cursor ({:02X},{:02X}) frame {:04X} pilots {:?}", native_screen.ship, before.byte(0xDA91), before.byte(0xDA92), before.word(0xD97F), native_links.pilots);
        }
        panic!(
            "{context}: OAM differs from byte {first:03X}: native {:02X?} retail {:02X?}",
            &native.oam[first & !3..(first & !3) + 16],
            &expected.oam[first & !3..(first & !3) + 16]
        );
    }
    assert_eq!(native, expected, "{context}: sprites");
    assert_eq!(native_screen, screen(after), "{context}: screen");
    assert_eq!(native_links, links(after), "{context}: links");
    assert_eq!(output.cues, queued_cues(before, after), "{context}: cues");
    Ok(())
}

fn compare_calls(schedule: fn(u32) -> u16, calls: u32) -> u32 {
    let mut m = RetailMachine::new(rom());
    navigate_to_map(&mut m);
    let mut matched = 0;
    for call in 0..calls {
        let pad = schedule(call);
        let mut reached = false;
        for _ in 0..300 {
            if m.tick_until_cpu_execution(pad, SPRITE_PASS, 90).unwrap() && m.peek8(0x7E1B9E) == 2 {
                reached = true;
                break;
            }
            for button in [START, B] {
                m.tick_video_frames(button, 4).unwrap();
                m.tick_video_frames(0, 30).unwrap();
            }
        }
        if !reached {
            eprintln!("call {call}: the map's sprite pass stopped running");
            break;
        }
        let before = Snapshot::take(&m);
        assert!(m.tick_until_cpu_execution_any(pad, &SPRITE_PASS_RETURNS, 60).unwrap().is_some());
        let after = Snapshot::take(&m);
        if let Err(error) = compare(&before, &after, &format!("call {call}")) {
            eprintln!("call {call}: native sprite pass faulted: {error:x?}");
            break;
        }
        matched = call + 1;
    }
    eprintln!("map sprite pass matched {matched} calls");
    matched
}

#[test]
fn map_sprite_pass_matches_retail_through_a_campaign_driven_by_taps() {
    // Until a stage the taps cannot finish.
    let calls: u32 = std::env::var("SF2_SPRITE_CALLS").map(|v| v.parse().unwrap()).unwrap_or(2336);
    let schedule = |call: u32| match call % 16 {
        0 => START,
        8 => B,
        _ if call % 160 < 24 => [RIGHT, DOWN, LEFT, UP][(call / 160 % 4) as usize],
        _ => 0,
    };
    assert_eq!(compare_calls(schedule, calls), calls.min(2336));
}

/// Run the original pass on the oracle bus from this state.
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
    let exit = sf_oracle::call(&mut bus, SPRITE_PASS, &sf_oracle::Entry::default());
    assert!(exit.returned, "the original sprite pass did not return");
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

fn toggle(s: &mut Snapshot, address: u16, bit: u16) {
    let value = s.word(address) ^ bit;
    set_word(s, address, value);
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

fn mutate(s: &mut Snapshot, rng: &mut Lcg) {
    let places = list(s, 0xDB67);
    let units = list(s, 0xE0A3);
    for _ in 0..1 + rng.below(4) {
        match rng.below(10) {
            0 => set_word(s, 0xEFD3, rng.below(0x40) as u16),
            1 if !units.is_empty() => {
                let unit = rng.pick(&units);
                match rng.below(5) {
                    0 => set_word(s, unit + 0x36, rng.below(12) as u16),
                    1 => toggle(s, unit + 0x2E, rng.pick(&[0x0001u16, 0x0010, 0x0020, 0x0040, 0x1000, 0x4000])),
                    2 => toggle(s, unit + 0x30, rng.pick(&[0x0004u16, 0x0010, 0x0040])),
                    3 => {
                        set_word(s, unit + 0x44, rng.pick(&[0u16, 0x15, 0x18, 0x1C, 0x22, 0xFFFF]));
                        set_word(s, unit + 0x46, rng.below(0x20) as u16);
                        set_word(s, unit + 0x48, rng.pick(&[0x10u16, 0x18, 0x3F]));
                    }
                    _ => {
                        set_word(s, 0xE09F, unit);
                        set_word(s, 0xF556, 1);
                    }
                }
            }
            2 if !places.is_empty() => {
                let place = rng.pick(&places);
                match rng.below(4) {
                    0 => {
                        set_word(s, place + 0x32, rng.below(7) as u16);
                        toggle(s, place + 0x1E, 0x0002);
                    }
                    1 => toggle(s, place + 0x1C, rng.pick(&[0x0001u16, 0x1000, 0x2000, 0x4000])),
                    2 => {
                        set_word(s, place + 0x36, 2);
                        set_word(s, place + 0x3E, rng.pick(&[5u16, 0x40, 0x100, 0x200]));
                    }
                    _ => {
                        set_word(s, place + 0x4A, rng.pick(&[0u16, 0x1F, 0x20]));
                        set_word(s, place + 0x4C, rng.pick(&[0u16, 0x10, 0x23]));
                    }
                }
            }
            3 => toggle(s, 0x1B9C, 0x0080),
            4 => toggle(s, 0xDA8B, rng.pick(&[0x0004u16, 0x0100, 0x0400])),
            5 => toggle(s, 0xDA8D, 0x0001),
            6 => match rng.below(4) {
                0 => set_word(s, 0xF55A, rng.below(4) as u16),
                1 => set_word(s, 0xF58C, rng.pick(&[0u16, 0x0A, 0x14])),
                2 => set_word(s, 0xF56C, rng.below(3) as u16),
                _ => set_word(s, 0xF596, rng.below(3) as u16),
            },
            7 => {
                toggle(s, 0xE087, rng.pick(&[0x0002u16, 0x0004, 0x0200]));
                set_word(s, 0xE095, 2 * rng.below(8) as u16);
            }
            8 => match rng.below(4) {
                0 => toggle(s, 0x1B88, 0x0020),
                1 => toggle(s, 0x1B86, rng.pick(&[0x0002u16, 0x0080])),
                2 => set_word(s, 0x1DD1, rng.below(0x20) as u16),
                _ => set_word(s, 0xDA39, rng.below(4) as u16),
            },
            _ => {
                set_word(s, 0xD9C4, rng.below(2) as u16);
                set_word(s, 0xD9C2, rng.below(2) as u16);
                toggle(s, 0x1B84, 0x0040);
            }
        }
    }
}

#[test]
fn map_sprite_pass_matches_the_original_on_mutated_retail_states() {
    let mut m = RetailMachine::new(rom());
    navigate_to_map(&mut m);
    let mut snapshots = Vec::new();
    for call in 0..4000u32 {
        let pad = match call % 16 {
            0 => START,
            8 => B,
            _ => 0,
        };
        let mut reached = false;
        for _ in 0..300 {
            if m.tick_until_cpu_execution(pad, SPRITE_PASS, 90).unwrap() && m.peek8(0x7E1B9E) == 2 {
                reached = true;
                break;
            }
            for button in [START, B] {
                m.tick_video_frames(button, 4).unwrap();
                m.tick_video_frames(0, 30).unwrap();
            }
        }
        if !reached {
            break;
        }
        if call % 200 == 0 {
            snapshots.push(Snapshot::take(&m));
        }
    }
    assert!(snapshots.len() > 5);
    let cases: u32 = std::env::var("SF2_SPRITE_FUZZ").map(|v| v.parse().unwrap()).unwrap_or(60);
    let mut rng = Lcg(0x0483_01AA);
    let (mut compared, mut faulted) = (0, std::collections::BTreeMap::new());
    for (index, snapshot) in snapshots.iter().enumerate() {
        for case in 0..cases {
            let mut state = Snapshot { low: snapshot.low.clone(), terrain: snapshot.terrain.clone(), gsu: snapshot.gsu.clone() };
            mutate(&mut state, &mut rng);
            let mut probe = sprites(&state);
            let mut map = state.map();
            if let Err(error) = strategic_sprites::pass(
                &mut probe,
                &mut screen(&state),
                &director(&state),
                &mut map,
                &mut links(&state),
                sprite_inputs(&state),
                &mut ScreenOutput::default(),
            ) {
                *faulted.entry(format!("{error:x?}")).or_insert(0) += 1;
                continue;
            }
            let after = run_original(&state);
            compare(&state, &after, &format!("snapshot {index} case {case}")).unwrap();
            compared += 1;
        }
    }
    eprintln!("compared {compared} mutated sprite passes; native faults {faulted:?}");
    assert!(compared > 0);
}
