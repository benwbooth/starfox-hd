use super::super::render::MaterialSetId;
use super::super::weapon_launch::{HostileLaunchCounts, LaunchParameters};
use super::super::{authored_paths, Angle, Behavior, Object, ObjectKind, ShapeId, Vector3};
use super::*;

fn actor() -> Object {
    Object::new(
        ObjectKind::Projectile,
        ShapeId::from_catalog_index(7),
        Behavior::FollowPath,
    )
}

#[test]
fn absent_reflection_gate_and_empty_contact_list_do_not_read_unreached_inputs() {
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor()).unwrap();
    let mut random = RandomState::default();
    let original = objects.clone();
    let before_random = random;
    let contacts = ContactStore::default();
    let mut world = ReflectionWorld {
        contacts: None,
        rules: None,
        weapons: None,
        defaults: None,
        primary: None,
        secondary: None,
        random: &mut random,
    };
    assert_eq!(reflect_contacts(&mut objects, owner, &mut world), Ok(()));
    assert_eq!(objects, original);
    objects.get_mut(owner).unwrap().base.contacts.skip_contacts = true;
    let original = objects.clone();
    assert_eq!(
        reflect_contacts(&mut objects, owner, &mut world),
        Err(ReflectionError::MissingContacts)
    );
    assert_eq!(objects, original);
    world.contacts = Some(&contacts);
    assert_eq!(reflect_contacts(&mut objects, owner, &mut world), Ok(()));
    assert_eq!(objects, original);
    assert_eq!(random, before_random);
}

#[test]
fn missing_live_inputs_fail_before_disabling_shot_or_consuming_randomness() {
    for case in 0..8 {
        let mut objects = ObjectStore::new();
        let primary = objects.allocate(actor()).unwrap();
        let secondary = objects.allocate(actor()).unwrap();
        let armor = objects.allocate(actor()).unwrap();
        let owner = if case == 2 { primary } else { armor };
        objects.get_mut(owner).unwrap().base.contacts.skip_contacts = true;
        let mut shot = actor();
        shot.base.contacts.credits_hit_side = true;
        let incoming = objects.allocate(shot).unwrap();
        let mut contacts = ContactStore::default();
        contacts.record_pair(owner, incoming, [None, None]).unwrap();
        if case == 7 {
            while objects.len() < OBJECT_CAPACITY {
                objects.allocate(actor()).unwrap();
            }
        }
        let original = objects.clone();
        let mut random = RandomState::default();
        let original_random = random;
        let mut weapons = WeaponState::default();
        let original_weapons = weapons;
        let mut world = ReflectionWorld {
            contacts: Some(&contacts),
            rules: if case == 0 {
                None
            } else {
                Some(ReflectionRules {
                    owner: if case == 1 { secondary } else { owner },
                    process_all: false,
                    player_scatter: None,
                })
            },
            weapons: if case == 4 { None } else { Some(&mut weapons) },
            defaults: if case == 3 {
                None
            } else {
                Some(ObjectSpawnDefaults::default())
            },
            primary: if case == 5 { None } else { Some(primary) },
            secondary: if case == 6 { None } else { Some(secondary) },
            random: &mut random,
        };
        let error = match case {
            0 => ReflectionError::MissingRules,
            1 => ReflectionError::WrongRulesOwner {
                expected: owner,
                supplied: secondary,
            },
            2 => ReflectionError::MissingPlayerScatter,
            3 => ReflectionError::MissingSpawnDefaults,
            4 => ReflectionError::MissingWeaponState,
            5 => ReflectionError::Launch(LaunchError::MissingPrimary),
            6 => ReflectionError::Launch(LaunchError::MissingSecondary),
            _ => ReflectionError::MissingFallback,
        };
        assert_eq!(
            reflect_contacts(&mut objects, owner, &mut world),
            Err(error)
        );
        assert_eq!(objects, original);
        assert_eq!(random, original_random);
        assert_eq!(weapons, original_weapons);
    }
}

#[test]
fn reflection_order_scatter_sprite_and_retained_shape_follow_live_contact_list() {
    for caller in 0..3 {
        for scatter in [false, true] {
            for process_all in [false, true] {
                for seed in 0..=31 {
                    let mut objects = ObjectStore::new();
                    let primary = objects.allocate(actor()).unwrap();
                    let secondary = objects.allocate(actor()).unwrap();
                    let armor = objects.allocate(actor()).unwrap();
                    let owner = [primary, secondary, armor][caller];
                    let reflector = objects.get_mut(owner).unwrap();
                    reflector.base.contacts.skip_contacts = true;
                    reflector.base.pitch = Angle::from_units(73);
                    reflector.base.yaw = Angle::from_units(119);
                    reflector.base.position = Vector3 {
                        x: -301,
                        y: 109,
                        z: -501,
                    };
                    reflector.base.speed = 37;
                    let owner_rotation = (reflector.base.pitch.units(), reflector.base.yaw.units());
                    let primary_yaw = objects.get(primary).unwrap().base.yaw.units();
                    let mut contacts = ContactStore::default();
                    let mut incoming_ids = Vec::new();
                    for index in 0..4_u8 {
                        let mut incoming = actor();
                        incoming.base.position = Vector3 {
                            x: 100 + i16::from(index),
                            y: i16::MIN,
                            z: i16::MAX,
                        };
                        incoming.base.pitch = Angle::from_units(index.wrapping_mul(71));
                        incoming.base.yaw = Angle::from_units(index.wrapping_mul(61));
                        incoming.base.roll = Angle::from_units(197);
                        incoming.base.speed = 97;
                        incoming.base.velocity = Vector3 {
                            x: 17,
                            y: -29,
                            z: 31,
                        };
                        incoming.base.contacts.credits_hit_side = index != 0;
                        incoming.base.flags.scaled_sprite = index == 2;
                        incoming.extension.depth_offset = 0xBCA7;
                        incoming.extension.texture_scroll_x = 199;
                        incoming.extension.material_set =
                            Some(MaterialSetId::from_catalog_token(3));
                        if index != 3 {
                            incoming.extension.reflection_shape =
                                Some(ShapeId::from_catalog_index(20 + u16::from(index)));
                        }
                        let id = objects.allocate(incoming).unwrap();
                        incoming_ids.push(id);
                        contacts.record_pair(owner, id, [None, None]).unwrap();
                    }
                    let originals: Vec<_> = incoming_ids
                        .iter()
                        .map(|&id| objects.get(id).unwrap().clone())
                        .collect();
                    // New entries follow the retained head: 0,3,2,1. The
                    // ineligible head is skipped without random consumption.
                    let visited: &[usize] = if process_all { &[3, 2, 1] } else { &[3] };
                    let mut random = RandomState::new([seed, 29, 73, 191]);
                    let mut expected_random = random;
                    let mut expected_counts = HostileLaunchCounts {
                        collision_disabled: 254,
                        aligned_half_plane: 255,
                    };
                    let mut expected_parameters = LaunchParameters::default();
                    let mut expectations = Vec::new();
                    for &index in visited {
                        let incoming = &originals[index];
                        let mut pitch = incoming.base.pitch.units().wrapping_neg();
                        let mut yaw = incoming
                            .base
                            .yaw
                            .units()
                            .wrapping_add(128)
                            .wrapping_sub(owner_rotation.1);
                        if caller == 2 || scatter {
                            pitch = pitch
                                .wrapping_add(expected_random.next_byte() & 63)
                                .wrapping_sub(32);
                            yaw = yaw
                                .wrapping_add(expected_random.next_byte() & 63)
                                .wrapping_sub(32);
                        }
                        expected_parameters = LaunchParameters {
                            pitch_offset: pitch as i8,
                            yaw_offset: yaw as i8,
                            ..Default::default()
                        };
                        let pitch = pitch.wrapping_add(owner_rotation.0);
                        let yaw = yaw.wrapping_add(owner_rotation.1);
                        let mut disabled = false;
                        if caller == 2 && primary_yaw.wrapping_add(192).wrapping_sub(yaw) >= 128 {
                            disabled = expected_random.next_byte() & 3 != 0;
                            expected_counts.aligned_half_plane =
                                expected_counts.aligned_half_plane.wrapping_add(1);
                            if disabled {
                                expected_counts.collision_disabled =
                                    expected_counts.collision_disabled.wrapping_add(1);
                            }
                        }
                        expectations.push((index, pitch, yaw, disabled));
                    }
                    let before_count = objects.len();
                    let mut weapons = WeaponState {
                        hostile_counts: HostileLaunchCounts {
                            collision_disabled: 254,
                            aligned_half_plane: 255,
                        },
                        ..Default::default()
                    };
                    reflect_contacts(
                        &mut objects,
                        owner,
                        &mut ReflectionWorld {
                            contacts: Some(&contacts),
                            rules: Some(ReflectionRules {
                                owner,
                                process_all,
                                player_scatter: if caller == 2 { None } else { Some(scatter) },
                            }),
                            weapons: Some(&mut weapons),
                            defaults: Some(ObjectSpawnDefaults::default()),
                            primary: Some(primary),
                            secondary: Some(secondary),
                            random: &mut random,
                        },
                    )
                    .unwrap();
                    assert_eq!(objects.len(), before_count + visited.len());
                    assert_eq!(random, expected_random);
                    assert_eq!(weapons.hostile_counts, expected_counts);
                    assert_eq!(weapons.parameters, expected_parameters);
                    for (index, pitch, yaw, disabled) in expectations {
                        let original = &originals[index];
                        let reflected = objects
                            .active_ids()
                            .iter()
                            .copied()
                            .find(|id| {
                                !incoming_ids.contains(id)
                                    && *id != primary
                                    && *id != secondary
                                    && *id != armor
                                    && objects.get(*id).unwrap().base.position
                                        == original.base.position
                            })
                            .unwrap();
                        let shot = objects.get(reflected).unwrap();
                        assert_eq!(
                            (
                                shot.base.pitch.units(),
                                shot.base.yaw.units(),
                                shot.base.roll.units()
                            ),
                            (pitch, yaw, 0)
                        );
                        assert_eq!(
                            shot.base.shape,
                            original
                                .extension
                                .reflection_shape
                                .unwrap_or(original.base.shape)
                        );
                        assert_eq!(shot.extension.material_set, original.extension.material_set);
                        assert_eq!(
                            shot.base.path,
                            Some(if caller == 2 {
                                authored_paths::SURFACE_OR_GROUND_LIMITED
                            } else {
                                authored_paths::PRIMARY_MOTION_GROUND_LIMITED
                            })
                        );
                        assert_eq!((shot.base.hit_points, shot.base.attack_power), (120, 2));
                        assert_eq!(shot.base.flags.collision_disabled, disabled);
                        assert_eq!(shot.base.flags.scaled_sprite, index == 2);
                        assert_eq!(shot.base.wait_timer, 37);
                        assert_eq!(shot.base.linked_object, Some(owner));
                        if index == 2 {
                            assert_eq!(shot.extension.depth_offset, 0xA7);
                            assert_eq!(shot.extension.texture_scroll_x, 199);
                        }
                    }
                    for (index, &id) in incoming_ids.iter().enumerate() {
                        let actual = objects.get(id).unwrap();
                        let mut expected = originals[index].clone();
                        expected.base.next = actual.base.next;
                        expected.base.previous = actual.base.previous;
                        if visited.contains(&index) {
                            expected.base.flags.collision_disabled = true;
                            if index == 2 {
                                expected.base.speed = 60;
                            }
                        }
                        assert_eq!(actual, &expected);
                    }
                }
            }
        }
    }
}

#[test]
fn full_pool_still_disables_incoming_and_formats_fallback_without_hostile_draw() {
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor()).unwrap();
    let fallback = objects.allocate(actor()).unwrap();
    let incoming = objects.allocate(actor()).unwrap();
    objects.get_mut(owner).unwrap().base.contacts.skip_contacts = true;
    let shot = objects.get_mut(incoming).unwrap();
    shot.base.contacts.credits_hit_side = true;
    shot.base.flags.scaled_sprite = true;
    shot.base.position = Vector3 {
        x: -301,
        y: i16::MAX,
        z: i16::MIN,
    };
    shot.extension.depth_offset = 0x9FA7;
    shot.extension.texture_scroll_x = 137;
    shot.extension.material_set = Some(MaterialSetId::from_catalog_token(7));
    let original = shot.clone();
    objects.get_mut(fallback).unwrap().extension.depth_offset = 0xCB19;
    while objects.len() < OBJECT_CAPACITY {
        objects.allocate(actor()).unwrap();
    }
    let mut contacts = ContactStore::default();
    contacts.record_pair(owner, incoming, [None, None]).unwrap();
    let mut random = RandomState::default();
    let mut expected_random = random;
    expected_random.next_byte();
    expected_random.next_byte();
    let mut weapons = WeaponState {
        fallback: Some(fallback),
        ..Default::default()
    };
    let mut expected_fallback = objects.get(fallback).unwrap().clone();
    expected_fallback.base.shape = original.base.shape;
    expected_fallback.base.position = original.base.position;
    expected_fallback.extension.material_set = original.extension.material_set;
    expected_fallback.base.flags.scaled_sprite = true;
    expected_fallback.extension.depth_offset = 0xCBA7;
    expected_fallback.extension.texture_scroll_x = 137;
    reflect_contacts(
        &mut objects,
        owner,
        &mut ReflectionWorld {
            contacts: Some(&contacts),
            rules: Some(ReflectionRules {
                owner,
                process_all: false,
                player_scatter: None,
            }),
            weapons: Some(&mut weapons),
            defaults: Some(ObjectSpawnDefaults::default()),
            primary: None,
            secondary: None,
            random: &mut random,
        },
    )
    .unwrap();
    assert_eq!(objects.len(), OBJECT_CAPACITY);
    assert_eq!(objects.get(fallback).unwrap(), &expected_fallback);
    assert_eq!(objects.get(incoming).unwrap().base.speed, 60);
    assert!(objects.get(incoming).unwrap().base.flags.collision_disabled);
    assert_eq!(weapons.hostile_counts, HostileLaunchCounts::default());
    assert_eq!(random, expected_random);
}
