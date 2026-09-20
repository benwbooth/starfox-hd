//! Static-source rectangular patrol scenarios; no recorded gameplay inputs.
use super::super::path_player_control::{PlayerTargetControl, PrimaryControl};
use super::super::path_relationships::find_child;
use super::super::path_scene_state::EncounterCoordination;
use super::super::{
    authored_paths, Angle, AudioState, Behavior, ObjectKind, ObjectSpawnDefaults, ShapeId, Vector3,
};
use super::paired_patrol_tests::callbacks;
use super::projectile_tests::audio;
use super::tests::{setup, world};
use super::*;

const ROOTS: [PathCursor; 3] = [
    authored_paths::WIDE_RECTANGULAR_PATROL,
    authored_paths::GATED_RECTANGULAR_PATROL,
    authored_paths::RECTANGULAR_PATROL,
];

fn selected(objects: &mut ObjectStore) -> ObjectId {
    let mut player = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
    player.base.position.z = 1000;
    objects.allocate(player).unwrap()
}

#[test]
fn retired_identity_and_primary_sentinel_end_before_spawn_or_selection_dependencies() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for identity in 1..=8 {
            for mask in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(root);
                actor.base.hit_points = identity;
                let mut shared = EncounterCoordination {
                    retired_actors: mask,
                    progress: if mask & (1 << (identity - 1)) == 0 {
                        254
                    } else {
                        0
                    },
                    ..Default::default()
                };
                let original_random = random;
                let mut inputs = world(&mut random);
                inputs.coordination = Some(&mut shared);
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Ended
                );
                let actor = objects.get(owner).unwrap();
                assert!(actor.base.flags.remove_after_tick);
                assert_eq!(actor.base.hit_points, identity);
                assert_eq!(actor.extension.path_state.script_parameter, identity);
                assert_eq!(objects.len(), 1);
                assert_eq!(inputs.random, &original_random);
            }
        }
    }
}

#[test]
fn secondary_gate_waits_for_exact_sentinel_and_preserves_initialization() {
    let catalog = authored_paths::catalog();
    for secondary in 0..=254 {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(ROOTS[1]);
        actor.base.hit_points = 3;
        actor.base.yaw = Angle::from_units(64);
        let original_random = random;
        let mut shared = EncounterCoordination {
            secondary_progress: secondary,
            ..Default::default()
        };
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        for _ in 0..3 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(objects.len(), 1);
            assert!(actor.base.flags.collision_disabled);
            assert_eq!(actor.base.position.y, -100);
            assert_eq!(actor.base.hit_points, 100);
            assert_eq!(actor.extension.path_state.script_parameter, 3);
            assert_eq!(inputs.random, &original_random);
        }
        inputs
            .coordination
            .as_deref_mut()
            .unwrap()
            .secondary_progress = 255;
        inputs.selected = Some(selected(&mut objects));
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let child = find_child(&objects, owner, 1).unwrap().unwrap();
        let part = objects.get(child).unwrap();
        assert_eq!(part.base.shape, ShapeId::from_catalog_index(512));
        assert_eq!((part.base.hit_points, part.base.attack_power), (100, 4));
        assert_eq!(
            part.extension.relative_position,
            Vector3 {
                x: 0,
                y: -128,
                z: 0
            }
        );
        assert!(!objects.get(owner).unwrap().base.flags.collision_disabled);
    }
}

#[test]
fn rectangular_motion_keeps_source_turn_chase_endpoint_and_same_visit_transitions() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for variant in [0_u8, 1] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(root);
            actor.base.position = Vector3 {
                x: 32760,
                y: i16::from(variant),
                z: -32760,
            };
            actor.base.yaw = Angle::from_units(if variant == 0 { 64 } else { 192 });
            let player = selected(&mut objects);
            let mut shared = EncounterCoordination {
                secondary_progress: 255,
                ..Default::default()
            };
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.selected = Some(player);
            let original_random = *inputs.random;
            let mut position = Vector3 {
                x: 32760,
                y: -100,
                z: -32760,
            };
            let mut relative = if variant == 0 {
                Vector3::default()
            } else {
                Vector3 {
                    x: -1024,
                    y: 0,
                    z: -1024,
                }
            };
            let mut step_x = if variant == 0 { -10_i16 } else { 10 };
            let mut step_z = step_x;
            let mut yaw = if variant == 0 { 64_u8 } else { 192 };
            let mut target = yaw;
            let mut stage = 0;
            let mut turns = 0;
            for visit in 1..=3000 {
                // A turn may finish and start traversal in the same visit;
                // an out-of-range traversal may likewise enter the next turn.
                loop {
                    if stage == 0 || stage == 2 {
                        let delta = target.wrapping_sub(yaw) as i8;
                        let delta = if delta < 0 {
                            delta.min(-8)
                        } else if delta > 0 {
                            delta.max(8)
                        } else {
                            0
                        };
                        yaw = yaw.wrapping_add((delta / 8) as u8);
                        if yaw != target {
                            break;
                        }
                        target = target.wrapping_add(64);
                        turns += 1;
                        stage += 1;
                    } else {
                        let (coordinate, progress, speed, lower) = if stage == 1 {
                            (
                                &mut position.x,
                                &mut relative.x,
                                &mut step_x,
                                if root == ROOTS[0] { -2048 } else { -1024 },
                            )
                        } else {
                            (
                                &mut position.z,
                                &mut relative.z,
                                &mut step_z,
                                if root == ROOTS[0] { -3072 } else { -1024 },
                            )
                        };
                        *coordinate = coordinate.wrapping_add(*speed);
                        *progress = progress.wrapping_add(*speed);
                        if *progress > lower && *progress <= 0 {
                            break;
                        }
                        *speed = speed.wrapping_neg();
                        stage = (stage + 1) % 4;
                    }
                }
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(
                    actor.base.position, position,
                    "root {root:?}, variant {variant}, visit {visit}"
                );
                assert_eq!(actor.extension.relative_position, relative);
                assert_eq!(actor.base.yaw.units(), yaw);
                assert_eq!(actor.extension.relative_rotation.yaw.units(), target);
                assert_eq!(
                    actor.extension.path_state.platform_carry.saved_position,
                    Vector3 {
                        x: step_x,
                        y: 0,
                        z: step_z
                    }
                );
                assert_eq!(inputs.random, &original_random);
                if visit > 1 {
                    callbacks(
                        &mut runtime,
                        &catalog,
                        &mut objects,
                        owner,
                        &mut inputs,
                        visit as u8,
                    );
                }
            }
            assert!(turns >= 8);
            assert_eq!(objects.len(), 3);
        }
    }
}

#[test]
fn contact_speed_updates_keep_signed_byte_thresholds_and_retained_half_steps() {
    let catalog = authored_paths::catalog();
    for health in 0..=u8::MAX {
        for threshold in [0_u8, 5, 95, 127, 128, 255] {
            for speed in [i16::MIN, -11_i16, 11, i16::MAX] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                objects.get_mut(owner).unwrap().base.path = Some(ROOTS[2]);
                let player = selected(&mut objects);
                let mut shared = EncounterCoordination::default();
                let mut inputs = world(&mut random);
                inputs.coordination = Some(&mut shared);
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                inputs.selected = Some(player);
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                let actor = objects.get_mut(owner).unwrap();
                actor.base.hit_points = health;
                actor.base.contacts.new_contact_latched = true;
                actor.extension.path_state.part = threshold;
                actor.extension.path_state.motion_phase = 0xFF00;
                actor.extension.path_state.motion_delta = Vector3 {
                    x: 71,
                    y: 37,
                    z: -91,
                };
                actor.extension.path_state.platform_carry.saved_position = Vector3 {
                    x: speed,
                    y: 517,
                    z: speed.wrapping_neg(),
                };
                let original_path = actor.base.path;
                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
                let actor = objects.get(owner).unwrap();
                assert_eq!(
                    actor.base.path != original_path,
                    70_u8.wrapping_sub(health) & 128 == 0
                );
                let faster = threshold.wrapping_sub(health) & 128 == 0;
                assert_eq!(
                    actor.extension.path_state.part,
                    if faster {
                        threshold.wrapping_sub(5)
                    } else {
                        threshold
                    }
                );
                let half_x = speed / 2;
                let half_z = speed.wrapping_neg() / 2;
                assert_eq!(
                    actor.extension.path_state.motion_delta,
                    if faster {
                        Vector3 {
                            x: half_x,
                            y: 37,
                            z: half_z,
                        }
                    } else {
                        Vector3 {
                            x: 71,
                            y: 37,
                            z: -91,
                        }
                    }
                );
                assert_eq!(
                    actor.extension.path_state.platform_carry.saved_position,
                    Vector3 {
                        x: speed.wrapping_add(if faster { half_x } else { 0 }),
                        y: 517,
                        z: speed
                            .wrapping_neg()
                            .wrapping_add(if faster { half_z } else { 0 })
                    }
                );
                assert_eq!(
                    actor.extension.path_state.motion_phase,
                    if faster { 70 } else { 0xFF46 }
                );
            }
        }
    }
}

#[test]
fn death_halves_the_active_axis_then_signals_unlinks_scores_and_retires_identity() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for identity in 1..=8 {
            for axis in [0, 1, 2, 255] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(root);
                actor.base.hit_points = identity;
                let player = selected(&mut objects);
                let mut shared = EncounterCoordination {
                    progress: 255,
                    secondary_progress: 255,
                    ..Default::default()
                };
                let mut control = PlayerTargetControl::default();
                let mut score = super::super::path_score::PlayerScore::default();
                let mut events = AudioState::default();
                let mut inputs = world(&mut random);
                inputs.coordination = Some(&mut shared);
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                inputs.primary_player = Some(player);
                inputs.selected = Some(player);
                inputs.selected_score = Some(&mut score);
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
                let child = find_child(&objects, owner, 1).unwrap().unwrap();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.hit_points = 70;
                actor.base.contacts.new_contact_latched = true;
                actor.extension.relative_rotation.pitch = Angle::from_units(axis);
                actor.extension.path_state.part = 0;
                actor.extension.path_state.platform_carry.saved_position = Vector3 {
                    x: -101,
                    y: 517,
                    z: 101,
                };
                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
                let mut position = objects.get(owner).unwrap().base.position;
                let mut speed = if axis == 1 { -101_i16 } else { 101 };
                let last_visit = if axis == 1 { 6 } else { 7 };
                let original_random = *inputs.random;
                for visit in 1..=last_visit {
                    if axis != 0 && visit <= 5 {
                        speed /= 2;
                        if axis == 1 {
                            position.x = position.x.wrapping_add(speed);
                        } else {
                            position.z = position.z.wrapping_add(speed);
                        }
                    }
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                            .unwrap()
                            .step,
                        if visit == last_visit {
                            ControlStep::MovementTail
                        } else {
                            ControlStep::Movement
                        }
                    );
                    assert_eq!(objects.get(owner).unwrap().base.position, position);
                    assert_eq!(inputs.random, &original_random);
                }
                assert_eq!(inputs.coordination.as_deref().unwrap().progress, 0);
                assert_eq!(
                    inputs.coordination.as_deref().unwrap().secondary_progress,
                    if root == ROOTS[1] { 0 } else { 255 }
                );
                assert_eq!(
                    inputs.coordination.as_deref().unwrap().retired_actors,
                    1 << (identity - 1)
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.hit_points, 0);
                assert!(actor.base.flags.collision_disabled);
                assert!(!actor.base.flags.remove_after_tick);
                assert_eq!(objects.len(), 5); // owner, player, part, two fade effects
                assert_eq!(objects.get(child).unwrap().base.attachment, None);
                assert_eq!(objects.get(child).unwrap().extension.parent, Some(owner));
                assert!(
                    objects
                        .get(child)
                        .unwrap()
                        .extension
                        .path_state
                        .conditions
                        .hit_event_pending
                );
                assert_eq!(control.owner, Some(owner));
                assert_eq!(control.range, 2);
                assert_eq!(score.points(), 100);
            }
        }
    }
}

#[test]
fn attachment_fires_on_thirty_visit_cooldown_with_authored_spawn_metadata() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(ROOTS[2]);
    let player = selected(&mut objects);
    let mut shared = EncounterCoordination::default();
    let mut events = AudioState::default();
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut shared);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    inputs.selected = Some(player);
    inputs.audio = Some(audio(&mut events));
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    let child = find_child(&objects, owner, 1).unwrap().unwrap();
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    let original_random = *inputs.random;
    for visit in 1..=120 {
        objects.get_mut(child).unwrap().base.yaw = Angle::ZERO;
        callbacks(
            &mut runtime,
            &catalog,
            &mut objects,
            child,
            &mut inputs,
            visit,
        );
        assert_eq!(objects.get(child).unwrap().base.yaw.units(), 254);
        assert_eq!(
            objects.get(child).unwrap().extension.path_state.part,
            30 - (visit - 1) % 30
        );
        assert_eq!(objects.len(), 3 + 2 * usize::from(1 + (visit - 1) / 30));
        assert_eq!(inputs.random, &original_random);
        if (visit - 1) % 30 == 0 {
            let projectile = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
            assert_eq!(projectile.base.shape, ShapeId::from_catalog_index(167));
            assert_eq!(
                projectile.base.path,
                Some(authored_paths::SURFACE_LIMITED_BALLISTIC_EFFECT)
            );
            assert_eq!(
                (projectile.base.hit_points, projectile.base.attack_power),
                (100, 4)
            );
            assert_eq!(projectile.base.yaw.units(), 254);
            let effect = find_child(&objects, child, 9).unwrap().unwrap();
            let effect = objects.get(effect).unwrap();
            assert_eq!(effect.base.shape, ShapeId::from_catalog_index(19));
            assert_eq!(
                effect.extension.relative_position,
                Vector3 {
                    x: 0,
                    y: -20,
                    z: 60
                }
            );
            assert_eq!((effect.base.hit_points, effect.base.attack_power), (1, 1));
        }
    }
}

#[test]
fn signaled_attachment_consumes_event_and_runs_eighteen_wrapping_curve_steps() {
    use super::super::path_relationships::{self, RelationshipCommand};
    const ARC: [i16; 18] = [
        -50, -40, -32, -24, -18, -12, -8, -4, -2, 0, 0, 2, 4, 8, 12, 18, 24, 32,
    ];
    let catalog = authored_paths::catalog();
    for initial_height in [i16::MIN, -1, 0, i16::MAX] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(ROOTS[2]);
        let player = selected(&mut objects);
        let mut shared = EncounterCoordination::default();
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.selected = Some(player);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let child = find_child(&objects, owner, 1).unwrap().unwrap();
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        objects.get_mut(child).unwrap().extension.path_state.part = 5;
        path_relationships::apply(
            &mut objects,
            owner,
            RelationshipCommand::SignalChild { number: 1 },
        )
        .unwrap();
        path_relationships::apply(
            &mut objects,
            owner,
            RelationshipCommand::UnlinkChild { number: 1 },
        )
        .unwrap();
        callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs, 1);
        let actor = objects.get_mut(child).unwrap();
        actor.base.position.y = initial_height;
        actor.base.yaw = Angle::from_units(237);
        actor.base.pitch = Angle::from_units(173);
        actor.extension.path_state.motion_phase = 0xABFF;
        let original_random = *inputs.random;
        let mut height = initial_height;
        for (step, delta) in ARC.into_iter().enumerate() {
            height = height.wrapping_add(delta);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
                    .unwrap()
                    .step,
                if step == 17 {
                    ControlStep::MovementTail
                } else {
                    ControlStep::Movement
                }
            );
            let actor = objects.get(child).unwrap();
            assert_eq!(actor.base.position.y, height);
            assert_eq!(
                actor.base.yaw.units(),
                237_u8.wrapping_add(32_u8.wrapping_mul(step as u8 + 1))
            );
            assert_eq!(
                actor.base.pitch.units(),
                173_u8.wrapping_add(32_u8.wrapping_mul(step as u8 + 1))
            );
            assert_eq!(
                actor.extension.path_state.motion_phase,
                0xAB00 | (step as u16 + 1)
            );
            assert!(!actor.extension.path_state.conditions.hit_event_pending);
            assert!(actor.base.flags.collision_disabled);
            assert!(actor.base.contacts.run_when_paused);
            assert!(!actor.base.flags.remove_after_tick);
            assert!(actor
                .extension
                .path_state
                .triggers
                .entries(&runtime.resources, child)
                .unwrap()
                .is_empty());
            assert_eq!(inputs.random, &original_random);
        }
        assert_eq!(objects.get(child).unwrap().base.hit_points, 0);
    }
}
