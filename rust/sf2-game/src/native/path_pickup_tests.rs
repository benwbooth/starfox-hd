//! Complete authored pickup graphs; tests supply live domain state, never a
//! source machine, trace, or gameplay recording.
use super::super::collision_surface::SurfaceMode;
use super::super::path_control::PlayerTarget;
use super::super::path_equipment::SelectedEquipment;
use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
use super::super::path_score::PlayerScore;
use super::super::path_sound::{AuthoredCue, CueListener, PathAudio};
use super::super::path_triggers::TriggerKind;
use super::super::player_hit_control::ShieldRecoveryRequest;
use super::super::{
    authored_paths, Angle, AudioState, Behavior, ObjectKind, ShapeId, SoundEvent, Vector3,
};
use super::tests::{setup, world};
use super::*;

const PICKUPS: [(PathCursor, u8, u8, u8); 5] = [
    (authored_paths::SHIELD_RECOVERY_PICKUP, 5, 0, 12),
    (authored_paths::WEAPON_UPGRADE_PICKUP, 4, 0, 8),
    (authored_paths::CONSUMABLE_PICKUP_TYPE_ZERO, 3, 0, 10),
    (authored_paths::CONSUMABLE_PICKUP_TYPE_THREE, 1, 3, 4),
    (authored_paths::CONSUMABLE_PICKUP_TYPE_ONE, 0, 1, 0),
];

fn callbacks(
    runtime: &mut PathRuntime,
    catalog: &PathCatalog,
    objects: &mut ObjectStore,
    owner: ObjectId,
    inputs: &mut PathWorld<'_>,
    trigger_inputs: TriggerWorldInputs,
) -> usize {
    assert!(runtime.begin_callbacks(objects, owner).unwrap());
    let mut count = 0;
    for _ in 0..10 {
        match runtime
            .step_callbacks(objects, owner, trigger_inputs)
            .unwrap()
        {
            CallbackStep::Run(_) => {
                count += 1;
                assert_eq!(
                    runtime.resume_program(catalog, objects, owner, inputs, 64).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(ControlStep::ResumeCallbacks)
                );
            }
            CallbackStep::Skipped | CallbackStep::Expired => {}
            CallbackStep::Complete => return count,
        }
    }
    panic!("pickup callback list did not complete");
}

#[test]
fn authored_pickups_initialize_collect_cancel_and_award_each_kind() {
    let catalog = authored_paths::catalog();
    for (entry, phase, kind, color) in PICKUPS {
        for identity in [0, 1, 8, 16] {
            for surface in [0, 1, 8, 255] {
                for health in [0, 10, 100] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let player = objects
                        .allocate(Object::new(
                            ObjectKind::Player,
                            ShapeId::EMPTY,
                            Behavior::PlayerFlight,
                        ))
                        .unwrap();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(entry);
                    actor.base.shape = ShapeId::from_catalog_index(516);
                    actor.base.hit_points = health;
                    actor.base.pitch = Angle::from_units(37);
                    actor.base.yaw = Angle::from_units(identity);
                    actor.base.roll = Angle::from_units(211);
                    actor.base.attack_power = 250;
                    actor.base.flags.casts_shadow = true;
                    let original_random = random;
                    let mut history = PickupHistory {
                        collected_mask: 0x0040,
                    };
                    let mut equipment = SelectedEquipment {
                        packed_consumables: 0xA7,
                        consumable_type: kind,
                        weapon_level: 1,
                    };
                    let mut score = PlayerScore::from_parts(65500, 93);
                    let mut recovery = ShieldRecoveryRequest { amount: 251 };
                    let mut audio = AudioState::default();
                    let mut inputs = world(&mut random);
                    inputs.scene.encounter_location = Some(5);
                    inputs.scene.active_weapon_level = Some(2);
                    inputs.surface_mode = Some(SurfaceMode { flags: surface });
                    inputs.selected = Some(player);
                    inputs.primary_player = Some(player);
                    inputs.fixed_players = [Some(player); 2];
                    inputs.pickup_history = Some(&mut history);
                    inputs.selected_equipment = Some(&mut equipment);
                    inputs.selected_score = Some(&mut score);
                    inputs.shield_recovery = Some(&mut recovery);
                    inputs.audio = Some(PathAudio {
                        events: &mut audio,
                        listeners: [CueListener::PrimaryPlayer; 2],
                        markers: None,
                    });
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(ControlStep::Movement)
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(
                        actor.extension.path_state.motion_phase,
                        u16::from(phase) | u16::from(kind) << 8
                    );
                    assert_eq!(actor.base.attack_power, identity);
                    assert_eq!(
                        (
                            actor.base.pitch.units(),
                            actor.base.yaw.units(),
                            actor.base.roll.units()
                        ),
                        (0, 0, 0)
                    );
                    assert!(actor.base.flags.collision_disabled);
                    assert!(!actor.base.flags.casts_shadow);
                    assert_eq!(
                        actor.extension.path_state.animation.color.fixed_frame(),
                        Some(color)
                    );
                    assert_eq!(
                        actor.base.shape.catalog_index(),
                        if surface == 0 { 517 } else { 516 }
                    );
                    assert_eq!(
                        actor.base.hit_points,
                        if health == 0 { 100 } else { health }
                    );
                    assert_eq!(actor.extension.path_state.hold_latched, health != 10);
                    assert_eq!(
                        actor
                            .extension
                            .path_state
                            .triggers
                            .entries(&runtime.resources, owner)
                            .unwrap()
                            .len(),
                        2
                    );
                    let interrupted = actor.base.path;
                    assert_eq!(
                        callbacks(
                            &mut runtime,
                            &catalog,
                            &mut objects,
                            owner,
                            &mut inputs,
                            TriggerWorldInputs::default()
                        ),
                        2
                    );
                    assert_ne!(objects.get(owner).unwrap().base.path, interrupted);
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(ControlStep::Movement)
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(
                        actor
                            .extension
                            .path_state
                            .triggers
                            .entries(&runtime.resources, owner)
                            .unwrap()
                            .len(),
                        1
                    );
                    assert!(!actor.base.flags.remove_after_tick);
                    assert_eq!(
                        inputs.selected_score.as_deref().unwrap().points(),
                        93 * 65536 + 65500
                    );
                    // Collection waits exactly one visit after cancellation.
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 24).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(ControlStep::Ended)
                    );
                    let actor = objects.get(owner).unwrap();
                    assert!(actor.base.flags.remove_after_tick);
                    let mask = if identity == 0 {
                        0
                    } else {
                        1 << (identity - 1)
                    };
                    assert_eq!(
                        inputs.pickup_history.as_deref().unwrap().collected_mask,
                        0x0040 | mask
                    );
                    let equipment = inputs.selected_equipment.as_deref().unwrap();
                    assert_eq!(
                        equipment.packed_consumables,
                        if phase < 4 { 0xA8 } else { 0xA7 }
                    );
                    assert_eq!(equipment.weapon_level, if phase == 4 { 2 } else { 1 });
                    assert_eq!(
                        inputs.shield_recovery.as_deref().unwrap().amount,
                        if phase == 5 { 3 } else { 251 }
                    );
                    assert_eq!(
                        inputs.selected_score.as_deref().unwrap().points(),
                        93 * 65536 + if phase == 5 { 65501 } else { 65535 }
                    );
                    let expected = match phase {
                        5 => vec![],
                        4 => vec![SoundEvent::Authored(AuthoredCue::new(
                            67,
                            0,
                            PlayerTarget::Primary,
                        ))],
                        _ => vec![SoundEvent::Authored(AuthoredCue::new(
                            69,
                            0,
                            PlayerTarget::Primary,
                        ))],
                    };
                    assert_eq!(
                        inputs
                            .audio
                            .as_mut()
                            .unwrap()
                            .events
                            .take_events()
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>(),
                        expected
                    );
                    assert_eq!(inputs.random, &original_random);
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }
}

#[test]
fn authored_pickup_history_suppresses_every_already_collected_identity_before_world_services() {
    let catalog = authored_paths::catalog();
    for (entry, _, _, _) in PICKUPS {
        for identity in 1..=16 {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.path = Some(entry);
            objects.get_mut(owner).unwrap().base.yaw = Angle::from_units(identity);
            let mut history = PickupHistory {
                collected_mask: 1 << (identity - 1),
            };
            let mut inputs = world(&mut random);
            inputs.scene.encounter_location = Some(5);
            inputs.scene.active_weapon_level = Some(0);
            inputs.pickup_history = Some(&mut history);
            // Equipment, surface, score, audio and players are absent: the
            // already-collected branch must not touch any of them.
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 32).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                Ok(ControlStep::Ended)
            );
            let actor = objects.get(owner).unwrap();
            assert!(!actor.base.flags.visible);
            assert!(actor.base.flags.remove_after_tick);
            assert_eq!(
                actor
                    .extension
                    .path_state
                    .triggers
                    .entries(&runtime.resources, owner)
                    .unwrap()
                    .len(),
                0
            );
            assert_eq!(
                inputs.pickup_history.as_deref().unwrap().collected_mask,
                1 << (identity - 1)
            );
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}

#[test]
fn authored_weapon_pickup_fallback_uses_published_level_and_one_shared_random_draw() {
    let catalog = authored_paths::catalog();
    let mut seen = [false; 8];
    for level in 0..=u8::MAX {
        for seed in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, _) = setup();
            let mut random = RandomState::new([1, 0, 0, seed]);
            let mut expected_random = random;
            let (phase, kind, color): (u8, u8, u8) = if level == 3 {
                let draw = expected_random.next_byte() & 7;
                seen[usize::from(draw)] = true;
                match draw {
                    1 => (0, 1, 0),
                    2 => (1, 3, 4),
                    4 => (3, 0, 10),
                    other => (5, other, 12),
                }
            } else {
                (4, 213, 8)
            };
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::WEAPON_UPGRADE_PICKUP);
            actor.base.hit_points = 100;
            actor.extension.path_state.motion_phase = 213 << 8;
            let mut inputs = world(&mut random);
            inputs.scene.encounter_location = Some(5);
            inputs.scene.active_weapon_level = Some(level);
            inputs.surface_mode = Some(SurfaceMode { flags: 1 });
            // No fresh equipment input: eligibility reads only publication.
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                Ok(ControlStep::Movement)
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                actor.extension.path_state.motion_phase,
                u16::from(phase) | u16::from(kind) << 8
            );
            assert_eq!(
                actor.extension.path_state.animation.color.fixed_frame(),
                Some(color)
            );
            assert_eq!(inputs.random, &expected_random);
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
    assert_eq!(seen, [true; 8]);
}

#[test]
fn authored_consumable_full_retry_reinstalls_collection_and_only_success_signals_the_parent() {
    let catalog = authored_paths::catalog();
    for (entry, _, kind, _) in PICKUPS.into_iter().skip(2) {
        for packed in 0..=u8::MAX {
            for same_type in [false, true] {
                for part in [0, 1] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let parent = objects
                        .allocate(Object::new(
                            ObjectKind::Enemy,
                            ShapeId::EMPTY,
                            Behavior::FollowPath,
                        ))
                        .unwrap();
                    let player = objects
                        .allocate(Object::new(
                            ObjectKind::Player,
                            ShapeId::EMPTY,
                            Behavior::PlayerFlight,
                        ))
                        .unwrap();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(entry);
                    actor.base.hit_points = 100;
                    actor.base.yaw = Angle::from_units(3);
                    actor.base.attachment = Some(parent);
                    actor.extension.path_state.part = part;
                    actor.extension.path_state.conditions.hit_event_pending = true;
                    actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
                    let mut history = PickupHistory {
                        collected_mask: 0x8000,
                    };
                    let mut equipment = SelectedEquipment {
                        packed_consumables: packed,
                        consumable_type: if same_type { kind } else { kind ^ 255 },
                        weapon_level: 2,
                    };
                    let mut score = PlayerScore::from_parts(13, 5);
                    let mut audio = AudioState::default();
                    let mut inputs = world(&mut random);
                    inputs.scene.encounter_location = Some(0);
                    inputs.surface_mode = Some(SurfaceMode { flags: 1 });
                    inputs.selected = Some(player);
                    inputs.primary_player = Some(parent);
                    inputs.fixed_players = [Some(player); 2];
                    inputs.pickup_history = Some(&mut history);
                    inputs.selected_equipment = Some(&mut equipment);
                    inputs.selected_score = Some(&mut score);
                    inputs.audio = Some(PathAudio {
                        events: &mut audio,
                        listeners: [CueListener::Other; 2],
                        markers: None,
                    });
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(ControlStep::Movement)
                    );
                    let full = packed & 15 >= 9;
                    let rejected = same_type && full;
                    for attempt in 0..=usize::from(rejected) {
                        assert_eq!(
                            callbacks(
                                &mut runtime,
                                &catalog,
                                &mut objects,
                                owner,
                                &mut inputs,
                                TriggerWorldInputs::default()
                            ),
                            2
                        );
                        assert_eq!(
                            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                            Ok(ControlStep::Movement)
                        );
                        let rejects_now = rejected && attempt == 0;
                        assert_eq!(
                            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 48).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                            Ok(if rejects_now {
                                ControlStep::Movement
                            } else {
                                ControlStep::Ended
                            })
                        );
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.base.flags.remove_after_tick, !rejects_now);
                        assert_eq!(
                            actor
                                .extension
                                .path_state
                                .triggers
                                .entries(&runtime.resources, owner)
                                .unwrap()
                                .len(),
                            if rejects_now { 2 } else { 1 }
                        );
                        assert_eq!(
                            inputs.pickup_history.as_deref().unwrap().collected_mask,
                            if rejects_now { 0x8000 } else { 0x8004 }
                        );
                        assert_eq!(
                            inputs.selected_score.as_deref().unwrap().points(),
                            5 * 65536 + if rejects_now { 13 } else { 113 }
                        );
                        assert_eq!(
                            objects
                                .get(parent)
                                .unwrap()
                                .extension
                                .path_state
                                .conditions
                                .hit_event_pending,
                            !rejects_now && part != 0
                        );
                        assert_eq!(
                            inputs
                                .selected_equipment
                                .as_deref()
                                .unwrap()
                                .consumable_type,
                            kind
                        );
                        let expected_count = if rejected && attempt == 1 {
                            9
                        } else if full {
                            packed & 15
                        } else {
                            (packed & 15) + 1
                        };
                        assert_eq!(
                            inputs
                                .selected_equipment
                                .as_deref()
                                .unwrap()
                                .packed_consumables,
                            packed & 0xF0 | expected_count
                        );
                        let events = inputs
                            .audio
                            .as_mut()
                            .unwrap()
                            .events
                            .take_events()
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>();
                        assert_eq!(
                            events,
                            if rejects_now {
                                vec![]
                            } else {
                                vec![SoundEvent::Authored(AuthoredCue::new(
                                    69,
                                    0,
                                    PlayerTarget::Secondary,
                                ))]
                            }
                        );
                        if rejects_now {
                            // A fresh count is sampled on retry, even though
                            // the actor's main path is already held.
                            inputs
                                .selected_equipment
                                .as_deref_mut()
                                .unwrap()
                                .packed_consumables = packed & 0xF0 | 8;
                        }
                    }
                    runtime.release_actor_programs(&mut objects, owner).unwrap();
                }
            }
        }
    }
}

#[test]
fn authored_pickup_proximity_uses_strict_depth_and_xy_sum_with_three_authored_limits() {
    let catalog = authored_paths::catalog();
    for part in [0, 1, 255] {
        for surface in [0, 1, 8, 255] {
            let limit = if part != 0 {
                70
            } else if surface == 0 {
                500
            } else {
                150
            };
            for (offset, collect) in [
                (
                    Vector3 {
                        x: limit - 1,
                        y: 0,
                        z: 0,
                    },
                    true,
                ),
                (
                    Vector3 {
                        x: limit,
                        y: 0,
                        z: 0,
                    },
                    false,
                ),
                (
                    Vector3 {
                        x: -(limit - 1),
                        y: 0,
                        z: 0,
                    },
                    true,
                ),
                (
                    Vector3 {
                        x: -limit,
                        y: 0,
                        z: 0,
                    },
                    false,
                ),
                (
                    Vector3 {
                        x: 0,
                        y: 0,
                        z: limit - 1,
                    },
                    true,
                ),
                (
                    Vector3 {
                        x: 0,
                        y: 0,
                        z: limit,
                    },
                    false,
                ),
                (
                    Vector3 {
                        x: 0,
                        y: 0,
                        z: -limit,
                    },
                    false,
                ),
                (
                    Vector3 {
                        x: limit / 2,
                        y: limit / 2,
                        z: 0,
                    },
                    false,
                ),
                (
                    Vector3 {
                        x: limit / 2,
                        y: limit / 2 - 1,
                        z: 0,
                    },
                    true,
                ),
            ] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let player = objects
                    .allocate(Object::new(
                        ObjectKind::Player,
                        ShapeId::EMPTY,
                        Behavior::PlayerFlight,
                    ))
                    .unwrap();
                objects.get_mut(player).unwrap().base.position = offset;
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::CONSUMABLE_PICKUP_TYPE_ONE);
                actor.base.hit_points = 100;
                actor.extension.path_state.part = part;
                actor.extension.path_state.conditions.hit_event_pending = true;
                let mut inputs = world(&mut random);
                inputs.scene.encounter_location = Some(0);
                inputs.surface_mode = Some(SurfaceMode { flags: surface });
                inputs.selected = Some(player);
                inputs.primary_player = Some(player);
                inputs.fixed_players = [Some(player); 2];
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(ControlStep::Movement)
                );
                let held = objects.get(owner).unwrap().base.path;
                assert_eq!(
                    callbacks(
                        &mut runtime,
                        &catalog,
                        &mut objects,
                        owner,
                        &mut inputs,
                        TriggerWorldInputs::default()
                    ),
                    2
                );
                assert_eq!(objects.get(owner).unwrap().base.path != held, collect);
                // Force-after-callbacks does not itself award or retire.
                assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }
}

#[test]
fn authored_pickup_timed_lifetime_keeps_callbacks_through_wait_and_blink_loops() {
    let catalog = authored_paths::catalog();
    for health in [1, 2, 3, 10, 255] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let player = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        objects.get_mut(player).unwrap().base.position.z = 1000;
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::CONSUMABLE_PICKUP_TYPE_ONE);
        actor.base.hit_points = health;
        let mut inputs = world(&mut random);
        inputs.scene.encounter_location = Some(5);
        inputs.surface_mode = Some(SurfaceMode { flags: 1 });
        inputs.selected = Some(player);
        inputs.primary_player = Some(player);
        inputs.fixed_players = [Some(player); 2];
        let movement_count = usize::from(health) * 33 - 2;
        for visit in 1..=movement_count + 1 {
            let final_visit = visit == movement_count + 1;
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                Ok(if final_visit {
                    ControlStep::Ended
                } else {
                    ControlStep::Movement
                })
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.flags.remove_after_tick, final_visit);
            let first_blink_visit = usize::from(health) * 31;
            assert_eq!(
                actor.base.flags.visible,
                visit < first_blink_visit || (visit - first_blink_visit) % 2 == 1
            );
            assert!(actor.base.flags.collision_disabled);
            assert_eq!(actor.base.hit_points, health);
            if !final_visit {
                assert_eq!(
                    callbacks(
                        &mut runtime,
                        &catalog,
                        &mut objects,
                        owner,
                        &mut inputs,
                        TriggerWorldInputs::default()
                    ),
                    2
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(
                    actor.extension.path_state.animation.color.fixed_frame(),
                    Some((visit % 2) as u8)
                );
                assert_eq!(
                    actor.extension.path_state.script_value,
                    if visit % 2 == 0 { 1 } else { 65535 }
                );
            }
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }
}

#[test]
fn authored_pickup_part_gate_preserves_saved_position_and_uses_published_height_then_live_visibility_callbacks(
) {
    use super::super::path_motion::PublishedPlayerMotion;
    use super::super::path_trigger_conditions::ControlledAuxFlags;
    use super::super::render::ClippingPlaneSelection;
    let catalog = authored_paths::catalog();
    for part in [0, 1, 255] {
        for location in [0, 5, 255] {
            for (height, initially_visible) in [
                (-32768, false),
                (-1001, false),
                (-1000, false),
                (-999, true),
                (0, true),
                (32767, false),
            ] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let player = objects
                    .allocate(Object::new(
                        ObjectKind::Player,
                        ShapeId::EMPTY,
                        Behavior::PlayerFlight,
                    ))
                    .unwrap();
                objects.get_mut(player).unwrap().base.position = Vector3 {
                    x: 0,
                    y: 0,
                    z: 1000,
                };
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::CONSUMABLE_PICKUP_TYPE_THREE);
                actor.base.hit_points = 100;
                actor.extension.path_state.part = part;
                actor.extension.clipping_plane = ClippingPlaneSelection::from_selector_byte(0xA8);
                let saved = Vector3 {
                    x: -23456,
                    y: 123,
                    z: 32767,
                };
                actor.extension.path_state.platform_carry.saved_position = saved;
                let mut inputs = world(&mut random);
                inputs.scene.encounter_location = Some(location);
                inputs.selected = Some(player);
                inputs.primary_player = Some(player);
                inputs.fixed_players = [Some(player); 2];
                let special_part = part != 0 && location == 5;
                if special_part {
                    inputs.published_motion = Some(PublishedPlayerMotion {
                        position: Vector3 {
                            x: 213,
                            y: height,
                            z: -37,
                        },
                        ..Default::default()
                    });
                }
                if part == 0 {
                    inputs.surface_mode = Some(SurfaceMode { flags: 1 });
                }
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(ControlStep::Movement)
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.extension.path_state.motion_phase, 0x0301);
                assert_eq!(
                    actor.extension.path_state.platform_carry.saved_position,
                    saved
                );
                assert_eq!(actor.extension.path_state.script_value, 1);
                assert_eq!(actor.base.flags.visible, !special_part || initially_visible);
                assert_eq!(
                    actor.extension.clipping_plane.selector_byte(),
                    if special_part {
                        0
                    } else if location == 5 {
                        ClippingPlaneSelection::FIRST.selector_byte()
                    } else if part != 0 {
                        ClippingPlaneSelection::FIRST.selector_byte()
                    } else {
                        0xA8
                    }
                );
                let list = actor
                    .extension
                    .path_state
                    .triggers
                    .entries(&runtime.resources, owner)
                    .unwrap();
                assert_eq!(
                    list.len(),
                    if special_part {
                        3
                    } else if part == 0 {
                        2
                    } else {
                        1
                    }
                );
                assert_eq!(
                    list.iter()
                        .filter(|trigger| trigger.kind == TriggerKind::ControlledAuxFlagHigh)
                        .count(),
                    usize::from(special_part)
                );
                let gate = actor.base.path;
                if part != 0 {
                    // The part gate yields without reading surface mode and
                    // keeps the always callback alive until its hit event.
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(ControlStep::Movement)
                    );
                    assert_eq!(objects.get(owner).unwrap().base.path, gate);
                    objects
                        .get_mut(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .conditions
                        .hit_event_pending = true;
                    inputs.surface_mode = Some(SurfaceMode { flags: 1 });
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(ControlStep::Movement)
                    );
                    assert!(
                        !objects
                            .get(owner)
                            .unwrap()
                            .extension
                            .path_state
                            .conditions
                            .hit_event_pending
                    );
                }
                for visible in [false, true, false, true] {
                    let trigger_inputs = TriggerWorldInputs {
                        controlled_aux: ControlledAuxFlags {
                            high: visible,
                            low: !visible,
                        },
                        ..Default::default()
                    };
                    assert_eq!(
                        callbacks(
                            &mut runtime,
                            &catalog,
                            &mut objects,
                            owner,
                            &mut inputs,
                            trigger_inputs
                        ),
                        if special_part { 3 } else { 2 }
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.base.flags.visible, !special_part || visible);
                    assert_eq!(
                        actor.extension.path_state.platform_carry.saved_position,
                        saved
                    );
                    assert!(actor.base.flags.collision_disabled);
                    assert!(!actor.base.flags.remove_after_tick);
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }
}
