use super::*;
use crate::{Behavior, Object, ObjectKind, ShapeId, Vector3};

fn actor(z: i16) -> Object {
    let mut value = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Unassigned);
    value.base.position.z = z;
    value.base.flags.general_search_eligible = true;
    class(&mut value, 0x10);
    value
}

fn class(value: &mut Object, bits: u8) {
    value.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(bits);
    value.base.contacts.weapon_formatted = bits & 2 != 0;
    value.base.contacts.first_strategy_visit = bits & 4 != 0;
    value.base.contacts.suppress_attack_damage = bits & 1 != 0;
    value.base.contacts.credits_hit_side = bits & 8 != 0;
    value.base.contacts.mutually_non_damaging = bits & 0x80 != 0;
}

fn window() -> AimWindow {
    AimWindow {
        minimum_distance: 0,
        maximum_distance: 7_000,
        yaw_half_width: 10,
        pitch_half_width: 50,
    }
}

#[test]
fn both_player_windows_have_exact_half_open_byte_edges_for_all_headings() {
    for width in [10, 50] {
        for heading in 0..=u8::MAX {
            for angle in 0..=u8::MAX {
                let delta = heading.wrapping_add(angle) as i8;
                let expected = delta >= -(width as i8) && delta < width as i8;
                assert_eq!(
                    angle_in_window(heading, angle, width),
                    expected,
                    "heading {heading}, angle {angle}, width {width}"
                );
            }
        }
    }
}

#[test]
fn every_class_pair_uses_only_the_source_target_filters() {
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor(0)).unwrap();
    let target = objects.allocate(actor(100)).unwrap();
    for owner_class in 0..=u8::MAX {
        class(objects.get_mut(owner).unwrap(), owner_class);
        for target_class in 0..=u8::MAX {
            class(objects.get_mut(target).unwrap(), target_class);
            let admitted = owner_class & target_class & 0x20 == 0
                && target_class & 0x10 != 0
                && target_class & 0x8A == 0;
            assert_eq!(
                nearest(&objects, owner, window()).unwrap(),
                admitted.then_some(target),
                "owner {owner_class}, candidate {target_class}"
            );
        }
    }
}

#[test]
fn exact_range_comparisons_wrap_and_retain_first_on_ties() {
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor(0)).unwrap();
    let older = objects.allocate(actor(100)).unwrap();
    let first = objects.allocate(actor(100)).unwrap();
    assert_eq!(nearest(&objects, owner, window()).unwrap(), Some(first));
    objects.get_mut(older).unwrap().base.position.z = 90;
    assert_eq!(nearest(&objects, owner, window()).unwrap(), Some(older));
    objects
        .get_mut(older)
        .unwrap()
        .base
        .flags
        .general_search_eligible = false;
    let distance = sf2_xz_angle_distance(0, 100);
    for bound in i16::MIN..=i16::MAX {
        let negative = ((i32::from(distance) - i32::from(bound)) & 0x8000) != 0;
        assert_eq!(
            nearest(
                &objects,
                owner,
                AimWindow {
                    maximum_distance: bound,
                    ..window()
                }
            )
            .unwrap(),
            negative.then_some(first),
            "maximum {bound}"
        );
        assert_eq!(
            nearest(
                &objects,
                owner,
                AimWindow {
                    minimum_distance: bound,
                    ..window()
                }
            )
            .unwrap(),
            (!negative).then_some(first),
            "minimum {bound}"
        );
    }
}

#[test]
fn source_flags_are_distinct_from_health_death_newborn_and_shape_search() {
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor(0)).unwrap();
    let candidate = objects.allocate(actor(100)).unwrap();
    for mask in 0..=255 {
        let value = objects.get_mut(candidate).unwrap();
        value.base.flags.general_search_eligible = mask & 1 != 0;
        value.base.flags.collision_disabled = mask & 2 != 0;
        value.base.contacts.suppress_contacts_next_epoch = mask & 4 != 0;
        value.base.flags.exclude_from_shape_footprint_search = mask & 8 != 0;
        value.base.flags.remove_after_tick = mask & 16 != 0;
        value.base.hit_points = if mask & 32 != 0 { 0 } else { 1 };
        value.base.contacts.first_strategy_visit = mask & 64 != 0;
        value.base.contacts.skip_contacts = mask & 128 != 0;
        assert_eq!(
            nearest(&objects, owner, window()).unwrap(),
            (mask & 7 == 1).then_some(candidate)
        );
    }
}

#[test]
fn target_angles_use_actual_owner_pose_and_wrapping_deltas() {
    let mut objects = ObjectStore::new();
    let owner = objects.allocate(actor(0)).unwrap();
    let target = objects.allocate(actor(0)).unwrap();
    for origin in [
        Vector3::default(),
        Vector3 {
            x: 32_767,
            y: -32_768,
            z: 32_750,
        },
    ] {
        objects.get_mut(owner).unwrap().base.position = origin;
        for delta in [
            Vector3 {
                x: 200,
                y: 300,
                z: 900,
            },
            Vector3 {
                x: -300,
                y: -400,
                z: -800,
            },
        ] {
            objects.get_mut(target).unwrap().base.position = Vector3 {
                x: origin.x.wrapping_add(delta.x),
                y: origin.y.wrapping_add(delta.y),
                z: origin.z.wrapping_add(delta.z),
            };
            let yaw = (sf2_atan16(delta.x, delta.z) >> 8) as u8;
            let pitch = (sf2_atan16(-delta.y, sf2_xz_angle_distance(delta.x, delta.z)) >> 8) as u8;
            for (dyaw, dpitch, accepted) in [
                (-10i8, 0i8, true),
                (9, 0, true),
                (10, 0, false),
                (-11, 0, false),
                (0, -50, true),
                (0, 49, true),
                (0, 50, false),
                (0, -51, false),
            ] {
                let value = objects.get_mut(owner).unwrap();
                value.base.yaw =
                    crate::Angle::from_units(yaw.wrapping_neg().wrapping_add_signed(dyaw));
                value.base.pitch =
                    crate::Angle::from_units(pitch.wrapping_neg().wrapping_add_signed(dpitch));
                assert_eq!(
                    nearest(&objects, owner, window()).unwrap(),
                    accepted.then_some(target)
                );
            }
        }
    }
    objects.remove(owner);
    assert_eq!(
        nearest(&objects, owner, window()),
        Err(TargetSearchError::MissingActor(owner))
    );
}
