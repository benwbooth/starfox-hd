use super::*;

fn region(x: u16, z: u16, size: u16, word: u16) -> MapRegion {
    MapRegion {
        origin_x: x,
        origin_z: z,
        width: size,
        depth: size,
        entry: Some(MapCursor::from_index(word)),
    }
}

fn at(x: i16, z: i16) -> Vector3 {
    Vector3 { x, y: 0, z }
}

#[test]
fn entering_a_region_selects_it_publishes_once_and_retires_the_group_left_behind() {
    let regions = MapRegions::new(vec![
        region(0, 0, 100, 0x1111),
        region(1000, 0, 100, 0x2222),
    ]);
    let none = RegionGroups {
        current: NO_REGION,
        previous: NO_REGION,
    };
    let first = select_regions(&regions, at(10, 10), none).unwrap();
    assert_eq!(
        first.groups,
        RegionGroups {
            current: 0,
            previous: NO_REGION
        }
    );
    assert_eq!(first.publication.unwrap().entry, MapCursor::from_index(0x1111));
    let again = select_regions(&regions, at(20, 20), first.groups).unwrap();
    assert_eq!(again.publication, None);
    let moved = select_regions(&regions, at(1010, 10), first.groups).unwrap();
    assert_eq!(
        moved.groups,
        RegionGroups {
            current: 1,
            previous: NO_REGION
        }
    );
    assert_eq!(moved.retired, [Some(0), None]);
}

#[test]
fn leaving_every_region_keeps_the_old_selection() {
    let regions = MapRegions::new(vec![region(0, 0, 100, 1)]);
    let old = RegionGroups {
        current: 0,
        previous: NO_REGION,
    };
    let update = select_regions(&regions, at(-500, -500), old).unwrap();
    assert_eq!(update.groups, old);
    assert_eq!(update.retired, [None, None]);
}

#[test]
fn unscannable_regions_are_ignored() {
    let mut hidden = region(0, 0, 100, 1);
    hidden.entry = None;
    let regions = MapRegions::new(vec![hidden]);
    let none = RegionGroups {
        current: NO_REGION,
        previous: NO_REGION,
    };
    assert_eq!(select_regions(&regions, at(5, 5), none).unwrap().groups, none);
}

#[test]
fn records_insert_after_the_head_and_free_slots_are_reused_last_in_first_out() {
    let mut store = MapRecordStore::new();
    let record = MapRecord {
        position: at(0, 0),
        rotation: [Angle::ZERO; 3],
        shape: ShapeId::EMPTY,
        shape_extent: 0,
        kind: ObjectKind::Scenery,
        program: MapProgram::Strategy(Behavior::Unassigned),
        flags: MapRecordFlags::default(),
        spawned: None,
        group: 3,
    };
    let a = store.allocate(record).unwrap();
    let b = store.allocate(record).unwrap();
    let c = store.allocate(record).unwrap();
    assert_eq!(store.active_ids(), &[a, c, b]);
    let mut objects = ObjectStore::new();
    retire_group(&mut objects, &mut store, 3);
    assert!(store.active_ids().is_empty());
    // Released in list order (a, c, b), so b is reused first.
    assert_eq!(store.allocate(record), Some(b));
}
