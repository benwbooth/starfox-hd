//! Probe the retail SF2 WRAM-resident map math routines.
//!
//! Usage:
//! `cargo run -p sf-oracle --example sf2_map_math -- <sf2-rom> <wram-dump> [gsu-ram-dump]`

use std::{env, fs};

use sf_oracle::{call, Entry, SnesBus};

fn main() {
    let mut args = env::args().skip(1);
    let rom_path = args.next().expect("SF2 ROM path");
    let wram_path = args.next().expect("128 KiB SF2 WRAM dump path");
    let rom = fs::read(rom_path).expect("read SF2 ROM");
    let wram = fs::read(wram_path).expect("read SF2 WRAM");
    assert_eq!(wram.len(), 0x20_000);
    let gsu_ram = args
        .next()
        .map(|path| fs::read(path).expect("read SF2 GSU RAM"));
    if let Some(gsu_ram) = &gsu_ram {
        assert_eq!(gsu_ram.len(), 0x1_0000);
    }

    for &(dx, dz) in &[
        (0i16, 1i16),
        (1, 0),
        (0, -1),
        (-1, 0),
        (100, 200),
        (-100, 200),
        (100, -200),
        (-100, -200),
    ] {
        let mut bus = SnesBus::new(rom.clone());
        for (index, &byte) in wram.iter().enumerate() {
            bus.write8(0x7E_0000 + index as u32, byte);
        }
        let x_object = 0x03BDu16;
        let y_object = 0x03FCu16;
        bus.write16(0x7E_0000 + u32::from(x_object + 0x0C), 0);
        bus.write16(0x7E_0000 + u32::from(x_object + 0x10), 0);
        bus.write16(0x7E_0000 + u32::from(y_object + 0x0C), dx as u16);
        bus.write16(0x7E_0000 + u32::from(y_object + 0x10), dz as u16);
        let exit = call(
            &mut bus,
            0x7F_2188,
            &Entry {
                x: x_object,
                y: y_object,
                dbr: 0x7E,
                p: 0x20,
                ..Entry::default()
            },
        );
        println!("angle dx={dx:6} dz={dz:6} -> C={:04X}", exit.c);
    }

    for &angle in &[0u8, 0x20, 0x40, 0x60, 0x80, 0xA0, 0xC0, 0xE0] {
        for &(x, y, z) in &[(0i16, 0i16, -0x4650i16), (100, -200, 300)] {
            let mut bus = SnesBus::new(rom.clone());
            for (index, &byte) in wram.iter().enumerate() {
                bus.write8(0x7E_0000 + index as u32, byte);
            }
            bus.write16(0x7E_0002, x as u16);
            bus.write16(0x7E_0004, 0);
            bus.write16(0x7E_0008, y as u16);
            bus.write16(0x7E_000A, 0);
            bus.write16(0x7E_0097, z as u16);
            bus.write16(0x7E_00E4, z as u16);
            call(
                &mut bus,
                0x7F_378A,
                &Entry {
                    a: u16::from(angle),
                    dbr: 0x7E,
                    p: 0x20,
                    ..Entry::default()
                },
            );
            println!(
                "rotate a={angle:02X} ({x:6},{y:6},{z:6}) -> ({:6},{:6},{:6})",
                bus.read16(0x7E_0004) as i16,
                bus.read16(0x7E_000A) as i16,
                bus.read16(0x7E_00E4) as i16,
            );
        }
    }

    if let Some(gsu_ram) = gsu_ram {
        for &(dx, dz) in &[
            (0i16, 0i16),
            (1, 0),
            (3, 4),
            (100, 200),
            (-100, 200),
            (30_000, 30_000),
            (i16::MIN, 0),
        ] {
            let mut bus = SnesBus::new(rom.clone());
            for (index, &byte) in wram.iter().enumerate() {
                bus.write8(0x7E_0000 + index as u32, byte);
            }
            for (index, &byte) in gsu_ram.iter().enumerate() {
                bus.write8(0x70_0000 + index as u32, byte);
            }
            bus.enable_gsu();
            let current = 0x03BDu16;
            let selected = 0x03FCu16;
            bus.write16(0x7E_0000 + u32::from(current + 0x0C), 0);
            bus.write16(0x7E_0000 + u32::from(current + 0x10), 0);
            bus.write16(0x7E_0000 + u32::from(selected + 0x0C), dx as u16);
            bus.write16(0x7E_0000 + u32::from(selected + 0x10), dz as u16);
            call(
                &mut bus,
                0x7F_8C25,
                &Entry {
                    x: current,
                    y: selected,
                    dbr: 0x7E,
                    p: 0x20,
                    ..Entry::default()
                },
            );
            println!(
                "distance dx={dx:6} dz={dz:6} -> {:04X}",
                bus.read16(0x7E_16B5)
            );
        }
    }
}
