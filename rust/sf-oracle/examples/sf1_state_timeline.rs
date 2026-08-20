//! Print stable Star Fox retail observables at the native port's 20 Hz cadence.
//! This is oracle archaeology; source storage and execution details stay here.

use sf_core::pad;
use sf_oracle::{
    RetailMachine, RETAIL_BGFLAGS, RETAIL_CURRENTBG, RETAIL_FADEDIR, RETAIL_GAMEFRAME, RETAIL_POOL,
    RETAIL_STAGECNT,
};
use std::collections::BTreeSet;
use std::path::Path;

const DEFAULT_TICKS: u32 = 240;
const VIDEO_FRAMES_PER_TICK: u32 = 3;
const WORK_RAM: u32 = 0x7E_0000;
const CONTROLLER_STATE: u32 = 0x1202;
const FRONT_END_CONFIRM_CADENCE_TICKS: u32 = 60;
const FRONT_END_CONFIRM_HOLD_TICKS: u32 = 2;
const FRONT_END_LAST_CONFIRM_TICK: u32 = 400;
const ROUTE_SELECT_CONFIRM_TICK: u32 = 420;
const PLANET_DISMISS_START_TICK: u32 = 540;
const PLANET_DISMISS_END_TICK: u32 = 600;
const PLANET_DISMISS_CADENCE_TICKS: u32 = 2;
const LASER_FIRE_CADENCE_TICKS: u32 = 8;
const LASER_FIRE_HOLD_TICKS: u32 = 4;

fn scripted_input(tick: u32) -> u16 {
    if (ROUTE_SELECT_CONFIRM_TICK..ROUTE_SELECT_CONFIRM_TICK + FRONT_END_CONFIRM_HOLD_TICKS)
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
    if (PLANET_DISMISS_START_TICK..PLANET_DISMISS_END_TICK).contains(&tick) {
        return if (tick - PLANET_DISMISS_START_TICK) % PLANET_DISMISS_CADENCE_TICKS == 0 {
            pad::B
        } else {
            0
        };
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
    let mut previous = None;

    println!(
        "tick video_frame input accepted_input game_frame background background_flags fade_direction stage active_objects nonblack execution"
    );
    for tick in 0..tick_limit {
        let input = scripted_input(tick);
        machine
            .tick_video_frames(input, VIDEO_FRAMES_PER_TICK)
            .unwrap_or_else(|error| panic!("retail machine failed: {error}"));
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
        if previous != Some(summary) || input != 0 {
            let nonblack = machine
                .ppu_frame()
                .rgba
                .chunks_exact(4)
                .filter(|pixel| pixel[..3] != [0, 0, 0])
                .count();
            println!(
                "{} {} {input:#06X} {:#04X} {} {} {:#04X} {} {} {} {} {:#08X}",
                tick,
                machine.video_frame(),
                machine.peek8(WORK_RAM | CONTROLLER_STATE),
                state.0,
                state.1,
                state.2,
                state.3,
                state.4,
                state.5,
                nonblack,
                machine.pc(),
            );
            previous = Some(summary);
        }
    }
}
