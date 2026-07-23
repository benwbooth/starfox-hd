//! Boot the retail Star Fox 2 ROM and report when its RAM-resident object
//! strategies are installed.
//!
//! This is an archaeology tool, not a gameplay emulator.  It reuses the
//! minimal raster/APU/controller hardware shims from the SF1 retail oracle and
//! watches an arbitrary WRAM range (defaulting to the two strategy targets
//! proven reachable from SF2's map bytecode).
//! Run from the workspace with:
//!
//!     cargo run -p sf-oracle --example sf2_boot_capture -- 50000000 7F7E00 40

use sf_oracle::RetailBootBus;
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use w65c816::CPU;

const REACHABLE_SHAPES: [u16; 16] = [
    0xBC9C, 0xBCF0, 0xD6A4, 0xD6C0, 0xD6F8, 0xDDDC, 0xE140, 0xE290, 0xE744, 0xED2C, 0xEF24, 0xF0E4,
    0xF7AC, 0xF800, 0xF9DC, 0xFA84,
];

const DISPATCH_TRACE_PCS: [u32; 10] = [
    0x7F_3596, 0x7F_363B, 0x7F_3650, 0x7F_3680, 0x7F_C4BC, 0x7F_C4BD, 0x7F_C4DF, 0x7F_7EA7,
    0x7F_7EE2, 0x00_FBBF,
];

fn parse_hex(value: &str) -> u32 {
    u32::from_str_radix(value.trim_start_matches("0x"), 16)
        .expect("address/length must be hexadecimal")
}

fn snapshot(bus: &RetailBootBus, start: u32, len: usize) -> Vec<u8> {
    (0..len).map(|i| bus.peek8(start + i as u32)).collect()
}

fn is_sf2_code_bank(bank: u8) -> bool {
    bank <= 0x1F || bank == 0x7F || (0x40..=0x5F).contains(&bank) || (0xC0..=0xDF).contains(&bank)
}

fn main() {
    let max_cycles = std::env::args()
        .nth(1)
        .map(|s| s.parse::<u64>().expect("cycle count must be an integer"))
        .unwrap_or(50_000_000);
    let watch_start = std::env::args()
        .nth(2)
        .map(|s| parse_hex(&s))
        .unwrap_or(0x7F_7E00);
    let watch_len = std::env::args()
        .nth(3)
        .map(|s| parse_hex(&s) as usize)
        .unwrap_or(0x40);
    let enable_gsu = std::env::args().nth(4).as_deref() == Some("gsu");
    let input_mode = std::env::args().nth(5).unwrap_or_default();
    let trace_dispatch = std::env::var_os("SF2_DISPATCH_TRACE").is_some();
    let trace_dma = std::env::var_os("SF2_DMA_TRACE").is_some();
    let quiet_watch = std::env::var_os("SF2_QUIET_WATCH").is_some();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root");
    let rom_path = root.join("Star Fox 2 (USA, Europe).sfc");
    let rom = std::fs::read(&rom_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", rom_path.display()));

    let mut bus = RetailBootBus::new(rom.clone());
    if enable_gsu {
        bus.enable_gsu();
    }
    let mut cpu = CPU::new();
    let mut previous = snapshot(&bus, watch_start, watch_len);
    let mut changes = 0usize;
    let mut pc_hits: HashMap<u32, u64> = HashMap::new();
    let mut recent_instructions = VecDeque::<u32>::with_capacity(256);
    let mut entered_cartridge_code = false;
    let mut previous_opcode_fetches = 0u64;

    for cycle in 0..max_cycles {
        // Generate distinct Start edges after boot.  Repeating the pulse lets
        // the capture skip whichever title/intro phase is current without
        // assuming a fixed cycle count for audio upload or raster waits.
        if !input_mode.is_empty() {
            let pulse = match input_mode.as_str() {
                "start-once" => (4_000_000..4_300_000).contains(&cycle),
                "autostart" => cycle >= 4_000_000 && cycle % 2_000_000 < 300_000,
                other if other.starts_with("start@") => {
                    let begin = other[6..]
                        .parse::<u64>()
                        .expect("start@ cycle must be a decimal integer");
                    (begin..begin + 300_000).contains(&cycle)
                }
                other => panic!(
                    "unknown input mode {other:?}; use start-once, start@CYCLE, or autostart"
                ),
            };
            bus.set_pad1(if pulse { 0x1000 } else { 0 });
        }
        let before_pc = ((cpu.pbr() as u32) << 16) | cpu.pc() as u32;
        bus.set_cpu_irq_masked(cpu.p() & 0x04 != 0);
        cpu.cycle(&mut bus);
        bus.tick_raster();

        for dma in bus.take_dma_events() {
            if trace_dma {
                println!("dma cycle={cycle} pc={before_pc:06X} {dma:?}");
            }
        }

        let (opcode_fetches, opcode_pc) = bus.opcode_fetch_state();
        if opcode_fetches != previous_opcode_fetches {
            previous_opcode_fetches = opcode_fetches;
            if recent_instructions.len() == 256 {
                recent_instructions.pop_front();
            }
            recent_instructions.push_back(opcode_pc);
            *pc_hits.entry(opcode_pc).or_default() += 1;

            if trace_dispatch && DISPATCH_TRACE_PCS.contains(&opcode_pc) {
                let stack: Vec<u8> = (0..16)
                    .map(|i| bus.peek8(0x7E_0000 | cpu.s().wrapping_add(i) as u32))
                    .collect();
                let dp_f9 = cpu.d().wrapping_add(0xF9);
                let indirect = u32::from(bus.peek8(0x7E_0000 | dp_f9 as u32))
                    | (u32::from(bus.peek8(0x7E_0000 | dp_f9.wrapping_add(1) as u32)) << 8)
                    | (u32::from(bus.peek8(0x7E_0000 | dp_f9.wrapping_add(2) as u32)) << 16);
                println!(
                    "dispatch cycle={cycle} pc={opcode_pc:06X} P={:02X} S={:04X} A={:04X} X={:04X} Y={:04X} D={:04X} DB={:02X} [$F9]={indirect:06X}->{:02X} $1911={:02X} stack={stack:02X?}",
                    cpu.p(),
                    cpu.s(),
                    cpu.c(),
                    cpu.x(),
                    cpu.y(),
                    cpu.d(),
                    cpu.dbr(),
                    bus.peek8(indirect),
                    bus.peek8(0x7E_1911),
                );
            }

            let opcode_bank = (opcode_pc >> 16) as u8;
            if is_sf2_code_bank(opcode_bank) {
                entered_cartridge_code = true;
            }
            if entered_cartridge_code && !is_sf2_code_bank(opcode_bank) {
                println!(
                    "invalid opcode fetch at cycle={cycle}: pc={opcode_bank:02X}:{:04X} s={:04X} p={:02X} d={:04X} a={:02X} x={:04X} y={:04X}",
                    opcode_pc as u16,
                    cpu.s(),
                    cpu.p(),
                    cpu.d(),
                    cpu.a(),
                    cpu.x(),
                    cpu.y()
                );
                println!("recent instruction PCs: {recent_instructions:06X?}");
                let stack = cpu.s();
                let bytes: Vec<u8> = (0..32)
                    .map(|i| bus.peek8(0x7E_0000 | stack.wrapping_add(i) as u32))
                    .collect();
                println!("stack from {stack:04X}: {bytes:02X?}");
                let vectors: Vec<u8> = (0..48).map(|i| bus.peek8(0x7E_0100 + i)).collect();
                println!("low-WRAM interrupt code at 0100: {vectors:02X?}");
                break;
            }
        }

        if !quiet_watch {
            let current = snapshot(&bus, watch_start, watch_len);
            if current != previous {
                for i in 0..watch_len {
                    if current[i] != previous[i] {
                        println!(
                            "cycle={cycle} pc={before_pc:06X} write={:06X} {:02X}->{:02X}",
                            watch_start + i as u32,
                            previous[i],
                            current[i]
                        );
                        changes += 1;
                    }
                }
                previous = current;
            }
        }
        if cpu.stopped() {
            println!(
                "CPU stopped at cycle={cycle} pc={:02X}:{:04X}",
                cpu.pbr(),
                cpu.pc()
            );
            break;
        }
    }

    let final_bytes = snapshot(&bus, watch_start, watch_len);
    println!(
        "final pc={:02X}:{:04X} dot={} changes={changes}",
        cpu.pbr(),
        cpu.pc(),
        bus.dot
    );
    println!("APU: {:?}", bus.apu_debug_state());
    for (row, bytes) in final_bytes.chunks(16).enumerate() {
        print!("{:06X}:", watch_start + (row * 16) as u32);
        for b in bytes {
            print!(" {b:02X}");
        }
        println!();
    }

    // If the installed code is copied verbatim from ROM, report every source
    // offset.  Absence of a hit is equally useful: it proves generated or
    // transformed code and directs the next trace to the writer.
    for (offset, bytes) in final_bytes.chunks(32).enumerate() {
        let target = watch_start + (offset * 32) as u32;
        let needle = bytes;
        let hits: Vec<usize> = if needle.len() >= 8 && needle.iter().any(|&b| b != 0) {
            rom.windows(needle.len())
                .enumerate()
                .filter_map(|(i, w)| (w == needle).then_some(i))
                .collect()
        } else {
            Vec::new()
        };
        println!(
            "target={target:06X} len={} rom_hits={hits:06X?}",
            needle.len()
        );
    }

    println!("reachable shape descriptors in bank $7E:");
    for shape in REACHABLE_SHAPES {
        let target = 0x7E_0000 | shape as u32;
        let bytes: Vec<u8> = (0..16).map(|i| bus.peek8(target + i)).collect();
        let words: Vec<String> = bytes
            .chunks_exact(2)
            .map(|w| format!("{:04X}", u16::from_le_bytes([w[0], w[1]])))
            .collect();
        let hits: Vec<usize> =
            if bytes.iter().any(|&b| b != 0) && bytes.windows(2).any(|w| w[0] != w[1]) {
                rom.windows(bytes.len())
                    .enumerate()
                    .filter_map(|(i, w)| (w == bytes).then_some(i))
                    .collect()
            } else {
                Vec::new()
            };
        println!("  {shape:04X}: {} rom_hits={hits:06X?}", words.join(" "));
    }

    let mut hot: Vec<(u32, u64)> = pc_hits.into_iter().collect();
    hot.sort_unstable_by_key(|&(_, count)| std::cmp::Reverse(count));
    println!("hot PCs: {:06X?}", &hot[..hot.len().min(12)]);
}
