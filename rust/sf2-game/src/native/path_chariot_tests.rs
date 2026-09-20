//! Heavy Chariot authored graph and rotated-offset spawn contracts.
use super::super::path_control::PlayerTarget;
use super::super::path_relationships::find_child;
use super::super::path_scene_state::{EncounterCoordination, EncounterHealthDisplay};
use super::super::path_spawn::{IndependentSpawn, OffsetSpawn};
use super::super::weapon_launch::MuzzleOffset;
use super::super::{
    authored_paths, Angle, AudioState, Behavior, ObjectKind, ObjectSpawnDefaults, PathId, Rotation,
    ShapeId, Vector3, OBJECT_CAPACITY,
};
use super::paired_patrol_tests::callbacks;
use super::projectile_tests::audio;
use super::tests::{setup, world};
use super::*;

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

// Independent signed-magnitude products from the source byte multipliers.
// Doubling happens at byte width, including the -128 -> zero corner case.
fn product(value: i8, coefficient: i8) -> i8 {
    let magnitude = (((i16::from(value).abs() * 2) & 255) * i16::from(coefficient).abs()) / 256;
    if (value < 0) != (coefficient < 0) {
        -magnitude as i8
    } else {
        magnitude as i8
    }
}
fn pair(first: i8, second: i8, angle: u8) -> (i8, i8) {
    let c = sf_core::snes_trig::COSTAB[usize::from(angle)];
    let s = sf_core::snes_trig::SINTAB[usize::from(angle)];
    (
        product(first, c).wrapping_add(product(second, s)),
        product(second, c).wrapping_sub(product(first, s)),
    )
}
fn source_offset(position: Vector3, rotation: Rotation, offset: MuzzleOffset) -> Vector3 {
    let (x, y) = pair(offset.x, offset.y, rotation.roll.units());
    let (y, z) = pair(y, offset.z, rotation.pitch.units());
    let (x, z) = pair(x, z, rotation.yaw.units().wrapping_neg());
    Vector3 {
        x: position.x.wrapping_add(i16::from(x) * 4),
        y: position.y.wrapping_add(i16::from(y) * 4),
        z: position.z.wrapping_add(i16::from(z) * 4),
    }
}

#[test]
fn offset_spawn_preserves_word_wrap_byte_rotation_and_default_player_selection() {
    for value in 0..=u8::MAX {
        for rotation in [
            Rotation::default(),
            Rotation {
                pitch: Angle::from_units(value),
                yaw: Angle::from_units(value.wrapping_add(37)),
                roll: Angle::from_units(value.wrapping_add(91)),
            },
        ] {
            for inverted in [false, true] {
                let parameters = OffsetSpawn {
                    actor: IndependentSpawn {
                        shape: ShapeId::from_catalog_index(167),
                        path: Some(at(1)),
                        hit_points: value,
                        attack_power: !value,
                    },
                    offset: MuzzleOffset {
                        x: value as i8,
                        y: value.rotate_left(3) as i8,
                        z: !value as i8,
                    },
                    rotation: Rotation {
                        pitch: Angle::from_units(173),
                        yaw: Angle::from_units(211),
                        roll: Angle::from_units(251),
                    },
                };
                let catalog = PathCatalog::new(vec![vec![
                    Statement::SpawnOffset {
                        kind: ObjectKind::Effect,
                        parameters,
                        next: at(1),
                    },
                    Statement::Control(ControlCommand::End),
                ]])
                .unwrap();
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.position = Vector3 {
                    x: i16::MIN,
                    y: i16::MAX,
                    z: i16::MIN,
                };
                actor.base.pitch = rotation.pitch;
                actor.base.yaw = rotation.yaw;
                actor.base.roll = rotation.roll;
                actor.extension.path_state.conditions.selected_player = PlayerTarget::Secondary;
                actor.extension.spawn_group = 171;
                actor.base.velocity = Vector3 {
                    x: -43,
                    y: 19,
                    z: -13,
                };
                let mut expected = objects.clone();
                let before_random = random;
                let defaults = ObjectSpawnDefaults {
                    group: 87,
                    run_when_paused: true,
                };
                let mut child = Object::new_authored(
                    ObjectKind::Effect,
                    parameters.actor.shape,
                    Behavior::FollowPath,
                    defaults,
                );
                child.extension.path_state.needs_path_initialization = true;
                child.base.path = parameters.actor.path;
                child.base.position = source_offset(
                    objects.get(owner).unwrap().base.position,
                    rotation,
                    parameters.offset,
                );
                child.base.pitch = Angle::from_units(rotation.pitch.units().wrapping_add(173));
                child.base.yaw = Angle::from_units(rotation.yaw.units().wrapping_add(211));
                child.base.roll = Angle::from_units(rotation.roll.units().wrapping_add(251));
                child.base.hit_points = value;
                child.base.attack_power = !value;
                child.extension.spawn_group = 171;
                let created = expected.allocate_scoped_after(owner, child).unwrap();
                expected.get_mut(owner).unwrap().base.path = Some(at(1));
                runtime.branch.invert_next = inverted;
                let mut inputs = world(&mut random);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::MissingSpawnDefaults)
                );
                inputs.spawn_defaults = Some(defaults);
                assert_eq!(
                    runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                    Err(ProgramError::BudgetExceeded {
                        cursor: at(1),
                        executed: 1
                    })
                );
                assert_eq!(objects, expected);
                assert_eq!(runtime.spawns.last_spawn, Some(created));
                assert_eq!(runtime.branch.invert_next, inverted);
                assert_eq!(random, before_random);
            }
        }
    }
}

#[test]
fn offset_spawn_full_pool_retains_last_selection_and_does_not_execute_child() {
    let (mut runtime, mut objects, owner, mut random) = setup();
    while objects.len() < OBJECT_CAPACITY {
        objects
            .allocate(Object::new(
                ObjectKind::Enemy,
                ShapeId::EMPTY,
                Behavior::FollowPath,
            ))
            .unwrap();
    }
    runtime.spawns.last_spawn = Some(owner);
    let parameters = OffsetSpawn {
        actor: IndependentSpawn {
            shape: ShapeId::EMPTY,
            path: Some(at(1)),
            hit_points: 1,
            attack_power: 2,
        },
        offset: MuzzleOffset {
            x: -128,
            y: -1,
            z: 127,
        },
        rotation: Rotation::default(),
    };
    let catalog = PathCatalog::new(vec![vec![
        Statement::SpawnOffset {
            kind: ObjectKind::Effect,
            parameters,
            next: at(1),
        },
        Statement::Control(ControlCommand::End),
    ]])
    .unwrap();
    let mut expected = objects.clone();
    expected.get_mut(owner).unwrap().base.path = Some(at(1));
    let before_random = random;
    let mut inputs = world(&mut random);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::BudgetExceeded {
            cursor: at(1),
            executed: 1
        })
    );
    assert_eq!(objects, expected);
    assert_eq!(runtime.spawns.last_spawn, Some(owner));
    assert_eq!(random, before_random);
}

#[test]
fn chariot_full_attack_cycle_keeps_open_close_parts_three_shots_and_visit_boundaries() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.path = Some(authored_paths::HEAVY_CHARIOT);
    actor.base.position = Vector3 {
        x: 1024,
        y: -200,
        z: 9216,
    };
    let selected = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap();
    objects.get_mut(selected).unwrap().base.position = Vector3 {
        x: 1024,
        y: 79,
        z: 10216,
    };
    let mut display = EncounterHealthDisplay::default();
    let mut shared = EncounterCoordination {
        active_messages: 1,
        ..Default::default()
    };
    let mut events = AudioState::default();
    let before_random = random;
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut shared);
    inputs.health_display = Some(&mut display);
    inputs.selected = Some(selected);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    inputs.audio = Some(audio(&mut events));
    let mut shot_count = 0;
    for visit in 1..=104 {
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
        let actor = objects.get(owner).unwrap();
        let before_position = actor.base.position;
        let mut before_rotation = Rotation {
            pitch: actor.base.pitch,
            yaw: actor.base.yaw,
            roll: actor.base.roll,
        };
        if visit == 75 {
            // The last of the fifteen facing iterations returns and emits
            // the first shot in the SAME visit, before its five facing steps.
            let delta = before_rotation.yaw.units().wrapping_neg() as i8;
            let step = if delta < 0 {
                delta.min(-4)
            } else if delta > 0 {
                delta.max(4)
            } else {
                0
            } / 4;
            before_rotation.yaw =
                Angle::from_units(before_rotation.yaw.units().wrapping_add(step as u8));
        }
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement,
            "visit {visit}"
        );
        assert_eq!(objects.get(owner).unwrap().base.position, before_position);
        assert_eq!(
            objects.get(owner).unwrap().base.speed,
            if visit <= 60 || visit == 104 { 40 } else { 0 }
        );
        if [75, 80, 85].contains(&visit) {
            shot_count += 1;
            let shot = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
            assert_eq!(
                shot.base.position,
                source_offset(
                    before_position,
                    before_rotation,
                    MuzzleOffset {
                        x: 0,
                        y: -32,
                        z: 32
                    }
                )
            );
            assert_eq!(
                (
                    shot.base.shape,
                    shot.base.hit_points,
                    shot.base.attack_power
                ),
                (ShapeId::from_catalog_index(167), 100, 4)
            );
            assert_eq!(
                shot.base.path,
                Some(authored_paths::SURFACE_LIMITED_BALLISTIC_EFFECT)
            );
            assert_eq!(shot.base.attachment, None);
            assert_eq!(shot.extension.parent, None);
            assert_eq!(
                (shot.base.pitch, shot.base.yaw, shot.base.roll),
                (
                    before_rotation.pitch,
                    before_rotation.yaw,
                    before_rotation.roll
                )
            );
            let muzzle = find_child(&objects, owner, 9).unwrap().unwrap();
            assert_eq!(
                objects.get(muzzle).unwrap().extension.relative_position,
                Vector3 {
                    x: 0,
                    y: -40,
                    z: 120
                }
            );
        }
        assert_eq!(objects.len(), 4 + 2 * shot_count, "visit {visit}");
        for (number, sign, shape) in [(1, 1, 128), (2, -1, 129)] {
            let child = objects
                .get(find_child(&objects, owner, number).unwrap().unwrap())
                .unwrap();
            let distance = if visit <= 6 {
                272 - (visit - 1) * 32
            } else if visit <= 61 {
                112
            } else if visit <= 66 {
                112 + (visit - 61) * 32
            } else {
                272
            };
            assert_eq!(
                child.extension.relative_position,
                Vector3 {
                    x: sign * distance,
                    y: -112,
                    z: 0
                },
                "visit {visit}"
            );
            assert_eq!(child.base.shape, ShapeId::from_catalog_index(shape));
            assert_eq!((child.base.hit_points, child.base.attack_power), (10, 4));
        }
        assert_eq!(
            inputs.health_display.as_deref().unwrap(),
            &EncounterHealthDisplay {
                current: 30,
                maximum: 30,
                label: Some("HEAVY CHARIOT")
            }
        );
    }
    assert_eq!(shot_count, 3);
    assert_eq!(inputs.random, &before_random);
}

#[test]
fn chariot_activation_gate_is_exactly_one_and_retired_sentinel_precedes_display() {
    let catalog = authored_paths::catalog();
    for gate in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::HEAVY_CHARIOT);
        let mut shared = EncounterCoordination {
            active_messages: gate,
            ..Default::default()
        };
        let mut display = EncounterHealthDisplay::default();
        let mut events = AudioState::default();
        let before_random = random;
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        inputs.health_display = Some(&mut display);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.audio = Some(audio(&mut events));
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(objects.len(), if gate == 1 { 3 } else { 1 });
        assert_eq!(
            inputs.health_display.as_ref().unwrap().label,
            if gate == 1 {
                Some("HEAVY CHARIOT")
            } else {
                None
            }
        );
        if gate != 1 {
            assert!(!objects.get(owner).unwrap().base.flags.visible);
            inputs.coordination.as_mut().unwrap().active_messages = 1;
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(objects.len(), 3);
        }
        assert_eq!(random, before_random);
    }
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(authored_paths::HEAVY_CHARIOT);
    let mut shared = EncounterCoordination {
        progress: 254,
        active_messages: 1,
        ..Default::default()
    };
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut shared);
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
            .unwrap()
            .step,
        ControlStep::Ended
    );
    assert_eq!(objects.len(), 1);
}

#[test]
fn chariot_contact_threshold_keeps_signed_wrap_cooldown_and_complete_death_tail() {
    use super::super::path_player_control::{PlayerTargetControl, PrimaryControl};
    use super::super::path_score::PlayerScore;
    use super::super::path_triggers::TriggerKind;
    let catalog = authored_paths::catalog();
    for health in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::HEAVY_CHARIOT);
        let primary = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::PlayerFlight,
            ))
            .unwrap();
        let mut shared = EncounterCoordination {
            progress: 255,
            active_messages: 1,
            ..Default::default()
        };
        let mut display = EncounterHealthDisplay::default();
        let mut events = AudioState::default();
        let mut control = PlayerTargetControl::default();
        let mut score = PlayerScore::from_parts(65400, 171);
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        inputs.health_display = Some(&mut display);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs.audio = Some(audio(&mut events));
        inputs.primary_player = Some(primary);
        inputs.selected = Some(primary);
        inputs.primary_control = Some(PrimaryControl {
            target: &mut control,
            linked_mode: false,
        });
        inputs.selected_score = Some(&mut score);
        let mut objective_counts = super::super::path_scene_state::EncounterObjectiveCounts { remaining_word: 0, ..Default::default() };
        inputs.objective_counts = Some(&mut objective_counts);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let contacts: Vec<_> = objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .triggers
            .entries(&runtime.resources, owner)
            .unwrap()
            .iter()
            .filter(|t| t.kind == TriggerKind::NewContact)
            .copied()
            .collect();
        assert_eq!(contacts.len(), 2);
        runtime.clear_triggers(&mut objects, owner).unwrap();
        for trigger in contacts {
            runtime.add_trigger(&mut objects, owner, trigger).unwrap();
        }
        objects.get_mut(owner).unwrap().base.hit_points = health;
        objects
            .get_mut(owner)
            .unwrap()
            .base
            .contacts
            .new_contact_latched = true;
        let before_random = *inputs.random;
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        objects
            .get_mut(owner)
            .unwrap()
            .base
            .contacts
            .new_contact_latched = false;
        let dying = 40_u8.wrapping_sub(health) as i8 >= 0;
        let adjusted = health.wrapping_sub(40);
        let adjusted = if adjusted == 1 { 2 } else { adjusted };
        assert_eq!(
            inputs.health_display.as_ref().unwrap().current,
            ((adjusted as i8) / 2) as u8
        );
        if dying {
            let mut expected_random = before_random;
            for _ in 0..15 {
                expected_random.next_byte();
            }
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
                assert_eq!(objects.len(), 4 + if visit < 15 { 0 } else { visit - 14 });
                if visit >= 15 {
                    let fade = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                    assert_eq!((fade.base.hit_points, fade.base.attack_power), (100, 8));
                    assert!(fade.base.contacts.run_when_paused);
                }
            }
            assert_eq!(inputs.primary_control.as_ref().unwrap().target.mode, 2);
            assert_eq!(
                inputs.primary_control.as_ref().unwrap().target.owner,
                Some(owner)
            );
            assert_eq!(
                inputs.selected_score.as_ref().unwrap().points(),
                171 * 65536 + 65535
            );
            assert_eq!(inputs.coordination.as_ref().unwrap().progress, 0);
            assert_eq!(objects.get(owner).unwrap().base.hit_points, 0);
            assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
            assert_eq!(inputs.random, &expected_random);
        } else {
            assert!(objects.get(owner).unwrap().base.flags.collision_disabled);
            for tick in 1..=5 {
                callbacks(
                    &mut runtime,
                    &catalog,
                    &mut objects,
                    owner,
                    &mut inputs,
                    tick,
                );
                assert_eq!(
                    objects.get(owner).unwrap().base.flags.collision_disabled,
                    tick < 4
                );
            }
            assert_eq!(objects.get(owner).unwrap().base.hit_points, health);
            assert_eq!(inputs.coordination.as_ref().unwrap().progress, 255);
            assert_eq!(objects.len(), 4);
            assert_eq!(inputs.random, &before_random);
        }
    }
}

#[test]
fn chariot_armor_callback_reflects_contacts_and_scrolls_without_replacing_last_spawn() {
    use super::super::collision_contacts::ContactStore;
    use super::super::weapon_dispatch::WeaponState;
    use super::super::weapon_reflection::ReflectionRules;
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(authored_paths::HEAVY_CHARIOT);
    let mut shared = EncounterCoordination {
        active_messages: 1,
        ..Default::default()
    };
    let mut display = EncounterHealthDisplay::default();
    let mut events = AudioState::default();
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut shared);
    inputs.health_display = Some(&mut display);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    inputs.audio = Some(audio(&mut events));
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    let armor = find_child(&objects, owner, 1).unwrap().unwrap();
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, armor, &mut inputs, 100)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    assert!(
        objects
            .get(armor)
            .unwrap()
            .base
            .contacts
            .suppress_contacts_next_epoch
    );
    let original_spawn = runtime.spawns.last_spawn;
    let primary = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap();
    let secondary = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap();
    let mut incoming = Object::new(
        ObjectKind::Projectile,
        ShapeId::from_catalog_index(17),
        Behavior::FollowPath,
    );
    incoming.base.contacts.credits_hit_side = true;
    let incoming = objects.allocate(incoming).unwrap();
    let mut contacts = ContactStore::default();
    contacts.record_pair(armor, incoming, [None, None]).unwrap();
    let mut weapons = WeaponState::default();
    inputs.primary_player = Some(primary);
    inputs.secondary_player = Some(secondary);
    inputs.contacts = Some(&contacts);
    inputs.weapons = Some(&mut weapons);
    inputs.reflection = Some(ReflectionRules {
        owner: armor,
        process_all: false,
        player_scatter: None,
    });
    // Collision cleanup, not path entry, copies the suppression flag into
    // the live skip-contacts gate. Supply that distinct epoch transition.
    callbacks(&mut runtime, &catalog, &mut objects, armor, &mut inputs, 1);
    assert_eq!(objects.len(), 6);
    assert_eq!(objects.get(armor).unwrap().extension.texture_scroll_y, 252);
    objects.get_mut(armor).unwrap().base.contacts.skip_contacts = true;
    callbacks(&mut runtime, &catalog, &mut objects, armor, &mut inputs, 2);
    assert_eq!(objects.len(), 7);
    assert_eq!(objects.get(armor).unwrap().extension.texture_scroll_y, 248);
    assert!(objects.get(incoming).unwrap().base.flags.collision_disabled);
    let reflected = objects.get(armor).unwrap().base.linked_object.unwrap();
    assert_eq!(
        objects.get(reflected).unwrap().base.shape,
        ShapeId::from_catalog_index(17)
    );
    assert_eq!(runtime.spawns.last_spawn, original_spawn);
}
