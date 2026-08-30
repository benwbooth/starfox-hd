//! Focused Super FX timing regressions measured from the retail Rev 2 cart.

use sf_oracle::{load_retail_rom, SnesBus};

const CARTRIDGE_RAM: u32 = 0x70_0000;
const RETAIL_CLEAR_BITMAP_FLAG: u32 = CARTRIDGE_RAM | 0x01B2;
const PROGRAM_BANK_REGISTER: u32 = 0x00_3034;
const CLOCK_SELECT_REGISTER: u32 = 0x00_3039;
const SCREEN_MODE_REGISTER: u32 = 0x00_303A;
const PROGRAM_COUNTER_LOW_REGISTER: u32 = 0x00_301E;
const PROGRAM_COUNTER_HIGH_REGISTER: u32 = 0x00_301F;
const STATUS_REGISTER: u32 = 0x00_3030;
const RUNNING_FLAG: u8 = 0x20;

const CLEAR_FIRST_BITMAP_BANK: u8 = 0x01;
const RETAIL_CLEAR_FIRST_BITMAP_ENTRY: u16 = 0xB0CB;
const SLOW_CLOCK_SELECT: u8 = 0;
const RAM_ONLY_ACCESS: u8 = 0x08;
const RAM_AND_ROM_ACCESS: u8 = 0x18;
const RETAIL_INSTRUCTION_COUNT: u64 = 21_516;
const FINAL_STOP_INSTRUCTION: u64 = RETAIL_INSTRUCTION_COUNT - 1;
const DENIED_ROM_WINDOW_MASTER_CLOCKS: u64 = 1_000;
const COMPLETION_BUDGET_MASTER_CLOCKS: u64 = 200_000;

#[test]
fn clear_first_bitmap_defers_stop_while_program_rom_is_denied() {
    let Some(rom) = load_retail_rom() else {
        eprintln!("skip: retail Rev 2 ROM not found at repository root");
        return;
    };

    let mut bus = SnesBus::new(rom);
    bus.enable_gsu();
    bus.write8(RETAIL_CLEAR_BITMAP_FLAG, 1);
    bus.write8(PROGRAM_BANK_REGISTER, CLEAR_FIRST_BITMAP_BANK);
    bus.write8(CLOCK_SELECT_REGISTER, SLOW_CLOCK_SELECT);
    bus.write8(SCREEN_MODE_REGISTER, RAM_AND_ROM_ACCESS);
    bus.write8(
        PROGRAM_COUNTER_LOW_REGISTER,
        RETAIL_CLEAR_FIRST_BITMAP_ENTRY as u8,
    );
    bus.write8(
        PROGRAM_COUNTER_HIGH_REGISTER,
        (RETAIL_CLEAR_FIRST_BITMAP_ENTRY >> 8) as u8,
    );

    let mut elapsed_master_clocks = 0;
    while bus
        .gsu_ref()
        .is_some_and(|gsu| gsu.last_run_steps < FINAL_STOP_INSTRUCTION)
        && elapsed_master_clocks < COMPLETION_BUDGET_MASTER_CLOCKS
    {
        bus.tick_gsu(1);
        elapsed_master_clocks += 1;
    }
    assert_eq!(
        bus.gsu_ref().expect("attached GSU").last_run_steps,
        FINAL_STOP_INSTRUCTION,
    );

    // The final STOP lies on a new cache line. SF1's bitmap-transfer IRQ
    // temporarily leaves the GSU its RAM bus but revokes its program ROM bus.
    // Mesen keeps the go flag set until a later IRQ restores both grants.
    bus.write8(SCREEN_MODE_REGISTER, RAM_ONLY_ACCESS);
    for _ in 0..DENIED_ROM_WINDOW_MASTER_CLOCKS {
        bus.tick_gsu(1);
    }
    assert_ne!(bus.read8(STATUS_REGISTER) & RUNNING_FLAG, 0);
    assert_eq!(
        bus.gsu_ref().expect("attached GSU").last_run_steps,
        FINAL_STOP_INSTRUCTION,
    );
    let denied_fetch_timing = bus.gsu_ref().expect("attached GSU").timing_breakdown();

    bus.write8(SCREEN_MODE_REGISTER, RAM_AND_ROM_ACCESS);
    bus.tick_gsu(1);
    elapsed_master_clocks += 1;
    while bus.read8(STATUS_REGISTER) & RUNNING_FLAG != 0
        && elapsed_master_clocks < COMPLETION_BUDGET_MASTER_CLOCKS
    {
        bus.tick_gsu(1);
        elapsed_master_clocks += 1;
    }

    let runs = bus.gsu_recent_runs();
    let run = runs.last().expect("bitmap clear must reach STOP");
    assert_eq!(run.pbr, CLEAR_FIRST_BITMAP_BANK);
    assert_eq!(run.pc, RETAIL_CLEAR_FIRST_BITMAP_ENTRY);
    assert_eq!(run.steps, RETAIL_INSTRUCTION_COUNT);
    assert_eq!(
        run.timing_breakdown, denied_fetch_timing,
        "restoring SCMR must retire the buffered STOP without a second fetch",
    );
}
