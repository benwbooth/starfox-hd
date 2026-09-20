use super::super::weapon_launch::format_pose;
use super::super::{Angle, Behavior, Object, ObjectKind, ShapeId, Vector3};
use super::*;

const PROFILES: [(u8, PathWeapon); 9] = [
    (2, PathWeapon::PlayerOrHostileHeavy),
    (18, PathWeapon::VariantGuided),
    (20, PathWeapon::DifficultyHoming),
    (22, PathWeapon::PrimaryMotionHoming),
    (24, PathWeapon::DoubledMotionHoming),
    (26, PathWeapon::OccupancySpeedSelected),
    (28, PathWeapon::OccupancyDefaultSpeed),
    (30, PathWeapon::OffsetGuided),
    (32, PathWeapon::PrimaryMotionSurfaceLimited),
];

fn actor() -> Object {
    Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath)
}

fn request(weapon: PathWeapon) -> LaunchRequest {
    LaunchRequest {
        weapon,
        parameters: LaunchParameters::default(),
        defaults: ObjectSpawnDefaults {
            group: 173,
            run_when_paused: true,
        },
    }
}

#[test]
fn selector_mapping_is_explicit_and_unreviewed_variants_are_rejected() {
    for selection in 0..=u8::MAX {
        assert_eq!(
            PathWeapon::from_selection(selection),
            PROFILES
                .iter()
                .find(|(value, _)| *value == selection)
                .map(|(_, profile)| *profile)
        );
    }
}

#[test]
fn every_profile_and_primary_mode_installs_exact_path_flags_speed_and_damage() {
    for mode in 0..=u8::MAX {
        for (_, profile) in PROFILES {
            for role in 0..3 {
                let mut objects = ObjectStore::new();
                let primary = objects.allocate(actor()).unwrap();
                let secondary = objects.allocate(actor()).unwrap();
                let enemy = objects.allocate(actor()).unwrap();
                let caller = [primary, secondary, enemy][role];
                let source = objects.get_mut(caller).unwrap();
                source.base.position = Vector3 {
                    x: i16::MIN,
                    y: -150,
                    z: i16::MAX,
                };
                source.base.yaw = Angle::from_units(173);
                source.base.pitch = Angle::from_units(219);
                source.base.speed = 199;
                let source = source.clone();
                let launch_request = request(profile);
                let mut random = RandomState::new([1, 2, 3, mode]);
                let mut expected_random = random;
                let mut counts = HostileLaunchCounts {
                    collision_disabled: 255,
                    aligned_half_plane: 255,
                };
                let mut expected_counts = counts;
                let mut expected = objects.clone();
                let expected_id = weapon_creation::common(
                    &mut expected,
                    caller,
                    launch_request.parameters,
                    launch_request.defaults,
                )
                .unwrap()
                .unwrap();
                let primary_yaw = objects.get(primary).unwrap().base.yaw.units();
                let hostile = profile != PathWeapon::VariantGuided
                    && !(profile == PathWeapon::PlayerOrHostileHeavy && role < 2);
                let actor = expected.get_mut(expected_id).unwrap();
                actor.base.path = Some(match profile {
                    PathWeapon::PlayerOrHostileHeavy if role < 2 => {
                        authored_paths::PRIMARY_MOTION_GROUND_LIMITED
                    }
                    PathWeapon::PlayerOrHostileHeavy => authored_paths::SURFACE_OR_GROUND_LIMITED,
                    PathWeapon::VariantGuided => authored_paths::VARIANT_GUIDED_PROJECTILE,
                    PathWeapon::DifficultyHoming => authored_paths::DIFFICULTY_HOMING_PROJECTILE,
                    PathWeapon::PrimaryMotionHoming => {
                        authored_paths::PRIMARY_MOTION_HOMING_PROJECTILE
                    }
                    PathWeapon::DoubledMotionHoming => {
                        authored_paths::DOUBLED_MOTION_HOMING_PROJECTILE
                    }
                    PathWeapon::OccupancySpeedSelected | PathWeapon::OccupancyDefaultSpeed => {
                        authored_paths::OCCUPANCY_SURFACE_LIMITED
                    }
                    PathWeapon::OffsetGuided => authored_paths::OFFSET_GUIDED_PROJECTILE,
                    PathWeapon::PrimaryMotionSurfaceLimited => {
                        authored_paths::PRIMARY_MOTION_SURFACE_LIMITED
                    }
                });
                if profile == PathWeapon::PlayerOrHostileHeavy {
                    actor.base.hit_points = 120;
                    actor.base.attack_power = 2;
                }
                actor.base.flags.suppress_death_effects = profile != PathWeapon::VariantGuided;
                let speed = match profile {
                    PathWeapon::DifficultyHoming => {
                        Some(if (16..32).contains(&mode) { 40 } else { 70 })
                    }
                    PathWeapon::OccupancySpeedSelected => {
                        Some(if (16..32).contains(&mode) { 40 } else { 60 })
                    }
                    _ => None,
                };
                if let Some(speed) = speed {
                    actor.base.speed = speed;
                    actor.base.velocity = super::super::path_motion::direction_velocity(
                        actor.base.pitch,
                        actor.base.yaw,
                        speed,
                        1,
                    );
                }
                if hostile {
                    actor.base.contacts.exclusion_groups =
                        ExclusionGroups::from_authored_class(0x50);
                    let relative = (i16::from(primary_yaw) + 192 - 173).rem_euclid(256);
                    if relative >= 128 {
                        let draw = expected_random.next_byte();
                        expected_counts.aligned_half_plane = 0;
                        if draw % 4 != 0 {
                            actor.base.flags.collision_disabled = true;
                            expected_counts.collision_disabled = 0;
                        }
                    }
                }
                let created = launch(
                    &mut objects,
                    caller,
                    launch_request,
                    &mut LaunchWorld {
                        primary: Some(primary),
                        secondary: Some(secondary),
                        primary_auxiliary_mode: Some(mode),
                        hostile_counts: Some(&mut counts),
                        random: &mut random,
                    },
                )
                .unwrap()
                .unwrap();
                assert_eq!(created, expected_id);
                assert_eq!(
                    objects, expected,
                    "mode={mode}, profile={profile:?}, role={role}"
                );
                assert_eq!(counts, expected_counts);
                assert_eq!(random, expected_random);
                assert_eq!(
                    objects.get(created).unwrap().base.wait_timer,
                    source.base.speed
                );
                assert!(
                    objects
                        .get(created)
                        .unwrap()
                        .extension
                        .path_state
                        .needs_path_initialization
                );
            }
        }
    }
}

#[test]
fn hostile_gate_uses_formatted_heading_and_primary_yaw_with_exact_shared_draw_count() {
    let mut seen = [false; 256];
    for yaw in 0..=u8::MAX {
        for primary_yaw in [0, 63, 64, 127, 128, 191, 192, 255] {
            for seed in [0, 1, 2, 3] {
                let mut objects = ObjectStore::new();
                let mut primary_object = actor();
                primary_object.base.yaw = Angle::from_units(primary_yaw);
                let primary = objects.allocate(primary_object).unwrap();
                let mut source = actor();
                source.base.yaw = Angle::from_units(yaw);
                let caller = objects.allocate(source).unwrap();
                let mut launch_request = request(PathWeapon::PrimaryMotionHoming);
                launch_request.parameters.yaw_offset = 17;
                let pose = format_pose(
                    Vector3::default(),
                    super::super::Rotation {
                        yaw: Angle::from_units(yaw),
                        ..Default::default()
                    },
                    launch_request.parameters,
                );
                let aligned = (i16::from(primary_yaw) + 192 - i16::from(pose.rotation.yaw.units()))
                    .rem_euclid(256)
                    >= 128;
                let mut random = RandomState::new([seed, 0, 0, yaw]);
                let mut expected_random = random;
                let mut expected_collision = false;
                if aligned {
                    let draw = expected_random.next_byte();
                    seen[usize::from(draw)] = true;
                    expected_collision = draw % 4 != 0;
                }
                let mut counts = HostileLaunchCounts {
                    collision_disabled: 255,
                    aligned_half_plane: 255,
                };
                let created = launch(
                    &mut objects,
                    caller,
                    launch_request,
                    &mut LaunchWorld {
                        primary: Some(primary),
                        secondary: None,
                        primary_auxiliary_mode: None,
                        hostile_counts: Some(&mut counts),
                        random: &mut random,
                    },
                )
                .unwrap()
                .unwrap();
                assert_eq!(
                    objects.get(created).unwrap().base.flags.collision_disabled,
                    expected_collision
                );
                assert_eq!(counts.aligned_half_plane, if aligned { 0 } else { 255 });
                assert_eq!(
                    counts.collision_disabled,
                    if expected_collision { 0 } else { 255 }
                );
                assert_eq!(random, expected_random);
            }
        }
    }
    assert!(seen.into_iter().all(|value| value));
}

#[test]
fn missing_inputs_fault_atomically_but_nonhostile_and_full_pool_need_no_unread_inputs() {
    for (_, profile) in PROFILES {
        let mut objects = ObjectStore::new();
        let caller = objects.allocate(actor()).unwrap();
        let primary = objects.allocate(actor()).unwrap();
        let mut random = RandomState::new([3, 4, 5, 6]);
        let before_random = random;
        if profile != PathWeapon::VariantGuided {
            let before = objects.clone();
            let outcome = launch(
                &mut objects,
                caller,
                request(profile),
                &mut LaunchWorld {
                    primary: None,
                    secondary: None,
                    primary_auxiliary_mode: None,
                    hostile_counts: None,
                    random: &mut random,
                },
            );
            assert_eq!(outcome, Err(LaunchError::MissingPrimary));
            assert_eq!(objects, before);
            assert_eq!(random, before_random);
        }
        while objects.len() < OBJECT_CAPACITY {
            objects.allocate(actor()).unwrap();
        }
        let before = objects.clone();
        assert_eq!(
            launch(
                &mut objects,
                caller,
                request(profile),
                &mut LaunchWorld {
                    primary: None,
                    secondary: None,
                    primary_auxiliary_mode: None,
                    hostile_counts: None,
                    random: &mut random,
                }
            ),
            Ok(None)
        );
        assert_eq!(objects, before);
        assert_eq!(random, before_random);
        objects.remove(primary).unwrap();
        if profile == PathWeapon::VariantGuided || profile == PathWeapon::PlayerOrHostileHeavy {
            let created = launch(
                &mut objects,
                caller,
                request(profile),
                &mut LaunchWorld {
                    primary: Some(caller),
                    secondary: None,
                    primary_auxiliary_mode: None,
                    hostile_counts: None,
                    random: &mut random,
                },
            )
            .unwrap()
            .unwrap();
            assert!(!objects.get(created).unwrap().base.flags.collision_disabled);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn each_required_world_input_is_validated_before_allocation_or_random_consumption() {
    for missing in 0..4 {
        let mut objects = ObjectStore::new();
        let caller = objects.allocate(actor()).unwrap();
        let primary = objects.allocate(actor()).unwrap();
        let mut counts = HostileLaunchCounts::default();
        let mut random = RandomState::new([9, 8, 7, 6]);
        let before_random = random;
        let profile = if missing == 0 {
            PathWeapon::PlayerOrHostileHeavy
        } else {
            PathWeapon::DifficultyHoming
        };
        if missing == 3 {
            objects.remove(primary).unwrap();
        }
        let expected_objects = objects.clone();
        let outcome = launch(
            &mut objects,
            caller,
            request(profile),
            &mut LaunchWorld {
                primary: Some(primary),
                secondary: None,
                primary_auxiliary_mode: if missing == 2 { None } else { Some(0x10) },
                hostile_counts: if missing == 1 {
                    None
                } else {
                    Some(&mut counts)
                },
                random: &mut random,
            },
        );
        assert_eq!(
            outcome,
            Err(match missing {
                0 => LaunchError::MissingSecondary,
                1 => LaunchError::MissingHostileCounts,
                2 => LaunchError::MissingPrimaryAuxiliaryMode,
                _ => LaunchError::Creation(CreationError::MissingActor(primary)),
            })
        );
        assert_eq!(objects, expected_objects);
        assert_eq!(random, before_random);
        assert_eq!(counts, HostileLaunchCounts::default());
    }
}
