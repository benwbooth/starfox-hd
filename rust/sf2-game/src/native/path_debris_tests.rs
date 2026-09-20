//! Authored debris graphs, with explicit path/movement/callback scheduling.
use super::super::{authored_paths, path_motion, Angle, Vector3};
use super::effect_tests::callbacks;
use super::tests::{setup, world};
use super::*;

fn movement(runtime: &mut PathRuntime, objects: &mut ObjectStore, owner: ObjectId, death: bool) {
    let pending = if death {
        runtime.begin_movement_tail(objects, owner).unwrap()
    } else {
        runtime
            .begin_movement(objects, owner, path_motion::PlayerDisplacement::default())
            .unwrap()
    };
    assert!(!pending);
    runtime.finish_movement(objects, &mut [None, None]).unwrap();
}

#[test]
fn randomized_debris_consumes_five_draws_moves_ten_times_and_skips_final_ordinary_movement() {
    let catalog = authored_paths::catalog();
    for first in [0, 1] {
        for last in 0..=255 {
            let (mut runtime, mut objects, owner, _) = setup();
            let mut random = RandomState::new([first, 0, 0, last]);
            let mut expected_random = random;
            let delta = Vector3 {
                x: i16::from(expected_random.next_byte() as i8 / 2),
                y: i16::from(expected_random.next_byte() as i8 / 2),
                z: i16::from(expected_random.next_byte() as i8 / 2),
            };
            let yaw_step = (expected_random.next_byte() & 15) + 16;
            let pitch_step = (expected_random.next_byte() & 15) + 208;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::RANDOMIZED_TUMBLING_DEBRIS);
            actor.base.hit_points = 100;
            actor.base.position = Vector3 {
                x: i16::MIN,
                y: i16::MAX,
                z: 123,
            };
            actor.base.velocity = Vector3 {
                x: -7,
                y: 11,
                z: -13,
            };
            actor.base.pitch = Angle::from_units(211);
            actor.base.yaw = Angle::from_units(251);
            let mut expected_position = actor.base.position;
            let velocity = actor.base.velocity;
            let mut inputs = world(&mut random);
            for visit in 1u8..=10 {
                let death = visit == 10;
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 24)
                        .unwrap()
                        .step,
                    if death {
                        ControlStep::MovementTail
                    } else {
                        ControlStep::Movement
                    }
                );
                expected_position.x = expected_position.x.wrapping_add(delta.x);
                expected_position.y = expected_position.y.wrapping_add(delta.y);
                expected_position.z = expected_position.z.wrapping_add(delta.z);
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.position, expected_position);
                assert_eq!(
                    actor.base.pitch.units(),
                    211u8.wrapping_add(pitch_step.wrapping_mul(visit))
                );
                assert_eq!(
                    actor.base.yaw.units(),
                    251u8.wrapping_add(yaw_step.wrapping_mul(visit))
                );
                assert_eq!(actor.base.hit_points, if death { 0 } else { 100 });
                assert!(!actor.base.flags.suppress_death_effects);
                assert!(actor.base.flags.collision_disabled);
                assert!(!actor.base.flags.remove_after_tick);
                assert_eq!(inputs.random, &expected_random);
                movement(&mut runtime, &mut objects, owner, death);
                if !death {
                    expected_position.x = expected_position.x.wrapping_add(velocity.x);
                    expected_position.y = expected_position.y.wrapping_add(velocity.y);
                    expected_position.z = expected_position.z.wrapping_add(velocity.z);
                }
                assert_eq!(objects.get(owner).unwrap().base.position, expected_position);
            }
        }
    }
}

#[test]
fn hit_released_arcs_share_twenty_samples_and_preserve_relative_death_tail_motion() {
    const ARC: [i16; 20] = [
        -100, -81, -64, -49, -36, -25, -16, -9, -4, -1, 1, 4, 9, 16, 25, 36, 49, 64, 81, 100,
    ];
    let catalog = authored_paths::catalog();
    for (root, increment) in [
        (authored_paths::HIT_RELEASED_ARC_ATTACHMENT, 0),
        (
            authored_paths::PHASE_INCREMENTED_HIT_RELEASED_ARC_ATTACHMENT,
            1,
        ),
    ] {
        for relative in [false, true] {
            for offset in [i16::MIN, -481, -3, 0, 3, 481, i16::MAX] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(root);
                actor.base.hit_points = 100;
                actor.base.pitch = Angle::from_units(250);
                actor.base.position.y = i16::MIN;
                actor.extension.relative_position = Vector3 {
                    x: offset,
                    y: 123,
                    z: offset.wrapping_neg(),
                };
                actor.extension.relative_rotation.yaw = Angle::from_units(247);
                actor.extension.path_state.motion.relative_coordinates = relative;
                actor.extension.path_state.motion_phase = 0xA5FF;
                let original_random = random;
                let mut inputs = world(&mut random);
                for visit in 1..=19 {
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 16)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(
                        actor.extension.path_state.motion_phase,
                        0xA500 | u16::from(255u8.wrapping_add(increment))
                    );
                    assert_eq!(
                        actor.extension.path_state.animation.shape.fixed_frame(),
                        Some(visit % 16)
                    );
                    assert!(actor.base.flags.casts_shadow);
                    assert!(actor.base.contacts.run_when_paused);
                    movement(&mut runtime, &mut objects, owner, false);
                }
                objects
                    .get_mut(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .conditions
                    .hit_event_pending = true;
                let mut expected_position = objects.get(owner).unwrap().base.position;
                let mut expected_relative = Vector3 {
                    x: offset / 4,
                    y: 123,
                    z: offset.wrapping_neg() / 4,
                };
                let velocity = Vector3 {
                    x: expected_relative.x,
                    y: 0,
                    z: expected_relative.z,
                };
                for (index, height) in ARC.into_iter().enumerate() {
                    let death = index == 19;
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 32)
                            .unwrap()
                            .step,
                        if death {
                            ControlStep::MovementTail
                        } else {
                            ControlStep::Movement
                        }
                    );
                    expected_position.y = expected_position.y.wrapping_add(height);
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.base.position, expected_position);
                    assert_eq!(actor.base.velocity, velocity);
                    assert_eq!(
                        actor.extension.path_state.motion_phase,
                        0xA500 | ((index + 1) % 20) as u16
                    );
                    assert_eq!(
                        actor.base.pitch.units(),
                        250u8.wrapping_add(247u8.wrapping_mul(index as u8 + 1))
                    );
                    assert_eq!(actor.base.hit_points, if death { 0 } else { 100 });
                    assert!(!actor.extension.path_state.conditions.hit_event_pending);
                    assert!(!actor.base.flags.remove_after_tick);
                    movement(&mut runtime, &mut objects, owner, death);
                    if relative {
                        // The death command skips ordinary integration but
                        // still reaches relative integration in the footer.
                        expected_relative.x = expected_relative.x.wrapping_add(velocity.x);
                        expected_relative.z = expected_relative.z.wrapping_add(velocity.z);
                    } else if !death {
                        expected_position.x = expected_position.x.wrapping_add(velocity.x);
                        expected_position.z = expected_position.z.wrapping_add(velocity.z);
                    }
                    assert_eq!(objects.get(owner).unwrap().base.position, expected_position);
                    assert_eq!(
                        objects.get(owner).unwrap().extension.relative_position,
                        expected_relative
                    );
                    assert_eq!(inputs.random, &original_random);
                }
            }
        }
    }
}

#[test]
fn contact_arc_cancels_both_callbacks_before_fifteen_sample_motion_and_death() {
    const ARC: [i16; 15] = [-32, -24, -18, -12, -8, -4, -2, 0, 0, 2, 4, 8, 12, 18, 24];
    let catalog = authored_paths::catalog();
    for seed in 0..=255 {
        let (mut runtime, mut objects, owner, _) = setup();
        let mut random = RandomState::new([1, seed, 19, 251]);
        let mut expected_random = random;
        let yaw_step = (expected_random.next_byte() & 63).wrapping_add(224);
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::CONTACT_RELEASED_ARC_ATTACHMENT);
        actor.base.hit_points = 100;
        actor.base.position = Vector3 {
            x: i16::MIN,
            y: -100,
            z: 251,
        };
        actor.base.yaw = Angle::from_units(250);
        actor.extension.relative_position.x = -483;
        let mut inputs = world(&mut random);
        for visit in 1..=10 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 8)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            assert_eq!(
                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                1
            );
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .animation
                    .shape
                    .fixed_frame(),
                Some(visit % 8)
            );
            assert!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .hold_latched
            );
        }
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending = true;
        assert_eq!(
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
            2
        );
        let mut expected_position = objects.get(owner).unwrap().base.position;
        for (index, height) in ARC.into_iter().enumerate() {
            let death = index == 14;
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 24)
                    .unwrap()
                    .step,
                if death {
                    ControlStep::MovementTail
                } else {
                    ControlStep::Movement
                }
            );
            expected_position.y = expected_position.y.wrapping_add(height);
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.position, expected_position);
            assert_eq!(actor.base.velocity.x, -120);
            assert_eq!(actor.extension.relative_position.x, -120);
            assert_eq!(
                actor.base.yaw.units(),
                250u8.wrapping_add(yaw_step.wrapping_mul(index as u8 + 1))
            );
            assert_eq!(
                actor.extension.path_state.motion_phase,
                u16::from(yaw_step) << 8 | (index + 3) as u16
            );
            assert!(actor
                .extension
                .path_state
                .triggers
                .entries(&runtime.resources, owner)
                .unwrap()
                .is_empty());
            // FORCE replaces the continuation, not the retained HOLD bit.
            assert!(actor.extension.path_state.hold_latched);
            assert!(!actor.extension.path_state.conditions.hit_event_pending);
            assert_eq!(actor.base.hit_points, if death { 0 } else { 100 });
            assert_eq!(inputs.random, &expected_random);
            movement(&mut runtime, &mut objects, owner, death);
            if !death {
                expected_position.x = expected_position.x.wrapping_sub(120);
            }
            assert_eq!(objects.get(owner).unwrap().base.position, expected_position);
        }
    }
}
