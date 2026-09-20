//! Four-panel objective scenarios derived from its complete source graph.
use super::super::path_relationships::find_child;
use super::super::path_scene_state::{EncounterCoordination, EncounterHandoff};
use super::super::{authored_paths, ObjectSpawnDefaults, ShapeId, Vector3};
use super::paired_patrol_tests::callbacks;
use super::tests::{setup, world};
use super::*;

fn child_path(catalog: &PathCatalog, shape: u16) -> PathCursor {
    (0..authored_paths::LOWERED_COMMAND_COUNT)
        .find_map(|index| {
            let cursor = PathCursor {
                path: super::super::PathId::from_catalog_index(0),
                command_index: index as u16,
            };
            match catalog.statement(cursor).unwrap() {
                Statement::SpawnChild { parameters, .. }
                    if parameters.shape == ShapeId::from_catalog_index(shape) =>
                {
                    parameters.path
                }
                _ => None,
            }
        })
        .unwrap()
}

#[test]
fn constructor_places_four_numbered_panels_center_and_two_detached_emitter_paths() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.path = Some(authored_paths::FOUR_PANEL_OBJECTIVE);
    actor.base.position = Vector3 {
        x: -32700,
        y: 173,
        z: 32500,
    };
    let mut coordination = EncounterCoordination {
        phase: 255,
        ..Default::default()
    };
    let mut handoff = EncounterHandoff {
        player_flags: 137,
        heading_word: 0xBBAA,
        ..Default::default()
    };
    let before_random = random;
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut coordination);
    inputs.handoff = Some(&mut handoff);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 150)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    assert_eq!(objects.len(), 8);
    let positions = [(96, 0, 192), (-96, 0, 64), (0, 96, 0), (0, -96, 128)];
    for number in 1..=7 {
        let id = find_child(&objects, owner, number).unwrap().unwrap();
        let child = objects.get(id).unwrap();
        assert_eq!(child.base.attachment, Some(owner));
        assert!(child.base.path.is_some());
        if number <= 4 {
            let (x, z, yaw) = positions[usize::from(number - 1)];
            assert_eq!(child.base.shape, ShapeId::from_catalog_index(494));
            assert_eq!(child.extension.relative_position, Vector3 { x, y: 96, z });
            assert_eq!(child.extension.relative_rotation.yaw.units(), yaw);
            assert_eq!((child.base.hit_points, child.base.attack_power), (100, 4));
            assert_eq!(child.extension.path_state.motion_phase, u16::from(number));
        } else if number == 5 {
            assert_eq!(child.base.shape, ShapeId::from_catalog_index(493));
            assert_eq!(
                child.extension.relative_position,
                Vector3 { x: 0, y: 240, z: 0 }
            );
        } else {
            assert_eq!(child.base.shape, ShapeId::from_catalog_index(482));
            let x = if number == 6 { 350 } else { -350 };
            assert_eq!(
                child.extension.relative_position,
                Vector3 { x, y: 40, z: -x }
            );
            assert_eq!((child.base.hit_points, child.base.attack_power), (10, 10));
        }
    }
    assert_eq!(runtime.spawns.parameter, Some(4));
    assert_eq!(inputs.coordination.as_deref().unwrap().phase, 0);
    assert_eq!(
        inputs.handoff.as_deref().unwrap(),
        &EncounterHandoff {
            x: -32700,
            z: 32500,
            player_flags: 137,
            heading_word: 0xBBAA,
        }
    );
    assert_eq!(*inputs.random, before_random);
    assert_eq!(objects.get(owner).unwrap().base.wait_timer, 1);
}

#[test]
fn panel_height_gate_is_live_and_health_threshold_requires_new_contact_with_signed_byte_wrap() {
    let catalog = authored_paths::catalog();
    let entry = child_path(&catalog, 494);
    for health in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(entry);
        objects.get_mut(owner).unwrap().base.hit_points = health;
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase = 0xA557;
        let before_random = random;
        let mut inputs = world(&mut random);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 20)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let held = objects.get(owner).unwrap().base.path;
        for height in [i16::MIN, -32705, -65, -64, -63, 0, 32703, 32704, i16::MAX] {
            objects.get_mut(owner).unwrap().base.position.y = height;
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                actor.base.flags.collision_disabled,
                height.wrapping_add(64) >= 0
            );
            assert_eq!(actor.base.path, held);
            assert_eq!(actor.extension.path_state.motion_phase, 0xA557);
        }
        objects
            .get_mut(owner)
            .unwrap()
            .base
            .contacts
            .new_contact_latched = true;
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.path != held, 80_u8.wrapping_sub(health) < 128);
        assert_eq!(actor.base.hit_points, health);
        assert_eq!(actor.extension.path_state.motion_phase, 0xA550);
        assert_eq!(actor.extension.path_state.script_value, (-64_i16) as u16);
        assert_eq!(*inputs.random, before_random);
        assert_eq!(objects.len(), 1); // Deferred redirection has not executed the break path yet.
    }
}

#[test]
fn emitter_consumes_one_signal_and_reads_live_full_byte_fighter_count() {
    let catalog = authored_paths::catalog();
    let entry = child_path(&catalog, 482);
    for phase in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(entry);
        actor.base.position = Vector3 {
            x: -31100,
            y: 32600,
            z: 27100,
        };
        let mut coordination = EncounterCoordination {
            phase,
            ..Default::default()
        };
        let before_random = random;
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut coordination);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 20)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let held = objects.get(owner).unwrap().base.path;
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        assert_eq!(objects.get(owner).unwrap().base.path, held);
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending = true;
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        assert!(
            !objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .conditions
                .hit_event_pending
        );
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 40)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let spawned = 1_u8.wrapping_sub(phase) < 128;
        assert_eq!(objects.len(), if spawned { 2 } else { 1 });
        assert_eq!(
            inputs.coordination.as_deref().unwrap().phase,
            phase.wrapping_add(u8::from(spawned))
        );
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.extension.path_state.part, 224);
        assert_eq!(
            actor.extension.path_state.motion_phase,
            256 | u16::from(phase)
        );
        assert!(actor.base.flags.collision_disabled);
        if spawned {
            let fighter = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
            assert_eq!(fighter.base.kind, super::super::ObjectKind::Enemy);
            assert_eq!(fighter.base.shape, ShapeId::from_catalog_index(496));
            assert_eq!(
                fighter.base.position,
                Vector3 {
                    x: -31100,
                    y: 32600_i16.wrapping_add(240),
                    z: 27100
                }
            );
            assert_eq!(fighter.base.attachment, None);
            assert_eq!((fighter.base.hit_points, fighter.base.attack_power), (1, 4));
            assert_eq!(fighter.extension.path_state.part, 32);
        }
        assert_eq!(*inputs.random, before_random);
    }
}

#[test]
fn objective_waits_for_live_campaign_gate_then_counts_five_player_contacts_before_redirecting() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(authored_paths::FOUR_PANEL_OBJECTIVE);
    let mut coordination = EncounterCoordination::default();
    let mut handoff = EncounterHandoff::default();
    let mut inputs = world(&mut random);
    inputs.coordination = Some(&mut coordination);
    inputs.handoff = Some(&mut handoff);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    for _ in 0..8 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 150)
                .unwrap()
                .step,
            ControlStep::Movement
        );
    }
    assert!(
        !objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .hold_latched
    );
    inputs.coordination.as_deref_mut().unwrap().progress = 255;
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 50)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    let held = objects.get(owner).unwrap().base.path;
    assert!(
        objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .hold_latched
    );
    for number in [6, 7] {
        let child = find_child(&objects, owner, number).unwrap().unwrap();
        assert!(
            objects
                .get(child)
                .unwrap()
                .extension
                .path_state
                .conditions
                .hit_event_pending
        );
    }
    objects
        .get_mut(owner)
        .unwrap()
        .base
        .contacts
        .new_contact_latched = true;
    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
    assert_eq!(
        objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .motion_phase
            >> 8,
        0
    );
    for contact in 1..=5 {
        objects.get_mut(owner).unwrap().base.hit_points = 19;
        objects.get_mut(owner).unwrap().base.contacts.hit_by_primary = true;
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.hit_points, 100);
        assert_eq!(actor.extension.path_state.motion_phase >> 8, contact);
        assert_eq!(actor.base.path != held, contact == 5);
    }
    assert!(!runtime.branch.invert_next);
    assert_eq!(objects.len(), 8);
}
