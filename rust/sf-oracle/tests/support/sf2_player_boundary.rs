//! Corridor geometry, admission, producer and retained player history against
//! unmodified original routines. No original outputs become native inputs.

use super::surface_particle_tests::{address, Native, OWNER, SLOT};
use super::{rom, Source, WRAM};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::player_boundary::{self, Corridor, RegionInputs};
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::PlayerPathRecords;
use sf2_game::weapon_dispatch::WeaponState;
use sf2_game::{Angle, ObjectId, Vector3};
use sf_oracle::{call_near, Entry};

struct Fixture {
    native: Native,
    runtime: PathRuntime,
    proxy: ObjectId,
    anchor: ObjectId,
}
impl Fixture {
    fn new(source: &mut Source) -> Self {
        let mut native = Native::new(source, 3, 0, 0, 0);
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut native.objects,
            &mut native.world,
            &mut runtime,
            native.owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 17,
                score: Default::default(),
            },
        )
        .unwrap();
        let others: Vec<_> = native
            .objects
            .active_ids()
            .iter()
            .copied()
            .filter(|&id| id != native.owner)
            .collect();
        let proxy = others[0];
        let anchor = others[1];
        native.world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        Self {
            native,
            runtime,
            proxy,
            anchor,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.native
            .world
            .player_mut(&self.native.objects, self.native.owner)
            .unwrap()
    }
    fn seed(&self, source: &mut Source) {
        source.bus.write16(0x14D6, address(Some(self.proxy)));
        for (id, object) in self.native.objects.active_objects() {
            let base = WRAM + u32::from(address(Some(id)));
            for (offset, value) in [
                (12, object.base.position.x),
                (14, object.base.position.y),
                (16, object.base.position.z),
            ] {
                source.bus.write16(base + offset, value as u16);
            }
            source.bus.write8(base + 0x14, object.base.yaw.units());
            source.bus.write8(
                base + 0x20,
                0xA0 | u8::from(object.base.contacts.hit_marked) * 2,
            );
        }
        let record = self
            .native
            .world
            .player(&self.native.objects, self.native.owner)
            .unwrap();
        let boundary = record.boundary.unwrap();
        for (offset, value) in [
            (0x6AAF, boundary.center.x),
            (0x6AB1, boundary.center.y),
            (0x6AB3, boundary.center.z),
            (0x6AB5, boundary.half_width),
            (0x6AB7, boundary.half_height),
            (0x6BED, boundary.return_position.x),
            (0x6BEF, boundary.return_position.y),
            (0x6BF1, boundary.return_position.z),
        ] {
            source.bus.write16(WRAM + SLOT + offset, value as u16);
        }
        source.bus.write8(
            WRAM + SLOT + 0x6AAE,
            record.steering.unwrap().locked_heading.units(),
        );
        source
            .bus
            .write8(WRAM + SLOT + 0x6B77, record.auxiliary.unwrap().action_flags);
        source
            .bus
            .write8(WRAM + SLOT + 0x6AA0, record.auxiliary.unwrap().mode);
        source.bus.write16(
            WRAM + SLOT + 0x6AB9,
            player_storage::get(
                &self.native.objects,
                &self.runtime.resources,
                self.native.owner,
            )
            .unwrap()
            .fine_pitch,
        );
    }
    fn compare(&mut self, source: &Source, mut before: PlayerPathRecords) {
        let word = |offset| source.bus.read16(WRAM + SLOT + offset) as i16;
        let boundary = before.boundary.as_mut().unwrap();
        boundary.center = Vector3 {
            x: word(0x6AAF),
            y: word(0x6AB1),
            z: word(0x6AB3),
        };
        boundary.half_width = word(0x6AB5);
        boundary.half_height = word(0x6AB7);
        boundary.return_position = Vector3 {
            x: word(0x6BED),
            y: word(0x6BEF),
            z: word(0x6BF1),
        };
        before.steering.as_mut().unwrap().locked_heading =
            Angle::from_units(source.bus.read8(WRAM + SLOT + 0x6AAE));
        before.auxiliary.as_mut().unwrap().action_flags = source.bus.read8(WRAM + SLOT + 0x6B77);
        assert_eq!(*self.records(), before);
        assert_eq!(
            player_storage::get(
                &self.native.objects,
                &self.runtime.resources,
                self.native.owner
            )
            .unwrap()
            .fine_pitch,
            source.bus.read16(WRAM + SLOT + 0x6AB9)
        );
        for (id, actor) in self.native.objects.active_objects() {
            let base = WRAM + u32::from(address(Some(id)));
            assert_eq!(
                actor.base.position,
                Vector3 {
                    x: source.bus.read16(base + 12) as i16,
                    y: source.bus.read16(base + 14) as i16,
                    z: source.bus.read16(base + 16) as i16,
                },
                "position {id:?}"
            );
            assert_eq!(actor.base.yaw.units(), source.bus.read8(base + 0x14));
            assert_eq!(
                0xA0 | u8::from(actor.base.contacts.hit_marked) * 2,
                source.bus.read8(base + 0x20)
            );
        }
    }
    fn direct(&mut self, source: &mut Source, corridor: Corridor) -> bool {
        self.seed(source);
        let before = *self.records();
        for (offset, value) in [
            (0x1DBC, corridor.center.x),
            (0x1DB8, corridor.center.z),
            (0x1DB6, corridor.half_width),
            (0x1DB2, i16::from(corridor.heading.units())),
        ] {
            source.bus.write16(offset, value as u16);
        }
        let result = call_near(
            &mut source.bus,
            0x07E3D6,
            &Entry {
                x: OWNER,
                y: SLOT as u16,
                dbr: 0x7E,
                ..Default::default()
            },
        );
        assert!(result.returned);
        let changed =
            player_boundary::correct_proxy(&mut self.native.objects, self.proxy, corridor).unwrap();
        assert_eq!(changed, result.p & 1 != 0, "correction {corridor:?}");
        self.compare(source, before);
        changed
    }
    fn advance(&mut self, source: &mut Source) -> bool {
        let before = *self.records();
        source.run(0x07E2F3, None, 0, OWNER, true);
        let changed = player_boundary::advance(
            &mut self.native.objects,
            &mut self.native.world,
            &mut self.runtime.resources,
            self.native.owner,
        )
        .unwrap();
        self.compare(source, before);
        changed
    }
    fn install(&mut self, source: &mut Source, inputs: RegionInputs) -> bool {
        let before = *self.records();
        source.bus.write8(0xA7, inputs.heading_offset.units());
        for (offset, value) in [
            (4, inputs.half_width),
            (10, inputs.half_height),
            (0xE4, inputs.activation_radius),
        ] {
            source.bus.write16(offset, value as u16);
        }
        source.run_with_y(
            0x07F893,
            None,
            0,
            address(Some(self.anchor)),
            true,
            Some(OWNER),
        );
        let installed = player_boundary::install_region(
            &mut self.native.objects,
            &mut self.native.world,
            self.anchor,
            self.native.owner,
            inputs,
        )
        .unwrap();
        self.compare(source, before);
        for offset in [0x16B7, 0x16B9, 0x16BB] {
            assert_eq!(source.bus.read16(offset), 0);
        }
        installed
    }
}

#[test]
fn corridor_geometry_matches_original_all_headings_wrapped_limits_and_equality_cases() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = Fixture::new(&mut source);
    let mut corrected = 0;
    let mut unchanged = 0;
    for heading in 0..=u8::MAX {
        for width in [
            i16::MIN,
            -32767,
            -257,
            -256,
            -1,
            0,
            1,
            2,
            127,
            128,
            255,
            256,
            257,
            16383,
            16384,
            32767,
        ] {
            for center in [0i16, -32767, 32767] {
                for edge in [i16::MIN, -1, 0, 1, i16::MAX] {
                    let corridor = Corridor {
                        center: Vector3 {
                            x: center,
                            y: 71,
                            z: center.wrapping_mul(3),
                        },
                        half_width: width,
                        half_height: 13,
                        heading: Angle::from_units(heading),
                    };
                    let proxy = fixture.native.objects.get_mut(fixture.proxy).unwrap();
                    proxy.base.position = Vector3 {
                        x: center.wrapping_add(width).wrapping_add(edge),
                        y: -91,
                        z: center
                            .wrapping_mul(3)
                            .wrapping_sub(width)
                            .wrapping_sub(edge),
                    };
                    proxy.base.contacts.hit_marked = true;
                    if fixture.direct(&mut source, corridor) {
                        corrected += 1;
                    } else {
                        unchanged += 1;
                    }
                }
            }
        }
    }
    assert!(
        corrected > 10000 && unchanged > 1000,
        "{corrected} {unchanged}"
    );
}

#[test]
fn corridor_geometry_matches_original_every_width_center_and_position_word() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = Fixture::new(&mut source);
    for word in 0..=u16::MAX {
        fixture
            .native
            .objects
            .get_mut(fixture.proxy)
            .unwrap()
            .base
            .position = Vector3 {
            x: word.rotate_left(5) as i16,
            y: !word as i16,
            z: word.rotate_left(11) as i16,
        };
        fixture.direct(
            &mut source,
            Corridor {
                center: Vector3 {
                    x: word.rotate_left(1) as i16,
                    y: 0,
                    z: word.rotate_left(7) as i16,
                },
                half_width: word as i16,
                half_height: -1,
                heading: Angle::from_units(
                    (word as u8 & 0xE0).wrapping_add(u8::from(word & 1 != 0)),
                ),
            },
        );
    }
}

#[test]
fn corridor_player_matches_original_every_mode_action_flag_and_full_height_word() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = Fixture::new(&mut source);
    for word in 0..=u16::MAX {
        for level in [false, true] {
            let record = fixture.records();
            record.auxiliary.as_mut().unwrap().mode = if level { 0x1F } else { word as u8 };
            record.auxiliary.as_mut().unwrap().action_flags =
                if level { 0x85 } else { (word >> 8) as u8 };
            record.steering.as_mut().unwrap().locked_heading = Angle::from_units(word as u8 & 0xE0);
            let boundary = record.boundary.as_mut().unwrap();
            boundary.center = Vector3 {
                x: word.rotate_left(1) as i16,
                y: 19,
                z: word.rotate_left(3) as i16,
            };
            boundary.half_width = word.rotate_left(7) as i16;
            boundary.return_position = Vector3 {
                x: 313,
                y: -719,
                z: 991,
            };
            fixture
                .native
                .objects
                .get_mut(fixture.native.owner)
                .unwrap()
                .base
                .position = Vector3 {
                x: word.rotate_left(5) as i16,
                y: word as i16,
                z: word.rotate_left(9) as i16,
            };
            fixture
                .native
                .objects
                .get_mut(fixture.proxy)
                .unwrap()
                .base
                .contacts
                .hit_marked = true;
            fixture.seed(&mut source);
            fixture.advance(&mut source);
        }
    }
}

#[test]
fn corridor_producer_matches_original_all_heading_pairs_admission_and_signed_radius_gates() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = Fixture::new(&mut source);
    let mut accepted = 0;
    let mut rejected = 0;
    for word in 0..=u16::MAX {
        let anchor = fixture.native.objects.get_mut(fixture.anchor).unwrap();
        anchor.base.yaw = Angle::from_units(word as u8);
        anchor.base.position = Vector3 {
            x: word.rotate_left(3) as i16,
            y: (word as i16).wrapping_add(1),
            z: !word as i16,
        };
        fixture
            .native
            .objects
            .get_mut(fixture.native.owner)
            .unwrap()
            .base
            .position = Vector3 {
            x: word as i16,
            y: word as i16,
            z: word.rotate_left(5) as i16,
        };
        fixture.records().auxiliary.as_mut().unwrap().action_flags =
            ((word >> 7) as u8 & !4) | u8::from(word & 1 == 0) * 4;
        fixture
            .native
            .objects
            .get_mut(fixture.proxy)
            .unwrap()
            .base
            .contacts
            .hit_marked = true;
        fixture.seed(&mut source);
        let inputs = RegionInputs {
            heading_offset: Angle::from_units((word >> 8) as u8),
            half_width: word.rotate_left(1) as i16,
            half_height: word.rotate_left(7) as i16,
            activation_radius: word.rotate_left(11) as i16,
        };
        if fixture.install(&mut source, inputs) {
            accepted += 1;
        } else {
            rejected += 1;
        }
    }
    assert!(accepted > 1000 && rejected > 1000, "{accepted} {rejected}");
}

#[test]
fn corridor_continuous_installation_leveling_correction_and_history_match_original() {
    let mut source = Source::new(&rom(), 0);
    let mut fixture = Fixture::new(&mut source);
    fixture.seed(&mut source);
    let mut accepted = 0;
    let mut corrected = 0;
    for visit in 0..4096u16 {
        // Independent retained state on each side; only external per-visit
        // movement, region actor and mode inputs are applied equally.
        let owner = fixture
            .native
            .objects
            .get_mut(fixture.native.owner)
            .unwrap();
        let delta = Vector3 {
            x: (visit as i16 % 19 - 9) * 3,
            y: visit as i16 % 17 - 8,
            z: (visit as i16 % 23 - 11) * 5,
        };
        owner.base.position.x = owner.base.position.x.wrapping_add(delta.x);
        owner.base.position.y = owner.base.position.y.wrapping_add(delta.y);
        owner.base.position.z = owner.base.position.z.wrapping_add(delta.z);
        for (offset, change) in [(12, delta.x), (14, delta.y), (16, delta.z)] {
            let target = WRAM + u32::from(OWNER) + offset;
            source.bus.write16(
                target,
                source.bus.read16(target).wrapping_add(change as u16),
            );
        }
        let center = Vector3 {
            x: (visit as i16 / 64) * 17,
            y: -40,
            z: -(visit as i16 / 128) * 11,
        };
        let anchor = fixture.native.objects.get_mut(fixture.anchor).unwrap();
        anchor.base.position = center;
        anchor.base.yaw = Angle::from_units(visit as u8 & 0xE0);
        let base = WRAM + u32::from(address(Some(fixture.anchor)));
        for (offset, value) in [(12, center.x), (14, center.y), (16, center.z)] {
            source.bus.write16(base + offset, value as u16);
        }
        source.bus.write8(base + 0x14, visit as u8 & 0xE0);
        let mode = if visit % 7 == 0 { 0x20 } else { 0x10 };
        fixture.records().auxiliary.as_mut().unwrap().mode = mode;
        source.bus.write8(WRAM + SLOT + 0x6AA0, mode);
        if visit % 31 == 0 {
            fixture.records().auxiliary.as_mut().unwrap().action_flags &= !4;
            let target = WRAM + SLOT + 0x6B77;
            source.bus.write8(target, source.bus.read8(target) & !4);
        }
        if fixture.install(
            &mut source,
            RegionInputs {
                heading_offset: Angle::from_units(0),
                half_width: 300,
                half_height: -1,
                activation_radius: 16000,
            },
        ) {
            accepted += 1;
        }
        corrected += usize::from(fixture.advance(&mut source));
    }
    assert!(accepted > 1000, "{accepted}");
    assert!(corrected > 100, "{corrected}");
}
