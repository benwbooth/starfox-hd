//! Run SF2's retail GSU decompressor directly against one compressed stream.
//!
//! The host routine at `$03:D674` writes the source pointer to GSU RAM
//! `$0068/$006A`, clears `$00A2`, seeds `$002C = $3B50`, and launches
//! `$01:D9FF`.  This tool reproduces that exact ABI and reports changed RAM
//! ranges without guessing the compressed format.

use sf_oracle::gsu::Gsu;
use std::path::Path;

fn parse_hex(value: &str) -> u16 {
    u16::from_str_radix(value.trim_start_matches("0x"), 16)
        .expect("bank/address must be hexadecimal")
}

fn put_word(ram: &mut [u8], address: usize, value: u16) {
    ram[address..address + 2].copy_from_slice(&value.to_le_bytes());
}

fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811C_9DC5, |hash, byte| {
        (hash ^ *byte as u32).wrapping_mul(0x0100_0193)
    })
}

fn main() {
    let bank = std::env::args()
        .nth(1)
        .map(|s| parse_hex(&s) as u8)
        .unwrap_or(0x19);
    let address = std::env::args()
        .nth(2)
        .map(|s| parse_hex(&s))
        .unwrap_or(0x9F9C);
    let max_steps = std::env::args()
        .nth(3)
        .map(|s| s.parse::<u64>().expect("step limit must be decimal"))
        .unwrap_or(5_000_000);
    let trace = std::env::args().nth(4);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root");
    let rom_path = root.join("Star Fox 2 (USA, Europe).sfc");
    let rom = std::fs::read(&rom_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", rom_path.display()));

    let mut gsu = Gsu::new(rom);
    put_word(&mut gsu.ram, 0x002C, 0x3B50);
    put_word(&mut gsu.ram, 0x0068, address);
    put_word(&mut gsu.ram, 0x006A, bank as u16);
    put_word(&mut gsu.ram, 0x00A2, 0);
    let before = gsu.ram.clone();
    match trace.as_deref() {
        Some("trace") => gsu.trace_range = Some((0xD9FF, 0xDBDD)),
        Some("check") => gsu.trace_range = Some((0xDABF, 0xDAC4)),
        _ => {}
    }
    gsu.run_with_limit(0x01, 0xD9FF, max_steps);

    let changed: Vec<usize> = (0..gsu.ram.len())
        .filter(|&i| gsu.ram[i] != before[i])
        .collect();
    let (pbr, pc, sfr) = gsu.execution_state();
    println!(
        "source={bank:02X}:{address:04X} steps={} final={pbr:02X}:{pc:04X} sfr={sfr:04X}",
        gsu.last_run_steps
    );
    println!("registers={:04X?}", gsu.r);
    println!("output_fnv1a={:08X}", fnv1a(&gsu.ram[0x3B50..0x5B70]));
    println!(
        "changed bytes={} bounds=${:04X}..${:04X}",
        changed.len(),
        changed.first().copied().unwrap_or(0),
        changed.last().map_or(0, |v| v + 1)
    );
    for start in [0x0020usize, 0x0060, 0x3B40, 0x5BF0, 0x6FF0] {
        print!("${start:04X}:");
        for byte in &gsu.ram[start..start + 32] {
            print!(" {byte:02X}");
        }
        println!();
    }
}
