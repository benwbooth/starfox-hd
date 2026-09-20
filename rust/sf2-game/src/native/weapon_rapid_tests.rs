use super::super::actor_auxiliary::AuxiliaryRecord;
use super::super::program_resources::ProgramResources;
use super::super::weapon_dispatch::{self, LaunchRequest, PathWeapon};
use super::super::weapon_launch::MuzzleOffset;
use super::super::{Behavior, Object, ObjectKind, RandomState};
use super::*;

const VARIANTS: [RapidWeapon; 4] = [
    RapidWeapon::Basic,
    RapidWeapon::Upgraded,
    RapidWeapon::Maximum,
    RapidWeapon::Alternate,
];

fn actor() -> Object {
    Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight)
}

fn inputs(owner: ObjectId) -> CallerWeaponInputs {
    CallerWeaponInputs {
        owner,
        active_shots: Some(ActiveShots::from_count(7)),
        weapon_level: Some(1),
        roll_step: Some(Angle::from_units(237)),
        retained_aim: Some(Vector3 {
            x: -32700,
            y: 1021,
            z: 31200,
        }),
    }
}

fn request(weapon: RapidWeapon) -> LaunchRequest {
    LaunchRequest {
        weapon: PathWeapon::Rapid(weapon),
        parameters: LaunchParameters {
            muzzle: MuzzleOffset {
                x: -71,
                y: 53,
                z: 22,
            },
            pitch_offset: -113,
            yaw_offset: 95,
            target: None,
        },
        defaults: ObjectSpawnDefaults {
            run_when_paused: true,
            group: 99,
        },
    }
}

fn world<'a>(random: &'a mut RandomState, caller: ObjectId, proxy: ObjectId) -> LaunchWorld<'a> {
    LaunchWorld {
        caller_inputs: Some(inputs(caller)),
        fallback: Some(proxy),
        published_pitch: Some(Angle::from_units(201)),
        primary: None,
        secondary: None,
        primary_auxiliary_mode: None,
        hostile_counts: None,
        random,
    }
}

#[test]
fn all_rapid_spin_bytes_copy_only_for_upgraded_meshes_and_reaim_yaw_after_formatting() {
    use super::super::hit_response::HitSide;
    use super::super::path_control::PlayerTarget;
    for weapon in [
        RapidWeapon::Basic,
        RapidWeapon::Upgraded,
        RapidWeapon::Maximum,
    ] {
        for step in 0..=u8::MAX {
            for secondary in [false, true] {
                let mut objects = ObjectStore::new();
                let mut resources = ProgramResources::default();
                let mut source = actor();
                source.base.position = Vector3 {
                    x: 32710,
                    y: -530,
                    z: -31700,
                };
                source.base.pitch = Angle::from_units(step.wrapping_add(71));
                source.base.yaw = Angle::from_units(step.wrapping_add(153));
                source.base.roll = Angle::from_units(step.wrapping_add(43));
                source.base.speed = 187;
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
                let source_before = source.clone();
                let caller = objects.allocate(source).unwrap();
                let proxy = objects.allocate(actor()).unwrap();
                let mut random = RandomState::default();
                let random_before = random;
                let mut world = world(&mut random, caller, proxy);
                world.published_pitch = None;
                let caller_inputs = world.caller_inputs.as_mut().unwrap();
                caller_inputs.roll_step = if weapon == RapidWeapon::Basic {
                    None
                } else {
                    Some(Angle::from_units(step))
                };
                caller_inputs.weapon_level = None;
                let caller_inputs_before = world.caller_inputs;
                let mut request = request(weapon);
                if step & 1 != 0 {
                    request.parameters.target = Some(Vector3 {
                        x: 701,
                        y: -570,
                        z: -1703,
                    });
                }
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
                let aim = world.caller_inputs.unwrap().retained_aim.unwrap();
                expected.get_mut(proxy).unwrap().base.position = aim;
                let target = expected.get_mut(created).unwrap();
                target.base.path = Some(authored_paths::RAPID_IMPACT_PROJECTILE);
                target.base.shape = match weapon {
                    RapidWeapon::Basic => BASIC_SHAPE,
                    RapidWeapon::Upgraded => UPGRADED_SHAPE,
                    _ => MAXIMUM_SHAPE,
                };
                target.base.roll = source_before.base.roll;
                target.extension.relative_rotation.roll = if weapon == RapidWeapon::Basic {
                    Angle::ZERO
                } else {
                    Angle::from_units(step)
                };
                target.extension.path_state.animation.shape.initialize(3);
                let fine = sf_core::aim_angle::sf2_atan16(
                    aim.x.wrapping_sub(target.base.position.x),
                    aim.z.wrapping_sub(target.base.position.z),
                );
                target.base.yaw = Angle::from_units(((fine >> 8) as u8).wrapping_neg());
                target
                    .extension
                    .auxiliary
                    .set(
                        &mut expected_resources,
                        created,
                        AuxiliaryRecord::ReflectionShape(target.base.shape),
                    )
                    .unwrap();
                assert_eq!(
                    weapon_dispatch::launch(
                        &mut objects,
                        &mut resources,
                        caller,
                        request,
                        &mut world
                    ),
                    Ok(Some(created))
                );
                assert_eq!(objects, expected);
                assert_eq!(resources, expected_resources);
                assert_eq!(world.caller_inputs, caller_inputs_before);
                assert_eq!(*world.random, random_before);
                let target = objects.get(created).unwrap();
                assert_eq!(target.base.velocity, Vector3::default());
                assert_eq!(target.base.wait_timer, 187);
                assert_eq!(target.base.child_number, target.base.pitch.units());
                assert_eq!(target.base.hit_points, 1);
                assert_eq!(target.base.attack_power, 1);
                assert_eq!(target.extension.path_state.script_parameter, 0);
                assert_eq!(target.extension.path_state.script_value, 0);
                assert_eq!(
                    world.caller_inputs.unwrap().active_shots.unwrap().count(),
                    7
                );
            }
        }
    }
}

#[test]
fn rapid_shape_allocation_failure_occurs_after_formatting_and_live_aim_publication() {
    use super::super::actor_auxiliary::AuxiliaryError;
    use super::super::program_resources::{AllocationFailure, PROGRAM_CAPACITY};
    use super::super::program_state::ProgramData;
    for weapon in VARIANTS {
        let mut objects = ObjectStore::new();
        let caller = objects.allocate(actor()).unwrap();
        let proxy = objects.allocate(actor()).unwrap();
        let mut resources = ProgramResources::default();
        resources
            .allocate_shared(
                PROGRAM_CAPACITY - 8 - 2,
                ProgramData::PathStack(Default::default()),
            )
            .unwrap();
        let before_resources = resources.clone();
        let mut random = RandomState::default();
        let before_random = random;
        let mut world = world(&mut random, caller, proxy);
        let caller_inputs = world.caller_inputs;
        assert_eq!(
            weapon_dispatch::launch(
                &mut objects,
                &mut resources,
                caller,
                request(weapon),
                &mut world
            ),
            Err(LaunchError::Auxiliary(AuxiliaryError::Allocation(
                AllocationFailure::NoContiguousFit
            )))
        );
        assert_eq!(objects.len(), 3);
        assert_eq!(world.caller_inputs, caller_inputs);
        assert_eq!(*world.random, before_random);
        assert_eq!(resources, before_resources);
        let created = objects.get(caller).unwrap().base.linked_object.unwrap();
        let shot = objects.get(created).unwrap();
        assert_eq!(shot.base.path, Some(weapon.paths()[0]));
        assert_eq!(shot.base.attachment, Some(caller));
        assert_eq!(shot.base.hit_points, 1);
        assert_eq!(
            shot.base.attack_power,
            if weapon == RapidWeapon::Alternate {
                2
            } else {
                1
            }
        );
        assert_eq!(
            shot.extension
                .auxiliary
                .reflection_shape(&resources, created),
            Ok(None)
        );
        if weapon != RapidWeapon::Alternate {
            assert_eq!(
                objects.get(proxy).unwrap().base.position,
                caller_inputs.unwrap().retained_aim.unwrap()
            );
            assert_eq!(shot.extension.path_state.animation.shape.packed(), 131);
        }
    }
}

#[test]
fn alternate_rapid_masks_every_equipment_byte_and_copies_every_published_pitch() {
    for level in 0..=u8::MAX {
        for pitch in 0..=u8::MAX {
            let mut objects = ObjectStore::new();
            let mut resources = ProgramResources::default();
            let mut source = actor();
            source.base.pitch = Angle::from_units(pitch.wrapping_add(23));
            source.base.yaw = Angle::from_units(193);
            source.base.roll = Angle::from_units(117);
            let caller = objects.allocate(source).unwrap();
            let proxy = objects.allocate(actor()).unwrap();
            let mut random = RandomState::default();
            let random_before = random;
            let mut world = world(&mut random, caller, proxy);
            world.fallback = None;
            world.published_pitch = Some(Angle::from_units(pitch));
            let caller_inputs = world.caller_inputs.as_mut().unwrap();
            caller_inputs.weapon_level = Some(level);
            caller_inputs.roll_step = None;
            caller_inputs.retained_aim = None;
            if level & 3 == 0 {
                caller_inputs.active_shots = None;
            }
            let input_before = world.caller_inputs;
            let request = request(RapidWeapon::Alternate);
            let mut expected = objects.clone();
            let mut expected_resources = resources.clone();
            let created = if level & 3 == 0 {
                None
            } else {
                let created = weapon_creation::player_linked(
                    &mut expected,
                    caller,
                    request.parameters,
                    request.defaults,
                )
                .unwrap()
                .unwrap();
                let target = expected.get_mut(created).unwrap();
                target.base.shape = if level & 3 == 1 {
                    ALTERNATE_SPRITE
                } else {
                    MAXIMUM_SHAPE
                };
                target.base.path = Some(authored_paths::ALTERNATE_RAPID_IMPACT_PROJECTILE);
                target.base.pitch = Angle::from_units(pitch);
                target.base.attack_power = if level & 3 == 1 { 2 } else { 4 };
                if level & 3 == 1 {
                    target.base.flags.scaled_sprite = true;
                    target.extension.depth_offset &= 0xFF00;
                    target.extension.texture_scroll_x = 0;
                }
                target
                    .extension
                    .auxiliary
                    .set(
                        &mut expected_resources,
                        created,
                        AuxiliaryRecord::ReflectionShape(target.base.shape),
                    )
                    .unwrap();
                Some(created)
            };
            assert_eq!(
                weapon_dispatch::launch(&mut objects, &mut resources, caller, request, &mut world),
                Ok(created)
            );
            assert_eq!(objects, expected);
            assert_eq!(resources, expected_resources);
            assert_eq!(world.caller_inputs, input_before);
            assert_eq!(*world.random, random_before);
        }
    }
}

#[test]
fn every_count_uses_signed_admission_before_allocation_and_never_changes_the_counter() {
    for weapon in VARIANTS {
        for count in 0..=u8::MAX {
            for full in [false, true] {
                let mut objects = ObjectStore::new();
                let mut resources = ProgramResources::default();
                let caller = objects.allocate(actor()).unwrap();
                let proxy = objects.allocate(actor()).unwrap();
                if full {
                    while objects.len() < OBJECT_CAPACITY {
                        objects.allocate(actor()).unwrap();
                    }
                }
                let before = objects.clone();
                let mut random = RandomState::default();
                let random_before = random;
                let mut world = world(&mut random, caller, proxy);
                world.caller_inputs.as_mut().unwrap().active_shots =
                    Some(ActiveShots::from_count(count));
                let admitted = count < 8 || count >= 136;
                if full || !admitted {
                    world.published_pitch = None;
                    world.fallback = None;
                    world.caller_inputs.as_mut().unwrap().retained_aim = None;
                    world.caller_inputs.as_mut().unwrap().roll_step = None;
                }
                let result = weapon_dispatch::launch(
                    &mut objects,
                    &mut resources,
                    caller,
                    request(weapon),
                    &mut world,
                )
                .unwrap();
                assert_eq!(result.is_some(), admitted && !full);
                if result.is_none() {
                    assert_eq!(objects, before);
                }
                assert_eq!(
                    world.caller_inputs.unwrap().active_shots.unwrap().count(),
                    count
                );
                assert_eq!(*world.random, random_before);
            }
        }
    }
}

#[test]
fn required_rapid_inputs_fail_before_allocation_and_are_not_replaced_with_selected_state() {
    for case in 0..10 {
        let mut objects = ObjectStore::new();
        let mut resources = ProgramResources::default();
        let caller = objects.allocate(actor()).unwrap();
        let proxy = objects.allocate(actor()).unwrap();
        let mut random = RandomState::default();
        let mut world = world(&mut random, caller, proxy);
        let weapon = if case == 2 || case == 7 {
            RapidWeapon::Alternate
        } else {
            RapidWeapon::Upgraded
        };
        let expected = match case {
            0 => {
                world.caller_inputs = None;
                RapidLaunchError::MissingCallerInputs.into()
            }
            1 => {
                world.caller_inputs.as_mut().unwrap().owner = proxy;
                RapidLaunchError::WrongCallerInputs {
                    expected: caller,
                    supplied: proxy,
                }
                .into()
            }
            2 => {
                world.caller_inputs.as_mut().unwrap().weapon_level = None;
                RapidLaunchError::MissingWeaponLevel.into()
            }
            3 | 9 => {
                world.caller_inputs.as_mut().unwrap().active_shots = None;
                RapidLaunchError::MissingShotCount.into()
            }
            4 => {
                world.caller_inputs.as_mut().unwrap().roll_step = None;
                RapidLaunchError::MissingRollStep.into()
            }
            5 => {
                world.caller_inputs.as_mut().unwrap().retained_aim = None;
                RapidLaunchError::MissingRetainedAim.into()
            }
            6 => {
                world.fallback = None;
                RapidLaunchError::MissingAimProxy.into()
            }
            7 => {
                world.published_pitch = None;
                LaunchError::MissingPublishedPitch
            }
            _ => {
                objects.remove(proxy).unwrap();
                RapidLaunchError::MissingAimProxyActor(proxy).into()
            }
        };
        if case == 9 {
            while objects.len() < OBJECT_CAPACITY {
                objects.allocate(actor()).unwrap();
            }
        }
        let before = objects.clone();
        let random_before = *world.random;
        assert_eq!(
            weapon_dispatch::launch(
                &mut objects,
                &mut resources,
                caller,
                request(weapon),
                &mut world
            ),
            Err(expected)
        );
        assert_eq!(objects, before);
        assert_eq!(*world.random, random_before);
    }
}
