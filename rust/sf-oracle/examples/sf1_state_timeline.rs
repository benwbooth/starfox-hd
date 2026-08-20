//! Print stable Star Fox retail observables at the native port's 20 Hz cadence.
//! This is oracle archaeology; source storage and execution details stay here.

use sf_core::pad;
use sf_oracle::{
    RetailMachine, RETAIL_BGFLAGS, RETAIL_BRIEFING_CHOICE, RETAIL_CONTROLLER_TRIGGER_HIGH,
    RETAIL_CONTROLLER_TRIGGER_LOW, RETAIL_CURRENTBG, RETAIL_CURRENT_PLANET,
    RETAIL_DEFAULT_TRAINING, RETAIL_FADEDIR, RETAIL_GAMEFRAME, RETAIL_PEPPER_CHARACTERS,
    RETAIL_PLANET_BRIEFING_PREP_ENTRY, RETAIL_PLANET_CENTER_ENTRY, RETAIL_PLANET_DISMISS_ENTRY,
    RETAIL_PLANET_EXIT_FADE_ENTRY, RETAIL_PLANET_FADE_COUNT, RETAIL_PLANET_GAME_START_ENTRY,
    RETAIL_PLANET_INTERRUPT, RETAIL_PLANET_ISOLATION_ENTRY, RETAIL_PLANET_MAP_FADE_ENTRY,
    RETAIL_PLANET_MESSAGE_ENTRY, RETAIL_PLANET_NAME_ENTRY, RETAIL_PLANET_RADIUS,
    RETAIL_PLANET_SHIP_FLASH, RETAIL_PLANET_STAGE, RETAIL_PLANET_ZOOM_ENTRY, RETAIL_POOL,
    RETAIL_PSHIPFLAGS, RETAIL_STAGECNT, RETAIL_WHICH_ROUTE,
};
use std::collections::BTreeSet;
use std::path::Path;

const DEFAULT_TICKS: u32 = 240;
const VIDEO_FRAMES_PER_TICK: u32 = 3;
const WORK_RAM: u32 = 0x7E_0000;
const BRIEFING_CONTROL_DISABLED_MASK: u8 = 0x60;
const FRONT_END_CONFIRM_CADENCE_TICKS: u32 = 60;
const FRONT_END_CONFIRM_HOLD_TICKS: u32 = 2;
const FRONT_END_LAST_CONFIRM_TICK: u32 = 360;
const GAME_DESTINATION_SELECT_TICK: u32 = 380;
const GAME_DESTINATION_CONFIRM_TICK: u32 = 420;
const ROUTE_SELECTION_CONFIRM_TICK: u32 = 500;
const ROUTE_SELECTION_CONFIRM_HOLD_TICKS: u32 = 12;
const POST_ROUTE_TRACE_START_TICK: u32 = 490;
const PLANET_DISMISS_START_TICK: u32 = 840;
const PLANET_DISMISS_END_TICK: u32 = 900;
const PLANET_DISMISS_CADENCE_TICKS: u32 = 2;
const LASER_FIRE_START_TICK: u32 = 1_000;
const LASER_FIRE_CADENCE_TICKS: u32 = 8;
const LASER_FIRE_HOLD_TICKS: u32 = 4;

fn scripted_input(tick: u32) -> u16 {
    if (GAME_DESTINATION_SELECT_TICK..GAME_DESTINATION_SELECT_TICK + FRONT_END_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::DOWN;
    }
    if (GAME_DESTINATION_CONFIRM_TICK..GAME_DESTINATION_CONFIRM_TICK + FRONT_END_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::START;
    }
    if tick <= FRONT_END_LAST_CONFIRM_TICK {
        return if tick % FRONT_END_CONFIRM_CADENCE_TICKS < FRONT_END_CONFIRM_HOLD_TICKS {
            pad::START
        } else {
            0
        };
    }
    if (ROUTE_SELECTION_CONFIRM_TICK
        ..ROUTE_SELECTION_CONFIRM_TICK + ROUTE_SELECTION_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::START;
    }
    if (PLANET_DISMISS_START_TICK..PLANET_DISMISS_END_TICK).contains(&tick) {
        return if (tick - PLANET_DISMISS_START_TICK) % PLANET_DISMISS_CADENCE_TICKS == 0 {
            pad::B
        } else {
            0
        };
    }
    if tick < LASER_FIRE_START_TICK {
        return 0;
    }
    if tick % LASER_FIRE_CADENCE_TICKS < LASER_FIRE_HOLD_TICKS {
        pad::Y
    } else {
        0
    }
}

fn active_object_count(machine: &RetailMachine) -> Result<usize, String> {
    let mut current = machine.peek16(WORK_RAM | RETAIL_POOL.active_head);
    let mut visited = BTreeSet::new();
    while current != 0 {
        let offset = u32::from(current);
        let pool_end = RETAIL_POOL.base + RETAIL_POOL.stride * RETAIL_POOL.count;
        if offset < RETAIL_POOL.base
            || offset >= pool_end
            || (offset - RETAIL_POOL.base) % RETAIL_POOL.stride != 0
        {
            return Err(format!(
                "active object link {current:#06X} is outside the retail pool"
            ));
        }
        if !visited.insert(current) {
            return Err(format!("active object list loops at {current:#06X}"));
        }
        current = machine.peek16(WORK_RAM | (offset + RETAIL_POOL.al_next));
    }
    Ok(visited.len())
}

fn main() {
    let tick_limit = std::env::args()
        .nth(1)
        .map(|value| value.parse::<u32>().expect("tick limit must be decimal"))
        .unwrap_or(DEFAULT_TICKS);
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace has a repository parent");
    let rom_path = repository.join("Star Fox (USA) (Rev 2).sfc");
    let rom = std::fs::read(&rom_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", rom_path.display()));
    let mut machine = RetailMachine::new(rom);
    machine.watch_cpu_execution(&[
        RETAIL_PLANET_MAP_FADE_ENTRY,
        RETAIL_PLANET_ISOLATION_ENTRY,
        RETAIL_PLANET_CENTER_ENTRY,
        RETAIL_PLANET_BRIEFING_PREP_ENTRY,
        RETAIL_PLANET_ZOOM_ENTRY,
        RETAIL_PLANET_NAME_ENTRY,
        RETAIL_PLANET_MESSAGE_ENTRY,
        RETAIL_PLANET_DISMISS_ENTRY,
        RETAIL_PLANET_EXIT_FADE_ENTRY,
        RETAIL_PLANET_GAME_START_ENTRY,
    ]);
    let mut previous = None;

    println!(
        "tick video_frame input trigger game_frame background background_flags fade_direction stage briefing_control_disabled destination default_training planet_interrupt route route_stage planet fade_count ship_flash pepper_characters planet_radius active_objects nonblack execution"
    );
    for tick in 0..tick_limit {
        let input = scripted_input(tick);
        machine
            .tick_video_frames(input, VIDEO_FRAMES_PER_TICK)
            .unwrap_or_else(|error| panic!("retail machine failed: {error}"));
        for entry in machine.take_cpu_execution_watch_hits() {
            println!("phase_entry {tick} {entry:#08X}");
        }
        let state = (
            machine.peek16(WORK_RAM | RETAIL_GAMEFRAME),
            machine.peek16(WORK_RAM | RETAIL_CURRENTBG),
            machine.peek8(WORK_RAM | RETAIL_BGFLAGS),
            machine.peek8(WORK_RAM | RETAIL_FADEDIR) as i8,
            machine.peek16(WORK_RAM | RETAIL_STAGECNT),
            active_object_count(&machine)
                .unwrap_or_else(|error| panic!("invalid retail object state: {error}")),
        );
        let summary = state;
        if previous != Some(summary) || input != 0 || tick >= POST_ROUTE_TRACE_START_TICK {
            let nonblack = machine
                .ppu_frame()
                .rgba
                .chunks_exact(4)
                .filter(|pixel| pixel[..3] != [0, 0, 0])
                .count();
            println!(
                "{} {} {input:#06X} {:#06X} {} {} {:#04X} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {:#08X}",
                tick,
                machine.video_frame(),
                u16::from_le_bytes([
                    machine.peek8(WORK_RAM | RETAIL_CONTROLLER_TRIGGER_LOW),
                    machine.peek8(WORK_RAM | RETAIL_CONTROLLER_TRIGGER_HIGH),
                ]),
                state.0,
                state.1,
                state.2,
                state.3,
                state.4,
                machine.peek8(WORK_RAM | RETAIL_PSHIPFLAGS) & BRIEFING_CONTROL_DISABLED_MASK,
                machine.peek8(RETAIL_BRIEFING_CHOICE),
                machine.peek8(WORK_RAM | RETAIL_DEFAULT_TRAINING),
                machine.peek8(WORK_RAM | RETAIL_PLANET_INTERRUPT),
                machine.peek8(WORK_RAM | RETAIL_WHICH_ROUTE),
                machine.peek8(WORK_RAM | RETAIL_PLANET_STAGE),
                machine.peek8(WORK_RAM | RETAIL_CURRENT_PLANET) as i8,
                machine.peek16(WORK_RAM | RETAIL_PLANET_FADE_COUNT),
                machine.peek8(WORK_RAM | RETAIL_PLANET_SHIP_FLASH),
                machine.peek8(RETAIL_PEPPER_CHARACTERS),
                machine.peek16(RETAIL_PLANET_RADIUS),
                state.5,
                nonblack,
                machine.pc(),
            );
            previous = Some(summary);
        }
    }
}
