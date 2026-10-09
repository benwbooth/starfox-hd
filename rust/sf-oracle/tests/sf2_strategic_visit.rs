//! A strategic map visit against the retail machine, one program frame at
//! a time: retail's state as a program frame begins is decoded into a
//! native `MapVisit`, which then runs the program frame and the eight
//! service frames that follow (with the pads retail read in each), and is
//! compared with retail's state as the next program frame begins.

#[path = "support/sf2_strategic_snapshot.rs"]
mod strategic_snapshot;
#[path = "support/sf2_map_snapshot.rs"]
mod map_snapshot;

use map_snapshot::*;
use sf2_game::strategic_screen::{MapPad, ScreenOutput};
use sf2_game::strategic_sim::{PLACE_CAPACITY, UNIT_CAPACITY};
use sf2_game::strategic_visit::MapVisit;
use sf_oracle::RetailMachine;
use strategic_snapshot::Snapshot;

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

/// Retail's next program frame under `schedule`, with the pads each
/// service frame read; `None` once the program stops running.
fn next_program_frame(m: &mut RetailMachine, schedule: &dyn Fn(u64) -> u16) -> Option<Vec<MapPad>> {
    let mut pads = Vec::new();
    for _ in 0..2000 {
        let pad = schedule(m.video_frame());
        match m.tick_until_cpu_execution_any(pad, &[FRAME, PROGRAM], 1).unwrap() {
            Some(FRAME) => pads.push(MapPad { held: m.peek16(0x7E1292), pressed: m.peek16(0x7E1296) }),
            Some(_) => return Some(pads),
            None => {}
        }
    }
    None
}

fn compare_visit(schedule: &dyn Fn(u64) -> u16, program_frames: u32) -> u32 {
    let mut m = RetailMachine::new(rom());
    navigate_to_map(&mut m);
    assert!(m.tick_until_cpu_execution(0, PROGRAM, 600).unwrap(), "no map program");
    let mut before = Snapshot::take(&m);
    let mut matched = 0;
    let mut scripts = std::collections::BTreeSet::new();
    let mut messages = std::collections::BTreeSet::new();
    for frame in 0..program_frames {
        let watch = std::env::var("SF2_VISIT_WATCH").ok().map(|v| u32::from_str_radix(&v, 16).unwrap());
        if let Some(address) = watch {
            m.arm_wram_write_watch(address);
        }
        let pads = next_program_frame(&mut m, schedule);
        if watch.is_some() {
            let hits = m.take_wram_write_watch();
            if !hits.is_empty() {
                eprintln!("program frame {frame}: writes {hits:06X?}");
            }
        }
        let Some(pads) = pads else {
            eprintln!("program frame {frame}: the map program stopped running");
            break;
        };
        let after = Snapshot::take(&m);
        let context = format!("program frame {frame}");
        let mut native = visit(&before);
        let mut output = ScreenOutput::default();
        let result = (|| {
            native.program_frame(&mut output)?;
            for &pad in &pads {
                native.interrupt(pad, &mut output)?;
            }
            Ok::<_, sf2_game::strategic_visit::VisitError>(())
        })();
        if let Err(error) = result {
            eprintln!("{context}: native visit faulted: {error:x?}");
            break;
        }
        assert_eq!(pads.len(), 8, "{context}: service frames");
        assert!(native.program_frame_due(), "{context}: the picture is not finished");
        native.begin_program_frame();
        let expected = visit(&after);
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
        assert_eq!(native.hud, expected.hud, "{context}: hud");
        assert_eq!(native.radio, expected.radio, "{context}: radio");
        assert_eq!(native.message_box, expected.message_box, "{context}: message box");
        assert_eq!(native.launch, expected.launch, "{context}: launch");

        assert_eq!(output.cues, queued_cues(&before, &after), "{context}: cues");
        scripts.insert(expected.links.message);
        messages.insert(expected.message_box.message);
        matched = frame + 1;
        before = after;
    }
    eprintln!("map visit matched {matched} program frames; scripts {scripts:04X?}; messages {messages:04X?}");
    matched
}

#[test]
fn map_visit_matches_retail_through_a_campaign_driven_by_taps() {
    // The taps let the planet fall at program frame 492; the radio tells of
    // it and the map program leaves after frame 531.
    let frames: u32 = std::env::var("SF2_VISIT_FRAMES").map(|v| v.parse().unwrap()).unwrap_or(531);
    let schedule = |frame: u64| match frame % 32 {
        0..=3 => START,
        16..=19 => B,
        24..=27 if frame % 256 < 128 => RIGHT,
        24..=27 => DOWN,
        _ => 0,
    };
    assert_eq!(compare_visit(&schedule, frames), frames);
}

#[test]
fn map_visit_matches_retail_with_the_radio_answered() {
    // A answers the radio and opens the boxes the cursor rests on.
    let frames: u32 = std::env::var("SF2_VISIT_FRAMES_A").map(|v| v.parse().unwrap()).unwrap_or(2000);
    // The cursor creeps across the map and rests between steps, so the
    // boxes for the places it passes open; START and A now and then.
    let schedule = |frame: u64| {
        let step = frame / 40;
        match frame % 40 {
            0..=5 => [RIGHT, DOWN, RIGHT, UP, LEFT, DOWN, LEFT, UP][(step / 6 % 8) as usize],
            20..=23 if step % 16 == 0 => START,
            20..=23 if step % 16 == 8 => A,
            _ => 0,
        }
    };
    let matched = compare_visit(&schedule, frames);
    assert!(matched > 0);
}
