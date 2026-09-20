//! Four-way turret encounter scenarios derived from static source scripts.
use super::super::path_fields::{Axis, ByteField, WordField};
use super::super::path_scene_state::{EncounterCoordination, EncounterHandoff};
use super::super::path_triggers::TriggerKind;
use super::super::{authored_paths, Angle, ObjectSpawnDefaults, PathId, ShapeId, Vector3};
use super::paired_patrol_tests::callbacks;
use super::tests::{setup, world};
use super::*;

fn cursor(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn constructor(catalog: &PathCatalog) -> PathCursor {
    (0..authored_paths::LOWERED_COMMAND_COUNT)
        .find_map(
            |index| match catalog.statement(cursor(index as u16)).unwrap() {
                Statement::SpawnIndependent { parameters, .. }
                    if parameters.shape == ShapeId::from_catalog_index(67) =>
                {
                    parameters.path
                }
                _ => None,
            },
        )
        .unwrap()
}

#[test]
fn turret_beam_waits_grows_then_fires_or_cancels_on_live_phase() {
    use super::super::weapon_dispatch::WeaponState;
    use super::super::{Behavior, ObjectKind};
    let catalog = authored_paths::catalog();
    let beam_path = (0..authored_paths::LOWERED_COMMAND_COUNT)
        .find_map(
            |index| match catalog.statement(cursor(index as u16)).unwrap() {
                Statement::SpawnChild { parameters, .. }
                    if parameters.shape == ShapeId::from_catalog_index(20)
                        && parameters.number == 9
                        && parameters.position.y == -56 =>
                {
                    parameters.path
                }
                _ => None,
            },
        )
        .unwrap();
    for mode in [0, 16] {
        for cancel in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(beam_path);
            actor.base.attack_power = 1;
            actor.base.hit_points = 100;
            actor.base.pitch = Angle::from_units(33);
            actor.base.yaw = Angle::from_units(77);
            actor.base.roll = Angle::from_units(99);
            let mut target =
                Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
            target.base.position = Vector3 {
                x: 1000,
                y: -300,
                z: 2000,
            };
            let player = objects.allocate(target).unwrap();
            let mut weapons = WeaponState::default();
            let mut inputs = world(&mut random);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.primary_player = Some(player);
            inputs.selected = Some(player);
            inputs.primary_motion = Some(PrimaryMotionInput {
                auxiliary_mode: mode,
                displacement: Vector3::default(),
            });
            inputs.weapons = Some(&mut weapons);
            for elapsed in 1..=5 {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 30)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                assert_eq!(objects.get(owner).unwrap().base.wait_timer, elapsed);
            }
            if cancel {
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase = 1;
                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 20)
                        .unwrap()
                        .step,
                    ControlStep::Ended
                );
                assert_eq!(objects.len(), 2);
                assert_eq!(objects.get(owner).unwrap().extension.texture_scroll_x, 0);
                continue;
            }
            for frame in 1..=16 {
                callbacks(
                    &mut runtime,
                    &catalog,
                    &mut objects,
                    owner,
                    &mut inputs,
                    frame,
                );
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 30)
                        .unwrap()
                        .step,
                    if frame == 16 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }
                );
                assert_eq!(
                    objects.get(owner).unwrap().extension.texture_scroll_x,
                    4 + frame * 2
                );
            }
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                (
                    actor.base.pitch.units(),
                    actor.base.yaw.units(),
                    actor.base.roll.units()
                ),
                (33, 77, 99)
            );
            assert_eq!(actor.extension.path_state.weapon_selection, 26);
            assert_eq!(objects.len(), 3);
            let shot = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
            assert_eq!(
                shot.base.path,
                Some(authored_paths::OCCUPANCY_SURFACE_LIMITED)
            );
            assert_eq!(shot.base.speed, if mode == 16 { 40 } else { 60 });
        }
    }
}

#[test]
fn radial_coordinates_check_all_selectors_and_preserve_state_on_failure() {
    for (axis, values) in [
        (Axis::X, &[0, 800, 0, -800][..]),
        (Axis::Z, &[-800, 0, 800, 0][..]),
    ] {
        let catalog = PathCatalog::new(vec![vec![
            Statement::SelectRelativeCoordinate {
                selector: ByteField::AttackPower,
                axis,
                values,
                next: cursor(1),
            },
            Statement::Control(ControlCommand::Hold),
        ]])
        .unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        for index in 0..=u8::MAX {
            for inverted in [false, true] {
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(cursor(0));
                actor.base.attack_power = index;
                actor.base.wait_timer = 199;
                actor.extension.relative_position = Vector3 {
                    x: -123,
                    y: 5678,
                    z: 2345,
                };
                runtime.branch.invert_next = inverted;
                runtime.enter(&objects, owner).unwrap();
                let mut inputs = world(&mut random);
                let before = (runtime.clone(), objects.clone());
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                    Err(ProgramError::BudgetExceeded {
                        cursor: cursor(0),
                        executed: 0
                    })
                );
                assert_eq!((&runtime, &objects), (&before.0, &before.1));
                let result = runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1);
                if index < 4 {
                    let mut expected = before.1;
                    let actor = expected.get_mut(owner).unwrap();
                    WordField::RelativePosition(axis)
                        .write(actor, values[usize::from(index)] as u16);
                    actor.base.path = Some(cursor(1));
                    assert_eq!(
                        result,
                        Err(ProgramError::BudgetExceeded {
                            cursor: cursor(1),
                            executed: 1
                        })
                    );
                    assert_eq!(objects, expected);
                    assert_eq!(runtime, before.0);
                } else {
                    assert_eq!(
                        result,
                        Err(ProgramError::CoordinateSelectionOutOfBounds { index, count: 4 })
                    );
                    assert_eq!((&runtime, &objects), (&before.0, &before.1));
                }
            }
        }
    }
}

#[test]
fn encounter_entry_resets_signals_publishes_anchor_and_waits_for_constructor() {
    let catalog = authored_paths::catalog();
    for health in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::FOUR_TURRET_ENCOUNTER);
        actor.base.hit_points = health;
        actor.base.attack_power = health;
        actor.base.position = Vector3 {
            x: -2345,
            y: 1234,
            z: 5678,
        };
        actor.base.yaw = Angle::from_units(health);
        let mut signals = EncounterSignals { raised: u16::MAX };
        let mut handoff = EncounterHandoff {
            heading_word: 0xABCD,
            player_flags: 7,
            ..Default::default()
        };
        let mut inputs = world(&mut random);
        inputs.encounter_signals = Some(&mut signals);
        inputs.handoff = Some(&mut handoff);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.hit_points, 50);
        assert_eq!(actor.base.attack_power, health);
        assert!(!actor.base.flags.visible);
        assert!(actor.base.contacts.suppress_contacts_next_epoch);
        assert_eq!(inputs.encounter_signals.as_deref().unwrap().raised, 0);
        assert_eq!(
            *inputs.handoff.as_deref().unwrap(),
            EncounterHandoff {
                x: -2345,
                z: 5678,
                heading_word: 0xABCD,
                player_flags: 7
            }
        );
        let shell = runtime.spawns.last_spawn.unwrap();
        let actor = objects.get(shell).unwrap();
        assert_eq!(actor.base.path, Some(constructor(&catalog)));
        assert_eq!(actor.base.hit_points, 5);
        assert_eq!(actor.base.attack_power, 1);
        assert_eq!(objects.len(), 2);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 10)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects.len(), 2);
    }
}

#[test]
fn fresh_constructor_assigns_four_distinct_coordinates_and_preserves_high_phase() {
    let catalog = authored_paths::catalog();
    for high in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(constructor(&catalog));
        actor.base.hit_points = 5;
        actor.base.position.y = -999;
        actor.extension.path_state.motion_phase = u16::from(high) << 8;
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.position.y, 0);
        assert_eq!(
            actor.extension.path_state.motion_phase,
            (u16::from(high) << 8) | 4
        );
        assert_eq!(runtime.spawns.parameter, Some(3));
        let children: Vec<_> = objects
            .active_objects()
            .filter(|(_, a)| a.base.shape == ShapeId::from_catalog_index(68))
            .map(|(id, a)| (id, a.base.attack_power))
            .collect();
        assert_eq!(children.len(), 4);
        for (child, selector) in children {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, child, &mut inputs, 30)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let actor = objects.get(child).unwrap();
            assert_eq!(actor.base.attachment, Some(owner));
            assert_eq!(
                actor.extension.relative_position,
                Vector3 {
                    x: [0, 800, 0, -800][usize::from(selector)],
                    y: -160,
                    z: [-800, 0, 800, 0][usize::from(selector)]
                }
            );
            assert_eq!(
                actor.extension.relative_rotation.yaw,
                Angle::from_units([128, 192, 0, 64][usize::from(selector)])
            );
            assert_eq!(actor.base.attack_power, 1);
            assert_eq!(actor.base.hit_points, 20);
            assert_eq!(actor.base.wait_timer, 1);
        }
    }
}

#[test]
fn turret_collision_gate_waits_for_exact_progress_sentinel_and_preserves_phase() {
    let catalog = authored_paths::catalog();
    for progress in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(constructor(&catalog));
        objects.get_mut(owner).unwrap().base.hit_points = 5;
        let mut shared = EncounterCoordination {
            progress,
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
        let child = runtime.spawns.last_spawn.unwrap();
        objects
            .get_mut(child)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xAB57;
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, child, &mut inputs, 40)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs, 1);
        let actor = objects.get(child).unwrap();
        assert_eq!(actor.base.flags.collision_disabled, progress != 255);
        assert_eq!(actor.extension.path_state.motion_phase, 0xAB57);
        assert_eq!(actor.base.wait_timer, 1);
        let count = actor
            .extension
            .path_state
            .triggers
            .entries(&runtime.resources, child)
            .unwrap()
            .iter()
            .filter(|t| t.kind == TriggerKind::Always)
            .count();
        assert_eq!(count, usize::from(progress != 255));
        inputs.coordination.as_deref_mut().unwrap().progress = 255;
        callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs, 2);
        assert!(!objects.get(child).unwrap().base.flags.collision_disabled);
        inputs.coordination.as_deref_mut().unwrap().progress = 0;
        callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs, 3);
        assert!(!objects.get(child).unwrap().base.flags.collision_disabled);
        assert_eq!(
            objects
                .get(child)
                .unwrap()
                .extension
                .path_state
                .motion_phase,
            0xAB57
        );
    }
}

#[test]
fn four_turret_deaths_restore_constructor_links_and_raise_release_signal_once() {
    use super::super::path_player_control::{PlayerTargetControl, PrimaryControl};
    use super::super::{AudioState, Behavior, ObjectKind};
    use super::projectile_tests::audio;
    let catalog = authored_paths::catalog();
    for has_beam in [false, true] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(constructor(&catalog));
        objects.get_mut(owner).unwrap().base.hit_points = 5;
        let player = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let beam = has_beam.then(|| {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::from_catalog_index(20),
                    Behavior::Effect,
                ))
                .unwrap()
        });
        if let Some(beam) = beam {
            objects
                .get_mut(beam)
                .unwrap()
                .extension
                .path_state
                .motion_phase = 0xABFE;
        }
        let mut shared = EncounterCoordination {
            progress: 255,
            ..Default::default()
        };
        let mut signals = EncounterSignals { raised: 0xA550 };
        let mut control = PlayerTargetControl::default();
        let mut events = AudioState::default();
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.coordination = Some(&mut shared);
        inputs.encounter_signals = Some(&mut signals);
        inputs.primary_player = Some(player);
        inputs.primary_control = Some(PrimaryControl {
            target: &mut control,
            linked_mode: false,
        });
        inputs.audio = Some(audio(&mut events));
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let children: Vec<_> = objects
            .active_objects()
            .filter(|(_, a)| a.base.shape == ShapeId::from_catalog_index(68))
            .map(|(id, _)| id)
            .collect();
        for (index, child) in children.into_iter().enumerate() {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, child, &mut inputs, 40)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs, 1);
            objects.get_mut(child).unwrap().base.hit_points = 0;
            callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs, 2);
            let actor = objects.get(child).unwrap();
            assert_eq!(actor.base.attachment, Some(owner));
            assert_eq!(actor.base.hit_points, 1);
            assert!(actor.base.flags.collision_disabled);
            assert_eq!(objects.get(owner).unwrap().base.hit_points, 4 - index as u8);
            if let Some(beam) = beam {
                assert_eq!(
                    objects.get(beam).unwrap().extension.path_state.motion_phase,
                    0xAB00 | u16::from(254_u8.wrapping_add(index as u8 + 1))
                );
            }
            // The forced replacement runs after callbacks, then removes the
            // death and firing registrations without clearing the restored link.
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, child, &mut inputs, 30)
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
            assert!(objects
                .get(child)
                .unwrap()
                .extension
                .path_state
                .triggers
                .entries(&runtime.resources, child)
                .unwrap()
                .is_empty());
            assert!(!runtime.begin_callbacks(&objects, child).unwrap());
            assert_eq!(objects.get(owner).unwrap().base.hit_points, 4 - index as u8);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 40)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(
                inputs.encounter_signals.as_deref().unwrap().raised,
                if index == 3 { 0xA551 } else { 0xA550 }
            );
        }
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.velocity.y, 20);
        assert_eq!(actor.base.wait_timer, 1);
        for expected in 2..=15 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 20)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(objects.get(owner).unwrap().base.wait_timer, expected);
        }
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 20)
                .unwrap()
                .step,
            ControlStep::Ended
        );
        assert_eq!(inputs.encounter_signals.as_deref().unwrap().raised, 0xA551);
    }
}

#[test]
fn encounter_release_reveals_three_effects_then_publishes_delayed_exit() {
    use super::super::path_player_control::{PlayerTargetControl, PrimaryControl};
    use super::super::{Behavior, ObjectKind};
    let catalog = authored_paths::catalog();
    for initial_heading in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::FOUR_TURRET_ENCOUNTER);
        actor.base.yaw = Angle::from_units(initial_heading);
        let player = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let mut shared = EncounterCoordination {
            progress: 255,
            ..Default::default()
        };
        let mut signals = EncounterSignals::default();
        let mut handoff = EncounterHandoff {
            player_flags: 3,
            heading_word: 0xCD00,
            ..Default::default()
        };
        let mut control = PlayerTargetControl::default();
        let mut inputs = world(&mut random);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.coordination = Some(&mut shared);
        inputs.encounter_signals = Some(&mut signals);
        inputs.handoff = Some(&mut handoff);
        inputs.primary_player = Some(player);
        inputs.primary_control = Some(PrimaryControl {
            target: &mut control,
            linked_mode: false,
        });
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        inputs.encounter_signals.as_deref_mut().unwrap().raised = 1;
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let actor = objects.get(owner).unwrap();
        assert!(actor.base.flags.visible);
        assert!(!actor.base.contacts.suppress_contacts_next_epoch);
        assert!(actor.extension.path_state.hold_latched);
        let children: Vec<_> = objects
            .active_objects()
            .filter(|(_, a)| a.base.attachment == Some(owner))
            .map(|(id, _)| id)
            .collect();
        assert_eq!(children.len(), 2);
        assert_eq!(objects.len(), 6);
        for (shape, path, height) in [
            (88, authored_paths::FOOTPRINT_YAW_EFFECT, 300),
            (124, authored_paths::RESET_ANIMATION_YAW_EFFECT, 308),
        ] {
            let actor = objects
                .active_objects()
                .find(|(_, a)| a.base.shape == ShapeId::from_catalog_index(shape))
                .unwrap()
                .1;
            assert_eq!(actor.base.path, Some(path));
            assert_eq!(actor.extension.relative_position.y, height);
        }
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        objects.get_mut(owner).unwrap().base.hit_points = 0;
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 2);
        assert_eq!(objects.get(owner).unwrap().base.hit_points, 1);
        assert!(objects.get(owner).unwrap().base.flags.collision_disabled);
        assert_eq!(
            inputs.handoff.as_deref().unwrap().heading_word,
            0xCD00 | u16::from(initial_heading.wrapping_add(2))
        );
        for elapsed in 1..=10 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 30)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(objects.get(owner).unwrap().base.wait_timer, elapsed);
            assert_eq!(inputs.handoff.as_deref().unwrap().player_flags, 3);
        }
        for elapsed in 1..=30 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 30)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(objects.get(owner).unwrap().base.wait_timer, elapsed);
            assert_eq!(inputs.handoff.as_deref().unwrap().player_flags, 3);
        }
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 30)
                .unwrap()
                .step,
            ControlStep::Ended
        );
        assert_eq!(inputs.handoff.as_deref().unwrap().player_flags, 0x43);
        assert_eq!(inputs.primary_control.as_ref().unwrap().target.range, 8);
        // Both authored children are numbered one. The cleanup command marks
        // only the first chain match; it does not unlink or retire either now.
        assert_eq!(
            children
                .iter()
                .filter(|id| objects.get(**id).unwrap().base.flags.remove_after_tick)
                .count(),
            1
        );
        assert_eq!(objects.len(), 6);
    }
}
