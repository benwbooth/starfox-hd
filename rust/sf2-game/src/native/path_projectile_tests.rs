//! Whole authored contact-projectile/effect graphs, verified without traces
//! or source-machine execution. Physics helpers have separate arithmetic tests.
use super::super::collision_pass::ExclusionGroups;
use super::super::path_control::PlayerTarget;
use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
use super::super::path_sound::{CueListener, CueMarker, MarkerInputs, PathAudio};
use super::super::path_steering::{face, FacingCommand, FacingTargets, SteeringState};
use super::super::path_triggers::TriggerKind;
use super::super::{
    authored_paths, path_motion, Angle, AudioState, Behavior, ObjectKind, ShapeId, SpatialLoop,
    Vector3,
};
use super::tests::{setup, world};
use super::*;

fn player(objects: &mut ObjectStore, position: Vector3) -> ObjectId {
    let mut actor = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
    actor.base.position = position;
    objects.allocate(actor).unwrap()
}

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
                    runtime.resume_program(catalog, objects, owner, inputs, 16).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(ControlStep::ResumeCallbacks)
                );
            }
            CallbackStep::Skipped | CallbackStep::Expired => {}
            CallbackStep::Complete => return count,
        }
    }
    panic!("projectile callbacks did not finish");
}

#[test]
fn height_staged_projectile_saturates_vertical_drift_until_published_height_is_crossed() {
    use super::super::{path_motion::PublishedPlayerMotion, ObjectSpawnDefaults};
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let selected = player(&mut objects, Vector3 { x: 0, y: 0, z: 10000 });
    objects.get_mut(owner).unwrap().base.path = Some(authored_paths::HEIGHT_STAGED_HOMING_PROJECTILE);
    let mut events = AudioState::default();
    let mut inputs = world(&mut random);
    inputs.selected = Some(selected);
    inputs.published_motion = Some(PublishedPlayerMotion { position: Vector3 { x: 0, y: 10000, z: 0 }, ..Default::default() });
    inputs.audio = Some(audio(&mut events));
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    let mut height = 0i16;
    let mut drift = 10i16;
    for visit in 1..=25 {
        assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 20).unwrap().step,
            ControlStep::Movement);
        height += drift;
        if drift != 100 { drift += 5; }
        if visit == 10 {
            // Final NEXT does not yield: the post-arc height loop also
            // contributes in this invocation before its GOTO boundary.
            height += drift;
            drift += 5;
        }
        let actor = objects.get(owner).unwrap();
        assert_eq!(actor.base.position.y, height);
        assert_eq!(actor.extension.relative_position.y, drift);
        assert_eq!(actor.base.first_child, None);
        assert_eq!(actor.base.target_speed, 0);
        if visit >= 10 { assert_eq!(actor.extension.path_state.script_value, 10000); }
    }
    assert_eq!(drift, 100);
    // Equality still executes one vertical step, since the source height
    // comparison is strict; only the next invocation enters guidance.
    inputs.published_motion.as_mut().unwrap().position.y = height;
    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 20).unwrap().step,
        ControlStep::Movement);
    height += 100;
    assert_eq!(objects.get(owner).unwrap().base.position.y, height);
    assert_eq!(objects.get(owner).unwrap().base.first_child, None);
    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 20).unwrap().step,
        ControlStep::Movement);
    let actor = objects.get(owner).unwrap();
    assert_eq!(actor.base.position.y, height);
    assert_eq!(actor.base.target_speed, 45);
    assert_eq!(actor.base.acceleration, 5);
    assert_eq!(actor.extension.path_state.script_value, 1);
    assert!(actor.base.first_child.is_some());
    assert_eq!(objects.len(), 3);
    assert_eq!(super::effect_tests::cues(&mut inputs), [116]);
}

#[test]
fn height_staged_projectile_both_launch_routes_guidance_exits_and_independent_child() {
    use super::super::{path_motion::PublishedPlayerMotion, ObjectSpawnDefaults};
    let catalog = authored_paths::catalog();
    for above in [false, true] {
        for close in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let selected = player(&mut objects, Vector3 { x: 0, y: if above { -100 } else { 1000 }, z: if close { 1000 } else { 10000 } });
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::HEIGHT_STAGED_HOMING_PROJECTILE);
            actor.base.hit_points = 1;
            actor.base.attack_power = 4;
            let original_random = random;
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.selected = Some(selected);
            inputs.published_motion = Some(PublishedPlayerMotion { position: Vector3 { x: 0, y: if above { -100 } else { 300 }, z: 0 }, ..Default::default() });
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            inputs.audio = Some(audio(&mut events));
            let mut height = 0;
            for visit in 1..=if above { 2 } else { 9 } {
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 20).unwrap().step,
                    ControlStep::Movement);
                let actor = objects.get(owner).unwrap();
                if above {
                    assert_eq!(actor.base.pitch.units(), 192);
                    assert!(actor.base.first_child.is_some());
                } else {
                    height += 5 + 5 * visit;
                    assert_eq!(actor.base.position.y, height);
                    assert_eq!(actor.extension.relative_position.y, 10 + 5 * visit);
                    assert_eq!(actor.base.first_child, None);
                }
                assert!(actor.extension.path_state.motion.generate_velocity_each_step);
                assert!(actor.extension.path_state.motion.quadruple_velocity);
                assert!(actor.extension.path_state.triggers.entries(&runtime.resources, owner).unwrap().is_empty());
            }
            if above {
                // The height gate reads the published snapshot, not selected
                // actor Y. Changing this input admits guidance immediately.
                inputs.published_motion.as_mut().unwrap().position.y = 1;
            }
            // Drive path/callback phases without ordinary integration so
            // near/far guidance stays at its explicitly controlled distance.
            let total = if close { 31 } else { 100 };
            for visit in 1..=total {
                let death = visit == total;
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 24).unwrap().step,
                    if death { ControlStep::MovementTail } else { ControlStep::Movement });
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.position.y, if above { 0 } else { 325 });
                assert_eq!(actor.base.target_speed, 45);
                assert_eq!(actor.base.acceleration, 5);
                assert_eq!(actor.extension.path_state.script_value, if close { 1 } else { visit as u16 });
                assert_eq!(actor.base.wait_timer, if close && !death { visit as u8 } else { 0 });
                assert_eq!(actor.extension.spatial_loop,
                    if above { None } else { SpatialLoop::from_authored_control(2) });
                assert_eq!(actor.base.hit_points, if death { 0 } else { 1 });
                assert_eq!(actor.base.flags.collision_disabled, death);
                assert!(!actor.base.flags.suppress_death_effects);
                assert!(!actor.base.flags.remove_after_tick);
                assert_eq!(callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs), 1);
                assert_eq!(objects.get(owner).unwrap().base.roll.units(), 16u8.wrapping_mul(visit as u8));
                if visit == 1 {
                    let child = objects.get(owner).unwrap().base.first_child.unwrap();
                    let parent_before = objects.get(owner).unwrap().clone();
                    let spawned = objects.get(child).unwrap();
                    assert_eq!((spawned.base.hit_points, spawned.base.attack_power, spawned.base.child_number), (80, 1, 1));
                    assert!(spawned.extension.path_state.needs_path_initialization);
                    runtime.initialize_path_strategy(&mut objects, child).unwrap();
                    for child_visit in 1..=4 {
                        assert_eq!(runtime.enter_program(&catalog, &mut objects, child, &mut inputs, 12).unwrap().step,
                            ControlStep::Movement);
                        let actor = objects.get(child).unwrap();
                        assert_eq!(actor.base.hit_points, 100);
                        assert_eq!(actor.extension.texture_scroll_x, 96);
                        assert_eq!(actor.extension.path_state.animation.color.fixed_frame(), Some(child_visit % 2));
                        assert_eq!(callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs), 0);
                    }
                    assert_eq!(objects.get(owner), Some(&parent_before));
                }
            }
            let child = objects.get(owner).unwrap().base.first_child.unwrap();
            let parent_before = objects.get(owner).unwrap().clone();
            let spawned = objects.get(child).unwrap();
            assert_eq!(spawned.base.shape, ShapeId::from_catalog_index(19));
            // Parent death marks its direct child dead, without unlinking
            // or running that child's independently retained callback.
            assert_eq!((spawned.base.hit_points, spawned.base.attack_power, spawned.base.child_number), (0, 1, 1));
            assert_eq!(spawned.base.attachment, Some(owner));
            assert_eq!(spawned.extension.parent, Some(owner));
            assert_eq!(spawned.extension.relative_position, Vector3 { x: 0, y: 0, z: -480 });
            assert!(!spawned.base.flags.remove_after_tick);
            assert!(spawned.base.flags.suppress_death_effects);
            assert!(!spawned.extension.path_state.needs_path_initialization);
            assert_eq!(callbacks(&mut runtime, &catalog, &mut objects, child, &mut inputs), 1);
            assert_eq!(runtime.enter_program(&catalog, &mut objects, child, &mut inputs, 1).unwrap().step,
                ControlStep::Ended);
            assert!(objects.get(child).unwrap().base.flags.remove_after_tick);
            assert_eq!(objects.get(owner), Some(&parent_before));
            let cues = super::effect_tests::cues(&mut inputs);
            assert_eq!(cues.len(), if above { 0 } else { 1 });
            if !above { assert_eq!(cues[0], 116); }
            assert_eq!(inputs.random, &original_random);
        }
    }
}

#[test]
fn signal_guided_projectile_delays_guidance_and_retires_after_each_exit_cause() {
    let catalog = authored_paths::catalog();
    for distance in [1000, 9999, 10000] {
        // Exit by proximity, shared signal, new contact, or contact during
        // the initial six-visit wait before guidance has been registered.
        for cause in 0..4 {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let selected = player(&mut objects, Vector3 { x: 0, y: 800, z: distance });
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::SIGNAL_GUIDED_PROJECTILE);
            actor.base.pitch = Angle::from_units(32);
            actor.base.yaw = Angle::from_units(64);
            let mut signals = EncounterSignals { raised: 0xA500 };
            let mut events = AudioState::default();
            let original_random = random;
            let mut inputs = world(&mut random);
            inputs.selected = Some(selected);
            inputs.primary_player = Some(selected);
            inputs.audio = Some(audio(&mut events));
            inputs.encounter_signals = Some(&mut signals);
            for visit in 0..if cause == 3 { 3 } else { 6 } {
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 24).unwrap().step, ControlStep::Movement);
                let actor = objects.get(owner).unwrap();
                assert_eq!((actor.base.hit_points, actor.base.attack_power, actor.base.speed), (1, 4, 50));
                assert!(actor.extension.path_state.motion.quadruple_velocity);
                assert!(actor.extension.path_state.motion.generate_velocity_each_step);
                assert!(actor.base.contacts.suppress_hit_marker);
                assert_eq!(actor.extension.spatial_loop, SpatialLoop::from_authored_control(5));
                assert_eq!(actor.base.wait_timer, visit + 1);
                assert_eq!(actor.extension.path_state.triggers.entries(&runtime.resources, owner).unwrap().len(), 1);
                assert_eq!(super::effect_tests::cues(&mut inputs), if visit == 0 { vec![116] } else { vec![] });
                assert_eq!(callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs), 0);
            }
            if cause != 3 {
                for visit in 0..3 {
                    assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 12).unwrap().step, ControlStep::Movement);
                    assert_eq!(objects.get(owner).unwrap().base.wait_timer, 0);
                    let old_pitch = objects.get(owner).unwrap().base.pitch;
                    assert_eq!(callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs), 1);
                    let actor = objects.get(owner).unwrap();
                    // The authored boundary is strict: distant guidance is
                    // yaw-only, while closer guidance also chases pitch.
                    if distance == 10000 { assert_eq!(actor.base.pitch, old_pitch); }
                    else if visit == 0 { assert_ne!(actor.base.pitch, old_pitch); }
                    assert_eq!(actor.extension.path_state.triggers.entries(&runtime.resources, owner).unwrap().len(), 2);
                }
            }
            match cause {
                0 => objects.get_mut(selected).unwrap().base.position.z = 999,
                1 => inputs.encounter_signals.as_deref_mut().unwrap().raised |= 8,
                _ => objects.get_mut(owner).unwrap().base.contacts.new_contact_latched = true,
            }
            if cause != 0 {
                assert_eq!(callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs), if cause == 2 { 2 } else { 1 });
                objects.get_mut(owner).unwrap().base.contacts.new_contact_latched = false;
            }
            // FORCE clears the elapsed timer even for early contact during
            // the initial wait, so each exit gets the full twenty visits.
            for timer in 0..=20 {
                let death = timer == 20;
                assert_eq!(runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8).unwrap().step,
                    if death { ControlStep::MovementTail } else { ControlStep::Movement });
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.wait_timer, if death { 0 } else { timer + 1 });
                assert_eq!(actor.base.hit_points, if death { 0 } else { 1 });
                assert_eq!(actor.base.flags.collision_disabled, death);
                assert!(!actor.base.flags.suppress_death_effects);
                assert!(!actor.base.flags.remove_after_tick);
                let entries = actor.extension.path_state.triggers.entries(&runtime.resources, owner).unwrap();
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].kind, TriggerKind::NewContact);
                assert_eq!(callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs), 0);
            }
            assert_eq!(inputs.random, &original_random);
            assert!(super::effect_tests::cues(&mut inputs).is_empty());
            assert_eq!(inputs.encounter_signals.as_deref().unwrap().raised, if cause == 1 { 0xA508 } else { 0xA500 });
        }
    }
}

#[test]
fn authored_random_texture_contact_sprite_samples_once_then_clears_health_and_holds() {
    let catalog = authored_paths::catalog();
    for elapsed in 0..=u8::MAX {
        for seed in [0, 1, 2, 3, 127, 255] {
            for each_step in [false, true] {
                let (mut runtime, mut objects, owner, _) = setup();
                let mut random = RandomState::new([1, 71, 131, seed]);
                let mut expected_random = random;
                let size = (expected_random.next_byte() & 3).wrapping_sub(1);
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::RANDOM_TEXTURE_CONTACT_SPRITE);
                actor.base.hit_points = 1;
                actor.base.attack_power = 4;
                actor.base.wait_timer = elapsed;
                actor.base.pitch = Angle::from_units(5);
                actor.base.yaw = Angle::from_units(69);
                actor.base.velocity = Vector3 {
                    x: -17,
                    y: 313,
                    z: -555,
                };
                actor.extension.depth_offset = 0xABCD;
                actor.extension.texture_scroll_x = 239;
                actor
                    .extension
                    .path_state
                    .motion
                    .generate_velocity_each_step = each_step;
                actor.base.contacts.exclusion_groups = ExclusionGroups::from_authored_class(255);
                actor.base.contacts.suppress_attack_damage = true;
                let retained_velocity = actor.base.velocity;
                let wait_yields = usize::from(30u8.wrapping_sub(elapsed));
                for visit in 0..=wait_yields {
                    assert_eq!(
                        runtime.enter_program(
                            &catalog,
                            &mut objects,
                            owner,
                            &mut world(&mut random),
                            16
                        ).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(ControlStep::Movement)
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.extension.texture_scroll_x, size);
                    assert_eq!(actor.extension.depth_offset, 0xAB00);
                    assert!(actor.base.flags.scaled_sprite);
                    assert_eq!(
                        actor.base.hit_points,
                        if visit == wait_yields { 0 } else { 1 }
                    );
                    assert_eq!(actor.base.attack_power, 4);
                    assert!(!actor.base.flags.remove_after_tick);
                    assert_eq!(
                        actor.extension.path_state.hold_latched,
                        visit == wait_yields
                    );
                    assert_eq!(
                        actor.base.wait_timer,
                        if visit == wait_yields {
                            0
                        } else {
                            elapsed.wrapping_add(visit as u8 + 1)
                        }
                    );
                    assert_eq!(actor.base.speed, 60);
                    assert!(actor.extension.path_state.motion.quadruple_velocity);
                    assert_eq!(
                        actor.base.velocity,
                        if each_step {
                            retained_velocity
                        } else {
                            path_motion::direction_velocity(
                                Angle::from_units(5),
                                Angle::from_units(69),
                                60,
                                4,
                            )
                        }
                    );
                    assert_eq!(
                        actor.base.contacts.exclusion_groups,
                        ExclusionGroups::from_authored_class(239)
                    );
                    assert!(actor.base.contacts.suppress_attack_damage);
                    assert_eq!(random, expected_random);
                }
                let held = objects.get(owner).unwrap().clone();
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
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }
}

#[test]
fn authored_distance_aimed_projectile_preserves_far_pitch_and_retires_after_plane_crossing_delay() {
    let catalog = authored_paths::catalog();
    for distance in [0, 9999, 10000, 10001, 20000] {
        for elapsed in [0, 4, 5, 6, 255] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let selected = player(
                &mut objects,
                Vector3 {
                    x: 0,
                    y: 1500,
                    z: -distance,
                },
            );
            // Plane projection doubles its displacement as a word. Turn
            // the far fixture around so overflow stays on the waiting side.
            if distance > 16383 {
                objects.get_mut(selected).unwrap().base.yaw = Angle::from_units(128);
            }
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(authored_paths::DISTANCE_AIMED_PROJECTILE);
            actor.base.pitch = Angle::from_units(201);
            actor.base.yaw = Angle::from_units(37);
            actor.base.roll = Angle::from_units(222);
            actor.base.hit_points = 100;
            actor.base.attack_power = 8;
            actor.base.wait_timer = elapsed;
            let before_player = objects.get(selected).unwrap().clone();
            let mut aimed = objects.clone();
            face(
                &mut aimed,
                owner,
                FacingCommand::SelectedImmediate,
                FacingTargets {
                    selected: Some(selected),
                    primary: Some(selected),
                    fixed_players: [Some(selected); 2],
                },
                &mut SteeringState::default(),
            )
            .unwrap();
            let aimed = aimed.get(owner).unwrap();
            let expected_pitch = if distance < 10000 {
                aimed.base.pitch
            } else {
                Angle::from_units(201)
            };
            let mut events = AudioState::default();
            let mut inputs = world(&mut random);
            inputs.selected = Some(selected);
            inputs.audio = Some(audio(&mut events));
            assert_eq!(
                runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 32).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                Ok(ControlStep::Movement)
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.pitch, expected_pitch);
            assert_eq!(actor.base.yaw, aimed.base.yaw);
            assert_eq!(actor.base.roll.units(), 222);
            assert_eq!(
                actor.base.velocity,
                path_motion::direction_velocity(expected_pitch, aimed.base.yaw, 100, 4)
            );
            assert_eq!(
                actor.extension.material_set,
                Some(super::super::render::MaterialSetId::from_catalog_token(
                    0x8404
                ))
            );
            assert_eq!(
                actor.extension.spatial_loop,
                SpatialLoop::from_authored_control(15)
            );
            assert_eq!(actor.base.wait_timer, elapsed);
            assert!(actor.extension.path_state.hold_latched);
            let held = actor.base.path;
            assert_eq!(
                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                1
            );
            assert_eq!(objects.get(owner).unwrap().base.path, held);
            assert_eq!(objects.get(selected).unwrap(), &before_player);
            // Changing only the selected pose crosses its forward plane;
            // the callback redirects but does not itself retire the actor.
            objects.get_mut(selected).unwrap().base.position.z = 1;
            objects.get_mut(selected).unwrap().base.yaw = Angle::ZERO;
            assert_eq!(
                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                1
            );
            assert_ne!(objects.get(owner).unwrap().base.path, held);
            // Forced redirection clears the wait and repeat counters even
            // when the old main path was held with an arbitrary wait byte.
            assert_eq!(objects.get(owner).unwrap().base.wait_timer, 0);
            let waits = 5;
            for visit in 0..=waits {
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 8).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(if visit == waits {
                        ControlStep::Ended
                    } else {
                        ControlStep::Movement
                    })
                );
                let actor = objects.get(owner).unwrap();
                assert_eq!(actor.base.flags.remove_after_tick, visit == waits);
                assert_eq!(
                    actor
                        .extension
                        .path_state
                        .triggers
                        .entries(&runtime.resources, owner)
                        .unwrap()
                        .len(),
                    0
                );
                assert_eq!(actor.base.hit_points, 100);
                assert_eq!(
                    actor.extension.spatial_loop,
                    SpatialLoop::from_authored_control(15)
                );
            }
            let cues = inputs
                .audio
                .as_mut()
                .unwrap()
                .events
                .take_events()
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            assert_eq!(cues.len(), 1);
            assert!(matches!(cues[0], super::super::SoundEvent::Authored(cue) if cue.id == 175));
            runtime.release_actor_programs(&mut objects, owner).unwrap();
        }
    }
}

#[test]
fn authored_delayed_contact_projectile_uses_eight_offsets_then_arms_and_expires_on_callback_timers()
{
    let catalog = authored_paths::catalog();
    let offsets: [(u8, u8); 8] = [
        (240, 0),
        (16, 0),
        (0, 16),
        (0, 240),
        (240, 240),
        (16, 240),
        (240, 16),
        (16, 16),
    ];
    for (pattern, (yaw_offset, pitch_offset)) in offsets.into_iter().enumerate() {
        for contact_visit in [0, 6, 7, 8, 9, 49] {
            for mark_only in [false, true] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let selected = player(
                    &mut objects,
                    Vector3 {
                        x: 300,
                        y: -400,
                        z: -1000,
                    },
                );
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::DELAYED_CONTACT_PROJECTILE);
                actor.base.hit_points = 10;
                actor.base.attack_power = pattern as u8;
                actor.base.roll = Angle::from_units(199);
                let mut aimed = objects.clone();
                face(
                    &mut aimed,
                    owner,
                    FacingCommand::SelectedImmediate,
                    FacingTargets {
                        selected: Some(selected),
                        primary: Some(selected),
                        fixed_players: [Some(selected); 2],
                    },
                    &mut SteeringState::default(),
                )
                .unwrap();
                let aimed = aimed.get(owner).unwrap();
                let pitch = Angle::from_units(aimed.base.pitch.units().wrapping_add(pitch_offset));
                let yaw = Angle::from_units(aimed.base.yaw.units().wrapping_add(yaw_offset));
                let mut inputs = world(&mut random);
                inputs.selected = Some(selected);
                // Registration on seven does not extend that pass. Expiry
                // on eight decrements the pass budget twice and skips the
                // last entry. Contact first runs on nine, as in 9ADE/9D89.
                let ends_after = if !mark_only && contact_visit >= 9 {
                    contact_visit
                } else {
                    50
                };
                for visit in 1..=ends_after {
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 32).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(ControlStep::Movement)
                    );
                    let actor = objects.get_mut(owner).unwrap();
                    assert_eq!(
                        (actor.base.pitch, actor.base.yaw, actor.base.roll.units()),
                        (pitch, yaw, 199)
                    );
                    assert_eq!(
                        actor.base.velocity,
                        path_motion::direction_velocity(pitch, yaw, 90, 1)
                    );
                    assert_eq!((actor.base.hit_points, actor.base.attack_power), (1, 4));
                    assert!(actor.base.contacts.suppress_hit_marker);
                    if visit == 1 {
                        assert_eq!(
                            actor.extension.path_state.motion_phase,
                            (pattern as u16 * 2) | u16::from(pitch_offset) << 8
                        );
                    }
                    actor.base.contacts.new_contact_latched = visit == contact_visit;
                    if mark_only && visit == contact_visit {
                        // The contact callback samples the live high phase,
                        // not the earlier angle-table value.
                        actor.extension.path_state.motion_phase =
                            (actor.extension.path_state.motion_phase & 255) | 256;
                    }
                    let waiting = actor.base.path;
                    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs);
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.base.flags.collision_disabled, visit < 7);
                    assert_eq!(
                        actor.base.contacts.hit_marked,
                        mark_only && contact_visit >= 9 && visit >= contact_visit
                    );
                    assert_eq!(actor.base.path != waiting, visit == ends_after, "pattern={pattern} contact_visit={contact_visit} mark_only={mark_only} visit={visit}");
                    assert!(!actor.base.flags.remove_after_tick);
                    if visit == 7 {
                        assert_eq!(
                            actor
                                .extension
                                .path_state
                                .triggers
                                .entries(&runtime.resources, owner)
                                .unwrap()
                                .iter()
                                .filter(|t| t.kind == TriggerKind::NewContact)
                                .count(),
                            1
                        );
                    }
                }
                assert_eq!(
                    runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 1).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                    Ok(ControlStep::Ended)
                );
                assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }
}

#[test]
fn authored_randomized_guided_projectile_preserves_rng_sign_order_yaw_gate_and_thirty_pass_loop() {
    let catalog = authored_paths::catalog();
    let mut observed = [false; 5];
    for seed in 0..=u8::MAX {
        for aim in [false, true] {
            for break_visit in [1, 7, 30, 31] {
                let (mut runtime, mut objects, owner, _) = setup();
                let mut random = RandomState::new([seed, 71, 131, seed]);
                let mut expected_random = random;
                let (pitch, yaw, phase): (u8, u8, u16) = if expected_random.next_byte() < 127 {
                    observed[0] = true;
                    (7, 70, 0xA755)
                } else {
                    let first: u8 = if expected_random.next_byte() < 127 {
                        16
                    } else {
                        240
                    };
                    let second = if expected_random.next_byte() < 127 {
                        first
                    } else {
                        first.wrapping_neg()
                    };
                    observed[1 + usize::from(first == 240) * 2 + usize::from(second == 240)] = true;
                    (
                        7u8.wrapping_add(first),
                        70u8.wrapping_add(second),
                        0xA700 | u16::from(second),
                    )
                };
                let selected = player(
                    &mut objects,
                    Vector3 {
                        x: 0,
                        y: 500,
                        z: -1000,
                    },
                );
                objects.get_mut(selected).unwrap().base.yaw =
                    Angle::from_units(if aim { 0 } else { 32 });
                let actor = objects.get_mut(owner).unwrap();
                actor.base.path = Some(authored_paths::RANDOMIZED_YAW_GUIDED_PROJECTILE);
                actor.base.pitch = Angle::from_units(7);
                actor.base.yaw = Angle::from_units(70);
                actor.base.roll = Angle::from_units(199);
                actor.base.hit_points = 251;
                actor.base.attack_power = 253;
                actor.base.wait_timer = 193;
                actor.base.velocity = Vector3 {
                    x: 17,
                    y: -123,
                    z: 511,
                };
                actor.extension.path_state.motion_phase = 0xA755;
                let mut aimed = objects.clone();
                face(
                    &mut aimed,
                    owner,
                    FacingCommand::SelectedImmediate,
                    FacingTargets {
                        selected: Some(selected),
                        primary: Some(selected),
                        fixed_players: [Some(selected); 2],
                    },
                    &mut SteeringState::default(),
                )
                .unwrap();
                let aimed = aimed.get(owner).unwrap();
                let mut events = AudioState::default();
                let mut inputs = world(&mut random);
                inputs.selected = Some(selected);
                inputs.audio = Some(audio(&mut events));
                let mut has_aimed = false;
                for visit in 1..=break_visit.min(30) {
                    if visit == break_visit {
                        objects.get_mut(selected).unwrap().base.yaw = Angle::from_units(128);
                    }
                    let terminal = visit == 30 || visit == break_visit;
                    if aim && visit != break_visit {
                        has_aimed = true;
                    }
                    assert_eq!(
                        runtime.enter_program(&catalog, &mut objects, owner, &mut inputs, 48).map(|exit| { assert_eq!(exit.actor, owner); exit.step }),
                        Ok(if terminal {
                            ControlStep::Ended
                        } else {
                            ControlStep::Movement
                        })
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(
                        (actor.base.pitch.units(), actor.base.yaw.units()),
                        if has_aimed {
                            (aimed.base.pitch.units(), aimed.base.yaw.units())
                        } else {
                            (pitch, yaw)
                        }
                    );
                    assert_eq!(actor.base.roll.units(), 199);
                    assert_eq!(actor.extension.path_state.motion_phase, phase);
                    assert_eq!(
                        (
                            actor.base.hit_points,
                            actor.base.attack_power,
                            actor.base.speed
                        ),
                        (100, 4, 100)
                    );
                    assert!(
                        actor
                            .extension
                            .path_state
                            .motion
                            .generate_velocity_each_step
                    );
                    assert!(actor.extension.path_state.motion.quadruple_velocity);
                    assert_eq!(
                        actor.base.velocity,
                        Vector3 {
                            x: 17,
                            y: -123,
                            z: 511
                        }
                    );
                    assert_eq!(actor.base.wait_timer, 193);
                    assert_eq!(actor.base.flags.remove_after_tick, terminal);
                    assert_eq!(inputs.random, &expected_random);
                }
                let cues = inputs
                    .audio
                    .as_mut()
                    .unwrap()
                    .events
                    .take_events()
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                assert_eq!(cues.len(), 1);
                assert!(
                    matches!(cues[0], super::super::SoundEvent::Authored(cue) if cue.id == 175)
                );
                runtime.release_actor_programs(&mut objects, owner).unwrap();
            }
        }
    }
    assert_eq!(observed, [true; 5]);
}
