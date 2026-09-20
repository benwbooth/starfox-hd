//! Immediate weapon dispatch and reserved-actor failure behavior. All path
//! entries are statically lowered; creation never executes the projectile.
use super::super::collision_pass::ExclusionGroups;
use super::super::weapon_dispatch::{self, LaunchRequest, LaunchWorld, PathWeapon, WeaponState};
use super::super::weapon_launch::{LaunchParameters, MuzzleOffset};
use super::super::{
    authored_paths, Angle, Behavior, ObjectKind, ObjectSpawnDefaults, PathId, ShapeId, Vector3,
    OBJECT_CAPACITY,
};
use super::tests::{setup, world};
use super::*;

fn actor() -> Object {
    Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath)
}

fn fire_catalog() -> (PathCatalog, PathCursor, PathCursor) {
    let mut catalog = authored_paths::catalog();
    let path = PathId::from_catalog_index(catalog.paths.len() as u16);
    let entry = PathCursor {
        path,
        command_index: 0,
    };
    let next = PathCursor {
        path,
        command_index: 1,
    };
    let finish = PathCursor {
        path,
        command_index: 2,
    };
    catalog.paths.push(vec![
        Statement::FireWeapon { next },
        Statement::Control(ControlCommand::WaitOne { next: finish }),
        Statement::Control(ControlCommand::End),
    ]);
    (catalog, entry, finish)
}

fn prior_state(fallback: Option<ObjectId>) -> WeaponState {
    WeaponState {
        published_pitch: Some(Angle::from_units(197)),
        fallback,
        parameters: LaunchParameters {
            muzzle: MuzzleOffset {
                x: 127,
                y: -128,
                z: 76,
            },
            pitch_offset: -33,
            yaw_offset: 44,
            target: Some(Vector3 {
                x: 21000,
                y: -12345,
                z: -257,
            }),
        },
        hostile_counts: super::super::weapon_launch::HostileLaunchCounts {
            collision_disabled: 253,
            aligned_half_plane: 254,
        },
    }
}

fn rapid_inputs(owner: ObjectId) -> super::super::weapon_rapid::CallerWeaponInputs {
    super::super::weapon_rapid::CallerWeaponInputs {
        owner,
        active_shots: Some(super::super::path_shots::ActiveShots::from_count(7)),
        weapon_level: Some(1),
        roll_step: Some(Angle::from_units(239)),
        retained_aim: Some(Vector3 {
            x: -23000,
            y: 900,
            z: 500,
        }),
    }
}

#[test]
fn fire_resets_shared_pose_inputs_and_publishes_spawn_without_consuming_ifnot_or_running_it() {
    let (catalog, entry, finish) = fire_catalog();
    for selection in [2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32] {
        for invert in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let primary = objects.allocate(actor()).unwrap();
            let secondary = objects.allocate(actor()).unwrap();
            let source = objects.get_mut(owner).unwrap();
            source.base.path = Some(entry);
            source.base.position = Vector3 {
                x: i16::MAX,
                y: i16::MIN,
                z: -100,
            };
            source.base.pitch = Angle::from_units(129);
            source.base.yaw = Angle::from_units(231);
            source.base.roll = Angle::from_units(64);
            source.base.speed = 155;
            source.extension.path_state.weapon_selection = selection;
            source.extension.path_state.friend_health_slot = 4;
            source.extension.path_state.repeat_counter = 123;
            runtime.branch.invert_next = invert;
            runtime.spawns.last_spawn = Some(primary);
            let defaults = ObjectSpawnDefaults {
                group: 56,
                run_when_paused: true,
            };
            let mut state = prior_state(Some(primary));
            let mut expected = objects.clone();
            let mut expected_random = random;
            let mut expected_state = state;
            expected_state.parameters = LaunchParameters::default();
            let created = weapon_dispatch::launch(
                &mut expected,
                owner,
                LaunchRequest {
                    weapon: PathWeapon::from_selection(selection).unwrap(),
                    parameters: LaunchParameters::default(),
                    defaults,
                },
                &mut LaunchWorld {
                    caller_inputs: Some(rapid_inputs(owner)),
                    fallback: Some(primary),
                    published_pitch: Some(Angle::from_units(197)),
                    primary: Some(primary),
                    secondary: Some(secondary),
                    primary_auxiliary_mode: Some(0x1F),
                    hostile_counts: Some(&mut expected_state.hostile_counts),
                    random: &mut expected_random,
                },
            )
            .unwrap()
            .unwrap();
            let weapon = expected.get_mut(created).unwrap();
            weapon.base.contacts.exclusion_groups = weapon
                .base
                .contacts
                .exclusion_groups
                .union(ExclusionGroups::PATH_SPAWN);
            expected.get_mut(owner).unwrap().base.path = Some(finish);
            let mut inputs = world(&mut random);
            inputs.primary_player = Some(primary);
            inputs.secondary_player = Some(secondary);
            inputs.primary_motion = Some(PrimaryMotionInput {
                auxiliary_mode: 0x1F,
                displacement: Vector3::default(),
            });
            inputs.spawn_defaults = Some(defaults);
            inputs.weapons = Some(&mut state);
            inputs.caller_weapon_inputs = Some(rapid_inputs(owner));
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 2)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(objects, expected);
            assert_eq!(state, expected_state);
            assert_eq!(random, expected_random);
            assert_eq!(runtime.spawns.last_spawn, Some(created));
            assert_eq!(runtime.branch.invert_next, invert);
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

#[test]
fn full_pool_publishes_real_fallback_and_only_adds_path_exclusion_membership() {
    let (catalog, entry, finish) = fire_catalog();
    for selection in [2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32] {
        for owner_is_fallback in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let fallback = if owner_is_fallback {
                owner
            } else {
                objects.allocate(actor()).unwrap()
            };
            let target = objects.get_mut(fallback).unwrap();
            target.base.position = Vector3 {
                x: 123,
                y: -234,
                z: 345,
            };
            target.base.hit_points = 177;
            target.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(0xA8);
            target.extension.path_state.repeat_counter = 222;
            target.extension.path_state.friend_health_slot = 4;
            objects.get_mut(owner).unwrap().base.path = Some(entry);
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .weapon_selection = selection;
            while objects.len() < OBJECT_CAPACITY {
                objects.allocate(actor()).unwrap();
            }
            let mut expected = objects.clone();
            expected.get_mut(owner).unwrap().base.path = Some(finish);
            expected
                .get_mut(fallback)
                .unwrap()
                .base
                .contacts
                .exclusion_groups = ExclusionGroups::from_authored_class(0xB8);
            let before_random = random;
            let mut state = prior_state(Some(fallback));
            let mut expected_state = state;
            expected_state.parameters = LaunchParameters::default();
            runtime.branch.invert_next = true;
            runtime.spawns.last_spawn = None;
            let mut inputs = world(&mut random);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.weapons = Some(&mut state);
            inputs.caller_weapon_inputs = Some(rapid_inputs(owner));
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 2)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(objects, expected);
            assert_eq!(random, before_random);
            assert_eq!(state, expected_state);
            assert_eq!(runtime.spawns.last_spawn, Some(fallback));
            assert!(runtime.branch.invert_next);
        }
    }
}

#[test]
fn admission_rejection_uses_real_fallback_even_with_free_pool_slots() {
    let (catalog, entry, finish) = fire_catalog();
    for selection in [4, 6, 8, 10] {
        for zero_level in [false, true] {
            if zero_level && selection != 4 {
                continue;
            }
            for fallback_case in 0..3 {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let fallback = objects.allocate(actor()).unwrap();
                if fallback_case == 2 {
                    objects.remove(fallback).unwrap();
                }
                let source = objects.get_mut(owner).unwrap();
                source.base.path = Some(entry);
                source.extension.path_state.weapon_selection = selection;
                let mut expected = objects.clone();
                if fallback_case == 0 {
                    expected.get_mut(owner).unwrap().base.path = Some(finish);
                    let actor = expected.get_mut(fallback).unwrap();
                    actor.base.contacts.exclusion_groups = actor
                        .base
                        .contacts
                        .exclusion_groups
                        .union(ExclusionGroups::PATH_SPAWN);
                }
                let mut state = prior_state(if fallback_case == 1 {
                    None
                } else {
                    Some(fallback)
                });
                let before_random = random;
                let mut inputs = world(&mut random);
                let caller = super::super::weapon_rapid::CallerWeaponInputs {
                    owner,
                    active_shots: if zero_level {
                        None
                    } else {
                        Some(super::super::path_shots::ActiveShots::from_count(8))
                    },
                    weapon_level: Some(if zero_level { 0xFC } else { 1 }),
                    roll_step: None,
                    retained_aim: None,
                };
                inputs.caller_weapon_inputs = Some(caller);
                inputs.weapons = Some(&mut state);
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                runtime.branch.invert_next = true;
                let result = runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 2);
                match fallback_case {
                    0 => {
                        assert_eq!(result.unwrap().step, ControlStep::Movement);
                        assert_eq!(runtime.spawns.last_spawn, Some(fallback));
                    }
                    1 => assert_eq!(result, Err(ProgramError::MissingWeaponFallback)),
                    _ => assert_eq!(
                        result,
                        Err(ProgramError::Runtime(PathRuntimeError::MissingActor(
                            fallback
                        )))
                    ),
                }
                assert_eq!(objects, expected);
                assert_eq!(inputs.caller_weapon_inputs, Some(caller));
                assert_eq!(*inputs.random, before_random);
                assert!(runtime.branch.invert_next);
                assert_eq!(state.parameters, LaunchParameters::default());
            }
        }
    }
}

#[test]
fn unported_selector_and_missing_fallback_fail_explicitly_without_synthesizing_weapons() {
    let (catalog, entry, _) = fire_catalog();
    for selection in 0..=u8::MAX {
        if PathWeapon::from_selection(selection).is_some() {
            continue;
        }
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(entry);
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .weapon_selection = selection;
        let before = objects.clone();
        let before_random = random;
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::UnsupportedWeaponSelection(selection))
        );
        assert_eq!(objects, before);
        assert_eq!(random, before_random);
        assert_eq!(runtime.spawns.last_spawn, None);
    }
    for missing in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(entry);
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .weapon_selection = 18;
        if missing {
            while objects.len() < OBJECT_CAPACITY {
                objects.allocate(actor()).unwrap();
            }
        }
        let before = objects.clone();
        let mut state = prior_state(None);
        let before_state = state;
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        if missing {
            inputs.weapons = Some(&mut state);
        }
        assert_eq!(
            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(if missing {
                ProgramError::MissingWeaponFallback
            } else {
                ProgramError::MissingWeaponState
            })
        );
        assert_eq!(objects, before);
        assert_eq!(state, before_state);
    }
}

#[test]
fn fire_requires_complete_projectile_catalog_and_live_spawn_defaults_before_mutation() {
    let (complete, entry, _) = fire_catalog();
    for incomplete in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let mut catalog = complete.clone();
        if incomplete {
            catalog.paths[0].clear();
        }
        objects.get_mut(owner).unwrap().base.path = Some(entry);
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .weapon_selection = 18;
        let before = objects.clone();
        let before_random = random;
        let outcome =
            runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 1);
        assert_eq!(
            outcome,
            Err(if incomplete {
                ProgramError::MissingStatement(authored_paths::VARIANT_GUIDED_PROJECTILE)
            } else {
                ProgramError::MissingSpawnDefaults
            })
        );
        assert_eq!(objects, before);
        assert_eq!(random, before_random);
        assert_eq!(runtime.spawns.last_spawn, None);
    }
}
