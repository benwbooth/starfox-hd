//! Map-installed core defender, driven by source waits and live actor inputs.
use super::super::path_relationships::{find_child, RelationshipError};
use super::super::path_scene_state::EncounterCoordination;
use super::super::render::MaterialSetId;
use super::super::{authored_paths, Behavior, ObjectKind, ObjectSpawnDefaults, ShapeId, Vector3};
use super::paired_patrol_tests::callbacks;
use super::tests::{setup, world};
use super::*;

#[test]
fn defender_waits_for_exact_live_progress_and_keeps_its_single_decorative_head() {
    let catalog = authored_paths::catalog();
    for progress in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::PLANETARY_CORE_DEFENDER);
        actor.base.shape = ShapeId::from_catalog_index(500);
        actor.base.hit_points = progress;
        actor.extension.path_state.motion_phase = 0xAB37;
        let mut shared = EncounterCoordination {
            progress,
            ..Default::default()
        };
        let before_random = random;
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        inputs.scene.player_configuration = Some(9);
        inputs.scene.encounter_location = Some(2);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.hit_points, 100);
        assert_eq!(actor.base.attack_power, 4);
        assert_eq!(actor.base.flags.collision_disabled, progress != 255);
        assert_eq!(
            actor.base.contacts.suppress_contacts_next_epoch,
            progress != 255
        );
        assert_eq!(actor.base.wait_timer, u8::from(progress == 255));
        assert_eq!(
            actor.extension.path_state.motion_phase,
            u16::from(progress) * 256 + 55
        );
        assert_eq!(
            actor.extension.material_set,
            Some(MaterialSetId::from_catalog_token(if progress == 255 {
                33_268
            } else {
                33_140
            }))
        );
        let head = find_child(&objects, owner, 1).unwrap().unwrap();
        assert_eq!(
            objects.get(head).unwrap().base.shape,
            ShapeId::from_catalog_index(499)
        );
        assert_eq!(
            objects.get(head).unwrap().extension.relative_position.y,
            160
        );
        assert_eq!(objects.get(head).unwrap().base.hit_points, 10);
        assert_eq!(objects.get(head).unwrap().base.attack_power, 10);
        assert_eq!(objects.len(), 2);
        // It runs the already-reviewed shape-filtered scenery path itself.
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, head, &mut inputs, 80)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let child = objects.get(head).unwrap();
        assert!(child.base.flags.collision_disabled);
        assert_eq!(child.base.hit_points, 100);
        assert_eq!(child.base.attack_power, 4);
        assert_eq!(
            child.extension.material_set,
            Some(MaterialSetId::from_catalog_token(33_796))
        );
        inputs.coordination.as_deref_mut().unwrap().progress = 255;
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert!(!objects.get(owner).unwrap().base.flags.collision_disabled);
        inputs.coordination.as_deref_mut().unwrap().progress = 0;
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert!(!objects.get(owner).unwrap().base.flags.collision_disabled);
        assert_eq!(objects.len(), 2);
        assert_eq!(random, before_random);
    }
}

#[test]
fn defender_draws_only_after_twenty_waits_and_a_separate_one_visit_yield() {
    let catalog = authored_paths::catalog();
    for seed in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, _) = setup();
        let mut random = RandomState::new([seed, 7, 39, 127]);
        let mut expected_random = random;
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::PLANETARY_CORE_DEFENDER);
        let mut shared = EncounterCoordination {
            progress: 255,
            ..Default::default()
        };
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        inputs.scene.player_configuration = Some(0);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        for elapsed in 1..=20 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(objects.get(owner).unwrap().base.wait_timer, elapsed);
            assert_eq!(*inputs.random, expected_random);
            assert_eq!(objects.len(), 2);
        }
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(*inputs.random, expected_random);
        let mut fired = false;
        for _ in 0..64 {
            let fires = expected_random.next_byte() >= 127;
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(*inputs.random, expected_random);
            assert_eq!(objects.len(), if fires { 3 } else { 2 });
            if fires {
                let beam = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                assert_eq!(beam.base.shape, ShapeId::from_catalog_index(20));
                assert_eq!(beam.base.child_number, 11);
                assert_eq!(beam.base.hit_points, 10);
                assert_eq!(beam.base.attack_power, 10);
                assert_eq!(beam.base.attachment, Some(owner));
                assert_eq!(
                    beam.extension.relative_position,
                    Vector3 {
                        x: -100,
                        y: 0,
                        z: 100
                    }
                );
                fired = true;
                break;
            }
        }
        assert!(fired);
    }
}

#[test]
fn defender_health_callback_requires_player_attribution_and_keeps_wrapped_signed_threshold() {
    let catalog = authored_paths::catalog();
    for health in 0..=u8::MAX {
        for contacted in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::PLANETARY_CORE_DEFENDER);
            let mut shared = EncounterCoordination {
                progress: 255,
                ..Default::default()
            };
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.scene.player_configuration = Some(0);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let old_path = objects.get(owner).unwrap().base.path;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.hit_points = health;
            actor.base.contacts.new_contact_latched = true;
            actor.base.contacts.hit_by_primary = contacted;
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                actor.base.path != old_path,
                contacted && 95_u8.wrapping_sub(health) < 128
            );
            assert_eq!(
                actor.extension.path_state.motion_phase,
                if contacted { 0xFF5F } else { 0xFF00 }
            );
            assert_eq!(actor.base.hit_points, health);
        }
    }
}

#[test]
fn defender_death_publishes_only_nearest_controller_high_byte_and_reports_missing_beam() {
    let catalog = authored_paths::catalog();
    for phase in 0..=u8::MAX {
        for has_beam in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::PLANETARY_CORE_DEFENDER);
            actor.base.shape = ShapeId::from_catalog_index(500);
            let mut target = Object::new(
                ObjectKind::Effect,
                ShapeId::from_catalog_index(497),
                Behavior::FollowPath,
            );
            target.base.position.x = 10;
            target.extension.path_state.motion_phase = u16::from(phase) * 256 + 55;
            let nearest = objects.allocate(target.clone()).unwrap();
            target.base.position.x = 1000;
            let farther = objects.allocate(target).unwrap();
            let mut shared = EncounterCoordination {
                progress: 255,
                ..Default::default()
            };
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.scene.player_configuration = Some(0);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let head = runtime.spawns.last_spawn.unwrap();
            if has_beam {
                for _ in 0..100 {
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                    if runtime.spawns.last_spawn != Some(head) {
                        break;
                    }
                }
                assert_ne!(runtime.spawns.last_spawn, Some(head));
            }
            let beam = if has_beam {
                runtime.spawns.last_spawn
            } else {
                None
            };
            objects.get_mut(owner).unwrap().base.hit_points = 95;
            objects.get_mut(owner).unwrap().base.contacts.hit_by_primary = true;
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
            let outcome = runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100);
            if has_beam {
                assert_eq!(outcome.unwrap().step, ControlStep::Movement);
                assert!(
                    objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .hold_latched
                );
                assert!(
                    objects
                        .get(beam.unwrap())
                        .unwrap()
                        .base
                        .flags
                        .remove_after_tick
                );
            } else {
                assert_eq!(
                    outcome,
                    Err(ProgramError::Relationship(
                        RelationshipError::MissingChild { owner, number: 11 }
                    ))
                );
                assert!(
                    !objects
                        .get(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .hold_latched
                );
                let before = objects.clone();
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 100),
                    outcome
                );
                assert_eq!(objects, before); // do not repeat score/progress/spawn prefix
            }
            assert_eq!(
                objects
                    .get(nearest)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase,
                u16::from(phase.wrapping_add(1)) * 256 + 55
            );
            assert_eq!(
                objects
                    .get(farther)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase,
                u16::from(phase) * 256 + 55
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.shape, ShapeId::from_catalog_index(501));
            assert_eq!(actor.base.attachment, Some(nearest));
            assert!(actor.base.flags.collision_disabled);
            assert!(actor
                .extension
                .path_state
                .triggers
                .entries(&runtime.resources, owner)
                .unwrap()
                .is_empty());
            assert!(!objects.get(head).unwrap().base.flags.remove_after_tick);
            let clone = runtime.spawns.last_spawn.unwrap();
            assert_eq!(
                objects.get(clone).unwrap().base.shape,
                ShapeId::from_catalog_index(500)
            );
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, clone, &mut inputs, 1)
                    .unwrap()
                    .step,
                ControlStep::MovementTail
            );
            assert_eq!(objects.get(clone).unwrap().base.hit_points, 0);
        }
    }
}

#[test]
fn two_or_four_live_defender_deaths_open_the_actual_controller_shield_gate() {
    use super::super::path_scene_state::EncounterHandoff;
    let catalog = authored_paths::catalog();
    for count in [2_u8, 4] {
        let (mut runtime, mut objects, controller, mut random) = setup();
        let actor = objects.get_mut(controller).unwrap();
        actor.base.path = Some(authored_paths::PLANETARY_CORE_OBJECTIVE);
        actor.base.shape = ShapeId::from_catalog_index(497);
        actor.base.hit_points = count;
        actor.extension.path_state.motion_phase = 0;
        let mut handoff = EncounterHandoff::default();
        let mut shared = EncounterCoordination {
            progress: 255,
            ..Default::default()
        };
        let mut inputs = world(&mut random);
        inputs.handoff = Some(&mut handoff);
        inputs.coordination = Some(&mut shared);
        inputs.scene.player_configuration = Some(0);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, controller, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let shield = find_child(&objects, controller, 2).unwrap().unwrap();
        for completed in 1..=count {
            let mut actor = Object::new(
                ObjectKind::Enemy,
                ShapeId::from_catalog_index(500),
                Behavior::FollowPath,
            );
            actor.base.path = Some(authored_paths::PLANETARY_CORE_DEFENDER);
            let defender = objects.allocate(actor).unwrap();
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, defender, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            for _ in 0..100 {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, defender, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                if find_child(&objects, defender, 11).unwrap().is_some() {
                    break;
                }
            }
            assert!(find_child(&objects, defender, 11).unwrap().is_some());
            objects.get_mut(defender).unwrap().base.hit_points = 95;
            objects
                .get_mut(defender)
                .unwrap()
                .base
                .contacts
                .hit_by_primary = true;
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                defender,
                &mut inputs,
                1,
            );
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, defender, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(
                objects
                    .get(controller)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase,
                u16::from(completed) * 256
            );
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, controller, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(
                objects.get(controller).unwrap().base.wait_timer,
                u8::from(completed == count)
            );
            assert_eq!(
                objects
                    .get(shield)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .hit_event_pending,
                completed == count
            );
            assert_eq!(
                objects.get(defender).unwrap().base.attachment,
                Some(controller)
            );
        }
    }
}
