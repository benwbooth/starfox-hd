//! Static-source scenarios for the complete multipart node objective.
use super::super::path_fields::{Axis, WordField, WordOperand};
use super::super::path_motion::PublishedPlayerMotion;
use super::super::path_relationships::find_child;
use super::super::path_scene_state::{ActiveNodeFlags, EncounterCoordination};
use super::super::path_triggers::TriggerKind;
use super::super::{
    authored_paths, Angle, Difficulty, ObjectSpawnDefaults, PathId, ShapeId, Vector3,
};
use super::paired_patrol_tests::callbacks;
use super::tests::{setup, world};
use super::*;

fn cursor(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn locate(catalog: &PathCatalog, predicate: impl Fn(Statement) -> bool) -> PathCursor {
    (0..authored_paths::LOWERED_COMMAND_COUNT as u16)
        .map(cursor)
        .find(|&at| predicate(catalog.statement(at).unwrap()))
        .unwrap()
}

#[test]
fn live_node_export_is_full_word_atomic_and_visible_to_the_next_import() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    let before_random = random;
    let catalog = PathCatalog::new(vec![vec![
        Statement::ExportActiveNodeFlags {
            source: WordOperand::Actor(WordField::ScriptValue),
            next: cursor(1),
        },
        Statement::ImportActiveNodeFlags {
            destination: WordField::MotionPhase,
            next: cursor(2),
        },
    ]])
    .unwrap();
    for inverted in [false, true] {
        runtime.branch.invert_next = inverted;
        objects.get_mut(owner).unwrap().base.path = Some(cursor(0));
        let before = objects.clone();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingActiveNodeFlags)
        );
        assert_eq!(objects, before);
        let mut flags = ActiveNodeFlags { bits: 0xA55A };
        let mut inputs = world(&mut random);
        inputs.active_node_flags = Some(&mut flags);
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
            Err(ProgramError::BudgetExceeded {
                cursor: cursor(0),
                executed: 0
            })
        );
        assert_eq!(inputs.active_node_flags.as_deref().unwrap().bits, 0xA55A);
        for value in 0..=u16::MAX {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(cursor(0));
            actor.extension.path_state.script_value = value;
            let mut expected = objects.clone();
            let actor = expected.get_mut(owner).unwrap();
            actor.base.path = Some(cursor(2));
            actor.extension.path_state.motion_phase = value;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2),
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(2),
                    executed: 2
                })
            );
            assert_eq!(objects, expected);
            assert_eq!(inputs.active_node_flags.as_deref().unwrap().bits, value);
            assert_eq!(runtime.branch.invert_next, inverted);
            assert_eq!(*inputs.random, before_random);
        }
    }
}

#[test]
fn node_entry_captures_initial_count_heading_and_completed_state() {
    let catalog = authored_paths::catalog();
    for count in 0..=u8::MAX {
        for difficulty in [Difficulty::Normal, Difficulty::Hard, Difficulty::Expert] {
            for bits in [0, 0xFFEF, 0x10, 0xFFFF] {
                let complete = bits & 0x10 != 0;
                let (mut runtime, mut objects, owner, mut random) = setup();
                let before_random = random;
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::MULTIPART_NODE_OBJECTIVE);
                actor.base.hit_points = count;
                actor.base.attack_power = count.wrapping_mul(31);
                actor.extension.path_state.motion_phase = 0xAB00;
                WordField::SavedPosition(Axis::X).write(actor, (-2345_i16) as u16);
                let mut flags = ActiveNodeFlags { bits };
                let mut inputs = world(&mut random);
                inputs.active_node_flags = Some(&mut flags);
                inputs.published_motion = Some(PublishedPlayerMotion::default());
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                inputs.campaign = Some(CampaignPathInputs {
                    difficulty,
                    encounter_variant: 0,
                });
                let exit = runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 120)
                    .unwrap();
                assert_eq!(exit.actor, owner);
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.hit_points, 100);
                assert_eq!(actor.base.attack_power, 4);
                assert_eq!(actor.base.yaw, Angle::from_units(count.wrapping_mul(31)));
                assert_eq!(actor.extension.path_state.script_parameter, count);
                assert_eq!(actor.extension.path_state.motion_phase, 0xAB05);
                assert_eq!(WordField::SavedPosition(Axis::X).read(actor) as i16, -2345);
                assert!(actor.base.flags.collision_disabled);
                assert!(actor.base.contacts.run_when_paused);
                let entries = actor
                    .extension
                    .path_state
                    .triggers
                    .entries(&runtime.resources, owner)
                    .unwrap();
                assert_eq!(
                    entries
                        .iter()
                        .filter(|t| t.kind == TriggerKind::PlayerContact)
                        .count(),
                    usize::from(!complete && difficulty != Difficulty::Expert)
                );
                assert_eq!(
                    entries
                        .iter()
                        .filter(|t| t.kind == TriggerKind::Always)
                        .count(),
                    if complete { 1 } else { 2 }
                );
                assert_eq!(find_child(&objects, owner, 9).unwrap().is_some(), !complete);
                if complete {
                    assert!(find_child(&objects, owner, 1).unwrap().is_none());
                    let exit_service = find_child(&objects, owner, 5).unwrap().unwrap();
                    assert_eq!(
                        objects.get(exit_service).unwrap().base.shape,
                        ShapeId::EMPTY
                    );
                    assert_eq!(actor.base.shape, ShapeId::from_catalog_index(239));
                } else {
                    let child = objects
                        .get(find_child(&objects, owner, 1).unwrap().unwrap())
                        .unwrap();
                    assert_eq!(child.extension.path_state.script_parameter, count);
                    assert_eq!(child.extension.relative_position.y, -614);
                    assert_eq!(child.base.position, Vector3::default());
                    assert_eq!(runtime.spawns.parameter, Some(count));
                }
                assert_eq!(inputs.active_node_flags.as_deref().unwrap().bits, bits);
                assert_eq!(*inputs.random, before_random);
            }
        }
    }
}

#[test]
fn node_parts_use_count_specific_layouts_and_retain_parent_selection() {
    let catalog = authored_paths::catalog();
    let entry = locate(&catalog, |s| {
        matches!(s, Statement::SpawnChild { parameters, .. }
        if parameters.shape == ShapeId::from_catalog_index(246))
    });
    let Statement::SpawnChild { parameters, .. } = catalog.statement(entry).unwrap() else {
        unreachable!()
    };
    for count in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = parameters.path;
        actor.extension.path_state.script_parameter = count;
        let mut inputs = world(&mut random);
        inputs.scene.encounter_location = Some(5);
        inputs.published_motion = Some(PublishedPlayerMotion::default());
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.fixed_players[0] = Some(owner);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 90)
                .unwrap()
                .actor,
            owner
        );
        let expected: &[(u8, i16, u8)] = match count {
            2 => &[(2, 150, 1), (3, -150, 2)],
            3 => &[(2, 0, 1), (3, 200, 2), (4, -200, 3)],
            _ => &[(2, 0, 1)],
        };
        assert_eq!(objects.active_objects().count(), expected.len() + 1);
        for &(number, x, attack) in expected {
            let child = objects
                .get(find_child(&objects, owner, number).unwrap().unwrap())
                .unwrap();
            assert_eq!(child.extension.relative_position.x, x);
            assert_eq!(child.base.attack_power, attack);
            assert_eq!(child.base.hit_points, 10);
            assert_eq!(child.base.shape, ShapeId::from_catalog_index(247));
        }
        assert_eq!(runtime.program_actor, Some(owner));
    }
}

#[test]
fn four_panel_reveal_has_bounded_shapes_distinct_offsets_and_independent_animation() {
    use super::projectile_tests::audio;
    let catalog = authored_paths::catalog();
    let spawn = locate(&catalog, |s| {
        matches!(s, Statement::SpawnChild { parameters, .. }
        if parameters.shape == ShapeId::EMPTY && parameters.position.y == -650)
    });
    let Statement::SpawnChild { parameters, .. } = catalog.statement(spawn).unwrap() else {
        unreachable!()
    };
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = parameters.path;
    let mut events = super::super::AudioState::default();
    let mut inputs = world(&mut random);
    inputs.audio = Some(audio(&mut events));
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 120)
            .unwrap()
            .actor,
        owner
    );
    assert_eq!(objects.active_objects().count(), 5);
    assert_eq!(runtime.spawns.parameter, Some(4));
    for (i, (x, shape)) in [(-180, 276), (-60, 278), (60, 262), (180, 274)]
        .into_iter()
        .enumerate()
    {
        let id = find_child(&objects, owner, i as u8 + 2).unwrap().unwrap();
        let child = objects.get(id).unwrap();
        assert_eq!(child.base.shape, ShapeId::from_catalog_index(shape));
        assert_eq!(child.extension.relative_position.x, x);
        assert_eq!(
            child.extension.path_state.motion_phase,
            ((i as u16) + 2) * 256
        );
        assert!(!child.base.flags.visible);
        assert!(child.base.contacts.run_when_paused);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, id, &mut inputs, 20)
                .unwrap()
                .actor,
            id
        );
        for visit in 1..=14 {
            callbacks(&mut runtime, &catalog, &mut objects, id, &mut inputs, visit);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, id, &mut inputs, 20)
                    .unwrap()
                    .actor,
                id
            );
            let child = objects.get(id).unwrap();
            assert_eq!(child.extension.path_state.motion_phase as u8, visit % 7);
            assert_eq!(child.extension.path_state.motion_phase >> 8, i as u16 + 2);
        }
        assert!(objects.get(id).unwrap().base.flags.visible);
        assert!(objects.get(id).unwrap().base.flags.collision_disabled);
    }
}

#[test]
fn node_completion_observes_live_bits_then_publishes_without_losing_unrelated_bits() {
    let catalog = authored_paths::catalog();
    for count in 1..=3 {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::MULTIPART_NODE_OBJECTIVE);
        actor.base.hit_points = count;
        let mut flags = ActiveNodeFlags { bits: 0xA500 };
        let mut coordination = EncounterCoordination {
            handshake: 0xFF,
            ..Default::default()
        };
        let mut inputs = world(&mut random);
        inputs.active_node_flags = Some(&mut flags);
        inputs.coordination = Some(&mut coordination);
        inputs.published_motion = Some(PublishedPlayerMotion::default());
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.campaign = Some(CampaignPathInputs {
            difficulty: Difficulty::Expert,
            encounter_variant: 0,
        });
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 120)
                .unwrap()
                .actor,
            owner
        );
        let constructor = find_child(&objects, owner, 1).unwrap().unwrap();
        inputs.fixed_players[0] = Some(owner);
        inputs.scene.encounter_location = Some(0);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, constructor, &mut inputs, 120)
                .unwrap()
                .actor,
            constructor
        );
        let held = objects.get(owner).unwrap().base.path;
        for bit in 0..count {
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut inputs,
                bit + 1,
            );
            assert_eq!(objects.get(owner).unwrap().base.path, held);
            inputs.active_node_flags.as_deref_mut().unwrap().bits |= 1 << bit;
        }
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 10);
        assert_ne!(objects.get(owner).unwrap().base.path, held);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .actor,
            owner
        );
        assert_eq!(
            inputs.active_node_flags.as_deref().unwrap().bits,
            0xA510 | ((1 << count) - 1)
        );
        assert_eq!(inputs.coordination.as_deref().unwrap().handshake, 0xFD);
        let waiting = objects.get(owner).unwrap().base.path;
        for _ in 0..5 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .actor,
                owner
            );
            assert_eq!(objects.get(owner).unwrap().base.path, waiting);
        }
        inputs.coordination.as_deref_mut().unwrap().handshake |= 2;
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .actor,
            owner
        );
        assert_eq!(objects.get(owner).unwrap().base.wait_timer, 1);
        assert_eq!(
            objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .motion_phase
                & 0xFF,
            5
        );
        for elapsed in 2..=13 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .actor,
                owner
            );
            assert_eq!(objects.get(owner).unwrap().base.wait_timer, elapsed);
        }
        let result = runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 100);
        if count < 3 {
            // Source 66 always asks for child 4 first, even in the installed
            // one/two-part layouts. Its failed lookup writes through zero.
            // Do not silently turn the existing native diagnostic into a
            // successful cleanup or fabricate the absent child.
            assert_eq!(
                result,
                Err(ProgramError::Relationship(
                    super::super::path_relationships::RelationshipError::MissingChild {
                        owner,
                        number: 4
                    }
                ))
            );
            assert!(
                !objects
                    .get(constructor)
                    .unwrap()
                    .base
                    .flags
                    .remove_after_tick
            );
        } else {
            assert_eq!(result.unwrap().actor, owner);
            for number in 1..=4 {
                let old = find_child(&objects, owner, number).unwrap().unwrap();
                assert!(objects.get(old).unwrap().base.flags.remove_after_tick);
            }
            let reveal = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
            assert_eq!(reveal.base.shape, ShapeId::EMPTY);
            assert_eq!(reveal.base.child_number, 1);
            assert_eq!(reveal.extension.relative_position.y, -650);
            assert!(!reveal.base.flags.remove_after_tick);
            assert_eq!(objects.get(owner).unwrap().base.wait_timer, 1);
        }
    }
}

#[test]
fn reveal_shape_selection_rejects_every_unconstructed_high_byte_without_mutation() {
    let source = authored_paths::catalog();
    let at = locate(
        &source,
        |s| matches!(s, Statement::SelectShape { shapes, .. } if shapes.len() == 4),
    );
    let Statement::SelectShape {
        selector, shapes, ..
    } = source.statement(at).unwrap()
    else {
        unreachable!()
    };
    let catalog = PathCatalog::new(vec![vec![Statement::SelectShape {
        selector,
        shapes,
        next: cursor(1),
    }]])
    .unwrap();
    for high in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase = (u16::from(high) << 8) | 0xAD;
        runtime.branch.invert_next = true;
        let mut expected = objects.clone();
        let before_random = random;
        let result =
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1);
        if high < 4 {
            expected.get_mut(owner).unwrap().base.shape =
                ShapeId::from_catalog_index([276, 278, 262, 274][usize::from(high)]);
            expected.get_mut(owner).unwrap().base.path = Some(cursor(1));
            assert_eq!(
                result,
                Err(ProgramError::BudgetExceeded {
                    cursor: cursor(1),
                    executed: 1
                })
            );
        } else {
            assert_eq!(
                result,
                Err(ProgramError::ShapeSelectionOutOfBounds {
                    index: high,
                    count: 4
                })
            );
        }
        assert_eq!(objects, expected);
        assert_eq!(random, before_random);
        assert!(runtime.branch.invert_next);
    }
}

#[test]
fn node_exit_publishes_once_and_its_detached_service_disables_only_the_nearest_ship() {
    use super::super::path_scene_state::EncounterHandoff;
    use super::super::{Behavior, ObjectKind};
    let catalog = authored_paths::catalog();
    let spawn = locate(&catalog, |s| {
        matches!(s, Statement::SpawnChild { parameters, .. }
        if parameters.shape == ShapeId::EMPTY && parameters.number == 5
        && parameters.hit_points == 100 && parameters.attack_power == 0)
    });
    let Statement::SpawnChild { parameters, .. } = catalog.statement(spawn).unwrap() else {
        unreachable!()
    };
    for location in 0..=u8::MAX {
        for with_ship in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = parameters.path;
            actor.base.position = Vector3 {
                x: -1234,
                y: 399,
                z: 23456,
            };
            actor.base.yaw = Angle::from_units(location);
            let mut ship = Object::new(
                ObjectKind::Player,
                ShapeId::from_catalog_index(3),
                Behavior::PlayerFlight,
            );
            ship.base.position = Vector3 {
                x: -1234,
                y: 399,
                z: 23457,
            };
            ship.extension.path_state.weapon_selection = 173;
            let ship_id = with_ship.then(|| objects.allocate(ship).unwrap());
            let mut handoff = EncounterHandoff {
                player_flags: location,
                heading_word: 0xCAFF,
                ..Default::default()
            };
            let mut inputs = world(&mut random);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.scene.encounter_location = Some(location);
            inputs.handoff = Some(&mut handoff);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 30)
                    .unwrap()
                    .actor,
                owner
            );
            assert_eq!(
                inputs.handoff.as_deref().unwrap(),
                &EncounterHandoff {
                    player_flags: location | 0x40,
                    x: -1234,
                    z: 23456,
                    heading_word: 0xCA00 | u16::from(location),
                }
            );
            let actor = objects.get(owner).unwrap();
            assert!(actor.extension.path_state.hold_latched);
            assert!(!actor.base.flags.remove_after_tick);
            assert!(!actor.base.flags.visible);
            assert_eq!(
                actor
                    .extension
                    .path_state
                    .triggers
                    .entries(&runtime.resources, owner)
                    .unwrap()
                    .len(),
                usize::from(location != 5)
            );
            let service = runtime.spawns.last_spawn.unwrap();
            inputs.handoff = None;
            inputs.scene.encounter_location = None;
            for _ in 0..5 {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 1)
                        .unwrap()
                        .actor,
                    owner
                );
            }
            let before = objects.clone();
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, service, &mut inputs, 20)
                    .unwrap(),
                ProgramExit {
                    actor: service,
                    step: ControlStep::Ended
                }
            );
            assert_eq!(objects.get(owner), before.get(owner));
            if let Some(ship) = ship_id {
                let mut expected = before.get(ship).unwrap().clone();
                expected.extension.path_state.weapon_selection = 255;
                assert_eq!(objects.get(ship), Some(&expected));
            }
            assert!(objects.get(service).unwrap().base.flags.remove_after_tick);
        }
    }
}

#[test]
fn node_part_polls_live_flags_and_blinks_after_the_handshake_bypass() {
    use super::projectile_tests::audio;
    let catalog = authored_paths::catalog();
    let spawn = locate(&catalog, |s| {
        matches!(s, Statement::SpawnChild { parameters, .. }
        if parameters.shape == ShapeId::from_catalog_index(247))
    });
    let Statement::SpawnChild { parameters, .. } = catalog.statement(spawn).unwrap() else {
        unreachable!()
    };
    for number in 1..=3 {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = parameters.path;
        actor.base.attack_power = number;
        actor.extension.path_state.motion_delta.x = 1;
        let mut flags = ActiveNodeFlags { bits: 0 };
        let mut events = super::super::AudioState::default();
        let mut inputs = world(&mut random);
        inputs.active_node_flags = Some(&mut flags);
        inputs.scene.encounter_location = Some(0);
        inputs.audio = Some(audio(&mut events));
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 60)
                .unwrap()
                .actor,
            owner
        );
        let poll = objects.get(owner).unwrap().base.path;
        for visit in 1..=10 {
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut inputs,
                visit,
            );
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 60)
                    .unwrap()
                    .actor,
                owner
            );
            assert_eq!(objects.get(owner).unwrap().base.path, poll);
            assert_eq!(objects.get(owner).unwrap().base.yaw.units(), visit * 8);
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .relative_rotation
                    .yaw
                    .units(),
                visit * 8
            );
        }
        inputs.active_node_flags.as_deref_mut().unwrap().bits = 1 << (number - 1);
        for visit in 0..10 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 60)
                    .unwrap()
                    .actor,
                owner
            );
            assert_eq!(
                objects.get(owner).unwrap().base.shape,
                ShapeId::from_catalog_index(if visit % 2 == 0 { 247 } else { 248 })
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
        assert_eq!(
            inputs.active_node_flags.as_deref().unwrap().bits,
            1 << (number - 1)
        );
    }
}
