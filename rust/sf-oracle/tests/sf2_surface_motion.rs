//! Complete original surface queries and collision-constrained movement.
//! ROM instructions, shape headers and coprocessor programs are unchanged.

use sf2_game::collision_surface::{self, SurfaceSearch};
use sf2_game::{Angle, Behavior, Object, ObjectId, ObjectKind, ObjectStore, ShapeId, Vector3};
use sf_oracle::{call, Entry, SnesBus};

const OWNER: u16 = 0x03BD;
const WRAM: u32 = 0x7E0000;

fn original() -> SnesBus {
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .expect("user-owned SF2 ROM required");
    let mut source = SnesBus::new(rom.clone());
    for (index, &byte) in rom[0x10000..0x17E00].iter().enumerate() {
        source.write8(0x7F0000 + index as u32, byte);
    }
    source.enable_gsu();
    source
}

fn address(id: Option<ObjectId>) -> u16 {
    id.map_or(0, |id| OWNER + id.index() as u16 * 0x3F)
}

fn seed(source: &mut SnesBus, objects: &ObjectStore) {
    source.write16(0x12A8, address(objects.active_ids().first().copied()));
    for (id, object) in objects.active_objects() {
        let base = WRAM + u32::from(address(Some(id)));
        for offset in 0..0x3F {
            source.write8(base + offset, 0);
            source.write8(base + 0x1CC1 + offset, 0);
        }
        source.write16(base, address(object.base.next));
        source.write16(base + 2, address(object.base.previous));
        source.write16(
            base + 4,
            0xBC9C + object.base.shape.catalog_index() as u16 * 28,
        );
        for (offset, value) in [
            (12, object.base.position.x),
            (14, object.base.position.y),
            (16, object.base.position.z),
        ] {
            source.write16(base + offset, value as u16);
        }
        source.write8(base + 0x14, object.base.yaw.units());
        source.write8(
            base + 0x24,
            u8::from(object.base.flags.exclude_from_shape_footprint_search) * 4
                | u8::from(object.base.flags.standing_on_surface) * 2,
        );
        source.write8(
            base + 0x31,
            u8::from(object.base.contacts.first_strategy_visit) * 4,
        );
        let delta = object.extension.path_state.motion_delta;
        source.write8(
            base + 0x1CCB,
            object.extension.path_state.animation.shape.packed(),
        );
        let saved = object.extension.path_state.platform_carry.saved_position;
        for (offset, value) in [
            (0x32, object.base.velocity.x),
            (0x34, object.base.velocity.y),
            (0x36, object.base.velocity.z),
            (0x39, saved.x),
            (0x3B, saved.y),
            (0x3D, saved.z),
            (0x1CC1, delta.x),
            (0x1CC3, delta.y),
            (0x1CC5, delta.z),
        ] {
            source.write16(base + offset, value as u16);
        }
        source.write16(
            base + 0x1CE8,
            address(object.extension.surface_contact.supporting_object),
        );
        source.write8(base + 0x1CEA, object.extension.surface_contact.group);
        source.write8(base + 0x1CEB, object.extension.surface_contact.flags);
    }
}

fn compare_motion(
    source: &mut SnesBus,
    objects: &mut ObjectStore,
    owner: ObjectId,
    inputs: sf2_game::surface_motion::SurfaceMotionInputs,
    initialize: bool,
) -> sf2_game::surface_motion::SurfaceMotionResult {
    if initialize {
        seed(source, objects);
    }
    source.write16(0x1B4D, u16::from(inputs.search == SurfaceSearch::Reduced));
    source.write8(0xC4, inputs.strategy_tick);
    source.write16(0x1DAB, inputs.gravity.unwrap_or(0) as u16);
    if initialize {
        source.write16(
            0x700020,
            u16::from(inputs.inherited_tilt.pitch.units()) | 0xAD00,
        );
        source.write16(
            0x700024,
            u16::from(inputs.inherited_tilt.roll.units()) | 0x5200,
        );
    }
    let result = call(
        source,
        0x0DB282,
        &Entry {
            a: u16::from(inputs.gravity.is_none()),
            x: address(Some(owner)),
            dbr: 0x7E,
            p: 0x20,
            ..Default::default()
        },
    );
    assert!(result.returned, "surface movement did not return");
    let actual = sf2_game::surface_motion::advance(objects, owner, inputs).unwrap();
    let actor = objects.get(owner).unwrap();
    let saved = actor.extension.path_state.platform_carry.saved_position;
    let delta = actor.extension.path_state.motion_delta;
    for (offset, value) in [
        (12, actor.base.position.x),
        (14, actor.base.position.y),
        (16, actor.base.position.z),
        (0x32, actor.base.velocity.x),
        (0x34, actor.base.velocity.y),
        (0x36, actor.base.velocity.z),
        (0x39, saved.x),
        (0x3B, saved.y),
        (0x3D, saved.z),
        (0x1CC1, delta.x),
        (0x1CC3, delta.y),
        (0x1CC5, delta.z),
    ] {
        assert_eq!(
            value,
            source.read16(WRAM + u32::from(address(Some(owner))) + offset) as i16,
            "field {offset:X} {inputs:?}"
        );
    }
    let base = WRAM + u32::from(address(Some(owner)));
    assert_eq!(
        address(actor.extension.surface_contact.supporting_object),
        source.read16(base + 0x1CE8)
    );
    assert_eq!(
        actor.extension.surface_contact.group,
        source.read8(base + 0x1CEA)
    );
    assert_eq!(
        actor.extension.surface_contact.flags,
        source.read8(base + 0x1CEB)
    );
    assert_eq!(
        actor.base.flags.standing_on_surface,
        source.read8(base + 0x24) & 2 != 0
    );
    assert_eq!(actual.retried, source.read16(0x1955) & 1 != 0);
    assert_eq!(
        actual.restored_horizontal_position,
        source.read16(0x1955) & 2 != 0
    );
    assert_eq!(actual.obstructed, source.read16(0x1955) & 4 != 0);
    assert_eq!(actual.travel_measure, source.read16(0x70002E));
    assert_eq!(actual.inherited_tilt.pitch.units(), source.read8(0x700020));
    assert_eq!(actual.inherited_tilt.roll.units(), source.read8(0x700024));
    actual
}

#[test]
fn surface_motion_matches_original_airborne_grounded_wrapping_and_dead_zone_cases() {
    use sf2_game::surface_motion::{SurfaceMotionInputs, SurfaceTilt};
    let mut source = original();
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    for seed_value in 0..=u16::MAX {
        let object = objects.get_mut(owner).unwrap();
        object.base.position = Vector3 {
            x: (seed_value ^ 0xF765) as i16,
            y: seed_value as i16,
            z: seed_value.rotate_left(3) as i16,
        };
        object.base.velocity = Vector3 {
            x: seed_value.rotate_right(9) as i16,
            y: seed_value.wrapping_mul(123) as i16,
            z: seed_value.rotate_left(5) as i16,
        };
        object.extension.path_state.platform_carry.saved_position = Vector3 { x: -3, y: 7, z: 11 };
        object.extension.path_state.motion_delta = Vector3 {
            x: seed_value as i16,
            y: seed_value.rotate_left(4) as i16,
            z: seed_value.rotate_right(4) as i16,
        };
        object.base.flags.standing_on_surface = seed_value & 2 != 0;
        compare_motion(
            &mut source,
            &mut objects,
            owner,
            SurfaceMotionInputs {
                search: if seed_value & 1 == 0 {
                    SurfaceSearch::Reduced
                } else {
                    SurfaceSearch::Full
                },
                strategy_tick: seed_value as u8,
                gravity: (seed_value & 4 != 0).then_some(seed_value.rotate_left(8) as i16),
                inherited_tilt: SurfaceTilt {
                    pitch: Angle::from_units(seed_value as u8),
                    roll: Angle::from_units((seed_value >> 8) as u8),
                },
            },
            true,
        );
    }
}

#[test]
fn surface_motion_matches_original_real_collider_geometry_and_previous_support() {
    use sf2_game::surface_motion::{SurfaceMotionInputs, SurfaceTilt};
    let mut source = original();
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let collider = objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    for index in 0..sf2_data::shape_data::SHAPE_DATA.len() {
        let shape = ShapeId::from_catalog_index(index as u16);
        objects.get_mut(collider).unwrap().base.shape = shape;
        objects
            .get_mut(collider)
            .unwrap()
            .base
            .contacts
            .first_strategy_visit = false;
        for case in 0..16_u8 {
            objects
                .get_mut(collider)
                .unwrap()
                .extension
                .path_state
                .animation
                .shape =
                sf2_game::path_appearance::AnimationControl::from_packed(if case & 8 != 0 {
                    0x80 | (case & 7)
                } else {
                    0
                });
            objects.get_mut(collider).unwrap().base.yaw = Angle::from_units(case.wrapping_mul(17));
            objects.get_mut(collider).unwrap().base.velocity.y = i16::from(case) - 7;
            let object = objects.get_mut(owner).unwrap();
            object.base.position = Vector3 {
                x: i16::from(case) * 7 - 50,
                y: i16::from(case) * 20 - 160,
                z: i16::from(case) * 11 - 80,
            };
            object.base.velocity = Vector3 {
                x: i16::from(case) - 8,
                y: i16::from(case) - 6,
                z: 8 - i16::from(case),
            };
            object.extension.path_state.platform_carry.saved_position =
                Vector3 { x: -3, y: 1, z: 5 };
            object.extension.path_state.motion_delta = Vector3 { x: 7, y: -3, z: 5 };
            object.extension.surface_contact = collision_surface::ActorSurfaceContact {
                supporting_object: (case & 1 != 0).then_some(collider),
                group: case / 2,
                flags: 0xA5,
            };
            object.base.flags.standing_on_surface = case & 2 != 0;
            compare_motion(
                &mut source,
                &mut objects,
                owner,
                SurfaceMotionInputs {
                    search: if case & 1 == 0 {
                        SurfaceSearch::Reduced
                    } else {
                        SurfaceSearch::Full
                    },
                    strategy_tick: case / 3,
                    gravity: (case & 4 != 0).then_some(3),
                    inherited_tilt: SurfaceTilt {
                        pitch: Angle::from_units(case),
                        roll: Angle::from_units(case.wrapping_neg()),
                    },
                },
                true,
            );
        }
    }
}

#[test]
fn surface_motion_matches_original_continuous_live_actor_sequences() {
    use sf2_game::surface_motion::{SurfaceMotionInputs, SurfaceTilt};
    let mut source = original();
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let collider = objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let mut grounded = 0;
    objects
        .get_mut(collider)
        .unwrap()
        .base
        .contacts
        .first_strategy_visit = false;
    let mut airborne = 0;
    let mut obstructed = 0;
    for scenario in 0..128_u16 {
        *objects.get_mut(owner).unwrap() =
            Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::Unassigned);
        // Preserve the list relationships; both machines start from this
        // same state and subsequently receive only the same external inputs.
        objects.get_mut(owner).unwrap().base.previous = Some(collider);
        objects.get_mut(collider).unwrap().base.next = Some(owner);
        objects.get_mut(collider).unwrap().base.shape = ShapeId::from_catalog_index(
            scenario * 7 % sf2_data::shape_data::SHAPE_DATA.len() as u16,
        );
        objects.get_mut(collider).unwrap().base.position = Vector3::default();
        objects.get_mut(owner).unwrap().base.position.y = -100;
        let mut tilt = SurfaceTilt {
            pitch: Angle::from_units(scenario as u8),
            roll: Angle::from_units((scenario * 3) as u8),
        };
        seed(&mut source, &objects);
        source.write8(0x700020, tilt.pitch.units());
        source.write8(0x700024, tilt.roll.units());
        for visit in 0..128_u16 {
            let velocity = Vector3 {
                x: ((visit * 7 + scenario) % 17) as i16 - 8,
                y: ((visit + scenario) % 11) as i16 - 3,
                z: ((visit * 3) % 13) as i16 - 6,
            };
            let delta = Vector3 {
                x: (visit % 7) as i16 - 3,
                y: (visit % 3) as i16 - 1,
                z: (visit % 5) as i16 - 2,
            };
            let actor = objects.get_mut(owner).unwrap();
            actor.base.velocity = velocity;
            actor.extension.path_state.motion_delta = delta;
            for (offset, value) in [
                (0x32, velocity.x),
                (0x34, velocity.y),
                (0x36, velocity.z),
                (0x1CC1, delta.x),
                (0x1CC3, delta.y),
                (0x1CC5, delta.z),
            ] {
                source.write16(WRAM + u32::from(OWNER) + offset, value as u16);
            }
            let support = objects.get_mut(collider).unwrap();
            support.base.yaw = Angle::from_units((scenario + visit) as u8);
            support.base.velocity.y = (visit % 5) as i16 - 2;
            source.write8(
                WRAM + u32::from(address(Some(collider))) + 0x14,
                support.base.yaw.units(),
            );
            source.write16(
                WRAM + u32::from(address(Some(collider))) + 0x34,
                support.base.velocity.y as u16,
            );
            let result = compare_motion(
                &mut source,
                &mut objects,
                owner,
                SurfaceMotionInputs {
                    search: if scenario & 1 == 0 {
                        SurfaceSearch::Full
                    } else {
                        SurfaceSearch::Reduced
                    },
                    strategy_tick: visit as u8,
                    gravity: (scenario & 2 != 0).then_some(3),
                    inherited_tilt: tilt,
                },
                false,
            );
            tilt = result.inherited_tilt;
            if objects.get(owner).unwrap().base.flags.standing_on_surface {
                grounded += 1;
            } else {
                airborne += 1;
            }
            obstructed += usize::from(result.obstructed);
        }
    }
    assert!(
        grounded > 0 && airborne > 0 && obstructed > 0,
        "{grounded} grounded, {airborne} airborne, {obstructed} obstructed"
    );
}

#[test]
fn downward_surface_query_matches_whole_original_with_real_shape_headers() {
    let mut source = original();
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let collider = objects
        .allocate(Object::new(
            ObjectKind::Enemy,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let mut selected_boxes = 0;
    let mut selected_compounds = 0;
    for index in 0..sf2_data::shape_data::SHAPE_DATA.len() {
        let shape = ShapeId::from_catalog_index(index as u16);
        objects.get_mut(collider).unwrap().base.shape = shape;
        objects
            .get_mut(collider)
            .unwrap()
            .base
            .contacts
            .first_strategy_visit = false;
        for yaw in [0, 1, 32, 64, 127, 128, 191, 255] {
            objects.get_mut(collider).unwrap().base.yaw = Angle::from_units(yaw);
            for (point, reduced) in [
                (Vector3 { x: 0, y: 0, z: 0 }, false),
                (Vector3 { x: 1, y: -1, z: -1 }, true),
                (
                    Vector3 {
                        x: -30,
                        y: -100,
                        z: 30,
                    },
                    false,
                ),
                (
                    Vector3 {
                        x: 99,
                        y: 50,
                        z: -99,
                    },
                    true,
                ),
            ] {
                objects.get_mut(owner).unwrap().base.position = point;
                seed(&mut source, &objects);
                source.write16(0x1B4D, u16::from(reduced));
                source.write8(0xC4, 0);
                let result = call(
                    &mut source,
                    0x0DAF3A,
                    &Entry {
                        x: OWNER,
                        dbr: 0x7E,
                        p: 0x20,
                        ..Default::default()
                    },
                );
                assert!(
                    result.returned,
                    "query shape {index}, yaw {yaw}, point {point:?}"
                );
                let native = collision_surface::query_object_surface(
                    &objects,
                    owner,
                    0,
                    if reduced {
                        SurfaceSearch::Reduced
                    } else {
                        SurfaceSearch::Full
                    },
                )
                .unwrap();
                assert_eq!(
                    native.height,
                    source.read16(0x08) as i16,
                    "shape {index}, yaw {yaw}, point {point:?}"
                );
                assert_eq!(
                    address(native.contact.supporting_object),
                    source.read16(WRAM + u32::from(OWNER) + 0x1CE8),
                    "shape {index}, yaw {yaw}, point {point:?}"
                );
                assert_eq!(
                    native.contact.group,
                    source.read8(WRAM + u32::from(OWNER) + 0x1CEA)
                );
                assert_eq!(
                    native.contact.flags,
                    source.read8(WRAM + u32::from(OWNER) + 0x1CEB)
                );
                let detail = collision_surface::query_object_surface_geometry(
                    &objects,
                    owner,
                    0,
                    if reduced {
                        SurfaceSearch::Reduced
                    } else {
                        SurfaceSearch::Full
                    },
                )
                .unwrap();
                if native.contact.supporting_object.is_some() {
                    if native.contact.group == 0 {
                        selected_boxes += 1;
                    } else {
                        selected_compounds += 1;
                    }
                }
                assert_eq!(
                    detail.geometry.normal,
                    Vector3 {
                        x: source.read16(0x1963) as i16,
                        y: source.read16(0x1965) as i16,
                        z: source.read16(0x1967) as i16
                    },
                    "normal shape {index}, yaw {yaw}"
                );
                if let collision_surface::SurfaceFootprint::Rectangle {
                    negative_x,
                    negative_z,
                    positive_x,
                    positive_z,
                } = detail.geometry.footprint
                {
                    assert_eq!(
                        [negative_x, negative_z, positive_x, positive_z],
                        [0x1A97, 0x1A9B, 0x1A99, 0x1A9D].map(|offset| source.read16(offset) as i16),
                        "footprint shape {index}, yaw {yaw}, point {point:?}"
                    );
                }
            }
        }
    }
    assert!(selected_boxes > 0 && selected_compounds > 0,
        "eligibility must exercise real colliders: {selected_boxes} ordinary, {selected_compounds} compound");
}

#[test]
fn surface_damping_and_normal_response_match_original_full_word_arithmetic() {
    use sf2_game::surface_motion::{damp_horizontal, normal_response};
    use sf_oracle::gsu::Gsu;
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .unwrap();
    let mut source = Gsu::new(rom);
    fn put(source: &mut Gsu, address: usize, value: i16) {
        source.ram[address..address + 2].copy_from_slice(&value.to_le_bytes());
    }
    fn word(source: &Gsu, address: usize) -> i16 {
        i16::from_le_bytes([source.ram[address], source.ram[address + 1]])
    }
    for value in 0..=u16::MAX {
        for (grounded, entry) in [(false, 0xFAC0), (true, 0xFAC6)] {
            put(&mut source, 0x26, value as i16);
            put(&mut source, 0x2A, value.rotate_left(5) as i16);
            source.run(1, entry);
            assert_eq!(
                damp_horizontal(value as i16, grounded),
                word(&source, 0x26),
                "{value} {grounded}"
            );
            assert_eq!(
                damp_horizontal(value.rotate_left(5) as i16, grounded),
                word(&source, 0x2A)
            );
        }
        let velocity = Vector3 {
            x: value as i16,
            y: value.rotate_left(7) as i16,
            z: value.rotate_right(3) as i16,
        };
        let normal = Vector3 {
            x: value.rotate_left(8) as i16,
            y: value.wrapping_mul(101) as i16,
            z: !value as i16,
        };
        for (offset, value) in [
            (0x26, velocity.x),
            (0x28, velocity.y),
            (0x2A, velocity.z),
            (0xAA, normal.x),
            (0xAC, normal.y),
            (0xAE, normal.z),
            (0xD6, value as i16),
            (0xB0, !value as i16),
        ] {
            put(&mut source, offset, value);
        }
        source.run(1, 0xFAE4);
        assert_eq!(
            normal_response(velocity, normal),
            Vector3 {
                x: word(&source, 0x26),
                y: word(&source, 0x28),
                z: word(&source, 0x2A)
            },
            "{velocity:?} {normal:?}"
        );
    }
}

#[test]
fn surface_probe_rotation_matches_original_all_yaws_and_boundaries() {
    use sf_oracle::gsu::Gsu;
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .unwrap();
    let mut source = Gsu::new(rom);
    for yaw in 1..=u8::MAX {
        for x in [
            -32768_i16, -8193, -8192, -129, -30, -1, 0, 1, 30, 129, 8191, 8192, 32767,
        ] {
            for z in [
                -32768_i16, -8193, -8192, -129, -30, -1, 0, 1, 30, 129, 8191, 8192, 32767,
            ] {
                for (offset, value) in [
                    (0x68, x as u16),
                    (0x2E, z as u16),
                    (0x22, u16::from(yaw).wrapping_neg()),
                ] {
                    source.ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
                }
                source.run(1, 0xFD62);
                let expected = [0x68, 0x2E]
                    .map(|offset| i16::from_le_bytes([source.ram[offset], source.ram[offset + 1]]));
                let (nx, nz) = sf2_game::collision_math::local_probe(Angle::from_units(yaw), x, z);
                assert_eq!([nx, nz], expected, "yaw {yaw}, x {x}, z {z}");
            }
        }
    }
}

#[test]
fn polygon_escape_matches_original_all_catalog_polygons_and_probe_boundaries() {
    use sf2_game::surface_motion::polygon_escape;
    use sf_oracle::gsu::Gsu;
    let rom = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Star Fox 2 (USA, Europe).sfc"),
    )
    .unwrap();
    let mut source = Gsu::new(rom);
    let mut visited = std::collections::BTreeSet::new();
    for index in 0..sf2_data::shape_data::SHAPE_DATA.len() {
        let Some(profile) = sf2_data::collision_data::collision_profile_by_index(index) else {
            continue;
        };
        for group in profile.groups {
            for record in group.variants {
                let Some(polygon) = record.polygon else {
                    continue;
                };
                if !visited.insert((polygon.source_address, polygon.scale)) {
                    continue;
                }
                for x in [-32768_i16, -513, -129, -65, -1, 0, 1, 65, 129, 513, 32767] {
                    for z in [-32768_i16, -513, -129, -65, -1, 0, 1, 65, 129, 513, 32767] {
                        for (offset, value) in [
                            (0x16, polygon.source_address),
                            (0x30, u16::from(polygon.scale)),
                            (0x26, x as u16),
                            (0x2A, z as u16),
                        ] {
                            source.ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
                        }
                        source.run(1, 0xFD8B);
                        let expected = [0x26, 0x2A].map(|offset| {
                            i16::from_le_bytes([source.ram[offset], source.ram[offset + 1]])
                        });
                        assert_eq!(
                            polygon_escape(polygon.vertices, polygon.scale, [x, z]).unwrap(),
                            expected,
                            "polygon {:04X} scale {} at {x},{z}",
                            polygon.source_address,
                            polygon.scale
                        );
                    }
                }
            }
        }
    }
    assert!(visited.len() >= sf2_data::collision_data::COLLISION_POLYGON_COUNT);
}
