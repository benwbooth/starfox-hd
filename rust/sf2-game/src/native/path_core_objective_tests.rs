//! Planetary core scenarios from the source catalog, without recorded timing.
use super::super::path_control::TriggerPeriod;
use super::super::path_scene_state::{EncounterHandoff, EncounterObjectiveCounts};
use super::super::path_triggers::TriggerKind;
use super::super::render::MaterialSetId;
use super::super::{authored_paths, ObjectSpawnDefaults, PathId, ShapeId, Vector3};
use super::paired_patrol_tests::callbacks;
use super::tests::{setup, world};
use super::*;

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn core_path(catalog: &PathCatalog) -> PathCursor {
    (0..authored_paths::LOWERED_COMMAND_COUNT)
        .find_map(|index| match catalog.statement(at(index as u16)).unwrap() {
            Statement::SpawnChild { parameters, .. }
                if parameters.shape == ShapeId::from_catalog_index(428) =>
            {
                parameters.path
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn material_identity_comparison_consumes_ifnot_and_never_changes_actor_materials() {
    let material = MaterialSetId::from_catalog_token(33_534);
    let catalog = PathCatalog::new(vec![vec![Statement::Compare {
        condition: ActorCondition::EqualMaterial(material),
        taken: at(2),
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    for candidate in std::iter::once(None)
        .chain((0..=u16::MAX).map(|n| Some(MaterialSetId::from_catalog_token(n))))
    {
        for invert in [false, true] {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(at(0));
            actor.extension.material_set = candidate;
            actor.base.wait_timer = 19;
            runtime.branch.invert_next = invert;
            let mut expected = objects.clone();
            let target = at(if (candidate == Some(material)) != invert {
                2
            } else {
                1
            });
            expected.get_mut(owner).unwrap().base.path = Some(target);
            let before_random = random;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: target,
                    executed: 1
                })
            );
            assert_eq!(objects, expected);
            assert!(!runtime.branch.invert_next);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn controller_retains_initial_count_before_common_setup_and_uses_wrapped_signed_progress() {
    let catalog = authored_paths::catalog();
    for count in 0..=u8::MAX {
        for progress in [
            0,
            count.wrapping_sub(1),
            count,
            count.wrapping_add(1),
            127,
            128,
            255,
        ] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::PLANETARY_CORE_OBJECTIVE);
            actor.base.hit_points = count;
            actor.base.attack_power = 10;
            actor.base.position = Vector3 {
                x: -32700,
                y: 20,
                z: 32700,
            };
            actor.extension.path_state.motion_phase = u16::from(progress) * 256 + 37;
            let mut handoff = EncounterHandoff {
                heading_word: 0xA579,
                player_flags: 3,
                ..Default::default()
            };
            let before_random = random;
            let mut inputs = world(&mut random);
            inputs.handoff = Some(&mut handoff);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 80)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let expected_open = count.wrapping_sub(1).wrapping_sub(progress) >= 128;
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                actor.extension.path_state.script_parameter,
                count.wrapping_sub(1)
            );
            assert_eq!(actor.base.hit_points, 100);
            assert_eq!(actor.base.attack_power, 4);
            assert_eq!(
                actor.extension.path_state.motion_phase,
                u16::from(progress) * 256 + 37
            );
            assert_eq!(actor.base.wait_timer, u8::from(expected_open));
            assert!(actor.base.flags.collision_disabled);
            assert!(actor.base.contacts.suppress_contacts_next_epoch);
            let children: Vec<_> = objects
                .active_objects()
                .filter(|(id, _)| *id != owner)
                .collect();
            assert_eq!(children.len(), 2);
            for (_, child) in children {
                assert_eq!(child.base.attachment, Some(owner));
                let core = child.base.shape == ShapeId::from_catalog_index(428);
                assert_eq!(
                    child.extension.path_state.conditions.hit_event_pending,
                    !core && expected_open
                );
                assert_eq!(child.base.hit_points, if core { 125 } else { 100 });
                assert_eq!(child.base.attack_power, 4);
                assert_eq!(
                    child.extension.relative_position.y,
                    if core { -160 } else { 0 }
                );
            }
            assert_eq!(handoff.x, -32700);
            assert_eq!(handoff.z, 32700);
            assert_eq!(handoff.heading_word, 0xA579);
            assert_eq!(handoff.player_flags, 3);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn controller_releases_shield_then_core_and_emits_four_numbered_beams_until_live_count_zero() {
    use super::super::{AudioState, Behavior, ObjectKind};
    use super::projectile_tests::audio;
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.path = Some(authored_paths::PLANETARY_CORE_OBJECTIVE);
    actor.base.hit_points = 2;
    actor.extension.path_state.motion_phase = 0x0257;
    let mut handoff = EncounterHandoff::default();
    let mut counts = EncounterObjectiveCounts {
        remaining_word: 0xAB01,
        node_record: 0x72,
    };
    let mut events = AudioState::default();
    let player = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap();
    let mut inputs = world(&mut random);
    inputs.handoff = Some(&mut handoff);
    inputs.objective_counts = Some(&mut counts);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    inputs.audio = Some(audio(&mut events));
    inputs.primary_player = Some(player);
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    let core = objects
        .active_objects()
        .find(|(_, a)| a.base.shape == ShapeId::from_catalog_index(428))
        .unwrap()
        .0;
    let shield = objects
        .active_objects()
        .find(|(_, a)| a.base.shape == ShapeId::from_catalog_index(498))
        .unwrap()
        .0;
    assert!(
        objects
            .get(shield)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending
    );
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, shield, &mut inputs, 40)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    callbacks(&mut runtime, &catalog, &mut objects, shield, &mut inputs, 1);
    assert!(
        !objects
            .get(shield)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending
    );
    for elapsed in 1..=15 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, shield, &mut inputs, 40)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects.get(shield).unwrap().base.wait_timer, elapsed);
    }
    for rise in 1..=20 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, shield, &mut inputs, 40)
                .unwrap()
                .step,
            if rise == 20 {
                ControlStep::Ended
            } else {
                ControlStep::Movement
            }
        );
        assert_eq!(
            objects.get(shield).unwrap().extension.relative_position.y,
            i16::from(rise) * 16
        );
        assert_eq!(
            objects
                .get(shield)
                .unwrap()
                .extension
                .relative_rotation
                .yaw
                .units(),
            rise * 4
        );
    }
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, core, &mut inputs, 20)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    assert_eq!(
        objects.get(core).unwrap().extension.material_set,
        Some(MaterialSetId::from_catalog_token(33_140))
    );
    for elapsed in 2..=50 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, elapsed);
        assert!(
            !objects
                .get(core)
                .unwrap()
                .extension
                .path_state
                .conditions
                .hit_event_pending
        );
    }
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 120)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    assert!(
        objects
            .get(core)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending
    );
    assert_eq!(
        objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase,
        0x0201
    );
    assert_eq!(runtime.spawns.parameter, Some(4));
    let beams: Vec<_> = objects
        .active_objects()
        .filter(|(_, a)| a.base.shape == ShapeId::from_catalog_index(20))
        .collect();
    assert_eq!(beams.len(), 4);
    for (number, (x, z)) in [(2, (0, 280)), (3, (0, -280)), (4, (280, 0)), (5, (-280, 0))] {
        let beam = beams
            .iter()
            .find(|(_, a)| a.base.child_number == number)
            .unwrap()
            .1;
        assert_eq!(beam.base.attack_power, number - 1);
        assert_eq!(beam.extension.relative_position, Vector3 { x, y: -280, z });
        assert_eq!(beam.base.attachment, Some(owner));
    }
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, core, &mut inputs, 30)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    assert_eq!(
        objects.get(core).unwrap().extension.material_set,
        Some(MaterialSetId::from_catalog_token(33_268))
    );
    assert!(
        !objects
            .get(core)
            .unwrap()
            .base
            .contacts
            .suppress_contacts_next_epoch
    );
    assert!(
        !objects
            .get(core)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending
    );
    inputs
        .objective_counts
        .as_deref_mut()
        .unwrap()
        .remaining_word = 0xAB00;
    for _ in 2..=32 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
    }
    // GOTO yields separately before the next live count import.
    for _ in 0..2 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
    }
    assert!(
        objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .hold_latched
    );
    assert_eq!(objects.len(), 8); // controller, player, shield, core, four beams
    assert_eq!(counts.node_record, 0x72);
}

#[test]
fn core_contact_thresholds_keep_signed_byte_wrap_and_do_not_duplicate_periodic_registration() {
    use super::super::path_player_control::PitchRecoil;
    use super::super::{AudioState, Behavior, ObjectKind};
    use super::projectile_tests::audio;
    let catalog = authored_paths::catalog();
    for health in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(core_path(&catalog));
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xAB00;
        let player = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let mut events = AudioState::default();
        let mut recoil = PitchRecoil::default();
        let mut inputs = world(&mut random);
        inputs.audio = Some(audio(&mut events));
        inputs.primary_player = Some(player);
        inputs.primary_pitch_recoil = Some(&mut recoil);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 20)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending = true;
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 30)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let original_path = objects.get(owner).unwrap().base.path;
        objects.get_mut(owner).unwrap().base.hit_points = health;
        // Without a new contact the threshold callback does not run.
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        assert_eq!(
            objects.get(owner).unwrap().extension.material_set,
            Some(MaterialSetId::from_catalog_token(33_268))
        );
        objects
            .get_mut(owner)
            .unwrap()
            .base
            .contacts
            .new_contact_latched = true;
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        let inner = 105_u8.wrapping_sub(health) < 128;
        let dead = inner && 75_u8.wrapping_sub(health) < 128;
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.hit_points, health);
        assert_eq!(
            actor.extension.material_set,
            Some(MaterialSetId::from_catalog_token(if inner {
                33_534
            } else {
                33_268
            }))
        );
        assert_eq!(
            actor.extension.auxiliary.impact_materials(&runtime.resources, owner).unwrap().ordinary,
            inner.then_some(4)
        );
        assert_eq!(
            actor.extension.path_state.motion_phase,
            0xAB00 | if inner { 75 } else { 105 }
        );
        assert_eq!(actor.base.path != original_path, dead);
        let triggers = actor
            .extension
            .path_state
            .triggers
            .entries(&runtime.resources, owner)
            .unwrap();
        assert_eq!(
            triggers
                .iter()
                .filter(|t| t.kind == TriggerKind::NewContact)
                .count(),
            usize::from(!dead)
        );
        assert_eq!(
            triggers
                .iter()
                .filter(|t| t.kind == TriggerKind::Periodic(TriggerPeriod::ThirtyTwo))
                .count(),
            usize::from(inner)
        );
        if inner && !dead {
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
            let triggers = objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .triggers
                .entries(&runtime.resources, owner)
                .unwrap();
            assert_eq!(
                triggers
                    .iter()
                    .filter(|t| t.kind == TriggerKind::Periodic(TriggerPeriod::ThirtyTwo))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn core_death_awards_once_decrements_both_bytes_and_retains_its_authored_aftermath() {
    use super::super::path_player_control::{PitchRecoil, PlayerTargetControl, PrimaryControl};
    use super::super::path_score::PlayerScore;
    use super::super::player_hit_control::{PlayerHitControl, PrimaryFeedback};
    use super::super::{AudioState, Behavior, ObjectKind};
    use super::projectile_tests::audio;
    let catalog = authored_paths::catalog();
    for initial in 0..=u8::MAX {
        for locked in [false, true] {
            let (mut runtime, mut objects, owner, _) = setup();
            let mut random = RandomState::new([initial, 7, 39, 127]);
            let mut expected_random = random;
            let parent = objects
                .allocate(Object::new(
                    ObjectKind::Effect,
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
            actor.base.path = Some(core_path(&catalog));
            actor.base.attachment = Some(parent);
            actor.base.position = Vector3 {
                x: 200,
                y: -160,
                z: -300,
            };
            let mut counts = EncounterObjectiveCounts {
                remaining_word: 0xAB00 | u16::from(initial),
                node_record: initial ^ 0xFF,
            };
            let mut score = PlayerScore::from_parts(65000, initial);
            let mut control = PlayerTargetControl {
                mode: 8,
                configuration_locked: locked,
                ..Default::default()
            };
            let mut hit = PlayerHitControl {
                feedback_flags: 0x80,
                ..Default::default()
            };
            let mut recoil = PitchRecoil::default();
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.objective_counts = Some(&mut counts);
            inputs.selected_score = Some(&mut score);
            inputs.audio = Some(audio(&mut events));
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.primary_player = Some(player);
            inputs.selected = Some(player);
            inputs.primary_control = Some(PrimaryControl {
                target: &mut control,
                linked_mode: false,
            });
            inputs.primary_feedback = Some(PrimaryFeedback {
                state: 1,
                hit: &mut hit,
            });
            inputs.primary_pitch_recoil = Some(&mut recoil);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 30)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .conditions
                .hit_event_pending = true;
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 30)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            objects.get_mut(owner).unwrap().base.hit_points = 75;
            objects
                .get_mut(owner)
                .unwrap()
                .base
                .contacts
                .new_contact_latched = true;
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
            let mut marked = false;
            for _ in 0..12 {
                let first_takes = expected_random.next_byte() < 127;
                let second_takes = !first_takes && expected_random.next_byte() < 127;
                marked |= !first_takes && !second_takes;
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                let actor = objects.get(owner).unwrap();
                assert!(!actor.base.flags.visible);
                assert!(actor.base.flags.collision_disabled);
                assert_eq!(actor.base.contacts.hit_marked, marked);
                assert_eq!(actor.base.hit_points, 75);
                assert_eq!(objects.len(), 4);
                assert_eq!(
                    inputs.selected_score.as_deref().unwrap().points(),
                    u32::from(initial) * 65536 + 65535
                );
                assert_eq!(
                    inputs.objective_counts.as_deref().unwrap(),
                    &EncounterObjectiveCounts {
                        remaining_word: 0xAB00 | u16::from(initial.wrapping_sub(1)),
                        node_record: (initial ^ 0xFF).wrapping_sub(1),
                    }
                );
                assert_eq!(
                    inputs.primary_pitch_recoil.as_deref().unwrap().amount,
                    if marked { 128 } else { 0 }
                );
                assert_eq!(*inputs.random, expected_random);
            }
            assert!(
                objects
                    .get(parent)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .hit_event_pending
            );
            assert_eq!(
                inputs.primary_control.as_ref().unwrap().target.mode,
                if locked { 8 } else { 2 }
            );
            assert_eq!(
                inputs.primary_feedback.as_ref().unwrap().hit.feedback_flags,
                if locked { 0xA4 } else { 0x80 }
            );
            assert_eq!(
                inputs
                    .primary_feedback
                    .as_ref()
                    .unwrap()
                    .hit
                    .feedback_duration,
                if locked { 4 } else { 0 }
            );
            let clone = runtime.spawns.last_spawn.unwrap();
            assert_eq!(
                objects.get(clone).unwrap().base.shape,
                ShapeId::from_catalog_index(428)
            );
            assert_eq!(
                objects.get(clone).unwrap().base.position,
                objects.get(owner).unwrap().base.position
            );
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, clone, &mut inputs, 1)
                    .unwrap()
                    .step,
                ControlStep::MovementTail
            );
            assert_eq!(objects.get(clone).unwrap().base.hit_points, 0);
            assert!(objects.get(clone).unwrap().base.flags.collision_disabled);
            assert_eq!(objects.len(), 4); // death marking is not retirement
                                          // The four-visit callback emits one or two paused-running effects.
            let extra = expected_random.next_byte() >= 127;
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 4);
            assert_eq!(objects.len(), 5 + usize::from(extra));
            assert_eq!(*inputs.random, expected_random);
            for (id, effect) in objects
                .active_objects()
                .filter(|(_, a)| a.base.shape == ShapeId::from_catalog_index(13))
            {
                assert_ne!(id, clone);
                assert!(effect.base.contacts.run_when_paused);
                assert_eq!(effect.extension.path_state.part, 1);
            }
        }
    }
}
