//! Complete authored paired-part patrol graphs, using source-defined inputs.
use super::super::path_player_control::{PlayerTargetControl, PrimaryControl};
use super::super::path_relationships::find_child;
use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
use super::super::path_scene_state::EncounterCoordination;
use super::super::{
    authored_paths, Angle, AudioState, Behavior, ObjectKind, ObjectSpawnDefaults, ShapeId, Vector3,
};
use super::projectile_tests::audio;
use super::tests::{setup, world};
use super::*;

const ROOTS: [PathCursor; 2] = [
    authored_paths::PAIRED_PART_YAW_PATROL,
    authored_paths::PAIRED_PART_PITCH_PATROL,
];

#[test]
fn death_drop_selects_all_five_pickups_and_consumes_only_the_reached_random_draws() {
    use super::super::Difficulty;
    let catalog = authored_paths::catalog();
    let drops = [
        authored_paths::WEAPON_UPGRADE_PICKUP,
        authored_paths::CONSUMABLE_PICKUP_TYPE_ZERO,
        authored_paths::CONSUMABLE_PICKUP_TYPE_THREE,
        authored_paths::CONSUMABLE_PICKUP_TYPE_ONE,
        authored_paths::SHIELD_RECOVERY_PICKUP,
    ];
    let mut seen = [false; 5];
    for difficulty in [Difficulty::Normal, Difficulty::Hard, Difficulty::Expert] {
        for seed in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, _) = setup();
            let mut random = RandomState::new([seed, 31, 73, 171]);
            let mut expected_random = random;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(ROOTS[0]);
            actor.base.hit_points = 1;
            let player = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let mut shared = EncounterCoordination {
                active_messages: 1,
                ..Default::default()
            };
            let mut control = PlayerTargetControl::default();
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.primary_player = Some(player);
            inputs.primary_control = Some(PrimaryControl {
                target: &mut control,
                linked_mode: false,
            });
            let mut objective_counts = super::super::path_scene_state::EncounterObjectiveCounts { remaining_word: 1, ..Default::default() };
            inputs.objective_counts = Some(&mut objective_counts);
            inputs.campaign = Some(CampaignPathInputs {
                difficulty,
                encounter_variant: 0,
            });
            inputs.audio = Some(audio(&mut events));
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            objects.get_mut(owner).unwrap().base.hit_points = 80;
            objects
                .get_mut(owner)
                .unwrap()
                .base
                .contacts
                .new_contact_latched = true;
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
            for _ in 0..15 {
                expected_random.next_byte();
            }
            let first = expected_random.next_byte() & 15;
            let expected = if first == 0 {
                0
            } else {
                match expected_random.next_byte() & 7 {
                    7 => 1,
                    6 => 2,
                    5 => 3,
                    _ if difficulty != Difficulty::Normal => 4,
                    _ => match expected_random.next_byte() & 3 {
                        3 => 1,
                        2 => 2,
                        1 => 3,
                        _ => 0,
                    },
                }
            };
            for visit in 1..=18 {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    if visit == 18 {
                        ControlStep::MovementTail
                    } else {
                        ControlStep::Movement
                    }
                );
            }
            let drop = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
            assert_eq!(drop.base.shape, ShapeId::from_catalog_index(516));
            assert_eq!(drop.base.path, Some(drops[expected]));
            assert!(drop.base.contacts.run_when_paused);
            assert_eq!(drop.base.yaw, Angle::ZERO);
            assert_eq!(drop.base.hit_points, 10);
            assert_eq!(drop.base.attack_power, 0);
            assert_eq!(inputs.random, &expected_random);
            assert_eq!(inputs.coordination.as_deref().unwrap().retired_actors, 1);
            seen[expected] = true;
        }
    }
    assert!(seen.into_iter().all(|value| value));
}

#[test]
fn pitch_patrol_emits_at_wrapped_endpoints_and_reverses_only_at_its_authored_heading() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(ROOTS[1]);
    let mut shared = EncounterCoordination {
        active_messages: 1,
        ..Default::default()
    };
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut shared);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    let mut pitch = 192_u8;
    let mut step = 254_u8;
    let mut launches = 0;
    for visit in 1..=520 {
        if visit >= 3 && matches!(pitch, 64 | 192) {
            launches += 1;
        }
        if visit >= 2 {
            if pitch == 192 {
                step = step.wrapping_neg();
            }
            pitch = pitch.wrapping_add(step);
        }
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects.get(owner).unwrap().base.pitch.units(), pitch);
        assert_eq!(objects.len(), 3 + launches);
        assert_eq!(
            (objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .motion_phase
                >> 8) as u8,
            step
        );
    }
    assert!(launches >= 4);
}

pub(super) fn callbacks(
    runtime: &mut PathRuntime,
    catalog: &PathCatalog,
    objects: &mut ObjectStore,
    owner: ObjectId,
    inputs: &mut PathWorld<'_>,
    tick: u8,
) {
    assert!(runtime.begin_callbacks(objects, owner).unwrap());
    for _ in 0..32 {
        match runtime
            .step_callbacks(
                objects,
                owner,
                TriggerWorldInputs {
                    strategy_tick: tick,
                    ..Default::default()
                },
            )
            .unwrap()
        {
            CallbackStep::Run(_) => assert_eq!(
                runtime.resume_program(catalog, objects, owner, inputs, 100),
                Ok(ProgramExit {
                    actor: owner,
                    step: ControlStep::ResumeCallbacks
                })
            ),
            CallbackStep::Skipped | CallbackStep::Expired => {}
            CallbackStep::Complete => return,
        }
    }
    panic!("paired patrol callbacks did not complete");
}

#[test]
fn remembered_identity_gate_checks_all_mask_bytes_before_any_child_or_world_dependency() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for identity in 1..=8 {
            for mask in 0..=u8::MAX {
                for inverted in [false, true] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(root);
                    actor.base.hit_points = identity;
                    actor.extension.path_state.motion_phase = 0xAB00;
                    let mut shared = EncounterCoordination {
                        retired_actors: mask,
                        active_messages: u8::from(inverted),
                        ..Default::default()
                    };
                    runtime.branch.invert_next = inverted;
                    let before_random = random;
                    let mut inputs = world(&mut random);
                    inputs.coordination = Some(&mut shared);
                    let retired = mask & (1 << (identity - 1)) != 0;
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100),
                        Ok(ProgramExit {
                            actor: owner,
                            step: if retired {
                                ControlStep::Ended
                            } else {
                                ControlStep::Movement
                            }
                        })
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.extension.path_state.script_parameter, identity);
                    assert_eq!(actor.base.hit_points, if retired { identity } else { 100 });
                    assert_eq!(actor.base.flags.remove_after_tick, retired);
                    assert_eq!(
                        actor.extension.path_state.motion_phase,
                        0xAB00 | u16::from(if retired { mask } else { u8::from(inverted) })
                    );
                    assert_eq!(objects.len(), 1);
                    assert_eq!(shared.retired_actors, mask);
                    assert_eq!(runtime.branch.invert_next, retired && inverted);
                    assert_eq!(random, before_random);
                }
            }
        }
    }
}

#[test]
fn activation_wait_resamples_exactly_one_and_installs_both_numbered_parts() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for gate in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(root);
            actor.base.hit_points = 3;
            actor.base.attack_power = 171;
            actor.base.yaw = Angle::from_units(79);
            actor.base.position = Vector3 {
                x: i16::MAX,
                y: -300,
                z: i16::MIN,
            };
            let mut shared = EncounterCoordination {
                active_messages: gate,
                ..Default::default()
            };
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            if gate != 1 {
                assert_eq!(objects.len(), 1);
                assert!(!objects.get(owner).unwrap().base.flags.visible);
                inputs.coordination.as_deref_mut().unwrap().active_messages = 1;
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
            }
            assert_eq!(objects.len(), 3);
            let actor = objects.get(owner).unwrap();
            assert!(actor.base.flags.visible);
            assert_eq!((actor.base.hit_points, actor.base.attack_power), (100, 4));
            assert_eq!(actor.extension.path_state.script_parameter, 3);
            assert_eq!(actor.base.speed, 20);
            if root == ROOTS[0] {
                assert_eq!(actor.base.position.y, -260);
                assert_eq!(actor.base.yaw.units(), 80);
                assert_eq!(actor.base.roll.units(), 128);
                assert_eq!(
                    actor.extension.relative_position.x,
                    i16::MAX.wrapping_add(924)
                );
                assert_eq!(
                    actor.extension.relative_position.z,
                    i16::MIN.wrapping_add(924)
                );
            } else {
                assert_eq!(actor.base.position.y, -300);
                assert_eq!(actor.base.yaw.units(), 171);
                assert_eq!(actor.base.pitch.units(), 192);
                assert_eq!(actor.base.roll.units(), 192);
                assert_eq!(actor.extension.relative_position.x, -100);
                assert_eq!(actor.extension.relative_position.y, -220);
            }
            for (number, shape, offset, animation) in [(1, 188, 30, 128), (2, 187, -30, 132)] {
                let child = find_child(&objects, owner, number).unwrap().unwrap();
                let actor = objects.get(child).unwrap();
                assert_eq!(actor.base.shape, ShapeId::from_catalog_index(shape));
                assert_eq!(
                    actor.extension.relative_position,
                    Vector3 {
                        x: offset,
                        y: 60,
                        z: 0
                    }
                );
                assert_eq!(actor.base.position, Vector3::default());
                assert_eq!(actor.base.attachment, Some(owner));
                assert_eq!(actor.extension.parent, Some(owner));
                assert_eq!((actor.base.hit_points, actor.base.attack_power), (100, 4));
                assert_eq!(
                    actor.extension.path_state.animation.shape.packed(),
                    animation
                );
                catalog.statement(actor.base.path.unwrap()).unwrap();
            }
        }
    }
}

#[test]
fn new_contact_damage_enters_full_death_sequence_and_records_original_identity() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for identity in 1..=8 {
            for health in [0, 1, 79, 80, 81, 100, 208, 209, 255] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                objects.get_mut(owner).unwrap().base.path = Some(root);
                objects.get_mut(owner).unwrap().base.hit_points = identity;
                let player = objects
                    .allocate(Object::new(
                        ObjectKind::Player,
                        ShapeId::EMPTY,
                        Behavior::PlayerFlight,
                    ))
                    .unwrap();
                let mut shared = EncounterCoordination {
                    active_messages: 1,
                    progress: 255,
                    ..Default::default()
                };
                let mut control = PlayerTargetControl::default();
                let mut events = AudioState::default();
                let mut inputs = world(&mut random);
                inputs.coordination = Some(&mut shared);
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                inputs.primary_player = Some(player);
                inputs.primary_control = Some(PrimaryControl {
                    target: &mut control,
                    linked_mode: false,
                });
                let mut objective_counts = super::super::path_scene_state::EncounterObjectiveCounts { remaining_word: 0, ..Default::default() };
                inputs.objective_counts = Some(&mut objective_counts);
                inputs.audio = Some(audio(&mut events));
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                let parts = [
                    find_child(&objects, owner, 1).unwrap().unwrap(),
                    find_child(&objects, owner, 2).unwrap().unwrap(),
                ];
                objects.get_mut(owner).unwrap().base.hit_points = health;
                objects
                    .get_mut(owner)
                    .unwrap()
                    .base
                    .contacts
                    .new_contact_latched = true;
                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
                let before_random = *inputs.random;
                let mut expected_random = before_random;
                if (80_u8.wrapping_sub(health) as i8) < 0 {
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                    assert_eq!(inputs.coordination.as_deref().unwrap().retired_actors, 0);
                    assert_eq!(objects.get(owner).unwrap().base.hit_points, health);
                    continue;
                }
                for visit in 1..=18 {
                    if visit <= 15 {
                        expected_random.next_byte();
                    }
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                            .unwrap()
                            .step,
                        if visit == 18 {
                            ControlStep::MovementTail
                        } else {
                            ControlStep::Movement
                        }
                    );
                    assert_eq!(inputs.random, &expected_random);
                }
                assert_eq!(inputs.coordination.as_deref().unwrap().progress, 0);
                assert_eq!(
                    inputs.coordination.as_deref().unwrap().retired_actors,
                    1 << (identity - 1)
                );
                assert_eq!(objects.get(owner).unwrap().base.hit_points, 0);
                assert!(objects.get(owner).unwrap().base.flags.collision_disabled);
                assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
                assert_eq!(objects.len(), 8); // owner, player, two parts, four fade effects
                for part in parts {
                    let actor = objects.get(part).unwrap();
                    assert_eq!(actor.base.attachment, None);
                    assert_eq!(actor.extension.parent, Some(owner));
                    assert_eq!(actor.base.hit_points, 100);
                }
                assert_eq!(control.owner, Some(owner));
                assert_eq!(control.range, 2);
            }
        }
    }
}

#[test]
fn signaled_parts_cancel_hold_callbacks_and_run_fifteen_signed_curve_steps() {
    use super::super::path_relationships::{self, RelationshipCommand};
    let catalog = authored_paths::catalog();
    const ARC: [i16; 15] = [-32, -24, -18, -12, -8, -4, -2, 0, 0, 2, 4, 8, 12, 18, 24];
    for number in [1, 2] {
        for seed in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, _) = setup();
            let mut random = RandomState::new([seed, 7, 39, 127]);
            let mut expected_random = random;
            objects.get_mut(owner).unwrap().base.path = Some(ROOTS[0]);
            let mut shared = EncounterCoordination {
                active_messages: 1,
                ..Default::default()
            };
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let child = find_child(&objects, owner, number).unwrap().unwrap();
            let actor = objects.get_mut(child).unwrap();
            actor.base.position.y = i16::MIN;
            actor.base.yaw = Angle::from_units(233);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert!(
                objects
                    .get(child)
                    .unwrap()
                    .extension
                    .path_state
                    .hold_latched
            );
            callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs, 1);
            assert_eq!(
                objects
                    .get(child)
                    .unwrap()
                    .extension
                    .path_state
                    .animation
                    .shape
                    .packed(),
                if number == 1 { 129 } else { 133 }
            );
            path_relationships::apply(
                &mut objects,
                owner,
                RelationshipCommand::SignalChild { number },
            )
            .unwrap();
            path_relationships::apply(
                &mut objects,
                owner,
                RelationshipCommand::UnlinkChild { number },
            )
            .unwrap();
            callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs, 2);
            assert!(
                !objects
                    .get(child)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .hit_event_pending
            );
            let yaw_step = (expected_random.next_byte() & 63).wrapping_add(224);
            let mut expected_y = i16::MIN;
            for (step, delta) in ARC.into_iter().enumerate() {
                expected_y = expected_y.wrapping_add(delta);
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
                        .unwrap()
                        .step,
                    if step == 14 {
                        ControlStep::MovementTail
                    } else {
                        ControlStep::Movement
                    }
                );
                let actor = objects.get(child).unwrap();
                assert_eq!(actor.base.position.y, expected_y);
                assert_eq!(
                    actor.base.yaw.units(),
                    233_u8.wrapping_add(yaw_step.wrapping_mul(step as u8 + 1))
                );
                assert_eq!(actor.base.velocity.x, if number == 1 { 7 } else { -7 });
                assert_eq!(
                    actor.extension.relative_position.x,
                    if number == 1 { 7 } else { -7 }
                );
                assert_eq!(actor.base.hit_points, if step == 14 { 0 } else { 100 });
                assert_eq!(inputs.random, &expected_random);
            }
            assert_eq!(objects.get(child).unwrap().base.attachment, None);
            assert_eq!(objects.get(child).unwrap().extension.parent, Some(owner));
        }
    }
}

#[test]
fn periodic_mount_spawns_two_charges_and_each_finishes_by_launching_a_native_weapon() {
    use super::super::weapon_dispatch::WeaponState;
    let catalog = authored_paths::catalog();
    for mode in [0, 16, 31, 32, 255] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(ROOTS[0]);
        let mut player = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
        player.base.position = Vector3 {
            x: 100,
            y: -50,
            z: 1000,
        };
        let player = objects.allocate(player).unwrap();
        let mut shared = EncounterCoordination {
            active_messages: 1,
            ..Default::default()
        };
        let mut events = AudioState::default();
        let mut weapons = WeaponState::default();
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.selected = Some(player);
        inputs.primary_player = Some(player);
        inputs.primary_motion = Some(PrimaryMotionInput {
            auxiliary_mode: mode,
            displacement: Vector3::default(),
        });
        inputs.audio = Some(audio(&mut events));
        inputs.weapons = Some(&mut weapons);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        callbacks(
            &mut runtime,
            &catalog,
            &mut objects,
            owner,
            &mut inputs,
            127,
        );
        assert_eq!(find_child(&objects, owner, 9).unwrap(), None);
        callbacks(
            &mut runtime,
            &catalog,
            &mut objects,
            owner,
            &mut inputs,
            128,
        );
        let mount = find_child(&objects, owner, 9).unwrap().unwrap();
        let mut charges = Vec::new();
        for visit in 1..=32 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, mount, &mut inputs, 100)
                    .unwrap()
                    .step,
                if visit == 32 {
                    ControlStep::Ended
                } else {
                    ControlStep::Movement
                }
            );
            if visit == 1 || visit == 17 {
                let charge = runtime.spawns.last_spawn.unwrap();
                assert_ne!(charge, mount);
                assert_eq!(objects.get(charge).unwrap().base.attachment, Some(owner));
                assert_eq!(objects.get(charge).unwrap().extension.parent, Some(mount));
                charges.push(charge);
            }
        }
        assert_ne!(charges[0], charges[1]);
        assert_eq!(objects.len(), 7);
        for charge in charges {
            for visit in 1..=10 {
                let before = objects.len();
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, charge, &mut inputs, 100)
                        .unwrap()
                        .step,
                    if visit == 10 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                assert_eq!(
                    objects.get(charge).unwrap().extension.texture_scroll_x,
                    8 + visit * 2
                );
                assert_eq!(objects.len(), before + usize::from(visit == 10));
            }
            let shot = runtime.spawns.last_spawn.unwrap();
            assert_eq!(
                objects.get(shot).unwrap().base.path,
                Some(authored_paths::OCCUPANCY_SURFACE_LIMITED)
            );
            assert_eq!(
                objects.get(shot).unwrap().base.speed,
                if mode & 0xF0 == 0x10 { 40 } else { 60 }
            );
            assert!(objects.get(charge).unwrap().base.flags.remove_after_tick);
        }
    }
}
