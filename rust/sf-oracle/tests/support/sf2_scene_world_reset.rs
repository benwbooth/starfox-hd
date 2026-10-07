//! The two original transition-world callers, including real actor/proxy
//! cleanup and all 16,384 occupancy cells. Region definitions survive.
use super::scene_clear_tests::{capture, compare, setup};
use super::surface_particle_tests::OWNER;
use super::{rom, Source, WRAM};
use sf2_game::scene_world_reset::{self, RegionSelection};
use sf2_game::world_occupancy::{WorldOccupancy, CELLS_PER_AXIS, CELL_SIZE};
use sf2_game::Vector3;

#[test]
fn world_reset_matches_original_both_callers_all_registration_and_group_bytes() {
    let mut source = Source::new(&rom(), 0);
    for selection in [RegionSelection::Retain, RegionSelection::Clear] {
        for seed in 0..=u8::MAX {
            let mut native = setup(&mut source, 6, seed);
            let owners = native.objects.active_ids().to_vec();
            for (index, &owner) in owners.iter().enumerate() {
                capture(&mut source, &mut native, owner, index as u16 + 0x8000);
            }
            capture(&mut source, &mut native, owners[0], 0xFFFF);
            let primary = seed.rotate_left(2);
            let secondary = seed.wrapping_mul(73);
            native.world.region_registration_count = Some(seed);
            native.world.spawn_defaults.as_mut().unwrap().group = primary;
            native.world.secondary_region_group = Some(secondary);
            source.bus.write8(0x1910, seed);
            source.bus.write8(0x190E, primary);
            source.bus.write8(0x190F, secondary);
            source.bus.write8(0x1911, 0xA5);
            source.bus.write16(0x1657, 0xA751);
            source.bus.write8(0x192E, 0x39);
            let definitions: Vec<_> = (0x686A..0x6A61)
                .map(|address| {
                    let byte = seed.wrapping_add((address as u8).wrapping_mul(113));
                    source.bus.write8(WRAM + address, byte);
                    byte
                })
                .collect();
            let random = native.world.random.clone();
            let mode = native.world.spawn_defaults.unwrap().run_when_paused;
            for repeat in 0..2 {
                // Each call starts from a fresh known plane; the second still
                // retains the preceding actor/proxy and group effects.
                let fill = if repeat == 0 { 0 } else { 255 };
                native.world.occupancy = Some(if fill == 0 {
                    WorldOccupancy::default()
                } else {
                    WorldOccupancy::fully_occupied()
                });
                source.bus.write8(WRAM + 0xCF35, 0x27);
                source.bus.write8(WRAM + 0xD736, 0xE1);
                for offset in 0..2048 {
                    source.bus.write8(WRAM + 0xCF36 + offset, fill);
                }
                match selection {
                    RegionSelection::Retain => {
                        // Stop only at the near-return instruction; every
                        // body instruction and both original callees ran.
                        source.run(0x0DC956, Some(0x0DC96B), 0, OWNER, true);
                    }
                    RegionSelection::Clear => {
                        source.run(0x0683F1, Some(0x068410), 0, OWNER, true);
                    }
                }
                scene_world_reset::clear(&mut native.objects, &mut native.world, selection)
                    .unwrap();
                compare(&source, &native);
                assert_eq!(
                    source.bus.read8(0x1910),
                    native.world.region_registration_count.unwrap()
                );
                assert_eq!(
                    source.bus.read8(0x190E),
                    native.world.spawn_defaults.unwrap().group
                );
                assert_eq!(
                    source.bus.read8(0x190F),
                    native.world.secondary_region_group.unwrap()
                );
                assert_eq!(source.bus.read8(0x1911), 0xA5);
                assert_eq!(source.bus.read16(0x1657), 0xA751);
                assert_eq!(source.bus.read8(0x192E), 0x39);
                assert_eq!(source.bus.read8(WRAM + 0xCF35), 0x27);
                assert_eq!(source.bus.read8(WRAM + 0xD736), 0xE1);
                for (index, &byte) in definitions.iter().enumerate() {
                    assert_eq!(source.bus.read8(WRAM + 0x686A + index as u32), byte);
                }
                let occupancy = native.world.occupancy.as_ref().unwrap();
                for z in 0..CELLS_PER_AXIS {
                    for x in 0..CELLS_PER_AXIS {
                        let byte = source.bus.read8(WRAM + 0xCF36 + (z * 16 + x / 8) as u32);
                        assert_eq!(
                            occupancy.contains(Vector3 {
                                x: (x as u16 * CELL_SIZE) as i16,
                                y: -517,
                                z: (z as u16 * CELL_SIZE) as i16,
                            }),
                            byte & (1 << (x % 8)) != 0,
                        );
                    }
                }
                assert_eq!(native.world.random, random);
                assert_eq!(native.world.spawn_defaults.unwrap().run_when_paused, mode);
            }
        }
    }
}

#[test]
fn full_occupancy_publisher_matches_original_all_prior_byte_values_without_a_scene() {
    let mut source = Source::new(&rom(), 0);
    for seed in 0..=u8::MAX {
        for offset in 0..2048 {
            source
                .bus
                .write8(WRAM + 0xCF36 + offset, seed.wrapping_add(offset as u8));
        }
        source.bus.write8(WRAM + 0xCF35, seed);
        source.bus.write8(WRAM + 0xD736, !seed);
        source.run(0x0DDA5F, None, 0, OWNER, true);
        let world = WorldOccupancy::fully_occupied();
        for z in 0..CELLS_PER_AXIS {
            for group in 0..16 {
                assert_eq!(
                    source.bus.read8(WRAM + 0xCF36 + (z * 16 + group) as u32),
                    255
                );
                for bit in 0..8 {
                    assert!(world.contains(Vector3 {
                        x: ((group * 8 + bit) as u16 * CELL_SIZE) as i16,
                        y: 0,
                        z: (z as u16 * CELL_SIZE) as i16
                    }));
                }
            }
        }
        assert_eq!(source.bus.read8(WRAM + 0xCF35), seed);
        assert_eq!(source.bus.read8(WRAM + 0xD736), !seed);
    }
}
