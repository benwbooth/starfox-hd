//! Native opening-artwork publications compared at original loader/service
//! boundaries. The source runs from reset with its original IRQ and DMA code.
//! This verifies publication order and content, not native IRQ timing, scene
//! mode setup or palette effects. Lighting publication is checked separately
//! at setup completion and at the final main-loop handoff.

use std::sync::Arc;

use sf2_data::opening_artwork::{ForegroundPaletteId, OpeningArtwork};
use sf2_game::intro_controller::{IntroColor, OpeningScenePalette};
use sf2_game::intro_material::{DepthColorFamily, DepthThresholdTable};
use sf2_game::intro_scene::OpeningScene;
use sf2_game::scene_artwork::{
    ArtworkLoadPhase, ArtworkPublication, ArtworkResume, ForegroundSelection,
};
use sf_oracle::{call, Entry, RetailMachine, SnesBus};

fn rom() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 retail ROM")
}

fn reach(source: &mut RetailMachine, address: u32) {
    assert!(
        source.tick_until_cpu_execution(0, address, 240).unwrap(),
        "original source did not reach {address:06X}, stopped at {:06X}",
        source.pc()
    );
}

fn compare_palette(source: &RetailMachine, scene: &OpeningScene) {
    for (index, (color, saved)) in scene
        .palette()
        .colors
        .iter()
        .zip(&scene.palette().saved_colors)
        .enumerate()
    {
        assert_eq!(
            color.bgr555(),
            source.peek16(0x7EEFE5 + index as u32 * 2),
            "live palette color {index}"
        );
        assert_eq!(
            saved.bgr555(),
            source.peek16(0x7EF2E5 + index as u32 * 2),
            "saved palette color {index}"
        );
    }
}

#[test]
fn opening_artwork_publications_match_original_boot_at_each_service_boundary() {
    let rom = rom();
    // Source packet operands at C813/C81D and the common palette helper.
    assert_eq!(&rom[0x1C816..0x1C81D], &[0xB8, 0xBF, 0x16, 0, 0, 0, 0x20]);
    assert_eq!(&rom[0x1C820..0x1C827], &[0xE4, 0xC4, 0x16, 0, 0, 0, 0x10]);
    let artwork = Arc::new(
        OpeningArtwork::decode(&rom[..0xB3FB8], &rom[..0xB44E4], &rom[..0xC2F24]).unwrap(),
    );
    let mut source = RetailMachine::new(rom);
    reach(&mut source, 0x0DBCCF);
    assert_eq!(source.peek16(0x46), 0xA000, "boot opening upload buffer");
    let mut palette = OpeningScenePalette::new(std::array::from_fn(|index| {
        IntroColor::from_bgr555(source.peek16(0x7EEFE5 + index as u32 * 2))
    }));
    palette.saved_colors = std::array::from_fn(|index| {
        IntroColor::from_bgr555(source.peek16(0x7EF2E5 + index as u32 * 2))
    });
    let mut native = OpeningScene::new(sf2_game::RandomState::default(), palette);
    reach(&mut source, 0x03C80B);
    native
        .begin_artwork_load(artwork.clone(), source.peek16(0x7E1B9C) & 0x0040 != 0)
        .unwrap();
    compare_palette(&source, &native);

    reach(&mut source, 0x7F0C24); // Setup's palette and threshold publications completed.
    assert_eq!(
        native.publish_artwork(),
        Some(ArtworkPublication::PolygonPalette)
    );
    compare_palette(&source, &native);
    assert_eq!(
        native.lighting().thresholds,
        Some(DepthThresholdTable::NORMAL)
    );
    assert_eq!(native.lighting().depth_colors, None);
    assert_eq!(
        [source.peek_gsu_ram(0x50), source.peek_gsu_ram(0x51)],
        [0x2C, 0x8F]
    );
    assert!(native.artwork().characters.is_none() && native.artwork().map.is_none());

    reach(&mut source, 0x03C813); // Setup has completed; main loader resumes.
    assert_eq!(
        native
            .resume_artwork_load(ForegroundSelection::STANDARD)
            .unwrap(),
        ArtworkResume::Queued(ArtworkPublication::BackgroundCharacters)
    );
    reach(&mut source, 0x7F0CB2); // Character DMA has completed.
    assert_eq!(source.peek16(0x7E17E6), 8192);
    let character_start = usize::from(source.peek16(0x7E17E4)) * 2;
    assert_eq!(
        native.publish_artwork(),
        Some(ArtworkPublication::BackgroundCharacters)
    );
    compare_palette(&source, &native);
    assert!(!native.artwork().skip_next_background_palette);
    assert_eq!(source.peek16(0x7E1B9C) & 0x0040, 0);
    assert!(native.artwork().map.is_none());
    // Re-encode native pixels and compare the actual emulator video-memory
    // transfer, not a second native decompression or a saved expected image.
    let video = source.ppu_frame();
    for (tile, pixels) in native
        .artwork()
        .characters
        .as_ref()
        .unwrap()
        .iter()
        .map(|tile| tile.pixels())
        .enumerate()
    {
        for plane in 0..4 {
            for row in 0..8 {
                let byte = (0..8).fold(0, |value, col| {
                    value | ((pixels[row * 8 + col] >> plane) & 1) << (7 - col)
                });
                let offset = tile * 32 + plane / 2 * 16 + row * 2 + plane % 2;
                assert_eq!(
                    byte,
                    video.vram[(character_start + offset) & 65535],
                    "character byte {offset}"
                );
            }
        }
    }

    reach(&mut source, 0x7F0D08); // Map DMA has completed, no main resume yet.
    assert_eq!(source.peek16(0x7E17EA), 4096);
    assert_eq!(
        native.publish_artwork(),
        Some(ArtworkPublication::BackgroundMap)
    );
    compare_palette(&source, &native);
    let map_start = usize::from(source.peek16(0x7E17E8)) * 2;
    let video = source.ppu_frame();
    for (index, cell) in native.artwork().map.as_ref().unwrap().iter().enumerate() {
        let word = cell.tile
            | u16::from(cell.palette) << 10
            | u16::from(cell.priority) << 13
            | u16::from(cell.flip_horizontal) << 14
            | u16::from(cell.flip_vertical) << 15;
        for (byte, value) in word.to_le_bytes().into_iter().enumerate() {
            assert_eq!(
                value,
                video.vram[(map_start + index * 2 + byte) & 65535],
                "map cell {index} byte {byte}"
            );
        }
    }
    assert_eq!(native.publish_artwork(), None);

    reach(&mut source, 0x03C84A);
    let selection = ForegroundSelection {
        use_catalog: source.peek16(0x7E1B86) & 0x2000 != 0,
        entry: source.peek8(0x7ED7F2),
    };
    assert_eq!(
        native.resume_artwork_load(selection).unwrap(),
        ArtworkResume::Queued(ArtworkPublication::ForegroundPalette(selection.palette()))
    );
    reach(&mut source, 0x03D509); // Foreground copy complete.
    assert_eq!(
        native.publish_artwork(),
        Some(ArtworkPublication::ForegroundPalette(selection.palette()))
    );
    compare_palette(&source, &native);
    assert_eq!(
        native.resume_artwork_load(selection).unwrap(),
        ArtworkResume::Queued(ArtworkPublication::SpritePalette)
    );
    reach(&mut source, 0x03D52C); // Sprite copy complete, before render handoff.
    assert_eq!(
        native.publish_artwork(),
        Some(ArtworkPublication::SpritePalette)
    );
    compare_palette(&source, &native);
    for (index, color) in native.artwork().sprite_colors.iter().enumerate() {
        assert_eq!(
            color.bgr555(),
            source.peek16(0x7EF0E5 + index as u32 * 2),
            "sprite color {index}"
        );
    }
    assert_eq!(
        native.artwork_load_phase(),
        Some(ArtworkLoadPhase::RenderHandoff)
    );
    assert_eq!(
        native.lighting().thresholds,
        Some(DepthThresholdTable::NORMAL)
    );
    assert_eq!(native.lighting().depth_colors, None);
    reach(&mut source, 0x03D56F);
    assert_eq!(
        native.resume_artwork_load(selection).unwrap(),
        ArtworkResume::Complete
    );
    assert_eq!(
        native.artwork_load_phase(),
        Some(ArtworkLoadPhase::Complete)
    );
    assert_eq!(
        native.lighting().thresholds,
        Some(DepthThresholdTable::OPENING)
    );
    assert_eq!(
        native.lighting().depth_colors,
        Some(DepthColorFamily::STANDARD)
    );
    assert_eq!(
        [source.peek_gsu_ram(0x50), source.peek_gsu_ram(0x51)],
        [0x40, 0x8F]
    );
    assert_eq!(
        [source.peek_gsu_ram(0x4E), source.peek_gsu_ram(0x4F)],
        [0x0C, 0x8B]
    );
    assert_eq!(
        native.controller().elapsed_updates(),
        0,
        "artwork service must not tick actors"
    );
}

#[test]
fn foreground_choice_matches_original_branch_for_every_selector() {
    let mut rom = rom();
    // Bound only the continuation after selection. The original selector and
    // all three descriptor-producing branches execute unchanged and return;
    // this isolated test does not model the continuation's asynchronous jobs.
    assert_eq!(
        &rom[0x1D4FD..0x1D505],
        &[0xA5, 0, 0xD0, 0xFC, 0xA9, 0x1C, 0x85, 0]
    );
    rom[0x1D4FD] = 0x6B;
    let mut original = SnesBus::new(rom);
    for flags in [0u16, 0x2000, 0xDFFF, 0xFFFF] {
        for entry in 0..=u8::MAX {
            original.write16(0x1B86, flags);
            original.write8(0x7ED7F2, entry);
            for field in [0x17EC, 0x17EE, 0x17EF, 0x17F2] {
                original.write16(field, 0xA5A5);
            }
            assert!(
                call(
                    &mut original,
                    0x03C84A,
                    &Entry {
                        p: 0x20,
                        ..Default::default()
                    }
                )
                .returned
            );
            let choice = ForegroundSelection {
                use_catalog: flags & 0x2000 != 0,
                entry,
            }
            .palette();
            let source = match choice {
                ForegroundPaletteId::Standard => 0x7218,
                ForegroundPaletteId::CatalogOne => 0x75D8,
                ForegroundPaletteId::CatalogTwo => 0x7578,
            };
            assert_eq!(original.read16(0x17EC), source);
            assert_eq!(original.read8(0x17EE), 0x70);
            assert_eq!(original.read16(0x17EF), 96);
            assert_eq!(original.read16(0x17F2), 64);
        }
    }
}
