//! Source-defined popup turret states, gates and discharge lifetimes.
use super::super::path_scene_state::EncounterCoordination;
use super::super::{
    authored_paths, Angle, AudioState, Behavior, ObjectKind, ObjectSpawnDefaults, PathId, ShapeId,
    Vector3,
};
use super::paired_patrol_tests::callbacks;
use super::projectile_tests::audio;
use super::tests::{setup, world};
use super::*;

const ROOTS: [PathCursor; 2] = [
    authored_paths::GATED_POPUP_TURRET,
    authored_paths::POPUP_TURRET,
];
const OFFSETS: [(i16, i16); 8] = [
    (-768, 768),
    (768, 768),
    (0, 512),
    (-512, 0),
    (512, 0),
    (0, -512),
    (-768, -768),
    (768, -768),
];

fn cursor(command_index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index,
    }
}

fn selected(objects: &mut ObjectStore) -> ObjectId {
    let mut player = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
    player.base.position.z = 1000;
    objects.allocate(player).unwrap()
}

fn discharge() -> (
    PathRuntime,
    ObjectStore,
    ObjectId,
    ObjectId,
    ObjectId,
    RandomState,
) {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(ROOTS[1]);
    let player = selected(&mut objects);
    let mut shared = EncounterCoordination::default();
    let mut events = AudioState::default();
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut shared);
    inputs.scene.encounter_location = Some(0);
    let mut objective_counts = super::super::path_scene_state::EncounterObjectiveCounts { remaining_word: 1, ..Default::default() };
    inputs.objective_counts = Some(&mut objective_counts);
    inputs.selected = Some(player);
    inputs.audio = Some(audio(&mut events));
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    for _ in 0..200 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        if let Some(child) = runtime.spawns.last_spawn {
            return (runtime, objects, owner, child, player, random);
        }
    }
    panic!("source turret did not emit its first discharge");
}

#[test]
fn stationary_variants_draw_only_at_the_end_of_each_thirty_visit_wait() {
    let catalog = authored_paths::catalog();
    for variant in [3_u8, 4, 5] {
        for seed in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, _) = setup();
            let mut random = RandomState::new([seed, 7, 39, 127]);
            let mut expected_random = random;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(ROOTS[1]);
            actor.extension.relative_rotation.pitch = Angle::from_units(variant - 1);
            let mut shared = EncounterCoordination {
                secondary_progress: 255,
                ..Default::default()
            };
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.scene.encounter_location = Some(0);
            inputs.audio = Some(audio(&mut events));
            let mut emerged = false;
            for visit in 1..=601 {
                if visit > 1 && (visit - 1) % 30 == 0 {
                    emerged = expected_random.next_byte() >= 127;
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
                    actor.base.position,
                    Vector3 {
                        x: 0,
                        y: if emerged { 240 } else { 280 },
                        z: 0
                    }
                );
                assert_eq!(actor.base.flags.visible, emerged);
                assert_eq!(inputs.random, &expected_random);
                if emerged {
                    break;
                }
            }
            assert!(emerged);
        }
    }
}

#[test]
fn damage_routes_keep_variant_specific_scores_retirement_and_progression() {
    use super::super::path_score::PlayerScore;
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for initial_variant in [0_u8, 1, 2, 3, 4, 5, 255] {
            for identity in 1..=8 {
                for health in [0, 49, 50, 51, 100, 178, 179, 255] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.path = Some(root);
                    actor.base.hit_points = identity;
                    actor.extension.relative_rotation.pitch = Angle::from_units(initial_variant);
                    let variant = if root == ROOTS[0] {
                        2
                    } else {
                        initial_variant.wrapping_add(1)
                    };
                    let player = selected(&mut objects);
                    let mut shared = EncounterCoordination {
                        secondary_progress: 255,
                        progress: 255,
                        ..Default::default()
                    };
                    let mut events = AudioState::default();
                    let initial_points = if identity & 1 == 0 { 65400_u16 } else { 0 };
                    let mut score = PlayerScore::from_parts(initial_points, 171);
                    let mut inputs = world(&mut random);
                    inputs.coordination = Some(&mut shared);
                    inputs.scene.encounter_location = Some(0);
                    let mut objective_counts = super::super::path_scene_state::EncounterObjectiveCounts { remaining_word: 0, ..Default::default() };
                    inputs.objective_counts = Some(&mut objective_counts);
                    inputs.selected = Some(player);
                    inputs.selected_score = Some(&mut score);
                    inputs.audio = Some(audio(&mut events));
                    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
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
                    let before = actor.base.path;
                    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
                    let dies = 50_u8.wrapping_sub(health) & 128 == 0;
                    assert_eq!(objects.get(owner).unwrap().base.path != before, dies);
                    if !dies {
                        continue;
                    }
                    let mut expected_random = *inputs.random;
                    if !matches!(variant, 1 | 5) {
                        expected_random.next_byte();
                    }
                    let original_y = objects.get(owner).unwrap().base.position.y;
                    for visit in 1..=4 {
                        assert_eq!(
                            runtime
                                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                                .unwrap()
                                .step,
                            if visit == 4 {
                                ControlStep::MovementTail
                            } else {
                                ControlStep::Movement
                            }
                        );
                        let fade = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                        assert_eq!(fade.extension.path_state.part, 2);
                        assert_eq!(fade.base.position.y, original_y.wrapping_sub(120));
                        assert_eq!((fade.base.hit_points, fade.base.attack_power), (100, 32));
                        assert!(fade.base.contacts.run_when_paused);
                        assert_eq!(objects.get(owner).unwrap().base.position.y, original_y);
                    }
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.base.hit_points, 0);
                    assert!(actor.base.flags.collision_disabled);
                    assert!(!actor.base.flags.remove_after_tick);
                    assert_eq!(
                        inputs.coordination.as_deref().unwrap().retired_actors,
                        if matches!(variant, 3 | 4 | 5) {
                            0
                        } else {
                            1 << (identity - 1)
                        }
                    );
                    assert_eq!(
                        inputs.coordination.as_deref().unwrap().progress,
                        if matches!(variant, 1 | 5) { 0 } else { 255 }
                    );
                    assert_eq!(
                        inputs.coordination.as_deref().unwrap().secondary_progress,
                        if matches!(variant, 0 | 1 | 3 | 5) {
                            255
                        } else {
                            0
                        }
                    );
                    assert_eq!(inputs.random, &expected_random);
                    assert_eq!(
                        score.points(),
                        171 * 65536
                            + u32::from(
                                initial_points.saturating_add(if matches!(variant, 1 | 5) {
                                    500
                                } else {
                                    300
                                })
                            )
                    );
                    assert_eq!(objects.len(), 6);
                }
            }
        }
    }
}

#[test]
fn discharge_recaptures_no_position_and_fires_exactly_once_after_ten_growth_steps() {
    use super::super::weapon_dispatch::WeaponState;
    let catalog = authored_paths::catalog();
    for mode in [0, 16, 31, 32, 255] {
        let (mut runtime, mut objects, _, child, player, mut random) = discharge();
        let captured = Vector3 {
            x: i16::MIN,
            y: 173,
            z: i16::MAX,
        };
        objects.get_mut(child).unwrap().base.position = captured;
        objects
            .get_mut(child)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xAB00;
        let mut events = AudioState::default();
        let mut weapons = WeaponState::default();
        let mut inputs = world(&mut random);
        inputs.selected = Some(player);
        inputs.primary_player = Some(player);
        inputs.primary_motion = Some(PrimaryMotionInput {
            auxiliary_mode: mode,
            displacement: Vector3::default(),
        });
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.weapons = Some(&mut weapons);
        inputs.audio = Some(audio(&mut events));
        let original_random = *inputs.random;
        for visit in 1..=10 {
            let target = Vector3 {
                x: (visit as i16).wrapping_mul(1000),
                y: -517,
                z: -173,
            };
            objects.get_mut(player).unwrap().base.position = target;
            if visit > 1 {
                objects.get_mut(child).unwrap().base.position = Vector3 {
                    x: 97,
                    y: -103,
                    z: 107,
                };
                callbacks(
                    &mut runtime,
                    &catalog,
                    &mut objects,
                    child,
                    &mut inputs,
                    visit,
                );
            }
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
                    .unwrap()
                    .step,
                if visit == 10 {
                    ControlStep::Ended
                } else {
                    ControlStep::Movement
                }
            );
            let actor = objects.get(child).unwrap();
            assert_eq!(actor.extension.path_state.motion_delta, captured);
            assert_eq!(
                actor.base.position,
                super::super::path_math::change_radius(captured, target, 30)
            );
            assert_eq!(actor.extension.texture_scroll_x, 8 + 2 * visit);
            assert!(actor.base.flags.collision_disabled);
            assert!(actor.base.flags.scaled_sprite);
            assert_eq!(actor.base.hit_points, 10);
            assert_eq!(actor.base.flags.remove_after_tick, visit == 10);
            if visit < 10 {
                assert_eq!(inputs.random, &original_random);
            }
        }
        assert_eq!(objects.len(), 4);
        let shot = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
        assert_eq!(
            shot.base.path,
            Some(authored_paths::OCCUPANCY_SURFACE_LIMITED)
        );
        assert_eq!(shot.base.speed, if mode & 0xF0 == 0x10 { 40 } else { 60 });
    }
}

#[test]
fn gated_entry_skips_primary_sentinel_but_both_entries_observe_retired_identity() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for identity in 1..=8 {
            for mask in 0..=u8::MAX {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(root);
                actor.base.hit_points = identity;
                actor.extension.relative_rotation.pitch = Angle::from_units(1);
                let original_random = random;
                let mut shared = EncounterCoordination {
                    progress: 254,
                    retired_actors: mask,
                    ..Default::default()
                };
                let mut inputs = world(&mut random);
                inputs.coordination = Some(&mut shared);
                let ended = root == ROOTS[1] || mask & (1 << (identity - 1)) != 0;
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    if ended {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.flags.remove_after_tick, ended);
                assert_eq!(actor.base.hit_points, if ended { identity } else { 100 });
                assert_eq!(actor.extension.path_state.script_parameter, identity);
                assert_eq!(inputs.random, &original_random);
            }
        }
    }
}

#[test]
fn dying_turret_signals_found_discharge_with_wrapping_low_byte_and_retains_parent_context() {
    let catalog = authored_paths::catalog();
    for low in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, child, player, mut random) = discharge();
        let mut shared = EncounterCoordination::default();
        let mut events = AudioState::default();
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        inputs.selected = Some(player);
        let mut objective_counts = super::super::path_scene_state::EncounterObjectiveCounts { remaining_word: 0, ..Default::default() };
        inputs.objective_counts = Some(&mut objective_counts);
        inputs.audio = Some(audio(&mut events));
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let actor = objects.get_mut(owner).unwrap();
        actor.base.hit_points = 50;
        actor.base.contacts.new_contact_latched = true;
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        objects
            .get_mut(child)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xAB00 | u16::from(low);
        let before = objects.get(child).unwrap().clone();
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let mut expected = before;
        expected.extension.path_state.motion_phase = 0xAB00 | u16::from(low.wrapping_add(1));
        // The parent's newly allocated fade is inserted immediately before
        // this discharge in the shared object list.
        expected.base.previous = runtime.spawns.last_spawn;
        assert_eq!(objects.get(child).unwrap(), &expected);
        assert_eq!(objects.get(owner).unwrap().base.hit_points, 50);
        assert_eq!(objects.get(owner).unwrap().base.attachment, Some(child));
        callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs, 2);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
                .unwrap()
                .step,
            if low == 255 {
                ControlStep::Movement
            } else {
                ControlStep::Ended
            }
        );
        assert_eq!(objects.len(), 4); // no new weapon, one parent fade
    }
}

#[test]
fn nonzero_discharge_signal_aborts_without_launch_or_health_death() {
    let catalog = authored_paths::catalog();
    for signal in 1..=u8::MAX {
        let (mut runtime, mut objects, _, child, player, mut random) = discharge();
        let mut events = AudioState::default();
        let mut inputs = world(&mut random);
        inputs.selected = Some(player);
        inputs.audio = Some(audio(&mut events));
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        objects
            .get_mut(child)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xAB00 | u16::from(signal);
        callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs, 1);
        let original_random = *inputs.random;
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Ended
        );
        assert_eq!(objects.len(), 3);
        assert_eq!(objects.get(child).unwrap().base.hit_points, 10);
        assert!(objects.get(child).unwrap().base.flags.remove_after_tick);
        assert_eq!(inputs.random, &original_random);
    }
}

#[test]
fn destination_fold_keeps_all_working_writes_and_one_draw_for_every_byte_outcome() {
    let catalog = PathCatalog::new(vec![vec![Statement::ChoosePatrolDestination {
        offsets: &OFFSETS,
        next: cursor(1),
    }]])
    .unwrap();
    let mut seen = [false; 256];
    for first in [0, 1] {
        for last in 0..=u8::MAX {
            for inverted in [false, true] {
                for origin in [i16::MIN, -1, 0, i16::MAX] {
                    let (mut runtime, mut objects, owner, _) = setup();
                    let mut random = RandomState::new([first, 0, 0, last]);
                    let mut expected_random = random;
                    let draw = expected_random.next_byte();
                    seen[usize::from(draw)] = true;
                    runtime.branch.invert_next = inverted;
                    let actor = objects.get_mut(owner).unwrap();
                    actor.extension.relative_position = Vector3 {
                        x: origin,
                        y: 517,
                        z: origin.wrapping_neg(),
                    };
                    actor.extension.path_state.motion_phase = 0xABEF;
                    actor.extension.path_state.script_value = 997;
                    actor.extension.path_state.platform_carry.saved_position = Vector3 {
                        x: 17,
                        y: -937,
                        z: -31,
                    };
                    let mut expected = objects.clone();
                    assert_eq!(
                        runtime.resume_program(
                            &catalog,
                            &mut objects,
                            owner,
                            &mut world(&mut random),
                            0
                        ),
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(0),
                            executed: 0
                        })
                    );
                    assert_eq!(objects, expected);
                    assert_eq!(
                        runtime.resume_program(
                            &catalog,
                            &mut objects,
                            owner,
                            &mut world(&mut random),
                            1
                        ),
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(1),
                            executed: 1
                        })
                    );
                    let (x, z) = OFFSETS[usize::from(draw & 7)];
                    let actor = expected.get_mut(owner).unwrap();
                    actor.extension.path_state.motion_phase = 0xAB00 | u16::from(draw & 7);
                    actor.extension.path_state.script_value = z as u16;
                    actor.extension.path_state.platform_carry.saved_position.x =
                        origin.wrapping_add(x);
                    actor.extension.path_state.platform_carry.saved_position.z =
                        origin.wrapping_neg().wrapping_add(z);
                    actor.base.path = Some(cursor(1));
                    assert_eq!(objects, expected);
                    assert_eq!(random, expected_random);
                    assert_eq!(runtime.branch.invert_next, inverted);
                }
            }
        }
    }
    assert!(seen.into_iter().all(|value| value));
}

#[test]
fn location_and_layout_gate_waits_only_for_the_exact_pair_and_shared_message_signal() {
    let catalog = authored_paths::catalog();
    for location in 0..=u8::MAX {
        for layout in [0, 3, 14, 15, 16, 255] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.path = Some(ROOTS[1]);
            let original_random = random;
            let mut expected_random = random;
            let mut shared = EncounterCoordination::default();
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.scene.encounter_location = Some(location);
            if location == 8 {
                inputs.scene.encounter_layout = Some(layout);
            }
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let waiting = location == 8 && layout == 15;
            if waiting {
                assert_eq!(inputs.random, &original_random);
                assert_eq!(
                    objects.get(owner).unwrap().base.position,
                    Vector3 { x: 0, y: 280, z: 0 }
                );
                for _ in 0..3 {
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                    assert_eq!(inputs.random, &original_random);
                }
                inputs.coordination.as_deref_mut().unwrap().active_messages = 128;
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
            }
            expected_random.next_byte();
            assert_eq!(inputs.random, &expected_random);
            assert_eq!(objects.len(), 1);
            assert!(!objects.get(owner).unwrap().base.flags.visible);
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .auxiliary.impact_materials(&runtime.resources, owner).unwrap()
                    .ordinary,
                Some(2)
            );
        }
    }
}

#[test]
fn complete_emergence_three_shots_and_retreat_follow_source_wait_and_next_cadence() {
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for seed in [0, 1, 17, 31, 64, 127, 128, 255] {
            let (mut runtime, mut objects, owner, _) = setup();
            let mut random = RandomState::new([seed, 7, 39, 127]);
            let mut expected_random = random;
            let choice = expected_random.next_byte() & 7;
            let (x, z) = OFFSETS[usize::from(choice)];
            objects.get_mut(owner).unwrap().base.path = Some(root);
            let player = selected(&mut objects);
            let mut shared = EncounterCoordination {
                secondary_progress: 255,
                ..Default::default()
            };
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.scene.encounter_location = Some(0);
            let mut objective_counts = super::super::path_scene_state::EncounterObjectiveCounts { remaining_word: 1, ..Default::default() };
            inputs.objective_counts = Some(&mut objective_counts);
            inputs.selected = Some(player);
            inputs.audio = Some(audio(&mut events));
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            let mut position = Vector3 { x: 0, y: 280, z: 0 };
            let mut chase = Vec::new();
            loop {
                for (value, target) in [(&mut position.x, x), (&mut position.z, z)] {
                    let delta = target.wrapping_sub(*value);
                    let delta = if delta < 0 {
                        delta.min(-8)
                    } else if delta > 0 {
                        delta.max(8)
                    } else {
                        0
                    };
                    *value = value.wrapping_add(delta / 8);
                }
                chase.push(position);
                if position.x == x && position.z == z {
                    break;
                }
            }
            let emerge = chase.len() + 1;
            let mut spawned = Vec::new();
            for visit in 1..=emerge + 139 {
                let before = runtime.spawns.last_spawn;
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                let actor = objects.get(owner).unwrap();
                let mut expected = if visit <= chase.len() {
                    chase[visit - 1]
                } else {
                    Vector3 { x, y: 0, z }
                };
                if visit >= emerge && visit < emerge + 6 {
                    expected.y = 280 - 40 * (visit - emerge + 1) as i16;
                }
                if visit >= emerge + 104 {
                    expected.y = 40 * (visit - (emerge + 104) + 1).min(7) as i16;
                }
                assert_eq!(
                    actor.base.position, expected,
                    "root {root:?}, seed {seed}, visit {visit}, emerge {emerge}"
                );
                assert_eq!(
                    actor.base.flags.visible,
                    visit >= emerge && visit < emerge + 110
                );
                assert_eq!(inputs.random, &expected_random);
                if runtime.spawns.last_spawn != before {
                    spawned.push(visit);
                    let discharge = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                    assert_eq!(discharge.base.shape, ShapeId::from_catalog_index(20));
                    assert_eq!(
                        (discharge.base.hit_points, discharge.base.attack_power),
                        (10, 10)
                    );
                    assert_eq!(discharge.base.position, Vector3 { x, y: -240, z });
                }
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
            assert_eq!(spawned, vec![emerge + 34, emerge + 55, emerge + 76]);
            assert_eq!(objects.len(), 5);
        }
    }
}
