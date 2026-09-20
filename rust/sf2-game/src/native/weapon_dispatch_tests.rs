use super::super::weapon_launch::format_pose;
use super::super::{Angle, Behavior, Object, ObjectKind, ShapeId, Vector3};
use super::*;

const PROFILES: [(u8, PathWeapon); 12] = [
    (2, PathWeapon::PlayerOrHostileHeavy),
    (12, PathWeapon::PlayerChargedMesh),
    (14, PathWeapon::PlayerChargedMesh),
    (16, PathWeapon::PlayerChargedMesh),
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
            match selection {
                4 => Some(PathWeapon::Rapid(
                    super::super::weapon_rapid::RapidWeapon::Alternate
                )),
                6 => Some(PathWeapon::Rapid(
                    super::super::weapon_rapid::RapidWeapon::Basic
                )),
                8 => Some(PathWeapon::Rapid(
                    super::super::weapon_rapid::RapidWeapon::Upgraded
                )),
                10 => Some(PathWeapon::Rapid(
                    super::super::weapon_rapid::RapidWeapon::Maximum
                )),
                _ => PROFILES
                    .iter()
                    .find(|(value, _)| *value == selection)
                    .map(|(_, profile)| *profile),
            }
        );
    }
}

#[test]
fn charged_mesh_uses_published_pitch_caller_roll_and_retained_reflection_shape_without_hostile_inputs(
) {
    use super::super::hit_response::HitSide;
    use super::super::path_control::PlayerTarget;
    for pitch in 0..=u8::MAX {
        for roll in [0, 127, 128, 255] {
            for secondary in [false, true] {
                let mut objects = ObjectStore::new();
                let mut resources = ProgramResources::default();
                let mut source = actor();
                source.base.pitch = Angle::from_units(pitch.wrapping_add(93));
                source.base.yaw = Angle::from_units(177);
                source.base.roll = Angle::from_units(roll);
                source.base.speed = 217;
                source.base.position = Vector3 {
                    x: 32767,
                    y: -32768,
                    z: 32700,
                };
                source.base.contacts.hit_side = if secondary {
                    HitSide::Secondary
                } else {
                    HitSide::Primary
                };
                source.extension.path_state.conditions.selected_player = if secondary {
                    PlayerTarget::Primary
                } else {
                    PlayerTarget::Secondary
                };
                let caller = objects.allocate(source.clone()).unwrap();
                let mut request = request(PathWeapon::PlayerChargedMesh);
                request.parameters.pitch_offset = -17;
                request.parameters.yaw_offset = 31;
                request.parameters.muzzle =
                    super::super::weapon_launch::MuzzleOffset { x: 8, y: -4, z: 12 };
                let mut expected = objects.clone();
                let mut expected_resources = resources.clone();
                let created = weapon_creation::player_linked(
                    &mut expected,
                    caller,
                    request.parameters,
                    request.defaults,
                )
                .unwrap()
                .unwrap();
                let result = expected.get_mut(created).unwrap();
                let original_child_number = result.base.child_number;
                result.base.path = Some(authored_paths::AIMED_IMPACT_PROJECTILE);
                result.base.shape = ShapeId::PLAYER_CHARGED_LASER_LAUNCH;
                result.base.pitch = Angle::from_units(pitch);
                result.base.roll = Angle::from_units(roll);
                result.base.hit_points = 120;
                result.base.attack_power = 10;
                result
                    .extension
                    .auxiliary
                    .set(
                        &mut expected_resources,
                        created,
                        AuxiliaryRecord::ReflectionShape(ShapeId::PLAYER_CHARGED_LASER_LAUNCH),
                    )
                    .unwrap();
                let mut random = RandomState::new([1, pitch, roll, 17]);
                let before_random = random;
                assert_eq!(
                    launch(
                        &mut objects,
                        &mut resources,
                        caller,
                        request,
                        &mut LaunchWorld {
                            caller_inputs: None,
                            fallback: None,
                            published_pitch: Some(Angle::from_units(pitch)),
                            primary: None,
                            secondary: None,
                            primary_auxiliary_mode: None,
                            hostile_counts: None,
                            random: &mut random,
                        }
                    )
                    .unwrap(),
                    Some(created)
                );
                assert_eq!(objects, expected);
                assert_eq!(resources, expected_resources);
                assert_eq!(random, before_random);
                let result = objects.get_mut(created).unwrap();
                // The formatter's aliased pitch byte predates the later
                // published-pitch replacement and must not follow it.
                assert_eq!(result.base.child_number, original_child_number);
                assert_eq!(result.base.wait_timer, 217);
                assert_eq!(result.base.velocity, Vector3::default());
                assert_eq!(result.base.attachment, Some(caller));
                assert_eq!(
                    result.extension.path_state.conditions.selected_player,
                    if secondary {
                        PlayerTarget::Secondary
                    } else {
                        PlayerTarget::Primary
                    }
                );
                result.base.shape = ShapeId::PLAYER_CHARGED_LASER_ACTIVE;
                assert_eq!(
                    result
                        .extension
                        .auxiliary
                        .reflection_shape(&resources, created)
                        .unwrap(),
                    Some(ShapeId::PLAYER_CHARGED_LASER_LAUNCH)
                );
            }
        }
    }
}

#[test]
fn charged_mesh_requires_a_pitch_snapshot_unless_allocation_already_fails() {
    let mut objects = ObjectStore::new();
    let mut resources = ProgramResources::default();
    let caller = objects.allocate(actor()).unwrap();
    let mut random = RandomState::default();
    let before_random = random;
    let before = objects.clone();
    let request = request(PathWeapon::PlayerChargedMesh);
    assert_eq!(
        launch(
            &mut objects,
            &mut resources,
            caller,
            request,
            &mut LaunchWorld {
                caller_inputs: None,
                fallback: None,
                published_pitch: None,
                primary: None,
                secondary: None,
                primary_auxiliary_mode: None,
                hostile_counts: None,
                random: &mut random,
            }
        ),
        Err(LaunchError::MissingPublishedPitch)
    );
    assert_eq!(objects, before);
    while objects.len() < OBJECT_CAPACITY {
        objects.allocate(actor()).unwrap();
    }
    let before = objects.clone();
    assert_eq!(
        launch(
            &mut objects,
            &mut resources,
            caller,
            request,
            &mut LaunchWorld {
                caller_inputs: None,
                fallback: None,
                published_pitch: None,
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
}

#[test]
fn charged_shape_allocation_failure_retains_created_actor_before_combat_overrides() {
    use super::super::program_resources::{AllocationFailure, PROGRAM_CAPACITY};
    let mut objects = ObjectStore::new();
    let caller = objects.allocate(actor()).unwrap();
    objects.get_mut(caller).unwrap().base.roll = Angle::from_units(177);
    let mut resources = ProgramResources::default();
    resources
        .allocate_shared(
            PROGRAM_CAPACITY - 8 - 2,
            ProgramData::PathStack(Default::default()),
        )
        .unwrap();
    let before_resources = resources.clone();
    let mut expected = objects.clone();
    let request = request(PathWeapon::PlayerChargedMesh);
    let created =
        weapon_creation::player_linked(&mut expected, caller, request.parameters, request.defaults)
            .unwrap()
            .unwrap();
    let expected_shot = expected.get_mut(created).unwrap();
    expected_shot.base.path = Some(authored_paths::AIMED_IMPACT_PROJECTILE);
    expected_shot.base.shape = ShapeId::PLAYER_CHARGED_LASER_LAUNCH;
    expected_shot.base.roll = Angle::from_units(177);
    expected_shot.base.pitch = Angle::from_units(103);
    let mut random = RandomState::default();
    let before_random = random;
    assert_eq!(
        launch(
            &mut objects,
            &mut resources,
            caller,
            request,
            &mut LaunchWorld {
                caller_inputs: None,
                fallback: None,
                published_pitch: Some(Angle::from_units(103)),
                primary: None,
                secondary: None,
                primary_auxiliary_mode: None,
                hostile_counts: None,
                random: &mut random,
            }
        ),
        Err(LaunchError::Auxiliary(AuxiliaryError::Allocation(
            AllocationFailure::NoContiguousFit
        )))
    );
    assert_eq!(objects, expected);
    assert_eq!(resources, before_resources);
    assert_eq!(random, before_random);
    let shot = objects.get(created).unwrap();
    assert_eq!((shot.base.hit_points, shot.base.attack_power), (1, 1));
    assert_eq!(
        shot.extension
            .auxiliary
            .reflection_shape(&resources, created),
        Ok(None)
    );
}

#[test]
fn every_profile_and_primary_mode_installs_exact_path_flags_speed_and_damage() {
    for mode in 0..=u8::MAX {
        for (_, profile) in PROFILES {
            for role in 0..3 {
                let mut objects = ObjectStore::new();
                let mut resources = ProgramResources::default();
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
                let mut expected_resources = resources.clone();
                let creation = if profile == PathWeapon::PlayerChargedMesh {
                    weapon_creation::player_linked
                } else {
                    weapon_creation::common
                };
                let expected_id = creation(
                    &mut expected,
                    caller,
                    launch_request.parameters,
                    launch_request.defaults,
                )
                .unwrap()
                .unwrap();
                let primary_yaw = objects.get(primary).unwrap().base.yaw.units();
                let hostile = profile != PathWeapon::VariantGuided
                    && profile != PathWeapon::PlayerChargedMesh
                    && !(profile == PathWeapon::PlayerOrHostileHeavy && role < 2);
                let actor = expected.get_mut(expected_id).unwrap();
                actor.base.path = Some(match profile {
                    PathWeapon::Rapid(weapon) => weapon.paths()[0],
                    PathWeapon::PlayerChargedMesh => authored_paths::AIMED_IMPACT_PROJECTILE,
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
                if profile == PathWeapon::PlayerChargedMesh {
                    actor.base.shape = ShapeId::PLAYER_CHARGED_LASER_LAUNCH;
                    actor.base.pitch = Angle::from_units(197);
                    actor.base.roll = source.base.roll;
                    actor
                        .extension
                        .auxiliary
                        .set(
                            &mut expected_resources,
                            expected_id,
                            AuxiliaryRecord::ReflectionShape(ShapeId::PLAYER_CHARGED_LASER_LAUNCH),
                        )
                        .unwrap();
                    actor.base.hit_points = 120;
                    actor.base.attack_power = 10;
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
                    &mut resources,
                    caller,
                    launch_request,
                    &mut LaunchWorld {
                        caller_inputs: None,
                        fallback: None,
                        published_pitch: Some(Angle::from_units(197)),
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
                let mut resources = ProgramResources::default();
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
                    &mut resources,
                    caller,
                    launch_request,
                    &mut LaunchWorld {
                        caller_inputs: None,
                        fallback: None,
                        published_pitch: Some(Angle::from_units(197)),
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
        let mut resources = ProgramResources::default();
        let caller = objects.allocate(actor()).unwrap();
        let primary = objects.allocate(actor()).unwrap();
        let mut random = RandomState::new([3, 4, 5, 6]);
        let before_random = random;
        if profile != PathWeapon::VariantGuided && profile != PathWeapon::PlayerChargedMesh {
            let before = objects.clone();
            let outcome = launch(
                &mut objects,
                &mut resources,
                caller,
                request(profile),
                &mut LaunchWorld {
                    caller_inputs: None,
                    fallback: None,
                    published_pitch: Some(Angle::from_units(197)),
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
                &mut resources,
                caller,
                request(profile),
                &mut LaunchWorld {
                    caller_inputs: None,
                    fallback: None,
                    published_pitch: Some(Angle::from_units(197)),
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
                &mut resources,
                caller,
                request(profile),
                &mut LaunchWorld {
                    caller_inputs: None,
                    fallback: None,
                    published_pitch: Some(Angle::from_units(197)),
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
        let mut resources = ProgramResources::default();
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
            &mut resources,
            caller,
            request(profile),
            &mut LaunchWorld {
                caller_inputs: None,
                fallback: None,
                published_pitch: Some(Angle::from_units(197)),
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
