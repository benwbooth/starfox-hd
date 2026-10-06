//! Original setup helpers and mode-job layout publications. The source bytes
//! are unchanged. The job test enters after the horizontal-blank wait and
//! stops before audio/IRQ continuation; it does not validate display timing.

use sf2_game::scene_display::DisplayBand;
use sf2_game::scene_video::{
    ArtworkPlane, SceneLayerMask, SceneLayerPolicy, SceneModePublication, SceneModeSetup,
    SceneTileMode, SceneVideo, TileMapGrid,
};
use sf_oracle::SnesBus;
use w65c816::{AddressType, Signals, System, CPU};

struct Source {
    bus: SnesBus,
    writes: Vec<(u32, u8)>,
    reset: bool,
}

impl System for Source {
    fn read(&mut self, address: u32, kind: AddressType, signals: &Signals) -> u8 {
        self.bus.read(address, kind, signals)
    }

    fn write(&mut self, address: u32, value: u8, kind: AddressType, signals: &Signals) {
        if (0x2100..=0x213F).contains(&(address & 65535)) {
            self.writes.push((address, value));
        }
        self.bus.write(address, value, kind, signals);
    }

    fn res(&mut self) -> bool {
        std::mem::take(&mut self.reset)
    }
}

impl Source {
    fn new() -> Self {
        let rom = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../Star Fox 2 (USA, Europe).sfc"),
        )
        .unwrap();
        let runtime = rom[0x10000..0x17E00].to_vec();
        let mut bus = SnesBus::new(rom);
        for (offset, byte) in runtime.into_iter().enumerate() {
            bus.write8(0x7F0000 + offset as u32, byte);
        }
        Self {
            bus,
            writes: Vec::new(),
            reset: false,
        }
    }

    fn run_to(&mut self, entry: u32, boundary: u32) {
        // Only this test-owned bootstrap is synthetic. All source helpers,
        // shared jobs, palette copies and their data execute unchanged.
        let boot = [
            0x18,
            0xFB,
            0xC2,
            0x30,
            0xE2,
            0x20, // native, byte A, word X/Y
            0x22,
            entry as u8,
            (entry >> 8) as u8,
            (entry >> 16) as u8,
            0xDB,
        ];
        for (index, byte) in boot.into_iter().enumerate() {
            self.bus.write8(0x200 + index as u32, byte);
        }
        self.writes.clear();
        self.reset = true;
        let mut cpu = CPU::new();
        for _ in 0..20_000 {
            cpu.cycle(self);
            if cpu.tcu() == 0
                && (u32::from(cpu.pbr()) << 16 | u32::from(cpu.pc().wrapping_sub(1))) == boundary
            {
                return;
            }
        }
        panic!(
            "source failed to reach {boundary:06X} from {entry:06X}, stopped at {:02X}:{:04X}",
            cpu.pbr(),
            cpu.pc()
        );
    }

    fn output(&self, address: u32) -> u8 {
        self.writes
            .iter()
            .rev()
            .find(|(at, _)| *at == address)
            .expect("source output written")
            .1
    }
}

const SETUPS: [(SceneModeSetup, u32, u32); 4] = [
    (SceneModeSetup::LayeredSmallTiles, 0x03D570, 0x03D5EC),
    (SceneModeSetup::LayeredLargeTiles, 0x03D58E, 0x03D5EC),
    (SceneModeSetup::OffsetWideMap, 0x03D5AC, 0x03D5FD),
    (SceneModeSetup::OffsetTallMap, 0x03D5CA, 0x03D5FD),
];

fn plane(flags: u8) -> ArtworkPlane {
    if flags & 8 != 0 {
        ArtworkPlane::First
    } else {
        ArtworkPlane::Second
    }
}

fn size_bits(sizes: [bool; 4]) -> u8 {
    sizes
        .into_iter()
        .enumerate()
        .fold(0, |mask, (index, large)| {
            mask | (u8::from(large) << (4 + index))
        })
}

fn grid_bits(grid: TileMapGrid) -> u8 {
    match grid {
        TileMapGrid::Square => 0,
        TileMapGrid::Wide => 1,
        TileMapGrid::Tall => 2,
        TileMapGrid::LargeSquare => 3,
    }
}

fn mode_bits(layout: SceneModePublication) -> u8 {
    let mode = match layout.mode {
        SceneTileMode::Layered => 1,
        SceneTileMode::ColumnOffsets => 2,
    };
    mode | size_bits(layout.large_characters) | u8::from(layout.foreground_priority) << 3
}

fn policy(artwork_plane: ArtworkPlane) -> SceneLayerPolicy {
    SceneLayerPolicy {
        artwork_plane,
        visible_layers: SceneLayerMask::STANDARD_SCENE,
        layered_large_characters: [false; 4],
        layered_foreground_priority: false,
        third_map_grid: TileMapGrid::Square,
    }
}

fn source_policy(source: &Source) -> SceneLayerPolicy {
    let extras = source.bus.read8(0x1A89);
    SceneLayerPolicy {
        artwork_plane: plane(source.bus.read8(0x1AA7)),
        visible_layers: SceneLayerMask::from_bits(source.bus.read8(0x1C52)),
        layered_large_characters: std::array::from_fn(|index| extras & (0x10 << index) != 0),
        layered_foreground_priority: extras & 8 != 0,
        third_map_grid: match (source.bus.read8(0x1C56) | source.bus.read8(0x1A8A)) & 3 {
            0 => TileMapGrid::Square,
            1 => TileMapGrid::Wide,
            2 => TileMapGrid::Tall,
            _ => TileMapGrid::LargeSquare,
        },
    }
}

#[test]
fn layer_policy_producers_change_only_their_owned_choices() {
    let mut source = Source::new();
    for plane_flags in [0xD0, 0xD8] {
        for extras in 0..32u8 {
            for grid in 0..4 {
                for visible in [0, 1, 3, 8, 0x13, 0x17, 0x80, 0xFF] {
                    source.bus.write8(0x1AA7, plane_flags);
                    source.bus.write8(0x1A89, extras << 3);
                    source.bus.write8(0x1A8A, grid | 0xA0);
                    source.bus.write8(0x1C52, visible);
                    source.bus.write8(0x1C56, 0x2C);
                    let mut native = source_policy(&source);

                    source.run_to(0x03B40B, 0x03B6E5);
                    native.select_opening_artwork_plane();
                    assert_eq!(native, source_policy(&source));

                    source.run_to(0x03AE33, 0x03AE4E);
                    native.initialize_video_layers();
                    assert_eq!(native, source_policy(&source));
                    assert_eq!(native.visible_layers.bits(), visible);
                    assert_eq!(source.bus.read8(0x1A8A), grid | 0xA0);

                    source.run_to(0x039D5B, 0x039D62);
                    native.begin_load_sequence();
                    assert_eq!(native, source_policy(&source));
                }
            }
        }
    }
}

#[test]
fn all_setup_helpers_preserve_other_flags_and_capture_the_large_tile_choice() {
    let mut source = Source::new();
    for (setup, entry, boundary) in SETUPS {
        for flags in 0..=u8::MAX {
            for selection in 0..=u8::MAX {
                source.bus.write8(0x1AA6, flags);
                source.bus.write8(0x1AA7, selection);
                source.bus.write8(0x1A88, 0xA5);
                source.run_to(entry, boundary);
                let mut native = SceneVideo::default();
                native.request_setup(setup, plane(selection)).unwrap();
                let layout = native.publish_setup(policy(plane(selection))).unwrap();
                assert_eq!(source.bus.read8(0x1A88), size_bits(layout.large_characters));
                assert_eq!(source.bus.read8(0x1AA7), selection);
                let tall = layout.artwork_map_grid == TileMapGrid::Tall;
                assert_eq!(
                    source.bus.read8(0x1AA6),
                    (flags & !0x10) | if tall { 0x10 } else { 0 }
                );
                assert_eq!(layout.mode == SceneTileMode::Layered, boundary == 0x03D5EC);
            }
        }
    }
}

#[test]
fn mode_services_publish_fresh_layer_policy_without_recapturing_the_tile_choice() {
    let mut source = Source::new();
    for (setup, entry, boundary) in SETUPS {
        for requested_plane in [ArtworkPlane::First, ArtworkPlane::Second] {
            for current_plane in [ArtworkPlane::First, ArtworkPlane::Second] {
                for extras in 0..32u8 {
                    for grid in [
                        TileMapGrid::Square,
                        TileMapGrid::Wide,
                        TileMapGrid::Tall,
                        TileMapGrid::LargeSquare,
                    ] {
                        for visibility in [0, 0x13, 0x17, 255] {
                            let mut native = SceneVideo::default();
                            native.request_setup(setup, requested_plane).unwrap();
                            source.bus.write8(0x1AA6, 0xA3);
                            source.bus.write8(
                                0x1AA7,
                                if requested_plane == ArtworkPlane::First {
                                    0xD8
                                } else {
                                    0xD0
                                },
                            );
                            source.run_to(entry, boundary);
                            let inputs = SceneLayerPolicy {
                                artwork_plane: current_plane,
                                visible_layers: SceneLayerMask::from_bits(visibility),
                                layered_large_characters: std::array::from_fn(|index| {
                                    extras & (2 << index) != 0
                                }),
                                layered_foreground_priority: extras & 1 != 0,
                                third_map_grid: grid,
                            };
                            source.bus.write8(
                                0x1AA7,
                                if current_plane == ArtworkPlane::First {
                                    0x08
                                } else {
                                    0
                                },
                            );
                            source.bus.write8(0x1A89, extras << 3);
                            source.bus.write8(0x1A8A, grid_bits(grid));
                            source.bus.write8(0x1C52, visibility);
                            source.bus.write8(0x1C51, !visibility);
                            source.bus.write16(0x1B3C, 0x5C00);
                            source.bus.write8(0x1C56, 0x2C);
                            source.bus.write8(0x17F1, 0); // standard polygon palette
                            source.bus.write8(0x00, 0xFF);
                            let job = if boundary == 0x03D5EC {
                                0x7F0B82
                            } else {
                                0x7F0C46
                            };
                            source.run_to(job, 0x7F0C26);
                            let layout = native.publish_setup(inputs).unwrap();
                            assert_eq!(source.output(0x2100), 0x8F);
                            assert_eq!(native.output(), Some(DisplayBand::BLANK_FULL));
                            assert_eq!(source.output(0x2105), mode_bits(layout));
                            assert_eq!(source.output(0x212C), layout.visible_layers.bits());
                            assert_eq!(source.bus.read8(0x1C51), layout.visible_layers.bits());
                            let artwork_output = if layout.artwork_plane == ArtworkPlane::First {
                                0x2107
                            } else {
                                0x2108
                            };
                            let untouched_output = if artwork_output == 0x2107 {
                                0x2108
                            } else {
                                0x2107
                            };
                            assert_eq!(
                                source.output(artwork_output),
                                0x5C | grid_bits(layout.artwork_map_grid)
                            );
                            assert!(!source.writes.iter().any(|(at, _)| *at == untouched_output));
                            assert_eq!(
                                source.output(0x2109),
                                0x2C | grid_bits(layout.third_map_grid)
                            );
                            assert_eq!(
                                source
                                    .writes
                                    .iter()
                                    .filter(|(at, _)| *at == 0x2110)
                                    .map(|(_, value)| *value)
                                    .collect::<Vec<_>>(),
                                [0, 0]
                            );
                            assert_eq!(layout.second_layer_vertical_scroll, 0);
                            assert_eq!(source.bus.read8(0x00), 0);
                        }
                    }
                }
            }
        }
    }
}
