//! The strategic map's real-time simulation (`$7F:537D`) against the
//! retail machine, one call at a time: retail's state when each call
//! begins is decoded, the native tick runs on it, and the result is
//! compared with retail's state when the call returns. The calls come from
//! the Star Wolf interception, flown with live input.

use sf2_game::strategic_sim::{
    MapPlace, MapUnit, PlaceId, SceneLinks, StrategicGlobals, StrategicInputs, StrategicMap, TickOutput,
    UnitId, ENCOUNTER_SLOTS, PLACE_CAPACITY, UNIT_CAPACITY,
};
use sf_oracle::RetailMachine;

const MISSION_LAUNCH: u32 = 0x03B90E;
const TICK: u32 = 0x7F537D;
const TICK_RETURN: u32 = 0x7F539B;
const PLACES: u16 = 0xDB69;
const PLACE_SIZE: u16 = 0x56;
const UNITS: u16 = 0xE0A7;
const UNIT_SIZE: u16 = 0x52;
const TERRAIN: u32 = 0x7FD400;
const TERRAIN_BYTES: u32 = 0x400;
const START: u16 = 0x1000;
const B: u16 = 0x8000;
const RIGHT: u16 = 0x0100;
const UP: u16 = 0x0800;

fn rom() -> Vec<u8> {
    std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"))
        .expect("user-owned SF2 ROM required")
}

/// Bytes of retail work RAM, captured at one instant.
struct Snapshot {
    low: Vec<u8>,
    terrain: Vec<u8>,
}

impl Snapshot {
    fn take(m: &RetailMachine) -> Self {
        Self {
            low: (0..0x10000u32).map(|a| m.peek8(0x7E0000 + a)).collect(),
            terrain: (0..TERRAIN_BYTES).map(|a| m.peek8(TERRAIN + a)).collect(),
        }
    }
    fn byte(&self, address: u16) -> u8 {
        self.low[usize::from(address)]
    }
    fn word(&self, address: u16) -> u16 {
        u16::from(self.byte(address)) | (u16::from(self.byte(address.wrapping_add(1))) << 8)
    }
    fn place_id(&self, pointer: u16) -> Option<PlaceId> {
        if pointer == 0 {
            return None;
        }
        let offset = pointer.wrapping_sub(PLACES);
        assert_eq!(offset % PLACE_SIZE, 0, "place pointer {pointer:04X}");
        let index = offset / PLACE_SIZE;
        assert!(usize::from(index) < PLACE_CAPACITY, "place pointer {pointer:04X}");
        Some(PlaceId(index as u8))
    }
    fn unit_id(&self, pointer: u16) -> Option<UnitId> {
        if pointer == 0 {
            return None;
        }
        let offset = pointer.wrapping_sub(UNITS);
        assert_eq!(offset % UNIT_SIZE, 0, "unit pointer {pointer:04X}");
        let index = offset / UNIT_SIZE;
        assert!(usize::from(index) < UNIT_CAPACITY, "unit pointer {pointer:04X}");
        Some(UnitId(index as u8))
    }

    fn place(&self, index: usize) -> MapPlace {
        let base = PLACES + index as u16 * PLACE_SIZE;
        let w = |offset: u16| self.word(base + offset);
        MapPlace {
            next: self.place_id(w(0x00)),
            prev: self.place_id(w(0x02)),
            kind: w(0x04),
            x: w(0x0C),
            y: w(0x0E),
            spawn_target_x: w(0x10),
            spawn_target_y: w(0x12),
            program: w(0x18),
            program_repeats: w(0x1A),
            flags: w(0x1C),
            status: w(0x1E),
            arrivals: w(0x22),
            heading: w(0x24),
            spawned_unit: self.unit_id(w(0x26)),
            held_unit: self.unit_id(w(0x28)),
            target_x: w(0x2A),
            target_y: w(0x2C),
            warning: w(0x32),
            warning_countdown: w(0x34),
            state: w(0x36),
            spawn_remaining: w(0x3A),
            spawn_batch: w(0x3C),
            timer: w(0x3E),
            warning_rate: w(0x4A),
            warning_bias: w(0x4C),
        }
    }

    fn unit(&self, index: usize) -> MapUnit {
        let base = UNITS + index as u16 * UNIT_SIZE;
        let w = |offset: u16| self.word(base + offset);
        let b = |offset: u16| self.byte(base + offset);
        MapUnit {
            next: self.unit_id(w(0x00)),
            prev: self.unit_id(w(0x02)),
            behavior: w(0x04),
            saved_behavior: w(0x06),
            timer: w(0x08),
            program: w(0x0A),
            program_timer: w(0x0C),
            motion: w(0x0E),
            saved_motion: w(0x10),
            home: self.place_id(w(0x12)),
            origin: self.place_id(w(0x14)),
            x_fraction: w(0x1A),
            x: b(0x1C),
            y_fraction: w(0x1D),
            y: b(0x1F),
            target_x: w(0x20),
            target_y: w(0x22),
            heading: w(0x24),
            speed: w(0x26),
            vx_fraction: w(0x28),
            vx: b(0x2A),
            vy_fraction: w(0x2B),
            vy: b(0x2D),
            flags: w(0x2E),
            flags2: w(0x30),
            category: w(0x32),
            kind: w(0x34),
            variant: w(0x36),
            strength: w(0x38),
            encounter_slot: w(0x3A),
            count: w(0x3C),
            node_variant: w(0x3E),
            radius: w(0x40),
            proximity_delay: w(0x42),
            sprite: w(0x44),
            frame: w(0x46),
            animation_timer: w(0x48),
            turn_rate: w(0x4A),
            turn_bias: w(0x4C),
            turn_countdown: w(0x4E),
        }
    }

    fn map(&self) -> StrategicMap {
        StrategicMap {
            places: std::array::from_fn(|index| self.place(index)),
            units: std::array::from_fn(|index| self.unit(index)),
            place_head: self.place_id(self.word(0xDB67)),
            place_free: self.place_id(self.word(0xDB65)),
            unit_head: self.unit_id(self.word(0xE0A3)),
            unit_free: self.unit_id(self.word(0xE0A5)),
            globals: self.globals(),
        }
    }

    fn globals(&self) -> StrategicGlobals {
        StrategicGlobals {
            hold: self.word(0x1B8C),
            pass_holds: self.word(0x1B90),
            speed_flags: self.word(0x1B92),
            campaign_flags: self.word(0x1B94),
            encounter_request: self.word(0x1B74),
            launch_count: self.word(0xDA39),
            unit_count: self.word(0xDA43),
            escort_count: self.word(0xDA45),
            fighter_launches: self.word(0xDA47),
            cruiser_count: self.word(0xDA4D),
            missile_count: self.word(0xDA53),
            missile_salvo: self.word(0xDA55),
            interceptor_count: self.word(0xDA57),
            threat_kind: self.word(0xDA69),
            threat_place: self.place_id(self.word(0xDA6B)),
            threat_countdown: self.word(0xDA71),
            threat_nearest: self.word(0xDA73),
            interceptors: self.word(0xD998),
            launches_pending: self.word(0xD9B0),
            spawn_pattern: self.word(0xD9F9),
            escort_pattern: self.word(0xD9FB),
            fast_vx_fraction: self.word(0xD984),
            fast_vx: self.byte(0xD986),
            fast_vy_fraction: self.word(0xD988),
            fast_vy: self.byte(0xD98A),
            map_events: self.word(0xDB33),
            spawn_variant_bit: self.word(0xDB3B),
            guarded_place: self.place_id(self.word(0xDB45)),
            base: self.place_id(self.word(0xDB61)),
            satellite: self.unit_id(self.word(0xE079)),
            satellite_phase: self.word(0xE085),
            satellite_flags: self.word(0xE087),
            satellite_hold: self.word(0xE089),
            satellite_target: self.unit_id(self.word(0xE091)),
            satellite_guarding: self.word(0xE093),
            satellite_facing: self.word(0xE095),
            met_unit: self.unit_id(self.word(0xE097)),
            intercepted_unit: self.unit_id(self.word(0xE099)),
            intercepted_motion: self.word(0xE09B),
            intercepted_behavior: self.word(0xE09D),
            alerts: self.word(0xF55C),
            threat_cue: self.byte(0x1CD3) & 0x10 != 0,
            stored_target: self.word(0xD7FC),
            stored_counter: self.word(0xDA11),
            slot_cursor: self.word(0xD7FE),
            slot_used: std::array::from_fn(|i| self.byte(0xD800 + i as u16)),
            slot_pattern: std::array::from_fn(|i| self.byte(0xD7C2 + i as u16)),
            slot_word: std::array::from_fn(|i| self.word(0xD7A2 + 2 * i as u16)),
            slot_kind: std::array::from_fn(|i| self.byte(0xE805 + i as u16)),
            slot_strength: std::array::from_fn(|i| self.byte(0xE815 + i as u16)),
        }
    }

    fn links(&self) -> SceneLinks {
        SceneLinks {
            stage_results: self.word(0x1B86),
            scene_events: self.word(0x1B88),
            campaign_events: self.word(0x1B8A),
            event_word: self.word(0x1C0E),
            planet_damage: self.word(0xDB4B),
            map_region: self.word(0xDB5B),
            encounter_result: self.word(0xD79D),
            node_variant: self.byte(0x1E09),
        }
    }
}

/// Title, attract and pilot selection by Start presses, to the map.
fn navigate_to_map(m: &mut RetailMachine) {
    m.tick_video_frames(0, 600).unwrap();
    for _ in 0..20 {
        m.tick_video_frames(START, 6).unwrap();
        m.tick_video_frames(0, 194).unwrap();
    }
    m.tick_video_frames(0, 60).unwrap();
}

/// The map cursor to the cyan planet's lane and B: the ship sets off.
fn navigate_to_travel(m: &mut RetailMachine) {
    navigate_to_map(m);
    m.tick_video_frames(RIGHT, 41).unwrap();
    m.tick_video_frames(UP, 20).unwrap();
    m.tick_video_frames(B, 6).unwrap();
}

fn navigate_to_launch(m: &mut RetailMachine) {
    navigate_to_travel(m);
    assert!(m.tick_until_cpu_execution(0, MISSION_LAUNCH, 2000).unwrap(), "no encounter launched");
}

/// Runs up to `calls` retail ticks with `pad` held; returns how many the
/// native tick reproduced before the first fault.
fn compare_calls(navigate: fn(&mut RetailMachine), schedule: fn(u32) -> u16, calls: u32) -> u32 {
    compare_calls_tapping(navigate, schedule, calls, false)
}

fn compare_calls_tapping(
    navigate: fn(&mut RetailMachine),
    schedule: fn(u32) -> u16,
    calls: u32,
    tap_through: bool,
) -> u32 {
    assert_eq!(ENCOUNTER_SLOTS, 16);
    let mut m = RetailMachine::new(rom());
    navigate(&mut m);
    let mut matched = 0;
    let mut changed = 0;
    let mut coverage = std::env::var("SF2_SIM_COVERAGE").ok().map(|_| std::collections::BTreeSet::new());
    for call in 0..calls {
        let pad = schedule(call);
        // Screens between ticks wait for presses: tap through them.
        let mut reached = false;
        for _ in 0..300 {
            if m.tick_until_cpu_execution(pad, TICK, 90).unwrap() {
                reached = true;
                break;
            }
            if tap_through {
                for button in [START, B] {
                    m.tick_video_frames(button, 4).unwrap();
                    m.tick_video_frames(0, 30).unwrap();
                }
            }
        }
        if !reached {
            break;
        }
        let before = Snapshot::take(&m);
        if coverage.is_some() {
            m.watch_cpu_execution(&(0x7F5300u32..0x7F7000).collect::<Vec<_>>());
        }
        assert!(m.tick_until_cpu_execution(pad, TICK_RETURN, 60).unwrap());
        if let Some(coverage) = coverage.as_mut() {
            coverage.extend(m.take_cpu_execution_watch_hits());
            m.watch_cpu_execution(&[]);
        }
        let after = Snapshot::take(&m);
        let mut map = before.map();
        let mut links = before.links();
        let inputs = StrategicInputs {
            difficulty: before.word(0xD7F2),
            player: (before.byte(0xDAF3), before.byte(0xDAF6)),
            batch_bonus: before.word(0xDA3B),
            satellite_timing: before.word(0x1BA3),
            satellite_busy: before.word(0xE089),
            terrain: &before.terrain,
        };
        let mut output = TickOutput::default();
        if let Err(error) = map.tick(&mut links, inputs, &mut output) {
            eprintln!("call {call}: native tick faulted: {error:x?}");
            break;
        }
        let expected = after.map();
        if expected != before.map() {
            changed += 1;
        }
        for index in 0..PLACE_CAPACITY {
            assert_eq!(map.places[index], expected.places[index], "call {call}: place {index}");
        }
        for index in 0..UNIT_CAPACITY {
            assert_eq!(map.units[index], expected.units[index], "call {call}: unit {index}");
        }
        assert_eq!(map.globals, expected.globals, "call {call}: globals");
        assert_eq!(
            (map.place_head, map.place_free, map.unit_head, map.unit_free),
            (expected.place_head, expected.place_free, expected.unit_head, expected.unit_free),
            "call {call}: list heads"
        );
        assert_eq!(links, after.links(), "call {call}: scene links");
        matched = call + 1;
    }
    eprintln!("strategic simulation matched {matched} calls, {changed} of which changed the map");
    if let Some(coverage) = coverage {
        let text: Vec<String> = coverage.iter().map(|a| format!("{:04X}", a & 0xFFFF)).collect();
        eprintln!("COVERAGE {}", text.join(" "));
    }
    matched
}

#[test]
fn strategic_simulation_matches_retail_through_the_star_wolf_interception() {
    // The interception's ticks with B held, until the stage ends in defeat.
    assert_eq!(compare_calls(navigate_to_launch, |_| B, 400), 326);
}

#[test]
fn strategic_simulation_matches_retail_while_the_ship_travels_and_is_shot_down() {
    // The travel to the interception, then the stage with no input.
    let calls: u32 = std::env::var("SF2_SIM_CALLS").map(|v| v.parse().unwrap()).unwrap_or(2000);
    assert_eq!(compare_calls(navigate_to_travel, |_| 0, calls), 591.min(calls));
}

#[test]
fn strategic_simulation_matches_retail_through_a_campaign_driven_by_taps() {
    // Start and B tapped in turn keep the campaign moving through its
    // screens; the ship's travels and stages advance the map.
    let calls: u32 = std::env::var("SF2_SIM_CALLS").map(|v| v.parse().unwrap()).unwrap_or(4000);
    let schedule = |call: u32| match call % 16 {
        0 => START,
        8 => B,
        _ => 0,
    };
    assert_eq!(compare_calls_tapping(navigate_to_travel, schedule, calls, true), calls);
}

/// Retail snapshots taken as the tick begins, every `every` calls.
fn collect_snapshots(navigate: fn(&mut RetailMachine), calls: u32, every: u32) -> Vec<Snapshot> {
    let mut m = RetailMachine::new(rom());
    navigate(&mut m);
    let mut snapshots = Vec::new();
    for call in 0..calls {
        let pad = match call % 16 {
            0 => START,
            8 => B,
            _ => 0,
        };
        let mut reached = false;
        for _ in 0..300 {
            if m.tick_until_cpu_execution(pad, TICK, 90).unwrap() {
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
        if call % every == 0 {
            snapshots.push(Snapshot::take(&m));
        }
    }
    snapshots
}

/// A small deterministic generator for the mutations.
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

impl Snapshot {
    fn set_word(&mut self, address: u16, value: u16) {
        self.low[usize::from(address)] = value as u8;
        self.low[usize::from(address.wrapping_add(1))] = (value >> 8) as u8;
    }
    fn toggle(&mut self, address: u16, bit: u16) {
        let value = self.word(address) ^ bit;
        self.set_word(address, value);
    }
    fn list(&self, head: u16) -> Vec<u16> {
        let mut out = Vec::new();
        let mut pointer = self.word(head);
        while pointer != 0 && out.len() < 32 {
            out.push(pointer);
            pointer = self.word(pointer);
        }
        out
    }

    /// Perturb non-link fields so that every handler is reached.
    fn mutate(&mut self, rng: &mut Lcg) {
        let places = self.list(0xDB67);
        let units = self.list(0xE0A3);
        for _ in 0..1 + rng.below(4) {
            match rng.below(5) {
                0 if !places.is_empty() => {
                    let place = rng.pick(&places);
                    match rng.below(6) {
                        0 => self.set_word(place + 0x36, rng.below(5) as u16),
                        1 => self.set_word(place + 0x3E, rng.pick(&[0u16, 1, 2, 4, 0x20])),
                        2 => self.toggle(place + 0x1C, rng.pick(&[0x0001u16, 0x0004, 0x0010, 0x0080, 0x0100, 0x0200, 0x0400, 0x0800, 0x1000])),
                        3 => self.set_word(place + 0x3A, rng.below(4) as u16),
                        4 => self.set_word(place + 0x18, rng.pick(&[0x1Au16, 0x1F, 0x24, 0x25, 0x27, 0x2C, 0x31, 0x32, 0x34, 0x39, 0x3E, 0x3F])),
                        _ => self.set_word(place + 0x1A, rng.below(4) as u16),
                    }
                }
                1 | 2 if !units.is_empty() => {
                    let unit = rng.pick(&units);
                    match rng.below(9) {
                        0 => self.set_word(unit + 0x04, rng.below(10) as u16),
                        1 => self.set_word(unit + 0x08, rng.pick(&[0u16, 1, 2, 3])),
                        2 => self.set_word(unit + 0x0E, rng.below(9) as u16),
                        3 => self.toggle(unit + 0x2E, rng.pick(&[0x0001u16, 0x0002, 0x0004, 0x0008, 0x0010, 0x0020, 0x0040, 0x0080, 0x0800, 0x1000, 0x2000, 0x4000, 0x8000])),
                        4 => self.toggle(unit + 0x30, rng.pick(&[0x0002u16, 0x0004, 0x0020, 0x0080, 0x0100, 0x0800, 0x1000])),
                        5 => {
                            self.set_word(unit + 0x0A, rng.below(5) as u16);
                            self.set_word(unit + 0x0C, rng.below(3) as u16);
                        }
                        6 => self.set_word(unit + 0x42, rng.below(2) as u16),
                        7 => {
                            // Put the unit on the player.
                            self.low[usize::from(unit + 0x1C)] = self.byte(0xDAF3);
                            self.low[usize::from(unit + 0x1F)] = self.byte(0xDAF6);
                        }
                        _ => self.set_word(unit + 0x34, 0x1C + rng.below(4) as u16),
                    }
                }
                3 => match rng.below(4) {
                    0 => self.set_word(0xDA43, rng.below(17) as u16),
                    1 => self.set_word(0xDA45, rng.below(7) as u16),
                    2 => self.set_word(0xDA4D, rng.below(3) as u16),
                    _ => self.set_word(0xE085, rng.below(4) as u16),
                },
                _ => match rng.below(8) {
                    0 => self.toggle(0x1B8C, 0x0001),
                    1 => self.toggle(0x1B90, rng.pick(&[0x0001u16, 0x0002, 0x0008, 0x0010, 0x0020])),
                    2 => self.toggle(0x1B92, rng.pick(&[0x0008u16, 0x0800])),
                    3 => self.toggle(0x1B88, rng.pick(&[0x0010u16, 0x0020, 0x0040, 0x0080])),
                    4 => self.toggle(0x1B8A, rng.pick(&[0x0010u16, 0x0020, 0x0080])),
                    5 => self.toggle(0x1B86, 0x0010),
                    6 => self.toggle(0xE087, 0x0008),
                    _ => self.set_word(0xE093, rng.below(2) as u16),
                },
            }
        }
    }

    /// Run the original tick on the oracle bus from this state.
    fn run_original(&self) -> Snapshot {
        let rom = rom();
        let runtime = rom[0x10000..0x17E00].to_vec();
        let mut bus = sf_oracle::SnesBus::new(rom);
        for (offset, byte) in runtime.into_iter().enumerate() {
            bus.write8(0x7F0000 + offset as u32, byte);
        }
        for (offset, &byte) in self.low.iter().enumerate() {
            bus.write8(0x7E0000 + offset as u32, byte);
        }
        for (offset, &byte) in self.terrain.iter().enumerate() {
            bus.write8(TERRAIN + offset as u32, byte);
        }
        let entry = sf_oracle::Entry { dbr: 0x7E, ..Default::default() };
        let exit = sf_oracle::call_near(&mut bus, TICK, &entry);
        let _ = exit;
        Snapshot {
            low: (0..0x10000u32).map(|a| bus.read8(0x7E0000 + a)).collect(),
            terrain: self.terrain.clone(),
        }
    }
}

#[test]
fn strategic_simulation_matches_the_original_on_mutated_retail_states() {
    let snapshots = collect_snapshots(navigate_to_travel, 1200, 40);
    assert!(snapshots.len() > 10);
    let cases: u32 = std::env::var("SF2_SIM_FUZZ").map(|v| v.parse().unwrap()).unwrap_or(40);
    let mut rng = Lcg(0x5F2D_1993);
    let (mut compared, mut faulted) = (0, std::collections::BTreeMap::new());
    for (index, snapshot) in snapshots.iter().enumerate() {
        for case in 0..cases {
            let mut state = Snapshot { low: snapshot.low.clone(), terrain: snapshot.terrain.clone() };
            state.mutate(&mut rng);
            let mut map = state.map();
            let mut links = state.links();
            let inputs = StrategicInputs {
                difficulty: state.word(0xD7F2),
                player: (state.byte(0xDAF3), state.byte(0xDAF6)),
                batch_bonus: state.word(0xDA3B),
                satellite_timing: state.word(0x1BA3),
                satellite_busy: state.word(0xE089),
                terrain: &state.terrain,
            };
            let mut output = TickOutput::default();
            if let Err(error) = map.tick(&mut links, inputs, &mut output) {
                *faulted.entry(format!("{error:x?}")).or_insert(0) += 1;
                continue;
            }
            let after = state.run_original();
            let expected = after.map();
            let context = format!("snapshot {index} case {case}");
            for place in 0..PLACE_CAPACITY {
                assert_eq!(map.places[place], expected.places[place], "{context}: place {place}");
            }
            for unit in 0..UNIT_CAPACITY {
                assert_eq!(map.units[unit], expected.units[unit], "{context}: unit {unit}");
            }
            assert_eq!(map.globals, expected.globals, "{context}: globals");
            assert_eq!(
                (map.place_head, map.place_free, map.unit_head, map.unit_free),
                (expected.place_head, expected.place_free, expected.unit_head, expected.unit_free),
                "{context}: list heads"
            );
            assert_eq!(links, after.links(), "{context}: scene links");
            compared += 1;
        }
    }
    eprintln!("compared {compared} mutated ticks; native faults {faulted:?}");
    assert!(compared > 0);
}
