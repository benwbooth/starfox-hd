//! Original scene clear plus actual proxy initialization, capture and reuse.
use super::surface_particle_tests::{address, Native, OWNER};
use super::{rom, Source, WRAM};
use sf2_game::program_resources::ProgramResources;
use sf2_game::program_state::ProgramData;
use sf2_game::scene_proxy::{SceneProxy, SceneProxyId, SceneProxyStore, SCENE_PROXY_CAPACITY};
use sf2_game::{Angle, ObjectId, PathCursor, PathId, ShapeId, Vector3};

fn proxy_address(id: Option<SceneProxyId>) -> u16 {
    id.map_or(0, |id| 0x342E + id.index() as u16 * 25)
}

pub(super) fn setup(source: &mut Source, count: usize, seed: u8) -> Native {
    let mut native = Native::new(source, count, seed, seed.rotate_left(2), 0xCAFE);
    let rom = rom();
    for (offset, &byte) in rom[0x50000..0x54E00].iter().enumerate() {
        source.bus.write8(0x7F7E00 + offset as u32, byte);
    }
    source.run(0x0DD5B7, None, 0, OWNER, true);
    for id in native.objects.active_ids().to_vec() {
        let index = id.index() as u8;
        let flags = seed.wrapping_add(index.wrapping_mul(47));
        let removal_flags = seed.rotate_left(u32::from(index & 7)) ^ 0xA5;
        let actor = native.objects.get_mut(id).unwrap();
        actor.base.flags.general_search_eligible = flags & 4 != 0;
        actor.base.flags.remove_after_tick = removal_flags & 8 != 0;
        actor.base.position = Vector3 {
            x: (u16::from(flags) * 131) as i16,
            y: (u16::from(removal_flags) * 193) as i16,
            z: i16::from(index) * -317,
        };
        actor.base.pitch = Angle::from_units(flags);
        actor.base.yaw = Angle::from_units(removal_flags);
        actor.base.roll = Angle::from_units(index);
        let base = u32::from(address(Some(id)));
        source.bus.write8(base + 0x22, flags);
        source.bus.write8(base + 0x25, removal_flags);
        for (offset, value) in [
            (12, actor.base.position.x),
            (14, actor.base.position.y),
            (16, actor.base.position.z),
        ] {
            source.bus.write16(base + offset, value as u16);
        }
        for (offset, value) in [(0x12, flags), (0x14, removal_flags), (0x16, index)] {
            source.bus.write8(base + offset, value);
        }
    }
    compare(source, &native);
    native
}

pub(super) fn compare(source: &Source, native: &Native) {
    native.compare_pool(source);
    let proxies = &native.world.proxies;
    assert_eq!(
        source.bus.read16(0x1285),
        proxy_address(proxies.active_ids().first().copied())
    );
    for (index, &id) in proxies.active_ids().iter().enumerate() {
        let base = WRAM + u32::from(proxy_address(Some(id)));
        let proxy = proxies.get(id).unwrap();
        assert_eq!(
            source.bus.read16(base),
            proxy_address(proxies.active_ids().get(index + 1).copied())
        );
        assert_eq!(
            source.bus.read16(base + 2),
            proxy_address(
                index
                    .checked_sub(1)
                    .map(|prior| proxies.active_ids()[prior])
            )
        );
        for (offset, value) in [
            (4, proxy.position.x),
            (6, proxy.position.y),
            (8, proxy.position.z),
        ] {
            assert_eq!(source.bus.read16(base + offset), value as u16);
        }
        for (offset, value) in [
            (10, proxy.rotation.pitch.units()),
            (11, proxy.rotation.yaw.units()),
            (12, proxy.rotation.roll.units()),
        ] {
            assert_eq!(source.bus.read8(base + offset), value);
        }
        assert_eq!(source.bus.read16(base + 13), 0xBC9C);
        assert_eq!(
            source.bus.read16(base + 15),
            proxy.continuation.command_index
        );
        assert_eq!(source.bus.read8(base + 18), proxy.flags.authored_bits());
        assert_eq!(source.bus.read16(base + 19), address(proxy.owner));
    }
    for (id, actor) in native.objects.active_objects() {
        assert_eq!(
            source
                .bus
                .read16(WRAM + u32::from(address(Some(id))) + 0x1CE6),
            proxy_address(actor.extension.scene_proxy)
        );
    }
    compare_free(source, proxies);
}

fn compare_free(source: &Source, proxies: &SceneProxyStore) {
    let mut copy = proxies.clone();
    let empty = SceneProxy {
        position: Default::default(),
        rotation: Default::default(),
        shape: ShapeId::EMPTY,
        continuation: PathCursor {
            path: PathId::from_catalog_index(2),
            command_index: 0,
        },
        flags: Default::default(),
        owner: None,
    };
    let mut current = source.bus.read16(0x1283);
    let mut free_count = 0;
    while let Some(id) = copy.allocate(empty) {
        assert_eq!(current, proxy_address(Some(id)));
        current = source.bus.read16(WRAM + u32::from(current));
        free_count += 1;
        assert!(free_count <= SCENE_PROXY_CAPACITY);
    }
    assert_eq!(current, 0);
    assert_eq!(free_count + proxies.len(), SCENE_PROXY_CAPACITY);
}

pub(super) fn capture(source: &mut Source, native: &mut Native, owner: ObjectId, continuation: u16) {
    let base = u32::from(address(Some(owner)));
    source
        .bus
        .write16(base + 0x2B, continuation.wrapping_sub(1));
    source.run(0x7FAF00, Some(0x7FCAE8), 0, base as u16, true);
    native
        .world
        .proxies
        .capture_actor(
            &mut native.objects,
            owner,
            PathCursor {
                path: PathId::from_catalog_index(2),
                command_index: continuation,
            },
            &ProgramResources::<ProgramData>::default(),
        )
        .unwrap();
    compare(source, native);
}

fn clear(source: &mut Source, native: &mut Native) {
    let mut expected = native.objects.clone();
    let mut flag_bytes = Vec::new();
    for id in native.objects.active_ids().to_vec() {
        let base = u32::from(address(Some(id)));
        let selection = source.bus.read8(base + 0x22);
        let flags = source.bus.read8(base + 0x25);
        flag_bytes.push((
            base,
            selection,
            flags | if selection & 4 != 0 { 8 } else { 0 },
        ));
        let actor = expected.get_mut(id).unwrap();
        if actor.base.flags.general_search_eligible {
            actor.extension.scene_proxy = None;
            actor.base.flags.remove_after_tick = true;
        }
    }
    source.run(0x03AC81, None, 0, OWNER, true);
    sf2_game::scene_clear::clear(&mut native.objects, &mut native.world.proxies).unwrap();
    compare(source, native);
    assert_eq!(native.objects, expected);
    assert!(native.world.proxies.is_empty());
    for (base, selection, removal) in flag_bytes {
        assert_eq!(source.bus.read8(base + 0x22), selection);
        assert_eq!(source.bus.read8(base + 0x25), removal);
    }
}

#[test]
fn scene_clear_matches_original_all_selection_and_removal_bytes_and_actual_reuse() {
    let mut source = Source::new(&rom(), 0);
    for seed in 0..=u8::MAX {
        let mut native = setup(&mut source, 6, seed);
        let owners = native.objects.active_ids().to_vec();
        for (index, &owner) in owners.iter().enumerate() {
            capture(&mut source, &mut native, owner, index as u16 + 0x8000);
        }
        // Repeated capture orphans the old snapshot; retirement retains a
        // detached snapshot. Both must be collected in the second pass.
        capture(&mut source, &mut native, owners[0], 0xFFFF);
        source.run(0x7F3425, None, 0, address(Some(owners[2])), true);
        native
            .world
            .proxies
            .retire_actor(&mut native.objects, owners[2])
            .unwrap();
        compare(&source, &native);
        clear(&mut source, &mut native);
        clear(&mut source, &mut native);
        for (index, &owner) in owners.iter().enumerate() {
            capture(&mut source, &mut native, owner, index as u16);
        }
        clear(&mut source, &mut native);
    }
}

#[test]
fn scene_clear_matches_original_pool_exhaustion_and_every_reclaimed_slot() {
    let mut source = Source::new(&rom(), 0);
    for count in [1, 2, 59, 60] {
        for captures in [0, 1, 2, 17, 511, 512] {
            let mut native = setup(&mut source, count, 211);
            let owners = native.objects.active_ids().to_vec();
            for index in 0..captures {
                capture(
                    &mut source,
                    &mut native,
                    owners[index % count],
                    index as u16,
                );
            }
            if captures == 512 {
                capture(&mut source, &mut native, owners[0], 999);
            }
            clear(&mut source, &mut native);
            for index in 0..512 {
                capture(
                    &mut source,
                    &mut native,
                    owners[index % count],
                    index as u16 ^ 0xABCD,
                );
            }
            capture(&mut source, &mut native, owners[0], 777);
            clear(&mut source, &mut native);
        }
    }
}

#[test]
fn scene_clear_matches_original_without_actors_but_with_retained_proxies() {
    let mut source = Source::new(&rom(), 0);
    let mut native = setup(&mut source, 1, 0);
    let owner = native.owner;
    for index in 0..17 {
        capture(&mut source, &mut native, owner, index);
    }
    // The actor-list input is empty; live proxy contents and source pool links
    // remain the results of real capture, not fabricated expected output.
    source.bus.write16(0x12A8, 0);
    native.objects = Default::default();
    source.run(0x03AC81, None, 0, OWNER, true);
    sf2_game::scene_clear::clear(&mut native.objects, &mut native.world.proxies).unwrap();
    assert!(native.world.proxies.is_empty());
    assert_eq!(source.bus.read16(0x1285), 0);
    compare_free(&source, &native.world.proxies);
}
