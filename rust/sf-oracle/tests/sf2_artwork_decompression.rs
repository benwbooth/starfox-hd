use sf2_data::compression::decode_artwork;

#[test]
fn native_opening_artwork_contains_the_queued_background_palette() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    let artwork = decode_artwork(&rom[..0xC2F24]).unwrap();
    let mut machine = sf_oracle::RetailMachine::new(rom);
    machine.watch_cpu_execution(&[0x0DBCCF, 0x7F0A76]);
    assert!(machine.tick_until_cpu_execution(0, 0x0DBCCF, 240).unwrap());
    assert!(machine.tick_until_cpu_execution(0, 0x7F0A76, 30).unwrap());
    assert_eq!(machine.peek16(0x7E17EC), 0x6E98);
    assert_eq!(machine.peek8(0x7E17EE), 0x70);
    assert_eq!(machine.peek16(0x7E17EF), 128);
    assert_eq!(machine.peek16(0x7E17F2), 0);
    for (index, byte) in artwork[0x80..0x100].iter().enumerate() {
        assert_eq!(
            *byte,
            machine.peek8(0x706E98 + index as u32),
            "palette byte {index}"
        );
    }
}
use sf_oracle::gsu::Gsu;

#[test]
fn native_artwork_matches_retail_decompressor_and_independent_hashes() {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    for (bank, end, length, hash) in [
        (0x16u16, 0xBFB8u16, 0x2020, 0x68444FCFu32),
        (0x16, 0xC4E4, 0x1000, 0x9B84A6AD),
        (0x18, 0xAF24, 0x24C0, 0x3E4A3761),
        (0x16, 0xEF70, 0x1000, 0x76C12B24),
        (0x16, 0xFD7C, 0x0800, 0xA2FFB62F),
        (0x19, 0x9F9C, 0x2020, 0xC78AFF13),
    ] {
        let offset = usize::from(bank) * 0x8000 + usize::from(end & 0x7FFF);
        let decoded = decode_artwork(&rom[..offset])
            .unwrap_or_else(|error| panic!("{bank:02X}:{end:04X}: {error:?}"));
        assert_eq!(decoded.len(), length);
        let mut gsu = Gsu::new(rom.clone());
        for (address, value) in [(0x2C, 0x3B50u16), (0x68, end), (0x6A, bank), (0xA2, 0)] {
            gsu.ram[address..address + 2].copy_from_slice(&value.to_le_bytes());
        }
        gsu.run_with_limit(1, 0xD9FF, 5_000_000);
        assert!(gsu.last_run_steps < 5_000_000);
        assert_eq!(
            decoded,
            gsu.ram[0x3B50..0x3B50 + length],
            "{bank:02X}:{end:04X}"
        );
        let actual = decoded.iter().fold(0x811C9DC5u32, |h, b| {
            (h ^ u32::from(*b)).wrapping_mul(0x01000193)
        });
        assert_eq!(actual, hash, "independent Mesen hash {bank:02X}:{end:04X}");
    }
}
