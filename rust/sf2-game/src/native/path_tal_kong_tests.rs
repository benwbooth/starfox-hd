//! Static-source Tal Kong controller, limb and camera-publication scenarios.
use super::super::path_relationships::find_child;
use super::super::path_scene_state::{
    EncounterCameraFocus, EncounterCoordination, EncounterHealthDisplay,
};
use super::super::path_triggers::{Trigger, TriggerKind};
use super::super::{
    authored_paths, Angle, AudioState, Behavior, ObjectKind, ObjectSpawnDefaults, ShapeId, Vector3,
};
use super::paired_patrol_tests::callbacks;
use super::projectile_tests::audio;
use super::tests::{setup, world};
use super::*;

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: super::super::PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn triggers(runtime: &PathRuntime, objects: &ObjectStore, owner: ObjectId) -> Vec<Trigger> {
    objects
        .get(owner)
        .unwrap()
        .extension
        .path_state
        .triggers
        .entries(&runtime.resources, owner)
        .unwrap()
        .to_vec()
}

#[test]
fn camera_focus_publishes_live_owner_words_without_touching_other_state() {
    let catalog = PathCatalog::new(vec![vec![Statement::PublishEncounterCameraFocus {
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let initial = objects.clone();
    let initial_random = random;
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::MissingEncounterCameraFocus)
    );
    assert_eq!(objects, initial);
    assert_eq!(random, initial_random);
    // Every signed word appears on all three axes; unrelated relative/velocity
    // fields and an independently selected actor must not become the focus.
    let selected = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap();
    objects.get_mut(selected).unwrap().base.position = Vector3 {
        x: 17,
        y: 29,
        z: 31,
    };
    for value in 0..=u16::MAX {
        for invert in [false, true] {
            let position = Vector3 {
                x: value as i16,
                y: value.rotate_left(5) as i16,
                z: !value as i16,
            };
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(at(0));
            actor.base.position = position;
            actor.base.velocity = Vector3 {
                x: 73,
                y: -71,
                z: 123,
            };
            actor.extension.relative_position = Vector3 {
                x: -17,
                y: 97,
                z: -317,
            };
            let mut expected = objects.clone();
            expected.get_mut(owner).unwrap().base.path = Some(at(1));
            let mut focus = EncounterCameraFocus::default();
            let mut inputs = world(&mut random);
            inputs.selected = Some(selected);
            inputs.camera_focus = Some(&mut focus);
            runtime.branch.invert_next = invert;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(0),
                    executed: 0
                })
            );
            assert_eq!(
                inputs.camera_focus.as_ref().unwrap().position,
                Vector3::default()
            );
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
            assert_eq!(focus.position, position);
            assert_eq!(objects, expected);
            assert_eq!(runtime.branch.invert_next, invert);
            assert_eq!(random, initial_random);
        }
    }
}

#[test]
fn tal_kong_initialization_preserves_pose_and_installs_asymmetric_limb_controllers() {
    let catalog = authored_paths::catalog();
    for yaw in [0, 64, 128, 192, 255] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::TAL_KONG);
        actor.base.position = Vector3 {
            x: i16::MIN,
            y: 79,
            z: i16::MAX,
        };
        actor.base.yaw = Angle::from_units(yaw);
        actor.base.attack_power = 171;
        let position = actor.base.position;
        let before_random = random;
        let mut display = EncounterHealthDisplay::default();
        let mut inputs = world(&mut random);
        inputs.health_display = Some(&mut display);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.position, position);
        assert_eq!(actor.base.yaw.units(), yaw);
        assert_eq!((actor.base.hit_points, actor.base.attack_power), (40, 171));
        assert_eq!((actor.base.target_speed, actor.base.acceleration), (30, 1));
        assert_eq!(actor.base.shape, ShapeId::from_catalog_index(326));
        assert_eq!(actor.extension.spawn_group, 255);
        assert_eq!(actor.extension.path_state.motion_phase, 0xFC28);
        assert_eq!(actor.extension.auxiliary.impact_materials(&runtime.resources, owner).unwrap().ordinary, Some(3));
        assert_eq!(
            actor.extension.spatial_loop,
            super::super::SpatialLoop::from_authored_control(9)
        );
        assert_eq!(
            inputs.health_display.as_deref().unwrap(),
            &EncounterHealthDisplay {
                current: 40,
                maximum: 40,
                label: Some("TAL KONG")
            }
        );
        assert_eq!(
            triggers(&runtime, &objects, owner)
                .iter()
                .map(|t| t.kind)
                .collect::<Vec<_>>(),
            [
                TriggerKind::ZeroHealth,
                TriggerKind::Always,
                TriggerKind::NewContact
            ]
        );
        assert_eq!(objects.len(), 3);
        for (number, x) in [(3, 208), (4, -208)] {
            let limb = find_child(&objects, owner, number).unwrap().unwrap();
            let actor = objects.get(limb).unwrap();
            assert_eq!(actor.base.kind, ObjectKind::Effect);
            assert_eq!(actor.base.shape, ShapeId::from_catalog_index(325));
            assert_eq!((actor.base.hit_points, actor.base.attack_power), (10, 10));
            assert_eq!(
                actor.extension.relative_position,
                Vector3 { x, y: -140, z: -20 }
            );
            assert_eq!(actor.extension.parent, Some(owner));
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, limb, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let actor = objects.get(limb).unwrap();
            assert!(actor.base.flags.collision_disabled);
            assert_eq!(actor.extension.spawn_group, 255);
            let hand = find_child(&objects, limb, 1).unwrap();
            assert_eq!(hand.is_some(), number == 4);
            if let Some(hand) = hand {
                let actor = objects.get(hand).unwrap();
                assert_eq!(
                    (actor.base.kind, actor.base.shape),
                    (ObjectKind::Enemy, ShapeId::from_catalog_index(323))
                );
                assert_eq!((actor.base.hit_points, actor.base.attack_power), (10, 4));
                assert_eq!(
                    actor.extension.relative_position,
                    Vector3 {
                        x: 208,
                        y: 140,
                        z: -140
                    }
                );
            }
        }
        assert_eq!(objects.len(), 4);
        assert_eq!(random, before_random);
    }
}

#[test]
fn tal_kong_contact_bar_and_timed_signal_use_live_health_without_clamping() {
    let catalog = authored_paths::catalog();
    for health in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::TAL_KONG);
        let mut display = EncounterHealthDisplay::default();
        let mut signals = EncounterSignals { raised: 0xA580 };
        let mut inputs = world(&mut random);
        inputs.health_display = Some(&mut display);
        inputs.encounter_signals = Some(&mut signals);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let contact = triggers(&runtime, &objects, owner)
            .into_iter()
            .find(|t| t.kind == TriggerKind::NewContact)
            .unwrap();
        runtime.clear_triggers(&mut objects, owner).unwrap();
        runtime.add_trigger(&mut objects, owner, contact).unwrap();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.hit_points = health;
        actor.base.contacts.new_contact_latched = true;
        actor.extension.path_state.motion_phase = 0x9713;
        let before_random = *inputs.random;
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        assert_eq!(inputs.health_display.as_ref().unwrap().current, health);
        assert_eq!(inputs.health_display.as_ref().unwrap().maximum, 40);
        assert_eq!(
            objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .motion_phase,
            0x9713
        );
        assert_eq!(inputs.encounter_signals.as_ref().unwrap().raised, 0xA581);
        objects
            .get_mut(owner)
            .unwrap()
            .base
            .contacts
            .new_contact_latched = false;
        // Additions do not extend the active scan; the encoded duration gets
        // its first decrement only on the next callback pass.
        let timer = triggers(&runtime, &objects, owner)
            .into_iter()
            .find(|t| t.kind == TriggerKind::TimerPenultimate)
            .unwrap()
            .timer;
        assert_eq!(timer, 41);
        for tick in 1..=41 {
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut inputs,
                tick,
            );
            assert_eq!(
                inputs.encounter_signals.as_ref().unwrap().raised,
                if tick < 40 { 0xA581 } else { 0xA580 }
            );
        }
        assert_eq!(triggers(&runtime, &objects, owner), [contact]);
        assert_eq!(inputs.random, &before_random);
    }
}

#[test]
fn tal_kong_death_separates_boss_score_from_camera_effect_and_final_progress() {
    use super::super::path_player_control::{PlayerTargetControl, PrimaryControl};
    use super::super::path_score::PlayerScore;
    use super::super::player_hit_control::{PlayerHitControl, PrimaryFeedback};
    let catalog = authored_paths::catalog();
    for mode in [0, 8, 0x108, u16::MAX] {
        for state in [0, 1, 255] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::TAL_KONG);
            actor.base.position = Vector3 {
                x: 157,
                y: -79,
                z: 317,
            };
            actor.base.yaw = Angle::from_units(31);
            let primary = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let selected = objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::PlayerFlight,
                ))
                .unwrap();
            let mut display = EncounterHealthDisplay::default();
            let mut focus = EncounterCameraFocus::default();
            let mut shared = EncounterCoordination {
                progress: 255,
                ..Default::default()
            };
            let mut control = PlayerTargetControl {
                mode,
                ..Default::default()
            };
            let mut hit = PlayerHitControl {
                feedback_duration: 91,
                feedback_flags: 0x81,
                recovery: 199,
                ..Default::default()
            };
            let mut score = PlayerScore::from_parts(65400, 171);
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.health_display = Some(&mut display);
            inputs.camera_focus = Some(&mut focus);
            inputs.coordination = Some(&mut shared);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.primary_player = Some(primary);
            inputs.selected = Some(selected);
            inputs.primary_control = Some(PrimaryControl {
                target: &mut control,
                linked_mode: false,
            });
            inputs.primary_feedback = Some(PrimaryFeedback {
                state,
                hit: &mut hit,
            });
            inputs.selected_score = Some(&mut score);
            inputs.audio = Some(audio(&mut events));
            let mut objective_counts = super::super::path_scene_state::EncounterObjectiveCounts { remaining_word: 0, ..Default::default() };
            inputs.objective_counts = Some(&mut objective_counts);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let death = triggers(&runtime, &objects, owner)
                .into_iter()
                .find(|t| t.kind == TriggerKind::ZeroHealth)
                .unwrap();
            runtime.clear_triggers(&mut objects, owner).unwrap();
            runtime.add_trigger(&mut objects, owner, death).unwrap();
            objects.get_mut(owner).unwrap().base.hit_points = 0;
            let before_random = *inputs.random;
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
            assert_eq!(inputs.health_display.as_ref().unwrap().current, 0);
            assert_eq!(inputs.health_display.as_ref().unwrap().maximum, 40);
            assert_eq!(
                inputs.selected_score.as_ref().unwrap().points(),
                171 * 65536 + 65535
            );
            assert_eq!(inputs.coordination.as_ref().unwrap().progress, 255);
            assert!(triggers(&runtime, &objects, owner).is_empty());
            assert!(objects.get(owner).unwrap().base.flags.collision_disabled);
            let effect = runtime.spawns.last_spawn.unwrap();
            assert_ne!(effect, owner);
            let actor = objects.get(effect).unwrap();
            assert_eq!(
                (actor.base.kind, actor.base.shape),
                (ObjectKind::Effect, ShapeId::from_catalog_index(326))
            );
            assert_eq!((actor.base.hit_points, actor.base.attack_power), (100, 0));
            assert_eq!(
                actor.base.position,
                objects.get(owner).unwrap().base.position
            );
            assert_eq!(actor.base.yaw.units(), 31);
            assert_eq!(objects.len(), 6);
            let mut burst_count = 0;
            let mut published = Vector3::default();
            for visit in 1..=26 {
                let position = Vector3 {
                    x: visit * 37,
                    y: -visit * 7,
                    z: visit * 101,
                };
                objects.get_mut(effect).unwrap().base.position = position;
                if visit > 1 {
                    callbacks(
                        &mut runtime,
                        &catalog,
                        &mut objects,
                        effect,
                        &mut inputs,
                        visit as u8,
                    );
                    if visit % 4 == 0 {
                        burst_count += 1;
                    }
                }
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, effect, &mut inputs, 100)
                        .unwrap()
                        .step,
                    if visit == 26 {
                        ControlStep::MovementTail
                    } else {
                        ControlStep::Movement
                    }
                );
                if visit == 1 || visit == 21 {
                    published = position;
                }
                if visit == 21 || visit == 24 {
                    burst_count += 1;
                }
                assert_eq!(
                    inputs.camera_focus.as_ref().unwrap().position,
                    published,
                    "visit {visit}"
                );
                assert_eq!(objects.len(), 6 + burst_count, "visit {visit}");
                assert_eq!(
                    inputs
                        .primary_feedback
                        .as_ref()
                        .unwrap()
                        .hit
                        .feedback_duration,
                    if mode == 8 && state != 0 && visit >= 21 {
                        4
                    } else {
                        91
                    }
                );
                assert_eq!(
                    inputs.primary_feedback.as_ref().unwrap().hit.feedback_flags,
                    if mode == 8 && state != 0 && visit >= 21 {
                        0xA5
                    } else {
                        0x81
                    }
                );
                assert_eq!(inputs.primary_feedback.as_ref().unwrap().hit.recovery, 199);
                assert_eq!(inputs.coordination.as_ref().unwrap().progress, 255);
                if burst_count != 0 {
                    let burst = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                    assert_eq!(
                        (
                            burst.base.shape,
                            burst.base.hit_points,
                            burst.base.attack_power
                        ),
                        (ShapeId::from_catalog_index(13), 100, 31)
                    );
                    assert!(burst.base.contacts.run_when_paused);
                    assert_eq!(burst.extension.path_state.part, 1);
                }
            }
            assert_eq!(burst_count, 8);
            assert_eq!(objects.get(effect).unwrap().base.hit_points, 0);
            assert!(objects.get(effect).unwrap().base.flags.collision_disabled);
            assert!(objects.get(effect).unwrap().base.contacts.run_when_paused);
            assert!(!objects.get(effect).unwrap().base.flags.remove_after_tick);
            assert_eq!(inputs.primary_control.as_ref().unwrap().target.mode, 2);
            assert_eq!(
                inputs.primary_control.as_ref().unwrap().target.owner,
                Some(effect)
            );
            assert_eq!(inputs.primary_control.as_ref().unwrap().target.range, 2);
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                effect,
                &mut inputs,
                27,
            );
            assert_eq!(inputs.coordination.as_ref().unwrap().progress, 0);
            assert_eq!(objects.len(), 14);
            assert_eq!(inputs.random, &before_random);
            assert_eq!(
                inputs.health_display.as_ref().unwrap().label,
                Some("TAL KONG")
            );
        }
    }
}

#[test]
fn tal_kong_limb_signal_rotates_twenty_five_times_then_releases_only_its_hand() {
    let catalog = authored_paths::catalog();
    for number in [3, 4] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::TAL_KONG);
        let mut display = EncounterHealthDisplay::default();
        let mut inputs = world(&mut random);
        inputs.health_display = Some(&mut display);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let limb = find_child(&objects, owner, number).unwrap().unwrap();
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, limb, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let hand = find_child(&objects, limb, 1).unwrap();
        let count = objects.len();
        let before_random = *inputs.random;
        objects
            .get_mut(limb)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending = true;
        for visit in 1..=24 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, limb, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let actor = objects.get(limb).unwrap();
            assert_eq!(actor.extension.relative_rotation.pitch.units(), visit * 4);
            assert!(!actor.extension.path_state.conditions.hit_event_pending);
            if let Some(hand) = hand {
                assert!(
                    !objects
                        .get(hand)
                        .unwrap()
                        .extension
                        .path_state
                        .conditions
                        .hit_event_pending
                );
            }
        }
        // The final NEXT does not yield. SignalChild and the first chase are
        // part of visit 25, reducing the angle 100 -> 88 before yielding.
        let mut angle = 100_u8;
        let mut visit = 25;
        while angle != 0 {
            let step = (-i16::from(angle)).min(-8) / 8;
            angle = angle.wrapping_add(step as u8);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, limb, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(
                objects
                    .get(limb)
                    .unwrap()
                    .extension
                    .relative_rotation
                    .pitch
                    .units(),
                angle,
                "visit {visit}"
            );
            assert_eq!(objects.len(), count);
            if let Some(hand) = hand {
                assert!(
                    objects
                        .get(hand)
                        .unwrap()
                        .extension
                        .path_state
                        .conditions
                        .hit_event_pending
                );
            }
            visit += 1;
        }
        // WaitChase tests the old byte. One further visit observes zero,
        // restarts the controller and installs the next right-hand actor.
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, limb, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects.len(), count + usize::from(number == 4));
        assert_eq!(inputs.random, &before_random);
    }
}
