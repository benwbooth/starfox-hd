use sf2_data::compression::decode_artwork;
use sf2_data::opening_artwork::{ForegroundPaletteId, OpeningArtworkPalettes};

#[test]
fn native_palette_installation_matches_completed_source_loader() {
    use sf2_game::intro_controller::{IntroColor, OpeningScenePalette};
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    let artwork = OpeningArtworkPalettes::decode(&rom[..0xC2F24]).unwrap();
    // Foreground loader operands, not hardcoded expected asset samples.
    let records = [
        (ForegroundPaletteId::Standard, 0x1C894),
        (ForegroundPaletteId::CatalogOne, 0x1C87A),
        (ForegroundPaletteId::CatalogTwo, 0x1C860),
    ]
    .map(|(id, offset)| {
        (
            id,
            u16::from_le_bytes(rom[offset..offset + 2].try_into().unwrap()),
        )
    });
    let mut machine = sf_oracle::RetailMachine::new(rom);
    machine.watch_cpu_execution(&[0x0DBCCF, 0x03D52C]);
    assert!(machine.tick_until_cpu_execution(0, 0x0DBCCF, 240).unwrap());
    let initial =
        std::array::from_fn(|i| IntroColor::from_bgr555(machine.peek16(0x7EEFE5 + i as u32 * 2)));
    let mut palette = OpeningScenePalette::new(initial);
    assert!(machine.tick_until_cpu_execution(0, 0x03D52C, 30).unwrap());
    // The live boot takes the standard branch. Other table rows are checked
    // against their original source operands and decompressed RAM below.
    assert!(machine.peek16(0x7E1B86) & 0x2000 == 0 || machine.peek8(0x7ED7F2) == 0);
    palette.install_background(&artwork);
    palette.install_foreground(&artwork, ForegroundPaletteId::Standard);
    palette.install_polygon_palette(sf2_data::palettes::PolygonPaletteId::Standard);
    for (index, color) in palette.colors.iter().enumerate() {
        assert_eq!(
            color.bgr555(),
            machine.peek16(0x7EEFE5 + index as u32 * 2),
            "loaded color {index}"
        );
    }
    for (id, source) in records {
        for (index, color) in artwork.foreground(id).iter().enumerate() {
            assert_eq!(
                *color,
                machine.peek16(0x700000 + u32::from(source) + index as u32 * 2),
                "foreground {id:?} color {index}"
            );
        }
    }
    for (index, color) in artwork.sprites.iter().enumerate() {
        assert_eq!(
            *color,
            machine.peek16(0x7EF0E5 + index as u32 * 2),
            "sprite color {index}"
        );
    }
}

#[test]
fn native_polygon_palette_installation_matches_original_for_every_catalog_row() {
    use sf2_data::palettes::PolygonPaletteId;
    use sf2_game::intro_controller::{IntroColor, OpeningScenePalette};
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM");
    for id in [
        PolygonPaletteId::Standard,
        PolygonPaletteId::CatalogOne,
        PolygonPaletteId::CatalogTwo,
        PolygonPaletteId::EladardSurface,
        PolygonPaletteId::AstropolisExterior,
    ] {
        let mut bus = sf_oracle::SnesBus::new(rom.clone());
        let mut palette = OpeningScenePalette::new([IntroColor::from_bgr555(0x1234); 128]);
        for index in 0..128 {
            bus.write16(0x7EEFE5 + index * 2, 0x1234);
        }
        sf_oracle::call(
            &mut bus,
            0x038584,
            &sf_oracle::Entry {
                a: id as u16,
                p: 0x20,
                ..Default::default()
            },
        );
        palette.install_polygon_palette(id);
        for (index, color) in palette.colors.iter().enumerate() {
            assert_eq!(
                color.bgr555(),
                bus.read16(0x7EEFE5 + index as u32 * 2),
                "polygon {id:?} color {index}"
            );
        }
    }
}

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
