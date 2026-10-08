//! Map regions, group retirement and proximity streaming (`$0D:D5FA`)
//! against the unchanged original routine. Both engines start from the same
//! records, regions, views and flags; only results are compared.
use super::surface_particle_tests::{address, Native, OWNER};
use super::{rom, Source, WRAM};
use sf2_game::map_streaming::{
    self, MapProgram, MapRecord, MapRecordFlags, MapRecordId, MapRecordStore, MapRegion,
    MapRegions, RegionGroups, RegionPublication, StreamingInputs, StreamingState, StreamingWorld,
    MAP_RECORD_CAPACITY, NO_REGION,
};
use sf2_game::{
    authored_paths, Angle, Behavior, ObjectKind, ObjectSpawnDefaults, ShapeId, Vector3,
};

const RECORD_BASE: u32 = 0x342E;
const RECORD_SIZE: u32 = 0x19;
const REGION_BASE: u32 = 0x686A;
const VIEWS: [u32; 2] = [0x033F, 0x037E];
const SCENE_NINE_PATH: u16 = 0xD40E;

fn record_address(id: MapRecordId) -> u16 {
    (RECORD_BASE + id.index() as u32 * RECORD_SIZE) as u16
}

/// Small deterministic generator for scenario layout (not game randomness).
struct Layout(u32);
impl Layout {
    fn next(&mut self) -> u16 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.0 >> 12) as u16
    }
}

fn shape_extent(bytes: &[u8], index: u16) -> u16 {
    let header = 0xBC9C + u32::from(index) * 28;
    let file = (header & 0x7FFF) as usize; // bank 00
    u16::from_le_bytes([bytes[file + 0x10], bytes[file + 0x11]])
}

struct Scenario {
    records: MapRecordStore,
    regions: Vec<MapRegion>,
    groups: RegionGroups,
    views: [Vector3; 2],
    player: Vector3,
    state: StreamingState,
    inputs: StreamingInputs,
}

fn seed(source: &mut Source, scenario: &Scenario) {
    // Record pool: active list then free list, both in the native order.
    for slot in 0..MAP_RECORD_CAPACITY as u32 {
        for offset in 0..RECORD_SIZE {
            source
                .bus
                .write8(WRAM + RECORD_BASE + slot * RECORD_SIZE + offset, 0);
        }
    }
    let active = scenario.records.active_ids();
    for (index, &id) in active.iter().enumerate() {
        let base = u32::from(record_address(id));
        let next = active.get(index + 1).map_or(0, |&n| record_address(n));
        let previous = index
            .checked_sub(1)
            .map_or(0, |p| record_address(active[p]));
        let record = scenario.records.get(id).unwrap();
        for (offset, value) in [
            (0u32, next),
            (2, previous),
            (4, record.position.x as u16),
            (6, record.position.y as u16),
            (8, record.position.z as u16),
            (0x0D, 0xBC9C + record.shape.catalog_index() as u16 * 28),
        ] {
            source.bus.write16(WRAM + base + offset, value);
        }
        for (offset, angle) in [
            (0x0A, record.rotation[0]),
            (0x0B, record.rotation[1]),
            (0x0C, record.rotation[2]),
        ] {
            source.bus.write8(WRAM + base + offset, angle.units());
        }
        match record.program {
            MapProgram::Path(_) => source.bus.write16(WRAM + base + 0x0F, SCENE_NINE_PATH),
            MapProgram::Strategy(_) => {
                source.bus.write16(WRAM + base + 0x0F, 0);
                source.bus.write8(WRAM + base + 0x11, 0);
            }
        }
        source.bus.write8(WRAM + base + 0x12, record.flags.0);
        source
            .bus
            .write16(WRAM + base + 0x13, address(record.spawned));
        source.bus.write8(WRAM + base + 0x18, record.group);
    }
    source.bus.write16(
        WRAM + 0x1285,
        active.first().map_or(0, |&id| record_address(id)),
    );
    // Free slots: the native pool pops from the end; chain them in that order.
    let free: Vec<u16> = (0..MAP_RECORD_CAPACITY as u16)
        .map(|slot| (RECORD_BASE + u32::from(slot) * RECORD_SIZE) as u16)
        .filter(|&addr| !active.iter().any(|&id| record_address(id) == addr))
        .collect();
    let top = free.first().copied().unwrap_or(0);
    // Rebuild so that the top is the lowest free slot and links ascend.
    for (index, &addr) in free.iter().enumerate() {
        let next = free.get(index + 1).copied().unwrap_or(0);
        source.bus.write16(WRAM + u32::from(addr), next);
    }
    source.bus.write16(WRAM + 0x1283, top);
    // Regions.
    for (index, region) in scenario.regions.iter().enumerate() {
        let base = REGION_BASE + index as u32 * 16;
        source.bus.write16(WRAM + base, region.origin_x);
        source.bus.write16(WRAM + base + 4, region.origin_z);
        source.bus.write16(WRAM + base + 6, region.width);
        source.bus.write16(WRAM + base + 0x0A, region.depth);
        source.bus.write16(WRAM + base + 0x0C, region.entry_word);
        source.bus.write8(WRAM + base + 0x0E, region.entry_byte);
        source.bus.write8(WRAM + base + 0x0F, region.flags);
    }
    source
        .bus
        .write8(WRAM + 0x1910, scenario.regions.len() as u8);
    source.bus.write8(WRAM + 0x190E, scenario.groups.current);
    source.bus.write8(WRAM + 0x190F, scenario.groups.previous);
    source.bus.write16(WRAM + 0x1657, 0xA5A5);
    source.bus.write8(WRAM + 0x192E, 0x5A);
    // Views and the primary player.
    for (view, position) in VIEWS.into_iter().zip(scenario.views) {
        source.bus.write16(view + 0x0C, position.x as u16);
        source.bus.write16(view + 0x0E, position.y as u16);
        source.bus.write16(view + 0x10, position.z as u16);
    }
    source.bus.write16(0x12C3, OWNER);
    source
        .bus
        .write16(u32::from(OWNER) + 0x0C, scenario.player.x as u16);
    source
        .bus
        .write16(u32::from(OWNER) + 0x10, scenario.player.z as u16);
    // Flags and state.
    let i = scenario.inputs;
    source
        .bus
        .write8(WRAM + 0x1E16, if i.hold_regions { 0x80 } else { 0 });
    source
        .bus
        .write16(WRAM + 0x1B84, if i.scripted_view { 2 } else { 0 });
    source.bus.write8(
        WRAM + 0x1AA6,
        u8::from(i.second_view_streams) | u8::from(i.first_view_decides) * 2,
    );
    source.bus.write16(WRAM + 0xD739, i.radius_limit);
    source.bus.write16(WRAM + 0x1A83, i.base_radius);
    source
        .bus
        .write8(WRAM + 0xCF35, scenario.state.rescan_countdown);
    for (index, (x, z)) in scenario.state.scanned_views.into_iter().enumerate() {
        source.bus.write16(WRAM + 0xCF21 + index as u32 * 4, x);
        source.bus.write16(WRAM + 0xCF23 + index as u32 * 4, z);
    }
}

fn compare(
    source: &Source,
    native: &Native,
    scenario: &Scenario,
    publication: Option<RegionPublication>,
    context: &str,
) {
    native.compare_pool(source);
    assert_eq!(
        source.bus.read8(WRAM + 0x190E),
        scenario.groups.current,
        "{context} 190E"
    );
    assert_eq!(
        source.bus.read8(WRAM + 0x190F),
        scenario.groups.previous,
        "{context} 190F"
    );
    match publication {
        Some(p) => {
            assert_eq!(
                source.bus.read16(WRAM + 0x1657),
                p.entry_word,
                "{context} 1657"
            );
            assert_eq!(
                source.bus.read8(WRAM + 0x192E),
                p.entry_byte,
                "{context} 192E"
            );
        }
        None => {
            assert_eq!(source.bus.read16(WRAM + 0x1657), 0xA5A5, "{context} 1657");
            assert_eq!(source.bus.read8(WRAM + 0x192E), 0x5A, "{context} 192E");
        }
    }
    assert_eq!(
        source.bus.read8(WRAM + 0xCF35),
        scenario.state.rescan_countdown,
        "{context} CF35"
    );
    for (index, (x, z)) in scenario.state.scanned_views.into_iter().enumerate() {
        assert_eq!(
            source.bus.read16(WRAM + 0xCF21 + index as u32 * 4),
            x,
            "{context} cell x {index}"
        );
        assert_eq!(
            source.bus.read16(WRAM + 0xCF23 + index as u32 * 4),
            z,
            "{context} cell z {index}"
        );
    }
    // Record list order and contents.
    let mut cursor = source.bus.read16(WRAM + 0x1285);
    for &id in scenario.records.active_ids() {
        assert_eq!(cursor, record_address(id), "{context} record order");
        let base = u32::from(cursor);
        let record = scenario.records.get(id).unwrap();
        assert_eq!(
            source.bus.read8(WRAM + base + 0x12),
            record.flags.0,
            "{context} record {} flags",
            id.index()
        );
        assert_eq!(
            source.bus.read16(WRAM + base + 0x13),
            address(record.spawned),
            "{context} record {} actor",
            id.index()
        );
        cursor = source.bus.read16(WRAM + base);
    }
    assert_eq!(cursor, 0, "{context} record list end");
    // Streamed actors.
    for (id, actor) in native.objects.active_objects() {
        let base = u32::from(address(Some(id)));
        assert_eq!(
            source.bus.read16(WRAM + base + 0x1CE6),
            actor.extension.map_record.map_or(0, record_address),
            "{context} actor {} record link",
            id.index()
        );
        let Some(record) = actor.extension.map_record else {
            continue;
        };
        let record = scenario.records.get(record).copied();
        assert_eq!(
            source.bus.read8(WRAM + base + 0x1CF0),
            actor.extension.spawn_group,
            "{context} group"
        );
        for (offset, value) in [
            (0x0C, actor.base.position.x),
            (0x0E, actor.base.position.y),
            (0x10, actor.base.position.z),
        ] {
            assert_eq!(
                source.bus.read16(base + offset) as i16,
                value,
                "{context} actor pos {offset:X}"
            );
        }
        for (offset, value) in [
            (0x12, actor.base.pitch.units()),
            (0x14, actor.base.yaw.units()),
            (0x16, actor.base.roll.units()),
        ] {
            assert_eq!(
                source.bus.read8(base + offset),
                value,
                "{context} actor rot {offset:X}"
            );
        }
        assert_eq!(
            source.bus.read16(base + 4),
            0xBC9C + actor.base.shape.catalog_index() as u16 * 28,
            "{context} actor shape"
        );
        if let Some(MapProgram::Path(_)) = record.map(|r| r.program) {
            assert_eq!(source.bus.read16(base + 0x2B), SCENE_NINE_PATH);
            // The one-time prefix ($7F:7E1E) has not run yet on either side.
            assert!(actor.extension.path_state.needs_path_initialization);
            assert_eq!(source.bus.read16(base + 0x19), 0x7E1E);
            assert_eq!(source.bus.read8(base + 0x1B), 0x7F);
        }
    }
}

#[test]
fn streaming_and_regions_match_original_across_generated_scenarios() {
    let bytes = rom();
    let mut layout = Layout(0x5EED);
    let (mut spawns, mut releases, mut publications, mut retirements, mut full_pools) =
        (0, 0, 0, 0, 0);
    for case in 0..400u32 {
        let mut source = Source::new(&bytes, 0);
        let mut native = Native::new(&mut source, 3 + (case as usize % 57), 0, 0, case as u16);
        let center = |l: &mut Layout| (l.next() % 6000) as i16 - 3000;
        let views = [
            Vector3 {
                x: center(&mut layout),
                y: 0,
                z: center(&mut layout),
            },
            Vector3 {
                x: center(&mut layout),
                y: 0,
                z: center(&mut layout),
            },
        ];
        let player = Vector3 {
            x: center(&mut layout),
            y: 0,
            z: center(&mut layout),
        };
        let mut regions = Vec::new();
        for _ in 0..(layout.next() % 5) {
            regions.push(MapRegion {
                origin_x: (center(&mut layout) - 1500) as u16,
                origin_z: (center(&mut layout) - 1500) as u16,
                width: 500 + layout.next() % 3000,
                depth: 500 + layout.next() % 3000,
                entry_word: layout.next(),
                entry_byte: layout.next() as u8,
                flags: if layout.next() % 5 == 0 { 0 } else { 2 },
            });
        }
        let pick = |l: &mut Layout| {
            let r = l.next() % 7;
            if r < regions.len() as u16 {
                r as u8
            } else {
                NO_REGION
            }
        };
        let groups = RegionGroups {
            current: pick(&mut layout),
            previous: pick(&mut layout),
        };
        let mut records = MapRecordStore::new();
        for _ in 0..(layout.next() % 24) {
            let shape = 1 + layout.next() % 200;
            let path = layout.next() % 3 == 0;
            let flags = (if path { MapRecordFlags::PATH } else { 0 })
                | (if layout.next() % 4 == 0 {
                    MapRecordFlags::FIXED_RADIUS
                } else {
                    0
                })
                | (if layout.next() % 6 == 0 {
                    MapRecordFlags::SUPPRESSED
                } else {
                    0
                });
            let anchor = views[usize::from(layout.next() % 2)];
            let spread = |l: &mut Layout| (l.next() % 4000) as i16 - 2000;
            records.allocate(MapRecord {
                position: Vector3 {
                    x: anchor.x.wrapping_add(spread(&mut layout)),
                    y: layout.next() as i16,
                    z: anchor.z.wrapping_add(spread(&mut layout)),
                },
                rotation: [
                    Angle::from_units(layout.next() as u8),
                    Angle::from_units(layout.next() as u8),
                    Angle::from_units(layout.next() as u8),
                ],
                shape: ShapeId::from_catalog_index(shape),
                shape_extent: shape_extent(&bytes, shape),
                kind: ObjectKind::Scenery,
                program: if path {
                    MapProgram::Path(authored_paths::SCENE_NINE)
                } else {
                    MapProgram::Strategy(Behavior::Unassigned)
                },
                flags: MapRecordFlags(flags),
                spawned: None,
                group: if layout.next() % 3 == 0 {
                    NO_REGION
                } else {
                    (layout.next() % 4) as u8
                },
            });
        }
        let inputs = StreamingInputs {
            hold_regions: layout.next() % 8 == 0,
            scripted_view: layout.next() % 10 == 0,
            second_view_streams: layout.next() % 2 == 0,
            first_view_decides: layout.next() % 3 == 0,
            radius_limit: 1000 + layout.next() % 3000,
            base_radius: 400 + layout.next() % 900,
        };
        let state = StreamingState {
            rescan_countdown: (layout.next() % 3) as u8,
            scanned_views: [
                (
                    views[0].x as u16 ^ (layout.next() & 0x0300),
                    views[0].z as u16,
                ),
                (
                    views[1].x as u16,
                    views[1].z as u16 ^ (layout.next() & 0x0100),
                ),
            ],
        };
        let mut scenario = Scenario {
            records,
            regions,
            groups,
            views,
            player,
            state,
            inputs,
        };
        seed(&mut source, &scenario);
        let regions = MapRegions::new(scenario.regions.clone());
        // Several consecutive frames: views drift so records leave and return.
        for frame in 0..6 {
            let mut publication = None;
            let before: Vec<_> = scenario
                .records
                .active_ids()
                .iter()
                .map(|&id| (id, scenario.records.get(id).unwrap().flags))
                .collect();
            if native.objects.len() == 60 {
                full_pools += 1;
            }
            source.run(0x0DD5FA, None, 0, OWNER, true);
            let defaults = ObjectSpawnDefaults {
                group: scenario.groups.current,
                run_when_paused: inputs.scripted_view,
            };
            map_streaming::stream(
                &mut native.objects,
                &mut scenario.records,
                StreamingWorld {
                    inputs,
                    state: &mut scenario.state,
                    regions: &regions,
                    groups: &mut scenario.groups,
                    publication: &mut publication,
                    primary_player: scenario.player,
                    views: scenario.views,
                    spawn_defaults: Some(defaults),
                },
            )
            .unwrap();
            let context = format!("case {case} frame {frame}");
            publications += usize::from(publication.is_some());
            retirements += usize::from(scenario.records.active_ids().len() < before.len());
            for (id, flags) in before {
                if let Some(record) = scenario.records.get(id) {
                    let was = flags.has(MapRecordFlags::SPAWNED);
                    let now = record.flags.has(MapRecordFlags::SPAWNED);
                    spawns += usize::from(!was && now);
                    releases += usize::from(was && !now);
                }
            }
            compare(&source, &native, &scenario, publication, &context);
            // Reseed the shared publication sentinels for the next frame.
            source.bus.write16(WRAM + 0x1657, 0xA5A5);
            source.bus.write8(WRAM + 0x192E, 0x5A);
            let step = (layout.next() % 1200) as i16 - 600;
            for (index, view) in scenario.views.iter_mut().enumerate() {
                view.x = view.x.wrapping_add(step);
                view.z = view.z.wrapping_sub(step / 2);
                source.bus.write16(VIEWS[index] + 0x0C, view.x as u16);
                source.bus.write16(VIEWS[index] + 0x10, view.z as u16);
            }
            scenario.player.x = scenario.player.x.wrapping_add(step);
            source
                .bus
                .write16(u32::from(OWNER) + 0x0C, scenario.player.x as u16);
        }
    }
    // The generated scenarios must exercise every outcome being compared.
    assert!(spawns > 100, "spawns {spawns}");
    assert!(releases > 20, "releases {releases}");
    assert!(publications > 10, "publications {publications}");
    assert!(retirements > 5, "retirements {retirements}");
    assert!(full_pools > 0, "full pools {full_pools}");
}
