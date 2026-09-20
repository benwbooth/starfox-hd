use super::super::path_runtime::PathRuntime;
use super::super::weapon_launch::MuzzleOffset;
use super::super::{Angle, Vector3, OBJECT_CAPACITY};
use super::*;

fn actor() -> Object {
    Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath)
}

#[test]
fn common_formatter_preserves_caller_and_sets_the_complete_fresh_record() {
    for heading in 0..=u8::MAX {
        for paused in [false, true] {
            let mut objects = ObjectStore::new();
            let older_link = objects.allocate(actor()).unwrap();
            let caller = objects.allocate(actor()).unwrap();
            let source = objects.get_mut(caller).unwrap();
            source.base.position = Vector3 {
                x: i16::MAX,
                y: i16::MIN,
                z: -27,
            };
            source.base.pitch = Angle::from_units(heading.wrapping_add(99));
            source.base.yaw = Angle::from_units(heading);
            source.base.roll = Angle::from_units(74);
            source.base.speed = heading.wrapping_add(33);
            source.base.linked_object = Some(older_link);
            source.base.attachment = Some(older_link);
            source.base.flags.collision_disabled = true;
            source.extension.spawn_group = 29;
            source.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
            source.extension.path_state.friend_health_slot = 4;
            let before = source.clone();
            let defaults = ObjectSpawnDefaults {
                run_when_paused: paused,
                group: 177,
            };
            let created = common(&mut objects, caller, LaunchParameters::default(), defaults)
                .unwrap()
                .unwrap();
            let mut expected = Object::new_authored(
                ObjectKind::Projectile,
                ShapeId::EMPTY,
                Behavior::FollowPath,
                defaults,
            );
            expected.base.previous = Some(caller);
            expected.base.next = Some(older_link);
            expected.base.position = before.base.position;
            expected.base.pitch = before.base.pitch;
            expected.base.yaw = before.base.yaw;
            expected.base.child_number = before.base.pitch.units();
            expected.extension.path_state.repeat_counter = heading;
            expected.base.wait_timer = before.base.speed;
            expected.base.linked_object = Some(caller);
            expected.base.hit_points = 1;
            expected.base.attack_power = 1;
            expected.base.flags.weapon_launch_formatted = true;
            expected.base.contacts.weapon_formatted = true;
            expected.base.flags.exclude_from_shape_footprint_search = true;
            expected.base.flags.suppress_death_effects = true;
            expected.base.flags.maximum_draw_distance = true;
            expected.extension.path_state.hold_latched = false;
            expected.extension.path_state.needs_path_initialization = true;
            expected.extension.path_state.animation.shape.initialize(0);
            assert_eq!(objects.get(created).unwrap(), &expected);
            let mut expected_caller = before;
            expected_caller.base.linked_object = Some(created);
            expected_caller.base.next = Some(created);
            assert_eq!(objects.get(caller).unwrap(), &expected_caller);
            assert_eq!(
                objects.get(older_link).unwrap().base.previous,
                Some(created)
            );
            assert_eq!(objects.active_ids(), &[caller, created, older_link]);
            let mut runtime = PathRuntime::default();
            runtime
                .initialize_path_strategy(&mut objects, created)
                .unwrap();
            let initialized = objects.get(created).unwrap();
            assert_eq!(initialized.base.child_number, expected.base.child_number);
            assert_eq!(initialized.extension.path_state.repeat_counter, heading);
            assert_eq!(initialized.base.wait_timer, expected.base.wait_timer);
            assert!(!initialized.extension.path_state.hold_latched);
            assert_eq!(initialized.extension.path_state.friend_health_slot, 0);
        }
    }
}

#[test]
fn rotated_muzzle_target_and_offsets_feed_both_pose_and_path_aliases() {
    for target in [
        None,
        Some(Vector3 {
            x: -30000,
            y: 29000,
            z: 150,
        }),
    ] {
        for heading in [0, 63, 64, 127, 128, 192, 255] {
            let mut objects = ObjectStore::new();
            let mut source = actor();
            source.base.position = Vector3 {
                x: i16::MAX,
                y: i16::MIN,
                z: 200,
            };
            source.base.pitch = Angle::from_units(heading);
            source.base.yaw = Angle::from_units(255 - heading);
            source.base.roll = Angle::from_units(64);
            let parameters = LaunchParameters {
                muzzle: MuzzleOffset {
                    x: -128,
                    y: 127,
                    z: 30,
                },
                pitch_offset: -128,
                yaw_offset: 127,
                target,
            };
            // Arithmetic itself has independently pinned geometry tests;
            // this verifies that creation uses the public formatter intact.
            let pose = format_pose(
                source.base.position,
                Rotation {
                    pitch: source.base.pitch,
                    yaw: source.base.yaw,
                    roll: source.base.roll,
                },
                parameters,
            );
            let caller = objects.allocate(source).unwrap();
            let created = common(
                &mut objects,
                caller,
                parameters,
                ObjectSpawnDefaults::default(),
            )
            .unwrap()
            .unwrap();
            let weapon = objects.get(created).unwrap();
            assert_eq!(weapon.base.position, pose.position);
            assert_eq!(weapon.base.pitch, pose.rotation.pitch);
            assert_eq!(weapon.base.yaw, pose.rotation.yaw);
            assert_eq!(weapon.base.roll, Angle::ZERO);
            assert_eq!(weapon.base.child_number, pose.rotation.pitch.units());
            assert_eq!(
                weapon.extension.path_state.repeat_counter,
                pose.rotation.yaw.units()
            );
        }
    }
}

#[test]
fn player_linked_uses_hit_side_not_selected_slot_and_does_not_create_child_ownership() {
    for side in [HitSide::Primary, HitSide::Secondary] {
        for selected in [PlayerTarget::Primary, PlayerTarget::Secondary] {
            let mut objects = ObjectStore::new();
            let mut source = actor();
            source.base.contacts.hit_side = side;
            source.extension.path_state.conditions.selected_player = selected;
            let caller = objects.allocate(source).unwrap();
            let defaults = ObjectSpawnDefaults {
                group: 40,
                run_when_paused: true,
            };
            let mut expected = objects.clone();
            let expected_id = common(&mut expected, caller, LaunchParameters::default(), defaults)
                .unwrap()
                .unwrap();
            let weapon = expected.get_mut(expected_id).unwrap();
            weapon.extension.spawn_group = 255;
            weapon.base.contacts.exclusion_groups = ExclusionGroups::MUTUALLY_NON_DAMAGING_CLASS;
            weapon.base.contacts.mutually_non_damaging = true;
            weapon.base.attachment = Some(caller);
            weapon.extension.path_state.conditions.selected_player = if side == HitSide::Primary {
                PlayerTarget::Primary
            } else {
                PlayerTarget::Secondary
            };
            let created =
                player_linked(&mut objects, caller, LaunchParameters::default(), defaults)
                    .unwrap()
                    .unwrap();
            assert_eq!(created, expected_id);
            assert_eq!(objects, expected);
            assert_eq!(objects.get(caller).unwrap().base.first_child, None);
            assert_eq!(objects.get(created).unwrap().extension.parent, None);
            assert_eq!(
                objects.get(created).unwrap().base.contacts.hit_side,
                HitSide::Primary
            );
        }
    }
}

#[test]
fn exhausted_pool_and_missing_caller_are_atomic_for_both_services() {
    let mut objects = ObjectStore::new();
    let caller = objects.allocate(actor()).unwrap();
    let missing = objects.allocate(actor()).unwrap();
    objects.remove(missing).unwrap();
    let before = objects.clone();
    for create in [common, player_linked] {
        assert_eq!(
            create(
                &mut objects,
                missing,
                LaunchParameters::default(),
                ObjectSpawnDefaults::default()
            ),
            Err(CreationError::MissingActor(missing))
        );
        assert_eq!(objects, before);
    }
    while objects.len() < OBJECT_CAPACITY {
        objects.allocate(actor()).unwrap();
    }
    let before = objects.clone();
    for create in [common, player_linked] {
        assert_eq!(
            create(
                &mut objects,
                caller,
                LaunchParameters::default(),
                ObjectSpawnDefaults::default()
            ),
            Ok(None)
        );
        assert_eq!(objects, before);
    }
}

#[test]
fn last_slot_creation_retains_scoped_pressure_and_real_head() {
    let mut objects = ObjectStore::new();
    let mut transient = actor();
    transient.base.flags.reclaim_on_pool_pressure = true;
    let tail = objects.allocate(transient.clone()).unwrap();
    let later = objects.allocate(transient.clone()).unwrap();
    let caller = objects.allocate(transient.clone()).unwrap();
    let earlier = objects.allocate(transient).unwrap();
    while objects.len() < OBJECT_CAPACITY - 1 {
        objects.allocate(actor()).unwrap();
    }
    let head = objects.active_ids()[0];
    let created = common(
        &mut objects,
        caller,
        LaunchParameters::default(),
        ObjectSpawnDefaults::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(objects.active_ids()[0], head);
    for id in [caller, later] {
        assert!(objects.get(id).unwrap().base.flags.remove_after_tick);
    }
    for id in [earlier, tail, created] {
        assert!(!objects.get(id).unwrap().base.flags.remove_after_tick);
    }
    assert_eq!(objects.get(caller).unwrap().base.next, Some(created));
    assert_eq!(objects.get(created).unwrap().base.next, Some(later));
    assert_eq!(objects.get(later).unwrap().base.previous, Some(created));
    assert_eq!(objects.len(), OBJECT_CAPACITY);
}
