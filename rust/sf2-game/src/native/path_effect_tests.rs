//! Complete statically authored child/effect lifecycles. No recorded traces
//! or source-machine execution; ordinary movement remains a scheduler phase.
use super::super::collision_pass::ExclusionGroups;
use super::super::path_control::PlayerTarget;
use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
use super::super::path_sound::{CueListener, CueMarker, MarkerInputs, PathAudio};
use super::super::{
    authored_paths, path_motion, Angle, AudioState, Behavior, ObjectKind, ObjectSpawnDefaults,
    ShapeId, SoundEvent, Vector3,
};
use super::tests::{setup, world};
use super::*;

fn audio(events: &mut AudioState) -> PathAudio<'_> {
    PathAudio {
        events,
        listeners: [CueListener::PrimaryPlayer; 2],
        markers: Some(MarkerInputs {
            selected_sides: [PlayerTarget::Primary; 2],
            markers: [CueMarker {
                identity: CueListener::PrimaryPlayer,
                position: Vector3::default(),
                bearing: Angle::ZERO,
            }; 2],
        }),
    }
}

fn cues(inputs: &mut PathWorld<'_>) -> Vec<u8> {
    inputs
        .audio
        .as_mut()
        .unwrap()
        .events
        .take_events()
        .into_iter()
        .flatten()
        .map(|event| match event {
            SoundEvent::Authored(cue) => cue.id,
            other => panic!("unexpected cue {other:?}"),
        })
        .collect()
}

fn callbacks(
    runtime: &mut PathRuntime,
    catalog: &PathCatalog,
    objects: &mut ObjectStore,
    owner: ObjectId,
    inputs: &mut PathWorld<'_>,
) -> usize {
    assert!(runtime.begin_callbacks(objects, owner).unwrap());
    let mut count = 0;
    for _ in 0..12 {
        match runtime
            .step_callbacks(objects, owner, TriggerWorldInputs::default())
            .unwrap()
        {
            CallbackStep::Run(_) => {
                count += 1;
                assert_eq!(
                    runtime.resume_program(catalog, objects, owner, inputs, 24).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(ControlStep::ResumeCallbacks)
                );
            }
            CallbackStep::Skipped | CallbackStep::Expired => {}
            CallbackStep::Complete => return count,
        }
    }
    panic!("effect callbacks did not finish");
}

#[test]
fn authored_terminal_children_preserve_pose_and_distinguish_retirement_from_hold() {
    let catalog = authored_paths::catalog();
    for root in [
        authored_paths::INVISIBLE_CHILD_RETIREMENT,
        authored_paths::ALTERNATE_INVISIBLE_CHILD_RETIREMENT,
        authored_paths::NONCOLLIDING_PROJECTILE_ATTACHMENT,
        authored_paths::NONCOLLIDING_ROLL_ATTACHMENT,
        authored_paths::NONCOLLIDING_MESH_ATTACHMENT,
        authored_paths::CONTACT_SUPPRESSED_ATTACHMENT,
        authored_paths::CLIPPED_SHADOWLESS_ATTACHMENT,
    ] {
        for value in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let original_random = random;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(root);
            actor.base.flags.visible = value & 1 != 0;
            actor.base.flags.collision_disabled = value & 2 != 0;
            actor.base.flags.casts_shadow = true;
            actor.base.position = Vector3 {
                x: i16::MIN,
                y: 31,
                z: i16::MAX,
            };
            actor.base.velocity = Vector3 {
                x: -11,
                y: 23,
                z: -41,
            };
            actor.base.hit_points = value;
            actor.base.attack_power = value ^ 255;
            actor.base.wait_timer = value;
            actor.extension.path_state.motion_phase = 0xA731;
            actor.extension.relative_position = Vector3 {
                x: 43,
                y: -53,
                z: 67,
            };
            let initial = actor.clone();
            let end = root == authored_paths::INVISIBLE_CHILD_RETIREMENT
                || root == authored_paths::ALTERNATE_INVISIBLE_CHILD_RETIREMENT;
            let suppression = root == authored_paths::CONTACT_SUPPRESSED_ATTACHMENT;
            let clipped = root == authored_paths::CLIPPED_SHADOWLESS_ATTACHMENT;
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                Ok(if end {
                    ControlStep::Ended
                } else {
                    ControlStep::Movement
                })
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.flags.visible, !end && initial.base.flags.visible);
            assert_eq!(
                actor.base.flags.collision_disabled,
                !suppression || initial.base.flags.collision_disabled
            );
            assert_eq!(actor.base.flags.casts_shadow, !clipped);
            assert_eq!(
                actor.base.contacts.suppress_contacts_next_epoch,
                suppression
            );
            assert_eq!(actor.base.flags.remove_after_tick, end);
            assert_eq!(actor.extension.path_state.hold_latched, !end);
            if clipped {
                assert_eq!(actor.extension.clipping_plane.selector_byte(), 4);
            }
            assert_eq!(actor.base.position, initial.base.position);
            assert_eq!(actor.base.velocity, initial.base.velocity);
            assert_eq!(
                actor.extension.relative_position,
                initial.extension.relative_position
            );
            assert_eq!(actor.extension.path_state.motion_phase, 0xA731);
            assert_eq!(
                (
                    actor.base.hit_points,
                    actor.base.attack_power,
                    actor.base.wait_timer
                ),
                (value, value ^ 255, value)
            );
            if !end {
                let held = actor.clone();
                for _ in 0..3 {
                    assert_eq!(
                        runtime.enter_program(
                            &catalog,
                            &mut objects,
                            owner,
                            &mut world(&mut random),
                            1
                        ).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(ControlStep::Movement)
                    );
                    assert_eq!(objects.get(owner).unwrap(), &held);
                }
            }
            assert_eq!(random, original_random);
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}

#[test]
fn authored_ten_tick_child_waits_for_byte_equality_then_ends() {
    let catalog = authored_paths::catalog();
    for elapsed in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::TEN_TICK_EFFECT);
        actor.base.wait_timer = elapsed;
        actor.base.hit_points = 100;
        let count = usize::from(10u8.wrapping_sub(elapsed));
        for visit in 0..=count {
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 3).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                Ok(if visit == count {
                    ControlStep::Ended
                } else {
                    ControlStep::Movement
                })
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                actor.base.wait_timer,
                if visit == count {
                    0
                } else {
                    elapsed.wrapping_add(visit as u8 + 1)
                }
            );
            assert_eq!(actor.base.flags.remove_after_tick, visit == count);
            assert_eq!(actor.base.hit_points, 100);
        }
        runtime.release_actor_programs(&mut objects, owner).unwrap();
    }
}

#[test]
fn authored_fast_contact_mesh_defers_shape_and_speed_until_after_first_yield() {
    let catalog = authored_paths::catalog();
    for elapsed in 0..=u8::MAX {
        for each_step in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::DEFERRED_FAST_CONTACT_MESH);
            actor.base.wait_timer = elapsed;
            actor.base.hit_points = 30;
            actor.base.attack_power = 6;
            actor.base.speed = 19;
            actor.base.pitch = Angle::from_units(7);
            actor.base.yaw = Angle::from_units(73);
            actor.base.velocity = Vector3 {
                x: 123,
                y: -456,
                z: 789,
            };
            actor.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(255);
            actor
                .extension
                .path_state
                .motion
                .generate_velocity_each_step = each_step;
            let initial = actor.clone();
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 8).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                Ok(ControlStep::Movement)
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.shape, initial.base.shape);
            assert_eq!(actor.base.speed, 19);
            assert_eq!(actor.base.velocity, initial.base.velocity);
            assert_eq!(actor.base.wait_timer, elapsed);
            assert_eq!(actor.extension.clipping_plane.selector_byte(), 5);
            assert!(actor.base.contacts.suppress_contacts_next_epoch);
            assert!(actor.extension.path_state.motion.quadruple_velocity);
            assert_eq!(
                actor.base.contacts.exclusion_groups,
                ExclusionGroups::from_authored_class(239)
            );
            let count = usize::from(20u8.wrapping_sub(elapsed));
            for visit in 0..=count {
                assert_eq!(
                    runtime.enter_program(
                        &catalog,
                        &mut objects,
                        owner,
                        &mut world(&mut random),
                        5
                    ).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(if visit == count {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    })
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.shape, ShapeId::from_catalog_index(118));
                assert_eq!(actor.base.speed, 127);
                assert_eq!(
                    actor.base.velocity,
                    if each_step {
                        initial.base.velocity
                    } else {
                        path_motion::direction_velocity(
                            initial.base.pitch,
                            initial.base.yaw,
                            127,
                            4,
                        )
                    }
                );
                assert_eq!((actor.base.hit_points, actor.base.attack_power), (30, 6));
                assert_eq!(actor.base.flags.remove_after_tick, visit == count);
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}

#[test]
fn authored_hit_released_rise_chases_relative_height_and_signals_link_only_after_crossing() {
    let catalog = authored_paths::catalog();
    for start in [
        i16::MIN,
        -32700,
        -1000,
        -500,
        -101,
        -1,
        0,
        1,
        99,
        100,
        101,
        32700,
        i16::MAX,
    ] {
        for linked in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let parent = objects
                .allocate(Object::new(
                    ObjectKind::Enemy,
                    ShapeId::EMPTY,
                    Behavior::FollowPath,
                ))
                .unwrap();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::HIT_RELEASED_RELATIVE_RISE);
            actor.base.attachment = linked.then_some(parent);
            actor.extension.relative_position = Vector3 {
                x: -37,
                y: start,
                z: 79,
            };
            actor.extension.path_state.platform_carry.saved_position = Vector3 {
                x: 41,
                y: 999,
                z: 83,
            };
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.audio = Some(audio(&mut events));
            for _ in 0..3 {
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 10).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(ControlStep::Movement)
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.extension.relative_position.y, start);
                assert_eq!(actor.extension.path_state.script_value, start as u16);
                assert_eq!(
                    actor.extension.path_state.platform_carry.saved_position,
                    Vector3 { x: 41, y: 0, z: 83 }
                );
                assert!(actor.base.flags.far_sort_bias);
                assert!(actor.base.flags.collision_disabled);
            }
            assert!(cues(&mut inputs).is_empty());
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .conditions
                .hit_event_pending = true;
            let mut height = start;
            let mut ended = false;
            for _ in 0..128 {
                // Source chase approaches literal 100 by one eighth of its
                // signed word difference, with at least one unit of progress.
                let difference = 100i16.wrapping_sub(height);
                let step = if difference == 0 {
                    0
                } else if difference > 0 {
                    (difference / 8).max(1)
                } else {
                    (difference / 8).min(-1)
                };
                height = height.wrapping_add(step);
                ended = 0i16.wrapping_sub(height) < 0;
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(if ended {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    }),
                    "start {start}"
                );
                assert_eq!(
                    objects.get(owner).unwrap().extension.relative_position,
                    Vector3 {
                        x: -37,
                        y: height,
                        z: 79
                    }
                );
                assert_eq!(
                    objects
                        .get(parent)
                        .unwrap()
                        .extension
                        .path_state
                        .conditions
                        .hit_event_pending,
                    ended && linked
                );
                if ended {
                    break;
                }
            }
            assert!(ended, "start {start}");
            assert_eq!(cues(&mut inputs), [155]);
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .script_value,
                start as u16
            );
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}

#[test]
fn authored_alternating_sprite_keeps_all_phase_bytes_and_callback_motion_until_final_color_step() {
    let catalog = authored_paths::catalog();
    for health in 0..=u8::MAX {
        for horizontal in [0u8, 1, 127, 128, 129, 255] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::ALTERNATING_DRIFT_SPRITE);
            actor.base.hit_points = health;
            actor.base.wait_timer = 171;
            actor.base.position = Vector3 {
                x: 32760,
                y: -32760,
                z: 919,
            };
            actor.extension.path_state.motion_phase = u16::from_le_bytes([213, horizontal]);
            actor.extension.depth_offset = 0xABCD;
            let (mut low, mut high, mut size) = (health, horizontal, 2u8);
            let mut position = actor.base.position;
            for visit in 1..=25u8 {
                let mut inputs = world(&mut random);
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 12).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(if visit == 25 {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    })
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(
                    actor.extension.path_state.animation.color.packed(),
                    128 | (visit & 1)
                );
                assert_eq!(actor.extension.depth_offset, 0xAB00);
                assert_eq!(actor.extension.texture_scroll_x, size);
                assert_eq!(actor.base.position, position);
                assert_eq!(actor.base.wait_timer, 171);
                assert!(actor.base.flags.collision_disabled);
                if visit != 25 {
                    assert_eq!(
                        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                        1
                    );
                    position.y = position.y.wrapping_sub(30);
                    position.x = position.x.wrapping_add(i16::from(high as i8));
                    size = size.wrapping_add(low).wrapping_add(1);
                    low = low.wrapping_neg();
                    high = high.wrapping_neg();
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.base.position, position);
                    assert_eq!(actor.extension.texture_scroll_x, size);
                    assert_eq!(
                        actor.extension.path_state.motion_phase,
                        u16::from_le_bytes([low, high])
                    );
                }
            }
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}

#[test]
fn authored_hit_released_attachment_waits_then_rotates_rises_and_exits_footprint_search() {
    let catalog = authored_paths::catalog();
    for initial_y in [i16::MIN, -500, 0, 32500, i16::MAX] {
        for yaw in [0u8, 1, 127, 128, 240, 255] {
            for elapsed in [0, 14, 15, 16, 255] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::HIT_RELEASED_ROTATING_ATTACHMENT);
                actor.base.wait_timer = elapsed;
                actor.base.flags.casts_shadow = true;
                actor.base.flags.maximum_draw_distance = true;
                actor.base.flags.exclude_from_shape_footprint_search = true;
                actor.extension.relative_position = Vector3 {
                    x: 37,
                    y: initial_y,
                    z: -71,
                };
                actor.extension.relative_rotation.yaw = Angle::from_units(yaw);
                let mut events = AudioState::default();
                let mut inputs = world(&mut random);
                inputs.audio = Some(audio(&mut events));
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 24).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(ControlStep::Movement)
                );
                let actor = objects.get(owner).unwrap();
                assert!(actor.extension.path_state.hold_latched);
                assert!(!actor.base.flags.exclude_from_shape_footprint_search);
                assert!(!actor.base.flags.casts_shadow);
                assert!(!actor.base.flags.maximum_draw_distance);
                assert!(actor.base.flags.collision_disabled);
                assert!(actor.base.contacts.suppress_contacts_next_epoch);
                assert_eq!((actor.base.hit_points, actor.base.attack_power), (100, 4));
                assert_eq!(actor.base.wait_timer, elapsed);
                assert_eq!(
                    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                    0
                );
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .hit_event_pending = true;
                assert_eq!(
                    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                    1
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.wait_timer, 0); // FORCE resets WAIT and loop counters.
                assert!(!actor.extension.path_state.conditions.hit_event_pending);
                for _ in 0..15 {
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(ControlStep::Movement)
                    );
                    assert_eq!(
                        objects.get(owner).unwrap().extension.relative_position.y,
                        initial_y
                    );
                }
                assert!(cues(&mut inputs).is_empty());
                for visit in 1..=20u8 {
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(if visit == 20 {
                            ControlStep::Ended
                        } else {
                            ControlStep::Movement
                        })
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(
                        actor.extension.relative_position,
                        Vector3 {
                            x: 37,
                            y: initial_y.wrapping_add(i16::from(visit) * 16),
                            z: -71
                        }
                    );
                    assert_eq!(
                        actor.extension.relative_rotation.yaw.units(),
                        yaw.wrapping_add(visit * 4)
                    );
                    assert_eq!(
                        actor.base.flags.exclude_from_shape_footprint_search,
                        visit == 20
                    );
                    assert_eq!(actor.base.flags.remove_after_tick, visit == 20);
                    assert_eq!(actor.extension.clipping_plane.selector_byte(), 1);
                    assert_eq!(
                        cues(&mut inputs),
                        if visit == 1 {
                            vec![149, 186]
                        } else if visit == 20 {
                            vec![203, 205]
                        } else {
                            vec![]
                        }
                    );
                }
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }
}

#[test]
fn authored_height_effect_selects_timed_retirement_or_complete_arc_and_independent_fades() {
    let catalog = authored_paths::catalog();
    let mut saw_timed_random = false;
    let mut saw_arc = false;
    for published_y in [
        i16::MIN,
        -32700,
        -1001,
        -1000,
        -999,
        0,
        999,
        1000,
        1001,
        i16::MAX,
    ] {
        for seed in 0..=u8::MAX {
            for yaw_inside in [false, true] {
                let (mut runtime, mut objects, owner, _) = setup();
                let mut random = RandomState::new([1, 71, 131, seed]);
                let mut expected_random = random;
                let inside = (-1000i16).wrapping_sub(published_y) < 0
                    && 1000i16.wrapping_sub(published_y) >= 0;
                let arc = !inside && expected_random.next_byte() < 127;
                saw_arc |= arc;
                saw_timed_random |= !inside && !arc;
                let mut selected =
                    Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
                selected.base.position = Vector3 {
                    x: 0,
                    y: 20000,
                    z: -10000,
                };
                selected.base.yaw = Angle::from_units(if yaw_inside { 0 } else { 128 });
                let selected = objects.allocate(selected).unwrap();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::HEIGHT_SELECTED_ARC_EFFECT);
                actor.base.position = Vector3 { x: 0, y: 777, z: 0 };
                actor.base.pitch = Angle::from_units(13);
                actor.base.yaw = Angle::from_units(27);
                actor.base.roll = Angle::from_units(97);
                actor.base.wait_timer = seed;
                actor.extension.path_state.motion_phase = 0xA753;
                actor.extension.spawn_group = 29;
                let mut events = AudioState::default();
                let mut inputs = world(&mut random);
                inputs.published_motion = Some(path_motion::PublishedPlayerMotion {
                    position: Vector3 {
                        x: 97,
                        y: published_y,
                        z: 101,
                    },
                    delta: Vector3::default(),
                });
                inputs.selected = Some(selected);
                inputs.audio = Some(audio(&mut events));
                // The timed route does not read allocator inputs.
                if arc {
                    inputs.spawn_defaults = Some(ObjectSpawnDefaults {
                        run_when_paused: true,
                        group: 31,
                    });
                }
                if !arc {
                    let first_wait = usize::from(10u8.wrapping_sub(seed));
                    let total = first_wait + if yaw_inside { 90 } else { 0 };
                    for visit in 0..=total {
                        assert_eq!(
                            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 12).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                            Ok(if visit == total {
                                ControlStep::Ended
                            } else {
                                ControlStep::Movement
                            })
                        );
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.extension.path_state.script_value, published_y as u16);
                        assert_eq!(actor.base.position.y, 777);
                        assert_eq!(actor.base.speed, 10);
                        assert_eq!(actor.extension.clipping_plane.selector_byte(), 1);
                        assert_eq!(actor.base.flags.remove_after_tick, visit == total);
                        assert!(
                            !actor
                                .extension
                                .path_state
                                .motion
                                .generate_velocity_each_step
                        );
                        let decrements = if visit < first_wait {
                            0
                        } else {
                            1 + (visit - first_wait) / 10
                        };
                        assert_eq!(
                            actor.extension.path_state.motion_phase,
                            0xA700 | (10 - decrements as u16)
                        );
                        assert_eq!(objects.len(), 2);
                    }
                    assert!(cues(&mut inputs).is_empty());
                } else {
                    let retained_velocity = path_motion::direction_velocity(
                        Angle::from_units(13),
                        Angle::from_units(27),
                        40,
                        1,
                    );
                    let mut first = None;
                    for visit in 0..=12u8 {
                        assert_eq!(
                            runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 16).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                            Ok(if visit == 12 {
                                ControlStep::Ended
                            } else {
                                ControlStep::Movement
                            })
                        );
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(actor.base.position.y, -1000);
                        assert_eq!(actor.base.pitch.units(), 208u8.wrapping_add(visit * 8));
                        assert_eq!(actor.base.velocity, retained_velocity);
                        assert_eq!(actor.base.wait_timer, seed);
                        assert_eq!(actor.extension.path_state.motion_phase, 0xA753);
                        assert!(
                            actor
                                .extension
                                .path_state
                                .motion
                                .generate_velocity_each_step
                        );
                        assert_eq!(actor.extension.clipping_plane.selector_byte(), 0);
                        assert_eq!(objects.len(), if visit == 12 { 4 } else { 3 });
                        if visit == 0 {
                            first = runtime.spawns.last_spawn;
                        }
                        assert_eq!(
                            cues(&mut inputs),
                            if visit == 0 {
                                vec![137]
                            } else if visit == 12 {
                                vec![136]
                            } else {
                                vec![]
                            }
                        );
                    }
                    let first = first.unwrap();
                    let second = runtime.spawns.last_spawn.unwrap();
                    assert_ne!(first, second);
                    for (child, pitch) in [(first, 13), (second, 48)] {
                        let actor = objects.get(child).unwrap();
                        assert_eq!(
                            actor.base.path,
                            Some(authored_paths::FIXED_SIZE_FADE_SPRITE)
                        );
                        assert_eq!(actor.base.shape, ShapeId::from_catalog_index(22));
                        assert_eq!(
                            actor.base.position,
                            Vector3 {
                                x: 0,
                                y: -1000,
                                z: 0
                            }
                        );
                        assert_eq!(actor.base.pitch.units(), pitch);
                        assert_eq!(actor.base.yaw.units(), 27);
                        assert_eq!(actor.base.roll.units(), 97);
                        assert_eq!(actor.base.attachment, None);
                        assert_eq!((actor.base.hit_points, actor.base.attack_power), (10, 10));
                        assert_eq!(actor.extension.spawn_group, 29);
                        assert!(actor.base.contacts.run_when_paused);
                        assert_eq!(actor.extension.texture_scroll_x, 0); // no eager child dispatch
                        assert!(!actor.base.flags.remove_after_tick);
                        for visit in 1..=8u8 {
                            assert_eq!(
                                runtime.enter_program(
                                    &catalog,
                                    &mut objects,
                                    child,
                                    &mut inputs,
                                    8
                                ).map(|exit| { assert_eq!(exit.actor, child); exit.step }),
                                Ok(if visit == 8 {
                                    ControlStep::Ended
                                } else {
                                    ControlStep::Movement
                                })
                            );
                            assert_eq!(objects.get(child).unwrap().extension.texture_scroll_x, 24);
                            assert_eq!(
                                objects
                                    .get(child)
                                    .unwrap()
                                    .extension
                                    .path_state
                                    .animation
                                    .color
                                    .packed(),
                                128 | (visit & 7)
                            );
                        }
                        runtime.release_actor_programs(&mut objects, child).unwrap();
                    }
                }
                assert_eq!(*inputs.random, expected_random);
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }
    assert!(saw_arc && saw_timed_random);
}
