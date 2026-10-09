//! The strategic map screen's frame (`$04:B5DA`) against the retail
//! machine, one call at a time: retail's state when each call begins is
//! decoded, the native frame runs on it, and the result is compared with
//! retail's state when the call returns. A second test runs the original
//! routine on mutated retail states.

#[path = "support/sf2_strategic_snapshot.rs"]
mod strategic_snapshot;

use sf2_game::strategic_director::{self, MapDirector, MapTally};
use sf2_game::strategic_screen::{
    self, MapCampaign, MapPad, MapScreen, MapShip, MapWingmate, PlaceInfo, Pulse, ScreenLinks, ScreenOutput,
    ScriptSubject, ShipFrame,
};
use sf2_game::strategic_sim::{StrategicMap, PLACE_CAPACITY, UNIT_CAPACITY};
use sf_oracle::RetailMachine;
use strategic_snapshot::{Snapshot, TERRAIN};

const FRAME: u32 = 0x04B5DA;
const FRAME_RETURN: u32 = 0x04B5FA;
const START: u16 = 0x1000;
const B: u16 = 0x8000;
const X: u16 = 0x0040;
const RIGHT: u16 = 0x0100;
const LEFT: u16 = 0x0200;
const DOWN: u16 = 0x0400;
const UP: u16 = 0x0800;
/// The cue ring (`$7F:6E09`) and its write index.
const CUE_RING: u16 = 0x1CF6;
const CUE_INDEX: u16 = 0x1D16;
/// The ship frames' table (`$04:D6DB`) and its bases.
const FRAME_TABLE: u16 = 0x6A61;

fn rom() -> Vec<u8> {
    std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"))
        .expect("user-owned SF2 ROM required")
}

fn navigate_to_map(m: &mut RetailMachine) {
    m.tick_video_frames(0, 600).unwrap();
    for _ in 0..20 {
        m.tick_video_frames(START, 6).unwrap();
        m.tick_video_frames(0, 194).unwrap();
    }
    m.tick_video_frames(0, 60).unwrap();
}

fn pulse(s: &Snapshot, base: u16) -> Pulse {
    let triple = |a: u16| u32::from(s.word(a)) | u32::from(s.byte(a + 2)) << 16;
    Pulse { x: triple(base), y: triple(base + 3), dx: triple(base + 6), dy: triple(base + 9), flags: s.word(base + 12) }
}

fn screen(s: &Snapshot) -> MapScreen {
    MapScreen {
        cursor_x: s.byte(0xDA91),
        cursor_y: s.byte(0xDA92),
        cursor_velocity_x: s.word(0xD978),
        cursor_velocity_y: s.word(0xD97A),
        cursor_speed: s.word(0xD97C),
        interface: s.word(0xDA8B),
        hover: s.word(0xDA8D),
        countdown: s.word(0xDA8F),
        selected_x: s.byte(0xDA93),
        selected_y: s.byte(0xDA94),
        arrival_x: s.byte(0xDA95),
        arrival_y: s.byte(0xDA96),
        pulses: std::array::from_fn(|k| pulse(s, 0xDA97 + 14 * k as u16)),
        clock_frames: s.word(0xDA5F),
        elapsed_steps: s.word(0xDA5B),
        threat_cue_timer: s.word(0xDA75),
        select_state: s.word(0xDA1B),
        script: s.word(0xDA7F),
        menu_choice: s.word(0xDAE7),
        menu_home: s.word(0xDAE9),
        menu_confirm: s.word(0xDAEF),
        menu_place: s.place_id(s.word(0xDAED)),
        ship: MapShip {
            x: s.word(0xDAF2),
            y: s.word(0xDAF5),
            heading: s.word(0xDAF7),
            sector: s.word(0xDB09),
            speed: s.word(0xDAFB),
            step_x: s.word(0xDAFD),
            step_y: s.word(0xDAFF),
            travel: s.word(0xDB01),
            destination: s.place_id(s.word(0xDB07)),
            span_x: s.byte(0xDB0D),
            span_y: s.byte(0xDB0E),
            error: s.byte(0xDB0F),
            target_x: s.byte(0xDB11),
            target_y: s.byte(0xDB12),
            target_pad: s.byte(0xDB13),
            x_pad: s.byte(0xDAF4),
            flight_steps: s.word(0xDB03),
            flight_countdown: s.word(0xDB05),
        },
        wingmate: MapWingmate {
            x: s.word(0xDB14),
            y: s.word(0xDB17),
            heading: s.word(0xDB19),
            speed: s.word(0xDB1B),
            velocity_x: s.word(0xDB1D),
            velocity_y: s.word(0xDB1F),
            sector: s.word(0xDB21),
            flags: s.word(0xDB23),
        },
        info: PlaceInfo {
            shown: s.word(0xDB2D),
            number: s.word(0xDB2F),
            detail: s.word(0xDB31),
            x: s.word(0xDB3D),
            y: s.word(0xDB3F),
            kind: s.word(0xDB57),
        },
        hovered_place: s.place_id(s.word(0xDB55)),
        ordered_place: s.place_id(s.word(0xDB53)),
        hovered_unit: s.unit_id(s.word(0xE09F)),
        planet_warnings: s.word(0x1C12),
        planet_warning: s.byte(0x1C10),
        warning_flash: [s.byte(0xF4EC), s.byte(0xF4ED), s.byte(0xF4EE)],
        travel_sound: s.byte(0x1CE5),
        script_offset: s.word(0xDA81),
        script_subject: subject(s, s.word(0xDA83)),
        script_countdown: s.word(0xDA85),
        saved_ship: (s.word(0xDA87), s.word(0xDA89)),
        warp_target: [s.byte(0xDB25), s.byte(0xDB26), s.byte(0xDB27)],
        warp_spare: s.byte(0xDB28),
        saved_player: (s.word(0xD99C), s.word(0xD99E)),
        saved_motion: s.word(0xE0A1),
        campaign: MapCampaign {
            marker_count: s.word(0xD99A),
            marker_cursor: s.word(0xDAEB),
            guards_left: s.word(0x1BCA),
            guard_kind: s.word(0x1BCC),
            markers_placed: s.word(0xDA2F),
            wave_cursor: s.word(0xDB51),
            wave_ships: s.word(0xD98F),
            wave_escorts: s.word(0xDA41),
            start_cursor: s.word(0xDA3F),
            bases_left: s.word(0xDA59),
        },
    }
}

/// DA83 holds a unit pointer or, after the guard placement, an index.
fn subject(s: &Snapshot, word: u16) -> ScriptSubject {
    let offset = word.wrapping_sub(strategic_snapshot::UNITS);
    if offset % strategic_snapshot::UNIT_SIZE == 0 && usize::from(offset / strategic_snapshot::UNIT_SIZE) < UNIT_CAPACITY {
        s.unit_id(word).map_or(ScriptSubject::Value(word), ScriptSubject::Unit)
    } else {
        ScriptSubject::Value(word)
    }
}

fn links(s: &Snapshot) -> ScreenLinks {
    ScreenLinks {
        scene: s.links(),
        service: s.word(0x1C08),
        mode: s.word(0x1B84),
        warp_place: s.place_id(s.word(0xDB5D)),
        warp_unit: s.unit_id(s.word(0xDB5F)),
        difficulty: s.word(0xD7F2),
        batch_bonus: s.word(0xDA3B),
        satellite_timing: s.word(0x1BA3),
        display_flags: s.word(0x1B9C),
        stage_flags: s.word(0xD7F8),
        presentation_countdown: s.word(0x1C6E),
        presentation_flags: s.word(0x1C67),
        message: s.word(0xF582),
        text_state: [s.word(0xF576), s.word(0xF578)],
        pilot_shields: [s.byte(0x1DD1), s.byte(0x1DD5), s.byte(0x1DD7), s.byte(0x1DDB)],
        launch_location: s.word(0x1BB5),
        launch_layout: s.word(0x1BA5),
        mission_result: s.word(0xD79D),
        mission_rank: s.word(0xD7F4),
        mission_extra: s.word(0x1BF2),
        random: s.word(0x1C00),
        missile_kinds: [s.byte(0x1C06), s.byte(0x1C07)],
        planet_health: s.word(0xDB47),
        pilots: [s.byte(0x1E14), s.byte(0x1E15)],
        planet_place: s.place_id(s.word(0xDB4D)),
        station_place: s.place_id(s.word(0xE07B)),
        pursuit_unit: s.unit_id(s.word(0xDB63)),
        menu_pad: [s.word(0x1C1F), s.word(0x1C21)],
        arrival_word: s.word(0xDA7D),
        final_stage: s.word(0xDB29),
        timeline: s.word(0xD9FD),
    }
}

fn pad(s: &Snapshot) -> MapPad {
    MapPad { held: s.word(0x1292), pressed: s.word(0x1296) }
}

/// The cue words retail queued between two snapshots.
fn queued_cues(before: &Snapshot, after: &Snapshot) -> Vec<u16> {
    let mut index = before.byte(CUE_INDEX);
    let end = after.byte(CUE_INDEX);
    let mut cues = Vec::new();
    while index != end {
        cues.push(after.word(CUE_RING + u16::from(index)));
        index = index.wrapping_add(2) & 0x1F;
    }
    cues
}

/// `$04:D6BD`'s frame pointer for a native frame choice.
fn frame_pointer(s: &Snapshot, frame: ShipFrame) -> u16 {
    let offset = u16::from(frame.sector) + 0x10 * u16::from(frame.pilot) + s.word(0xD9CE);
    let entry = u16::from(s.byte(FRAME_TABLE.wrapping_add(offset)));
    (entry << 5).wrapping_add(s.word(0xD9C8))
}

/// Runs the native frame on `before` and checks it against `after`.
/// `Err` carries a native fault; mismatches panic with `context`.
fn compare(before: &Snapshot, after: &Snapshot, context: &str) -> Result<(), strategic_screen::ScreenError> {
    let mut native = screen(before);
    let mut map = before.map();
    let mut native_links = links(before);
    let mut output = ScreenOutput::default();
    strategic_screen::frame(&mut native, &mut map, &mut native_links, &before.terrain, pad(before), &mut output)?;
    let expected_map: StrategicMap = after.map();
    for index in 0..PLACE_CAPACITY {
        assert_eq!(map.places[index], expected_map.places[index], "{context}: place {index}");
    }
    for index in 0..UNIT_CAPACITY {
        assert_eq!(map.units[index], expected_map.units[index], "{context}: unit {index}");
    }
    assert_eq!(map.globals, expected_map.globals, "{context}: globals");
    assert_eq!(native, screen(after), "{context}: screen");
    assert_eq!(native_links, links(after), "{context}: links");
    assert_eq!(output.cues, queued_cues(before, after), "{context}: cues");
    match output.radio {
        Some(voice) => assert_eq!((after.byte(0x1CDA), after.byte(0x1CD9)), (voice, 0), "{context}: radio"),
        None => assert_eq!(
            (after.byte(0x1CDA), after.byte(0x1CD9)),
            (before.byte(0x1CDA), before.byte(0x1CD9)),
            "{context}: radio"
        ),
    }
    for (frame, address) in [(output.ship_frame, 0x1C17u16), (output.wingmate_frame, 0x1C1B)] {
        let expected = match frame {
            Some(frame) => {
                let pointer = frame_pointer(before, frame);
                (pointer, pointer.wrapping_add(0x200))
            }
            None => (before.word(address), before.word(address + 2)),
        };
        assert_eq!((after.word(address), after.word(address + 2)), expected, "{context}: frame at {address:04X}");
    }
    Ok(())
}

/// Runs up to `calls` retail frames under `schedule`; returns how many the
/// native frame reproduced before the first fault.
fn compare_calls(schedule: fn(u32) -> u16, calls: u32) -> u32 {
    let mut m = RetailMachine::new(rom());
    navigate_to_map(&mut m);
    let mut matched = 0;
    for call in 0..calls {
        let pad = schedule(call);
        let mut reached = false;
        for _ in 0..300 {
            if m.tick_until_cpu_execution(pad, FRAME, 90).unwrap() {
                reached = true;
                break;
            }
            for button in [START, B] {
                m.tick_video_frames(button, 4).unwrap();
                m.tick_video_frames(0, 30).unwrap();
            }
        }
        if !reached {
            eprintln!("call {call}: the map frame stopped running (mode {:04X}, stage {:04X})", m.peek16(0x7E1B68), m.peek16(0x7E1B70));
            break;
        }
        let before = Snapshot::take(&m);
        let trace = std::env::var("SF2_SCREEN_TRACE").is_ok();
        if trace {
            m.watch_cpu_execution(&(0x7F5300u32..0x7F7000).chain([0x04B5F2]).collect::<Vec<_>>());
        }
        assert!(m.tick_until_cpu_execution(pad, FRAME_RETURN, 60).unwrap());
        if trace {
            let hits = m.take_cpu_execution_watch_hits();
            m.watch_cpu_execution(&[]);
            if hits.contains(&0x7F5DA3) {
                let mut hits = hits;
                hits.sort();
                hits.dedup();
                eprintln!("call {call}: hits {hits:06X?}");
            }
        }
        let after = Snapshot::take(&m);
        if std::env::var("SF2_SCREEN_TRACE").is_ok() && before.word(0xDA7F) != 0 {
            eprintln!(
                "call {call}: script {:04X} at {:04X} subject {:04X} met {:04X} unit flags {:04X}",
                before.word(0xDA7F),
                before.word(0xDA81),
                before.word(0xDA83),
                before.word(0xE097),
                before.word(before.word(0xE097).wrapping_add(0x2E))
            );
        }
        if let Err(error) = compare(&before, &after, &format!("call {call}")) {
            eprintln!(
                "call {call}: native frame faulted: {error:x?} (script {:04X} at {:04X}, step {:04X})",
                before.word(0xDA7F),
                before.word(0xDA81),
                before.word(0xDA83)
            );
            break;
        }
        matched = call + 1;
    }
    eprintln!("map screen matched {matched} frames");
    matched
}

/// The cursor wanders the map in long sweeps; B orders the ship, X cancels
/// and the shoulder-free menu keys step the place menu.
fn wander(call: u32) -> u16 {
    let sweep = [RIGHT, DOWN, LEFT, UP, RIGHT | DOWN, LEFT | UP, RIGHT | UP, LEFT | DOWN][(call / 37 % 8) as usize];
    match call % 211 {
        0..=2 => B,
        100..=101 => X,
        _ if call % 37 < 20 => sweep,
        _ => 0,
    }
}

#[test]
fn map_screen_matches_retail_while_the_cursor_wanders_and_orders_the_ship() {
    let calls: u32 = std::env::var("SF2_SCREEN_CALLS").map(|v| v.parse().unwrap()).unwrap_or(3000);
    assert_eq!(compare_calls(wander, calls), calls);
}

#[test]
fn map_screen_matches_retail_through_a_campaign_driven_by_taps() {
    // Start and B tapped in turn, with the cursor drifting, keep the
    // campaign moving through its screens, stages and map events, until a
    // stage the taps cannot finish.
    let calls: u32 = std::env::var("SF2_SCREEN_CALLS").map(|v| v.parse().unwrap()).unwrap_or(1360);
    let schedule = |call: u32| match call % 16 {
        0 => START,
        8 => B,
        _ if call % 160 < 24 => [RIGHT, DOWN, LEFT, UP][(call / 160 % 4) as usize],
        _ => 0,
    };
    assert_eq!(compare_calls(schedule, calls), calls.min(1360));
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

fn set_word(s: &mut Snapshot, address: u16, value: u16) {
    s.low[usize::from(address)] = value as u8;
    s.low[usize::from(address.wrapping_add(1))] = (value >> 8) as u8;
}

fn toggle(s: &mut Snapshot, address: u16, bit: u16) {
    let value = s.word(address) ^ bit;
    set_word(s, address, value);
}

/// The live list from `head`, as record pointers.
fn list(s: &Snapshot, head: u16) -> Vec<u16> {
    let mut out = Vec::new();
    let mut pointer = s.word(head);
    while pointer != 0 && out.len() < 32 {
        out.push(pointer);
        pointer = s.word(pointer);
    }
    out
}

/// Every step offset of the scripts (`$04:CD6E`), for mid-script states.
fn script_steps(rom: &[u8]) -> Vec<u16> {
    let base = 0x2_4D6E; // $04:CD6E
    let word = |offset: usize| u16::from(rom[base + offset]) | u16::from(rom[base + offset + 1]) << 8;
    let mut steps = Vec::new();
    for script in 0..20 {
        let mut offset = usize::from(word(script * 2));
        while word(offset) != 1 {
            steps.push(offset as u16);
            offset += 2;
        }
    }
    steps
}

fn mutate_script(s: &mut Snapshot, rng: &mut Lcg, steps: &[u16]) {
    let units = list(s, 0xE0A3);
    let places = list(s, 0xDB67);
    match rng.below(3) {
        0 => set_word(s, 0xDA7F, 2 * (1 + rng.below(19)) as u16),
        _ => {
            set_word(s, 0xDA7F, 0xFFFF);
            set_word(s, 0xDA81, rng.pick(steps));
            set_word(s, 0xDA85, rng.pick(&[1u16, 2, 0x10]));
        }
    }
    if !units.is_empty() {
        for address in [0xDA83u16, 0xE097, 0xE099, 0xDB5F] {
            if rng.below(2) == 0 {
                set_word(s, address, rng.pick(&units));
            }
        }
        let unit = rng.pick(&units);
        match rng.below(4) {
            0 => set_word(s, unit + 0x08, rng.pick(&[1u16, 2, 0x10])),
            1 => toggle(s, unit + 0x2E, rng.pick(&[0x0002u16, 0x0008, 0x0020])),
            2 => toggle(s, unit + 0x30, 0x0400),
            _ => {}
        }
    }
    if !places.is_empty() && rng.below(2) == 0 {
        set_word(s, 0xDB5D, rng.pick(&places));
    }
    match rng.below(6) {
        0 => {
            s.low[0xDB25] = 0x10 + rng.below(0xD0) as u8;
            s.low[0xDB26] = 0x10 + rng.below(0x90) as u8;
        }
        1 => toggle(s, 0x1B86, rng.pick(&[0x0100u16, 0x1000])),
        2 => toggle(s, 0xDB23, rng.pick(&[0x0002u16, 0x0200])),
        3 => set_word(s, 0xDB05, rng.pick(&[1u16, 2, 0x20])),
        4 => set_word(s, 0xD998, rng.below(2) as u16),
        _ => set_word(s, 0xD9B0, rng.below(2) as u16),
    }
}

fn mutate(s: &mut Snapshot, rng: &mut Lcg, steps: &[u16]) {
    if rng.below(3) == 0 {
        mutate_script(s, rng, steps);
    }
    for _ in 0..1 + rng.below(4) {
        match rng.below(9) {
            0 => set_word(s, 0x1296, rng.pick(&[0u16, B, X, 0x4000, RIGHT, LEFT, START, 0x2000])),
            1 => set_word(s, 0x1292, rng.pick(&[0u16, UP, DOWN, LEFT, RIGHT, UP | LEFT, DOWN | RIGHT])),
            2 => toggle(s, 0xDA8B, rng.pick(&[0x0001u16, 0x0002, 0x0004, 0x0020, 0x0040, 0x0080, 0x0100, 0x0200, 0x0400])),
            3 => toggle(s, 0xDA8D, rng.pick(&[0x0001u16, 0x0008, 0x0010, 0x0020])),
            4 => toggle(s, 0xDB01, rng.pick(&[0x0001u16, 0x0002, 0x0004, 0x0008, 0x0400])),
            5 => toggle(s, 0x1B8C, rng.pick(&[0x0001u16, 0x0002, 0x0004, 0x0008, 0x0010])),
            6 => match rng.below(5) {
                0 => toggle(s, 0x1B92, rng.pick(&[0x0008u16, 0x0020, 0x0400])),
                1 => toggle(s, 0x1B88, rng.pick(&[0x0020u16, 0x0400])),
                2 => set_word(s, 0x1C0E, 1 << rng.below(7)),
                3 => set_word(s, 0xDB47, rng.below(0x40) as u16),
                _ => set_word(s, 0x1C12, rng.below(3) as u16),
            },
            7 => match rng.below(4) {
                0 => toggle(s, 0x1CD3, 0x0010),
                1 => set_word(s, 0xDA75, rng.below(3) as u16),
                2 => set_word(s, 0xDA71, rng.pick(&[0u16, 0x30, 0x80, 0x100, 0x200, 0x300, 0x8000])),
                _ => set_word(s, 0xDA8F, rng.pick(&[0u16, 1, 0x0C, 0x0D, 0x0F])),
            },
            _ => match rng.below(4) {
                0 => toggle(s, 0xDB23, rng.pick(&[0x0002u16, 0x0200, 0x0400])),
                1 => {
                    // The cursor onto a place, or onto the ship.
                    let head = s.word(0xDB67);
                    if head != 0 && rng.below(2) == 0 {
                        s.low[0xDA91] = s.byte(head + 0x0C);
                        s.low[0xDA92] = s.byte(head + 0x0E);
                    } else {
                        s.low[0xDA91] = s.byte(0xDAF3);
                        s.low[0xDA92] = s.byte(0xDAF6);
                    }
                }
                2 => set_word(s, 0xDAFB, rng.pick(&[0x40u16, 0x80, 0x100, 0x180])),
                _ => {
                    set_word(s, 0xDAE7, rng.below(7) as u16);
                    toggle(s, 0xDA8B, 0x0400);
                }
            },
        }
    }
}

/// Run the original frame on the oracle bus from this state.
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
    let exit = sf_oracle::call(&mut bus, FRAME, &sf_oracle::Entry::default());
    assert!(exit.returned, "the original frame did not return");
    Snapshot { low: (0..0x10000u32).map(|a| bus.read8(0x7E0000 + a)).collect(), terrain: s.terrain.clone() }
}

#[test]
fn map_screen_matches_the_original_on_mutated_retail_states() {
    let mut m = RetailMachine::new(rom());
    navigate_to_map(&mut m);
    let mut snapshots = Vec::new();
    for call in 0..1500u32 {
        let pad = wander(call);
        if !m.tick_until_cpu_execution(pad, FRAME, 300).unwrap() {
            break;
        }
        if call % 50 == 0 {
            snapshots.push(Snapshot::take(&m));
        }
    }
    assert!(snapshots.len() > 10);
    let cases: u32 = std::env::var("SF2_SCREEN_FUZZ").map(|v| v.parse().unwrap()).unwrap_or(40);
    let mut rng = Lcg(0x0B5D_A1F0);
    let steps = script_steps(&rom());
    let (mut compared, mut faulted) = (0, std::collections::BTreeMap::new());
    for (index, snapshot) in snapshots.iter().enumerate() {
        for case in 0..cases {
            let mut state = Snapshot { low: snapshot.low.clone(), terrain: snapshot.terrain.clone() };
            mutate(&mut state, &mut rng, &steps);
            let mut probe = screen(&state);
            let mut map = state.map();
            let mut probe_links = links(&state);
            if let Err(error) =
                strategic_screen::frame(
                &mut probe,
                &mut map,
                &mut probe_links,
                &state.terrain,
                pad(&state),
                &mut ScreenOutput::default(),
            )
            {
                *faulted.entry(format!("{error:x?}")).or_insert(0) += 1;
                continue;
            }
            let after = run_original(&state);
            compare(&state, &after, &format!("snapshot {index} case {case}")).unwrap();
            compared += 1;
        }
    }
    eprintln!("compared {compared} mutated frames; native faults {faulted:?}");
    assert!(compared > 0);
}

// ---- the map program's frame ($04:B9A3) ----

const PROGRAM: u32 = 0x04B9A3;
const PROGRAM_RETURN: u32 = 0x04AEAB;

fn director(s: &Snapshot) -> MapDirector {
    MapDirector {
        aftermath: s.word(0xDA0D),
        aftermath_next: s.word(0xDA0F),
        dialog_hold: s.word(0xDA1D),
        message_wait: s.word(0xDA13),
        exit_state: s.word(0xDA21),
        timeline_next: s.word(0xD9FF),
        timeline_delay: s.word(0xDA05),
        schedule_cursor: s.word(0xDA01),
        schedule_steps: s.word(0xDA03),
        handshake: s.word(0xD7FA),
        recapture: s.word(0xDA15),
        ambush: s.word(0xDA17),
        clearing: s.word(0xDA19),
        alert: s.word(0xDA07),
        alert_timer: s.word(0xDA0B),
        threat_delay: s.word(0xDA6D),
        pending_alerts: s.word(0xDB37),
        saved_hold: s.word(0x1B8E),
        choice: s.word(0xD9C2),
        choosing: s.word(0xD9C4),
        sortie_timer: s.word(0xE08B),
        sortie_escort: s.unit_id(s.word(0xE08D)),
        salvo_index: s.word(0xD9B2),
        tally: MapTally {
            goal: s.word(0xDA25),
            enemies_left: s.word(0xDA29),
            marks_left: s.word(0xDA2D),
            marks_cleared: s.word(0xDA33),
            fleets: s.word(0xDA37),
            fleets_cleared: s.word(0xDA3D),
            other_cleared: s.word(0xDA49),
            bases_cleared: s.word(0xDA4F),
        },
    }
}

/// Runs the native program frame on `before` and checks it against `after`.
fn compare_program(before: &Snapshot, after: &Snapshot, context: &str) -> Result<(), strategic_screen::ScreenError> {
    let mut native = director(before);
    let mut native_screen = screen(before);
    let mut map = before.map();
    let mut native_links = links(before);
    let mut output = ScreenOutput::default();
    strategic_director::step(&mut native, &mut native_screen, &mut map, &mut native_links, &before.terrain, &mut output)?;
    let expected_map: StrategicMap = after.map();
    for index in 0..PLACE_CAPACITY {
        assert_eq!(map.places[index], expected_map.places[index], "{context}: place {index}");
    }
    for index in 0..UNIT_CAPACITY {
        assert_eq!(map.units[index], expected_map.units[index], "{context}: unit {index}");
    }
    assert_eq!(map.globals, expected_map.globals, "{context}: globals");
    assert_eq!(
        (map.place_head, map.place_free, map.unit_head, map.unit_free),
        (expected_map.place_head, expected_map.place_free, expected_map.unit_head, expected_map.unit_free),
        "{context}: list heads"
    );
    assert_eq!(native, director(after), "{context}: director");
    assert_eq!(native_screen, screen(after), "{context}: screen");
    assert_eq!(native_links, links(after), "{context}: links");
    assert_eq!(output.cues, queued_cues(before, after), "{context}: cues");
    match output.radio {
        Some(voice) => assert_eq!((after.byte(0x1CDA), after.byte(0x1CD9)), (voice, 0), "{context}: radio"),
        None => assert_eq!(
            (after.byte(0x1CDA), after.byte(0x1CD9)),
            (before.byte(0x1CDA), before.byte(0x1CD9)),
            "{context}: radio"
        ),
    }
    if let Some(frame) = output.ship_frame {
        let pointer = frame_pointer(before, frame);
        assert_eq!((after.word(0x1C17), after.word(0x1C19)), (pointer, pointer.wrapping_add(0x200)), "{context}: ship frame");
    }
    Ok(())
}

/// Runs up to `calls` retail program frames under `schedule`; returns how
/// many the native frame reproduced before the first fault. Calls that an
/// interrupt's screen frame lands inside are skipped.
fn compare_program_calls(schedule: fn(u32) -> u16, calls: u32) -> (u32, u32) {
    let mut m = RetailMachine::new(rom());
    navigate_to_map(&mut m);
    let (mut matched, mut skipped) = (0, 0);
    for call in 0..calls {
        let pad = schedule(call);
        let mut reached = false;
        for _ in 0..300 {
            if m.tick_until_cpu_execution(pad, PROGRAM, 90).unwrap() {
                reached = true;
                break;
            }
            for button in [START, B] {
                m.tick_video_frames(button, 4).unwrap();
                m.tick_video_frames(0, 30).unwrap();
            }
        }
        if !reached {
            eprintln!("call {call}: the map program stopped running (mode {:04X})", m.peek16(0x7E1B68));
            break;
        }
        let before = Snapshot::take(&m);
        m.watch_cpu_execution(&[FRAME, 0x7F537D]);
        assert!(m.tick_until_cpu_execution(pad, PROGRAM_RETURN, 60).unwrap());
        let interrupted = !m.take_cpu_execution_watch_hits().is_empty();
        m.watch_cpu_execution(&[]);
        let after = Snapshot::take(&m);
        if interrupted {
            skipped += 1;
            matched = call + 1;
            continue;
        }
        if let Err(error) = compare_program(&before, &after, &format!("call {call}")) {
            eprintln!(
                "call {call}: native program faulted: {error:x?} (aftermath {:04X}, timeline {:04X})",
                before.word(0xDA0D),
                before.word(0xD9FD)
            );
            break;
        }
        matched = call + 1;
    }
    eprintln!("map program matched {matched} frames ({skipped} interrupted, skipped)");
    (matched, skipped)
}

#[test]
fn map_program_matches_retail_through_a_campaign_driven_by_taps() {
    let calls: u32 = std::env::var("SF2_PROGRAM_CALLS").map(|v| v.parse().unwrap()).unwrap_or(3000);
    let schedule = |call: u32| match call % 16 {
        0 => START,
        8 => B,
        _ if call % 160 < 24 => [RIGHT, DOWN, LEFT, UP][(call / 160 % 4) as usize],
        _ => 0,
    };
    let (matched, _) = compare_program_calls(schedule, calls);
    assert_eq!(matched, calls);
}

#[test]
fn map_program_matches_retail_while_the_cursor_wanders() {
    // The wandering ship never stops the enemy: the campaign is lost.
    let calls: u32 = std::env::var("SF2_PROGRAM_CALLS").map(|v| v.parse().unwrap()).unwrap_or(1556);
    let (matched, _) = compare_program_calls(wander, calls);
    assert_eq!(matched, calls.min(1556));
}

fn mutate_program(s: &mut Snapshot, rng: &mut Lcg) {
    let places = list(s, 0xDB67);
    let units = list(s, 0xE0A3);
    for _ in 0..1 + rng.below(4) {
        match rng.below(12) {
            0 => {
                set_word(s, 0xDA0D, 2 * rng.below(0x35) as u16);
                set_word(s, 0xDA0F, 2 * rng.below(0x35) as u16);
            }
            1 => {
                set_word(s, 0xD9FD, 2 * rng.below(0x24) as u16);
                set_word(s, 0xD9FF, 2 * rng.below(0x24) as u16);
                set_word(s, 0xDA05, rng.below(3) as u16);
            }
            2 => toggle(s, 0xD7FA, 1 << rng.below(12)),
            3 => toggle(s, 0xD7F8, 1 << rng.below(11)),
            4 => match rng.below(5) {
                0 => set_word(s, 0xDA15, 2 * rng.below(6) as u16),
                1 => set_word(s, 0xDA17, 2 * rng.below(4) as u16),
                2 => set_word(s, 0xDA19, 2 * rng.below(10) as u16),
                3 => set_word(s, 0xDA21, 2 * rng.below(3) as u16),
                _ => set_word(s, 0xDB29, 2 * rng.below(10) as u16),
            },
            5 => {
                set_word(s, 0xDA69, rng.below(8) as u16);
                if !places.is_empty() {
                    set_word(s, 0xDA6B, rng.pick(&places));
                }
                set_word(s, 0xDA6D, rng.below(2) as u16);
            }
            6 => {
                set_word(s, 0xE089, rng.below(5) as u16);
                toggle(s, 0xE087, rng.pick(&[0x0008u16, 0x0040, 0x0080, 0x0800]));
                set_word(s, 0xE093, rng.below(2) as u16);
                set_word(s, 0xE08B, rng.below(3) as u16);
                if !units.is_empty() {
                    set_word(s, 0xE08D, rng.pick(&units));
                }
            }
            7 => match rng.below(6) {
                0 => set_word(s, 0xDA7F, rng.below(2) as u16 * 4),
                1 => set_word(s, 0xDA13, rng.below(2) as u16),
                2 => set_word(s, 0xDA11, rng.below(3) as u16),
                3 => set_word(s, 0xDA1D, rng.below(4) as u16),
                4 => set_word(s, 0xDA07, 2 * rng.below(5) as u16),
                _ => set_word(s, 0xDA0B, rng.below(3) as u16),
            },
            8 => match rng.below(8) {
                0 => set_word(s, 0xDA2F, rng.below(3) as u16),
                1 => set_word(s, 0xDA3B, rng.below(3) as u16),
                2 => set_word(s, 0xDA43, rng.below(3) as u16),
                3 => set_word(s, 0xDA45, rng.below(2) as u16),
                4 => set_word(s, 0xDA57, rng.below(2) as u16),
                5 => set_word(s, 0xDA29, rng.below(3) as u16),
                6 => set_word(s, 0xDA2D, rng.below(2) as u16),
                _ => set_word(s, 0xD9B0, rng.below(2) as u16),
            },
            9 => match rng.below(6) {
                0 => toggle(s, 0x1B94, 1 << rng.below(9)),
                1 => toggle(s, 0x1B8A, rng.pick(&[0x0004u16, 0x0008, 0x0020, 0x0040, 0x0080, 0x0200])),
                2 => toggle(s, 0x1B86, rng.pick(&[0x0001u16, 0x0100, 0x1000])),
                3 => toggle(s, 0x1B92, rng.pick(&[0x1000u16, 0x4000, 0x8000])),
                4 => set_word(s, 0xD79D, rng.pick(&[0u16, 1, 2, 0xFFFF])),
                _ => set_word(s, 0xD7F4, rng.below(3) as u16),
            },
            10 => {
                if !units.is_empty() {
                    set_word(s, 0xE097, rng.pick(&units));
                    set_word(s, 0xE099, rng.pick(&units));
                    let unit = rng.pick(&units);
                    toggle(s, unit + 0x30, 0x0400);
                }
                set_word(s, 0x1C21, rng.pick(&[0u16, 0x8000, 0x0040, 0x0800, 0x0400]));
            }
            _ => match rng.below(4) {
                0 => set_word(s, 0x1C6E, rng.below(3) as u16),
                1 => set_word(s, 0xD7F2, rng.below(3) as u16),
                2 => set_word(s, 0xDA5B, rng.below(0x40) as u16),
                _ => set_word(s, 0xDA01, rng.pick(&[0u16, 4, 8, 0x1A, 0x22, 0x50, 0x58])),
            },
        }
    }
}

/// Run the original program frame on the oracle bus from this state.
fn run_original_program(s: &Snapshot) -> Snapshot {
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
    let entry = sf_oracle::Entry { dbr: 0x7E, p: 0x20, ..Default::default() };
    let exit = sf_oracle::call_near(&mut bus, PROGRAM, &entry);
    assert!(exit.returned, "the original program frame did not return");
    Snapshot { low: (0..0x10000u32).map(|a| bus.read8(0x7E0000 + a)).collect(), terrain: s.terrain.clone() }
}

#[test]
fn map_program_matches_the_original_on_mutated_retail_states() {
    let mut m = RetailMachine::new(rom());
    navigate_to_map(&mut m);
    let mut snapshots = Vec::new();
    for call in 0..3000u32 {
        let pad = match call % 16 {
            0 => START,
            8 => B,
            _ => 0,
        };
        let mut reached = false;
        for _ in 0..300 {
            if m.tick_until_cpu_execution(pad, PROGRAM, 90).unwrap() {
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
        if call % 100 == 0 {
            snapshots.push(Snapshot::take(&m));
        }
    }
    assert!(snapshots.len() > 10);
    let cases: u32 = std::env::var("SF2_PROGRAM_FUZZ").map(|v| v.parse().unwrap()).unwrap_or(60);
    let mut rng = Lcg(0x04B9_A3D1);
    let (mut compared, mut faulted) = (0, std::collections::BTreeMap::new());
    for (index, snapshot) in snapshots.iter().enumerate() {
        for case in 0..cases {
            let mut state = Snapshot { low: snapshot.low.clone(), terrain: snapshot.terrain.clone() };
            mutate_program(&mut state, &mut rng);
            let mut probe = director(&state);
            let mut probe_screen = screen(&state);
            let mut map = state.map();
            let mut probe_links = links(&state);
            if let Err(error) = strategic_director::step(
                &mut probe,
                &mut probe_screen,
                &mut map,
                &mut probe_links,
                &state.terrain,
                &mut ScreenOutput::default(),
            ) {
                *faulted.entry(format!("{error:x?}")).or_insert(0) += 1;
                continue;
            }
            let after = run_original_program(&state);
            compare_program(&state, &after, &format!("snapshot {index} case {case}")).unwrap();
            compared += 1;
        }
    }
    eprintln!("compared {compared} mutated program frames; native faults {faulted:?}");
    assert!(compared > 0);
}
