//! Execute the original ordinary map-spawn handler and fresh-record formatter.
//! The synthetic data stream bounds dispatch at STOP; no executing code is
//! replaced. This covers allocation/defaults, not the installed strategies.

use sf2_data::map::{SpawnRecord, SPAWN_RECORDS};
use sf2_game::scene_map::{
    allocate_map_actor, MapActorSpawn, MapCatalog, MapCondition, MapCursor, MapFramePolicy,
    MapInstruction, MapStop, SceneMap, SceneMapHost,
};
use sf2_game::{
    Behavior, ObjectId, ObjectKind, ObjectSpawnDefaults, ObjectStore, ShapeId, Vector3,
};
use sf_oracle::{call, Entry, SnesBus};

fn original() -> SnesBus {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned retail SF2 ROM");
    let runtime = rom[0x10000..0x17E00].to_vec();
    let mut source = SnesBus::new(rom);
    for (offset, byte) in runtime.into_iter().enumerate() {
        source.write8(0x7F0000 + offset as u32, byte);
    }
    source
}

fn address(id: Option<ObjectId>) -> u16 {
    id.map_or(0, |id| 0x03BD + id.index() as u16 * 0x3F)
}

fn reset_pool(source: &mut SnesBus) {
    source.write16(0x12A8, 0);
    source.write16(0x12AA, 0x03BD);
    source.write16(0x1651, 0);
    for index in 0..60 {
        let actor = 0x03BD + index * 0x3F;
        // The formatter must clear recycled data, not rely on zeroed memory.
        for byte in 0..0x3F {
            source.write8(actor + byte, 0xA5);
            source.write8(0x7E1CC1 + actor + byte, 0x5A);
        }
        source.write16(actor, if index == 59 { 0 } else { actor as u16 + 0x3F });
    }
}

fn dispatch(source: &mut SnesBus, record: SpawnRecord, marker: u16) {
    let words = [
        marker,
        record.x as u16,
        record.y as u16,
        record.z as u16,
        record.shape,
    ];
    source.write8(0x7EA000, 0x86);
    for (index, value) in words.into_iter().enumerate() {
        source.write16(0x7EA001 + index as u32 * 2, value);
    }
    for byte in 0..3 {
        source.write8(0x7EA00B + byte, (record.strategy >> (byte * 8)) as u8);
    }
    source.write8(0x7EA00E, 2);
    source.write8(0x192E, 0x7E);
    source.write16(0x1657, 0x2000);
    let result = call(
        source,
        0x038FC9,
        &Entry {
            x: 0x2000,
            p: 0x20,
            ..Default::default()
        },
    );
    assert!(result.returned, "spawn dispatcher failed to return");
    assert_eq!(source.read16(0x1657), 0x200E);
    assert_eq!(source.read16(0x1655), marker);
}

struct PoolHost {
    objects: ObjectStore,
    defaults: ObjectSpawnDefaults,
}

impl SceneMapHost<(), MapActorSpawn> for PoolHost {
    type Error = std::convert::Infallible;
    fn condition(&self, _: MapCondition) -> Result<bool, Self::Error> {
        panic!("unexpected condition")
    }
    fn apply(&mut self, _: &()) -> Result<(), Self::Error> {
        panic!("unexpected effect")
    }
    fn apply_to_current(&mut self, _: ObjectId, _: &()) -> Result<(), Self::Error> {
        panic!("unexpected actor effect")
    }
    fn spawn(&mut self, spawn: &MapActorSpawn) -> Result<Option<ObjectId>, Self::Error> {
        Ok(allocate_map_actor(&mut self.objects, self.defaults, *spawn))
    }
}

fn specification(record: SpawnRecord) -> MapActorSpawn {
    assert_eq!(record.opcode, 0x86);
    assert!(record.linked_object.is_none());
    assert_eq!((record.shape - 0xBC9C) % 28, 0);
    MapActorSpawn {
        kind: ObjectKind::Effect,
        shape: ShapeId::from_catalog_index((record.shape - 0xBC9C) / 28),
        // No strategy is executed in this allocation-only test. The real
        // scene owner must bind each source strategy to its typed installer.
        behavior: Behavior::FollowPath,
        position: Vector3 {
            x: record.x,
            y: record.y,
            z: record.z,
        },
    }
}

fn compare(source: &SnesBus, host: &PoolHost, map: &SceneMap) {
    assert_eq!(source.read16(0x1651), address(map.current_object()));
    assert_eq!(
        source.read16(0x12A8),
        address(host.objects.active_ids().first().copied())
    );
    for (id, actor) in host.objects.active_objects() {
        let base = u32::from(address(Some(id)));
        assert_eq!(source.read16(base), address(actor.base.next));
        assert_eq!(source.read16(base + 2), address(actor.base.previous));
        assert_eq!(
            usize::from(source.read16(base + 4)),
            0xBC9C + actor.base.shape.catalog_index() * 28
        );
        assert_eq!(source.read16(base + 6), address(actor.base.attachment));
        assert_eq!(source.read16(base + 12) as i16, actor.base.position.x);
        assert_eq!(source.read16(base + 14) as i16, actor.base.position.y);
        assert_eq!(source.read16(base + 16) as i16, actor.base.position.z);
        assert_eq!(source.read8(base + 0x2D), actor.base.hit_points);
        assert_eq!(source.read8(base + 0x2E), actor.base.attack_power);
        assert_eq!(source.read8(base + 0x18), actor.base.speed);
        assert_eq!(source.read8(base + 0x0A), actor.base.target_speed);
        assert_eq!(
            source.read8(base + 0x08) & 0x10 != 0,
            actor.base.flags.draw_list_admitted
        );
        assert_eq!(
            source.read8(base + 0x09) & 0x08 != 0,
            actor.extension.path_state.hold_latched
        );
        assert_eq!(
            source.read8(base + 0x22) & 0x04 != 0,
            actor.base.flags.general_search_eligible
        );
        assert_eq!(
            source.read8(base + 0x26) & 0x08 != 0,
            actor.base.contacts.run_when_paused
        );
        assert_eq!(
            source.read8(base + 0x31) & 0x04 != 0,
            actor.base.contacts.first_strategy_visit
        );
        assert_eq!(
            source.read8(base + 0x25) & 0x08 != 0,
            actor.base.flags.remove_after_tick
        );
        assert_eq!(source.read8(0x7E1CF0 + base), actor.extension.spawn_group);
    }
}

#[test]
fn every_authored_spawn_preserves_the_original_pool_and_fresh_actor_contract() {
    let mut source = original();
    assert_eq!(SPAWN_RECORDS.len(), 232);
    for defaults in [
        ObjectSpawnDefaults::default(),
        ObjectSpawnDefaults {
            run_when_paused: true,
            group: 0xFF,
        },
    ] {
        source.write16(
            0x1B84,
            if defaults.run_when_paused {
                0xFFFF
            } else {
                0xFFFD
            },
        );
        source.write8(0x190E, defaults.group);
        for record in SPAWN_RECORDS {
            for marker in [0, 1, 5000, u16::MAX] {
                reset_pool(&mut source);
                let mut host = PoolHost {
                    objects: ObjectStore::new(),
                    defaults,
                };
                let program: [MapInstruction<(), MapActorSpawn>; 2] = [
                    MapInstruction::Spawn {
                        specification: specification(record),
                        marker,
                        next: MapCursor::from_index(1),
                    },
                    MapInstruction::Stop,
                ];
                let catalog = MapCatalog::new(&program, &[]).unwrap();
                let mut map = SceneMap::new(&catalog, MapCursor::from_index(0)).unwrap();
                source.write16(0x1655, 0xCAFE);
                dispatch(&mut source, record, marker);
                let report = map
                    .visit(&catalog, &mut host, MapFramePolicy::default(), 2)
                    .unwrap();
                assert_eq!(
                    report.stop,
                    if marker == 0 {
                        MapStop::Stopped
                    } else {
                        MapStop::Yielded(marker)
                    }
                );
                assert_eq!(map.yield_marker(), source.read16(0x1655));
                compare(&source, &host, &map);
                assert_eq!(source.read16(0x12AA), 0x03FC);
            }
        }
    }
}

#[test]
fn full_pool_clears_the_current_actor_without_pressure_retiring_earlier_effects() {
    let mut source = original();
    for marker in [0, 1, 5000, u16::MAX] {
        reset_pool(&mut source);
        let mut host = PoolHost {
            objects: ObjectStore::new(),
            defaults: ObjectSpawnDefaults::default(),
        };
        let record = SpawnRecord {
            shape: 0xBC9C + 9 * 28,
            ..SPAWN_RECORDS[0]
        };
        let program: [MapInstruction<(), MapActorSpawn>; 2] = [
            MapInstruction::Spawn {
                specification: specification(record),
                marker,
                next: MapCursor::from_index(1),
            },
            MapInstruction::Stop,
        ];
        let catalog = MapCatalog::new(&program, &[]).unwrap();
        let mut map = SceneMap::new(&catalog, MapCursor::from_index(0)).unwrap();
        for attempt in 0..61 {
            map.redirect(&catalog, MapCursor::from_index(0)).unwrap();
            dispatch(&mut source, record, marker);
            map.visit(&catalog, &mut host, MapFramePolicy::default(), 2)
                .unwrap();
            compare(&source, &host, &map);
            assert_eq!(map.current_object().is_some(), attempt < 60);
            assert_eq!(host.objects.active_ids().len(), (attempt + 1).min(60));
            assert_eq!(source.read16(0x12AA) == 0, attempt >= 59);
        }
    }
}
