//! Complete targeting-upgrade parent and independently scheduled children.
use super::super::path_radio::{PathRadio, RadioLayout, RadioRequest};
use super::super::path_target::TargetingUpgradeState;
use super::super::{
    authored_paths, AudioState, Behavior, ObjectKind, ObjectSpawnDefaults, ShapeId, Vector3,
};
use super::effect_tests::{audio, cues};
use super::tests::{setup, world};
use super::*;

#[test]
fn owned_upgrade_ends_without_demanding_skipped_world_inputs_or_spawning() {
    let catalog = authored_paths::catalog();
    for flags in 128..=255 {
        for inverted in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::TARGETING_UPGRADE_PICKUP);
            objects.get_mut(owner).unwrap().base.hit_points = 23;
            runtime.branch.invert_next = inverted;
            let mut upgrade = TargetingUpgradeState { pilot_flags: flags };
            let mut inputs = world(&mut random);
            inputs.targeting_upgrade = Some(&mut upgrade);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 2)
                    .unwrap()
                    .step,
                ControlStep::Ended
            );
            let actor = objects.get(owner).unwrap();
            assert!(actor.base.flags.remove_after_tick);
            assert!(actor.base.flags.visible);
            assert_eq!(actor.base.hit_points, 23);
            assert_eq!(actor.base.first_child, None);
            assert_eq!(objects.len(), 1);
            assert_eq!(
                inputs.targeting_upgrade.as_deref().unwrap().pilot_flags,
                flags
            );
        }
    }
}

#[test]
fn targeting_pickup_independent_children_collection_and_both_delayed_messages() {
    let catalog = authored_paths::catalog();
    for flags in [0, 1, 63, 127] {
        for secondary in [false, true] {
            for initial_wait in [0u8, 19, 20, 21, 255] {
                let (mut runtime, mut objects, owner, mut random) = setup();
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
                // Facing reads the fixed player, while collection reads the
                // current selected actor. These are deliberately distinct.
                objects.get_mut(primary).unwrap().base.position = Vector3 {
                    x: 3000,
                    y: 0,
                    z: 4000,
                };
                objects.get_mut(selected).unwrap().base.position = Vector3 {
                    x: 101,
                    y: 99,
                    z: 0,
                };
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::TARGETING_UPGRADE_PICKUP);
                actor.base.wait_timer = initial_wait;
                actor.base.position = Vector3 { x: 0, y: 0, z: 0 };
                let mut upgrade = TargetingUpgradeState { pilot_flags: flags };
                let mut events = AudioState::default();
                let mut radio = RadioRequest::default();
                let mut inputs = world(&mut random);
                inputs.selected = Some(selected);
                inputs.primary_player = Some(if secondary { primary } else { selected });
                inputs.fixed_players = [Some(primary); 2];
                inputs.targeting_upgrade = Some(&mut upgrade);
                inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                inputs.scene.player_configuration = Some(0);
                inputs.audio = Some(audio(&mut events));
                inputs.radio = Some(PathRadio {
                    request: &mut radio,
                    layout: RadioLayout {
                        compact_panel: secondary,
                        tracked_screen_y: 100,
                    },
                });
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 32)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.hit_points, 100);
                assert!(actor.base.flags.collision_disabled);
                assert_eq!(
                    actor.extension.radar_marker,
                    super::super::radar::RadarMarker::from_packed(130)
                );
                assert_eq!(actor.base.wait_timer, initial_wait);
                let scenery = actor.base.first_child.unwrap();
                let glow = objects.get(scenery).unwrap().base.next_sibling.unwrap();
                for (child, number, shape, entry) in [
                    (scenery, 1, 305, authored_paths::SCENE_MATERIAL_SCENERY),
                    (glow, 2, 516, authored_paths::TARGETING_UPGRADE_GLOW),
                ] {
                    let actor = objects.get(child).unwrap();
                    assert_eq!(
                        (
                            actor.base.child_number,
                            actor.base.shape.catalog_index(),
                            actor.base.path
                        ),
                        (number, shape, Some(entry))
                    );
                    assert_eq!((actor.base.hit_points, actor.base.attack_power), (10, 10));
                    assert_eq!(actor.base.attachment, Some(owner));
                    assert!(!actor.base.flags.remove_after_tick);
                }
                let parent_cursor = objects.get(owner).unwrap().base.path;
                // Children do not execute eagerly inside the parent spawn.
                for visit in 0..4 {
                    for child in [scenery, glow] {
                        assert_eq!(
                            runtime
                                .enter_program(&catalog, &mut objects, child, &mut inputs, 32)
                                .unwrap()
                                .step,
                            ControlStep::Movement
                        );
                    }
                    assert_eq!(objects.get(scenery).unwrap().base.hit_points, 100);
                    assert_eq!(
                        objects
                            .get(glow)
                            .unwrap()
                            .extension
                            .path_state
                            .animation
                            .color
                            .fixed_frame(),
                        Some(if visit % 2 == 0 { 2 } else { 3 })
                    );
                    assert_eq!(objects.get(owner).unwrap().base.path, parent_cursor);
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 8)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                    assert_eq!(objects.len(), 5);
                    assert!(cues(&mut inputs).is_empty());
                }
                // Strict plane boundary: 101+99 is outside, 100+99 inside.
                objects.get_mut(selected).unwrap().base.position.x = 100;
                let child_states = [
                    objects.get(scenery).unwrap().clone(),
                    objects.get(glow).unwrap().clone(),
                ];
                let mut expected_timer = initial_wait;
                let mut first_message = false;
                for visit in 0..=256 {
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 32)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                    let actor = objects.get(owner).unwrap();
                    assert!(!actor.base.flags.visible);
                    assert!(!actor.base.flags.remove_after_tick);
                    assert_eq!(
                        inputs.targeting_upgrade.as_deref().unwrap().pilot_flags,
                        flags | 128
                    );
                    assert_eq!(
                        cues(&mut inputs),
                        if visit == 0 { vec![69] } else { vec![] }
                    );
                    for (child, before) in [scenery, glow].into_iter().zip(&child_states) {
                        let mut expected = before.clone();
                        expected.base.flags.remove_after_tick = true;
                        assert_eq!(objects.get(child).unwrap(), &expected);
                    }
                    if expected_timer == 20 {
                        let request = &inputs.radio.as_ref().unwrap().request;
                        assert!(request.pending);
                        assert_eq!(request.message.index(), 213);
                        assert_eq!(request.panel_y, if secondary { 139 } else { 151 });
                        // The second wait starts in the same invocation.
                        assert_eq!(actor.base.wait_timer, 1);
                        first_message = true;
                        break;
                    }
                    expected_timer = expected_timer.wrapping_add(1);
                    assert_eq!(actor.base.wait_timer, expected_timer);
                    assert!(!inputs.radio.as_ref().unwrap().request.pending);
                }
                assert!(first_message);
                inputs.radio.as_mut().unwrap().request.pending = false;
                for timer in 1..60 {
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 4)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                    assert_eq!(objects.get(owner).unwrap().base.wait_timer, timer + 1);
                    assert!(!inputs.radio.as_ref().unwrap().request.pending);
                }
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 4)
                        .unwrap()
                        .step,
                    ControlStep::Ended
                );
                assert_eq!(inputs.radio.as_ref().unwrap().request.message.index(), 214);
                assert!(inputs.radio.as_ref().unwrap().request.pending);
                assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
                assert_eq!(objects.get(owner).unwrap().base.wait_timer, 0);
                assert_eq!(objects.get(owner).unwrap().base.first_child, Some(scenery));
                assert_eq!(objects.get(scenery).unwrap().base.next_sibling, Some(glow));
                assert_eq!(objects.len(), 5);
            }
        }
    }
}
