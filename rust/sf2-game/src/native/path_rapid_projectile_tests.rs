//! Whole source-authored rapid-shot graphs. No trace or machine execution.
use super::super::path_fields::{ByteField, ByteOperand, BytePart, WordField};
use super::super::path_impact::{ImpactMaterials, ImpactState};
use super::super::path_shots::{
    ActiveShots, FlightOverrideCommand, LinkedShotCount, ProjectileFlightOverride,
};
use super::super::{
    authored_paths, path_motion, Angle, AudioState, Behavior, ObjectKind, ObjectSpawnDefaults,
    ShapeId, Vector3,
};
use super::projectile_tests::{audio, callbacks};
use super::tests::{setup, world};
use super::*;

const SHAPES: &[ShapeId] = &[
    ShapeId::from_catalog_index(359),
    ShapeId::from_catalog_index(138),
    ShapeId::from_catalog_index(406),
    ShapeId::from_catalog_index(407),
];

fn at(command_index: u16) -> PathCursor {
    PathCursor {
        path: super::super::PathId::from_catalog_index(0),
        command_index,
    }
}

#[test]
fn decoded_shape_sequence_preserves_other_fields_and_faults_on_every_invalid_selector() {
    let catalog = PathCatalog::new(vec![vec![Statement::SelectShape {
        selector: ByteField::ScriptParameter,
        shapes: SHAPES,
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let before = objects.clone();
    let random_before = random;
    for index in 0..=u8::MAX {
        for invert in [false, true] {
            objects = before.clone();
            objects
                .get_mut(owner)
                .unwrap()
                .extension
                .path_state
                .script_parameter = index;
            let mut expected = objects.clone();
            let anticipated = if let Some(shape) = SHAPES.get(usize::from(index)) {
                let actor = expected.get_mut(owner).unwrap();
                actor.base.shape = *shape;
                actor.base.path = Some(at(1));
                ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1,
                }
            } else {
                ProgramError::ShapeSelectionOutOfBounds {
                    index,
                    count: SHAPES.len(),
                }
            };
            runtime.branch.invert_next = invert;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(anticipated)
            );
            assert_eq!(objects, expected);
            assert_eq!(runtime.branch.invert_next, invert);
        }
    }
    assert_eq!(random, random_before);
}

#[test]
fn flight_override_producer_and_import_keep_every_byte_and_require_shared_state() {
    let destination = ByteField::WordPart {
        field: WordField::MotionPhase,
        part: BytePart::High,
    };
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects
        .get_mut(owner)
        .unwrap()
        .extension
        .path_state
        .motion_phase = 0xA55A;
    let before = objects.clone();
    let random_before = random;
    for value in 0..=u8::MAX {
        let catalog = PathCatalog::new(vec![vec![
            Statement::ProjectileFlightOverride {
                command: FlightOverrideCommand::Assign(ByteOperand::Literal(value)),
                next: at(1),
            },
            Statement::ProjectileFlightOverride {
                command: FlightOverrideCommand::CopyTo(destination),
                next: at(2),
            },
        ]])
        .unwrap();
        objects = before.clone();
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(ProgramError::MissingProjectileFlightOverride)
        );
        assert_eq!(objects, before);
        for invert in [false, true] {
            objects = before.clone();
            let mut expected = objects.clone();
            destination.write(expected.get_mut(owner).unwrap(), value);
            expected.get_mut(owner).unwrap().base.path = Some(at(2));
            let mut state = ProjectileFlightOverride { code: !value };
            let mut inputs = world(&mut random);
            inputs.projectile_flight_override = Some(&mut state);
            runtime.branch.invert_next = invert;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 2),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(2),
                    executed: 2
                })
            );
            assert_eq!(objects, expected);
            assert_eq!(runtime.branch.invert_next, invert);
            assert_eq!(state.code, value);
        }
    }
    assert_eq!(random, random_before);
}

#[derive(Clone, Copy, Debug)]
struct Profile {
    alternate: bool,
    launch_shape: u16,
    flight_shapes: [u16; 4],
    surface_shapes: [u16; 4],
    group: u8,
    attack: u8,
    cue: u8,
}

const PROFILES: [Profile; 5] = [
    Profile {
        alternate: false,
        launch_shape: 358,
        flight_shapes: [359, 138, 406, 407],
        surface_shapes: [359, 358, 0, 0],
        group: 0,
        attack: 1,
        cue: 38,
    },
    Profile {
        alternate: false,
        launch_shape: 360,
        flight_shapes: [361, 139, 408, 407],
        surface_shapes: [361, 360, 0, 0],
        group: 4,
        attack: 2,
        cue: 39,
    },
    Profile {
        alternate: false,
        launch_shape: 363,
        flight_shapes: [364, 140, 365, 366],
        surface_shapes: [364, 363, 0, 0],
        group: 8,
        attack: 4,
        cue: 58,
    },
    Profile {
        alternate: true,
        launch_shape: 363,
        flight_shapes: [364, 140, 365, 366],
        surface_shapes: [364, 363, 0, 0],
        group: 0,
        attack: 17,
        cue: 58,
    },
    Profile {
        alternate: true,
        launch_shape: 25,
        flight_shapes: [26, 27, 28, 29],
        surface_shapes: [26, 25, 0, 0],
        group: 4,
        attack: 17,
        cue: 40,
    },
];

fn prepare(objects: &mut ObjectStore, owner: ObjectId, profile: Profile) -> ObjectId {
    let mut player = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
    player.base.flags.exclude_from_shape_footprint_search = true;
    let player = objects.allocate(player).unwrap();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.attachment = Some(player);
    actor.base.shape = ShapeId::from_catalog_index(profile.launch_shape);
    actor.base.attack_power = 17;
    actor.base.path = Some(if profile.alternate {
        authored_paths::ALTERNATE_RAPID_IMPACT_PROJECTILE
    } else {
        authored_paths::RAPID_IMPACT_PROJECTILE
    });
    actor.base.position = Vector3 {
        x: 40,
        y: -150,
        z: 80,
    };
    actor.base.roll = Angle::from_units(243);
    actor.extension.relative_rotation.roll = Angle::from_units(17);
    actor.extension.path_state.motion_phase = 0x7D55;
    player
}

#[test]
fn rapid_shot_complete_lifetimes_preserve_authored_shapes_inline_motion_and_linked_count() {
    let catalog = authored_paths::catalog();
    for profile in PROFILES {
        for mode in [0, 1, 8, 128, 255] {
            for override_code in [0, 1, 2, 255] {
                for configuration in [0, 9] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    let player = prepare(&mut objects, owner, profile);
                    let player_before = objects.get(player).unwrap().clone();
                    let extended = override_code == 1 || mode != 0;
                    let mode_seen = if override_code == 1 { 1 } else { mode };
                    let iterations = if extended {
                        if configuration == 9 {
                            25
                        } else {
                            10
                        }
                    } else if profile.alternate {
                        6
                    } else {
                        7
                    };
                    let last_visit = iterations + 2;
                    let shapes = if extended {
                        profile.surface_shapes
                    } else {
                        profile.flight_shapes
                    };
                    let group = profile.group
                        + if extended {
                            if profile.alternate {
                                8
                            } else {
                                12
                            }
                        } else {
                            0
                        };
                    let speed = if profile.alternate { 60 } else { 62 };
                    let velocity =
                        path_motion::direction_velocity(Angle::ZERO, Angle::ZERO, speed, 1);
                    let mut expected_velocity = velocity;
                    let mut expected_position = objects.get(owner).unwrap().base.position;
                    let mut expected_roll = 243u8;
                    let mut shape_index = 0usize;
                    let mut events = AudioState::default();
                    let mut impact = ImpactState::default();
                    let mut shots = ActiveShots::from_count(7);
                    let mut flight = ProjectileFlightOverride {
                        code: override_code,
                    };
                    let before_random = random;
                    let mut inputs = world(&mut random);
                    inputs.linked_shot_count = Some(LinkedShotCount {
                        owner: player,
                        state: &mut shots,
                    });
                    inputs.projectile_flight_override = Some(&mut flight);
                    inputs.impact = Some(&mut impact);
                    inputs.surface_mode =
                        Some(super::super::collision_surface::SurfaceMode { flags: mode });
                    inputs.audio = Some(audio(&mut events));
                    inputs.selected_occupancy_exempt = Some(true);
                    inputs.scene.player_configuration = Some(configuration);
                    for visit in 1..=last_visit {
                        if visit == 1 && profile.alternate && profile.launch_shape != 25 {
                            expected_position.z = expected_position.z.wrapping_add(velocity.z);
                        }
                        if visit == 2 {
                            shape_index = 1;
                            expected_velocity.z = expected_velocity.z.wrapping_mul(2);
                            if profile.alternate {
                                if !extended {
                                    expected_position.z =
                                        expected_position.z.wrapping_add(expected_velocity.z);
                                } else if configuration != 9 {
                                    expected_velocity.z = expected_velocity.z.wrapping_mul(3);
                                }
                            } else {
                                let count = if profile.group == 8 {
                                    1
                                } else {
                                    // IFNOT before the zero-mode comparison
                                    // skips this fourth integration in extended mode.
                                    3 + u16::from(!extended)
                                };
                                expected_position.z = expected_position
                                    .z
                                    .wrapping_add(expected_velocity.z.wrapping_mul(count as i16));
                            }
                        }
                        if visit == 3 {
                            if !extended {
                                expected_velocity.z = expected_velocity.z.wrapping_mul(16);
                            } else if !profile.alternate && configuration != 9 {
                                expected_velocity.z = expected_velocity.z.wrapping_mul(3);
                            }
                        }
                        if visit >= 3 && !profile.alternate {
                            expected_roll = expected_roll.wrapping_add(17);
                        }
                        let outcome = runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 200)
                            .unwrap();
                        assert_eq!(outcome.step, if visit == last_visit { ControlStep::Ended } else { ControlStep::Movement }, "{profile:?} mode={mode} override={override_code} configuration={configuration} visit={visit}");
                        let actor = objects.get(owner).unwrap();
                        assert_eq!(
                            actor.base.velocity, expected_velocity,
                            "velocity {profile:?} extended={extended} visit={visit}"
                        );
                        assert_eq!(
                            actor.base.position, expected_position,
                            "position {profile:?} extended={extended} visit={visit}"
                        );
                        assert_eq!(actor.base.roll.units(), expected_roll);
                        assert_eq!(
                            actor.base.shape,
                            ShapeId::from_catalog_index(shapes[shape_index])
                        );
                        assert_eq!(
                            actor.extension.path_state.script_parameter,
                            group + shape_index as u8 + 1
                        );
                        assert_eq!(
                            actor.extension.path_state.motion_phase,
                            u16::from(mode_seen) << 8 | 0x55
                        );
                        assert_eq!(
                            actor.base.hit_points,
                            if profile.alternate { 100 } else { 120 }
                        );
                        assert_eq!(actor.base.attack_power, profile.attack);
                        assert_eq!(actor.base.flags.remove_after_tick, visit == last_visit);
                        assert_eq!(
                            inputs.linked_shot_count.as_ref().unwrap().state.count(),
                            if visit == last_visit { 7 } else { 8 }
                        );
                        if visit < last_visit {
                            let calls =
                                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs);
                            assert_eq!(
                                calls,
                                usize::from(
                                    (visit >= 3 && !extended)
                                        || (extended
                                            && configuration == 9
                                            && visit >= if profile.alternate { 2 } else { 3 })
                                )
                            );
                            if visit >= 3 && !extended {
                                let callback_count = visit - 2;
                                if callback_count == 1
                                    || callback_count == if profile.alternate { 4 } else { 3 }
                                {
                                    shape_index += 1;
                                }
                                assert_eq!(
                                    objects.get(owner).unwrap().base.shape,
                                    ShapeId::from_catalog_index(shapes[shape_index])
                                );
                                assert_eq!(
                                    objects
                                        .get(owner)
                                        .unwrap()
                                        .extension
                                        .path_state
                                        .script_value,
                                    callback_count
                                );
                            }
                        }
                        assert_eq!(*inputs.random, before_random);
                        assert_eq!(objects.get(player).unwrap(), &player_before);
                    }
                    assert_eq!(super::effect_tests::cues(&mut inputs), [profile.cue]);
                    assert_eq!(objects.len(), 2);
                }
            }
        }
    }
}

#[test]
fn rapid_shot_all_materials_and_contact_stages_release_count_and_spawn_only_ordinary_burst() {
    use super::super::collision_contacts::ContactStore;
    use super::super::program_state::PathStackError;
    let catalog = authored_paths::catalog();
    for profile in [PROFILES[0], PROFILES[4]] {
        for completed_visits in [0, 1, 2, 5] {
            for extended in [false, true] {
                for class in 0..3 {
                    for material in 0..=u8::MAX {
                        let (mut runtime, mut objects, owner, mut random) = setup();
                        let player = prepare(&mut objects, owner, profile);
                        let mut peer =
                            Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::FollowPath);
                        peer.base.flags.exclude_from_shape_footprint_search = true;
                        peer.base.hit_points = if class == 0 { 0 } else { 80 };
                        peer.base.contacts.suppress_contacts_next_epoch = class == 2;
                        let materials = ImpactMaterials {
                            ordinary: Some(material),
                            suppressed: Some(material),
                        };
                        let peer = objects.allocate(peer).unwrap();
                        super::super::path_impact::material_fixture(objects.get_mut(peer).unwrap(), &mut runtime.resources, peer, materials);
                        let peer_before = objects.get(peer).unwrap().clone();
                        let mut contacts = ContactStore::default();
                        contacts.record_pair(owner, peer, [None; 2]).unwrap();
                        let mut events = AudioState::default();
                        let mut impact = ImpactState {
                            material: !material,
                            pair_suppressed: true,
                        };
                        let mut shots = ActiveShots::from_count(7);
                        let mut flight = ProjectileFlightOverride {
                            code: u8::from(extended),
                        };
                        let before_random = random;
                        let mut inputs = world(&mut random);
                        inputs.linked_shot_count = Some(LinkedShotCount {
                            owner: player,
                            state: &mut shots,
                        });
                        inputs.projectile_flight_override = Some(&mut flight);
                        inputs.impact = Some(&mut impact);
                        inputs.surface_mode = Some(Default::default());
                        inputs.audio = Some(audio(&mut events));
                        inputs.contacts = Some(&contacts);
                        inputs.selected_occupancy_exempt = Some(true);
                        inputs.scene.player_configuration = Some(0);
                        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
                        for _ in 0..completed_visits {
                            assert_eq!(
                                runtime
                                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 200)
                                    .unwrap()
                                    .step,
                                ControlStep::Movement
                            );
                            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs);
                        }
                        objects.get_mut(owner).unwrap().base.contacts.pending_hit = true;
                        assert_eq!(
                            runtime
                                .enter_program(&catalog, &mut objects, owner, &mut inputs, 200)
                                .unwrap()
                                .step,
                            ControlStep::Ended
                        );
                        assert_eq!(inputs.linked_shot_count.as_ref().unwrap().state.count(), 7);
                        let actor = objects.get_mut(owner).unwrap();
                        assert!(actor.base.contacts.pending_hit);
                        assert!(actor.base.flags.remove_after_tick);
                        assert_eq!(
                            actor.extension.path_state.motion_phase,
                            (u16::from(extended) << 8) | if class == 1 { 0 } else { 0x55 }
                        );
                        assert_eq!(
                            actor
                                .extension
                                .path_state
                                .stack
                                .next(&mut runtime.resources),
                            Err(PathStackError::MissingLoop)
                        );
                        assert_eq!(objects.get(peer).unwrap(), &peer_before);
                        assert_eq!(objects.len(), if class == 1 { 4 } else { 3 });
                        assert_eq!(runtime.spawns.last_spawn.is_some(), class == 1);
                        let mut cues = vec![profile.cue];
                        if class == 1 {
                            cues.push(match material {
                                1 => 179,
                                2 => 128,
                                3 => 68,
                                4 | 6 => 101,
                                5 => 191,
                                _ => 113,
                            });
                        } else if class == 2 {
                            cues.push(if material == 1 { 35 } else { 158 });
                        }
                        assert_eq!(super::effect_tests::cues(&mut inputs), cues);
                        assert_eq!(*inputs.random, before_random);
                    }
                }
            }
        }
    }
}

#[test]
fn rapid_shot_new_contact_callbacks_force_cleanup_before_and_during_counted_loops() {
    let catalog = authored_paths::catalog();
    for profile in [PROFILES[1], PROFILES[3]] {
        for completed_visits in [1, 2, 3, 5] {
            for count in [0, 1, 127, 255] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let player = prepare(&mut objects, owner, profile);
                let mut events = AudioState::default();
                let mut impact = ImpactState::default();
                let mut shots = ActiveShots::from_count(count);
                let mut flight = ProjectileFlightOverride::default();
                let mut inputs = world(&mut random);
                inputs.linked_shot_count = Some(LinkedShotCount {
                    owner: player,
                    state: &mut shots,
                });
                inputs.projectile_flight_override = Some(&mut flight);
                inputs.impact = Some(&mut impact);
                inputs.surface_mode = Some(Default::default());
                inputs.audio = Some(audio(&mut events));
                inputs.selected_occupancy_exempt = Some(true);
                for visit in 1..=completed_visits {
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 200)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                    if visit < completed_visits {
                        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs);
                    }
                }
                objects
                    .get_mut(owner)
                    .unwrap()
                    .base
                    .contacts
                    .new_contact_latched = true;
                assert!(callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs) >= 1);
                objects
                    .get_mut(owner)
                    .unwrap()
                    .base
                    .contacts
                    .new_contact_latched = false;
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 20)
                        .unwrap()
                        .step,
                    ControlStep::Ended
                );
                assert_eq!(
                    inputs.linked_shot_count.as_ref().unwrap().state.count(),
                    count.wrapping_add(1).saturating_sub(1)
                );
                assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
                assert_eq!(super::effect_tests::cues(&mut inputs), [profile.cue]);
                assert_eq!(runtime.spawns.last_spawn, None);
            }
        }
    }
}

#[test]
fn rapid_shot_surface_latch_retains_previous_pair_suppression_for_burst_decision() {
    let catalog = authored_paths::catalog();
    for profile in [PROFILES[0], PROFILES[4]] {
        for stale_suppression in [false, true] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let player = prepare(&mut objects, owner, profile);
            objects.get_mut(owner).unwrap().base.position = Vector3 {
                x: 0,
                y: -403,
                z: 0,
            };
            let mut surface = Object::new(
                ObjectKind::Enemy,
                ShapeId::from_catalog_index(156),
                Behavior::FollowPath,
            );
            surface.base.hit_points = 30;
            surface.base.contacts.first_strategy_visit = false;
            surface.base.contacts.latch_new_contact = true;
            let surface = objects.allocate(surface).unwrap();
            super::super::path_impact::material_fixture(objects.get_mut(surface).unwrap(), &mut runtime.resources, surface,
                ImpactMaterials { ordinary: Some(1), suppressed: None });
            let mut events = AudioState::default();
            let mut impact = ImpactState {
                material: 99,
                pair_suppressed: stale_suppression,
            };
            let mut shots = ActiveShots::from_count(7);
            let mut flight = ProjectileFlightOverride::default();
            let mut inputs = world(&mut random);
            inputs.linked_shot_count = Some(LinkedShotCount {
                owner: player,
                state: &mut shots,
            });
            inputs.projectile_flight_override = Some(&mut flight);
            inputs.impact = Some(&mut impact);
            inputs.surface_mode = Some(Default::default());
            inputs.audio = Some(audio(&mut events));
            inputs.selected_occupancy_exempt = Some(true);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 200)
                    .unwrap()
                    .step,
                ControlStep::Ended
            );
            assert_eq!(inputs.linked_shot_count.as_ref().unwrap().state.count(), 7);
            assert_eq!(
                inputs.impact.as_deref().unwrap().pair_suppressed,
                stale_suppression
            );
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion_phase,
                u16::from(stale_suppression)
            );
            assert!(
                objects
                    .get(surface)
                    .unwrap()
                    .base
                    .contacts
                    .new_contact_latched
            );
            assert_eq!(runtime.spawns.last_spawn.is_none(), stale_suppression);
            assert_eq!(objects.len(), if stale_suppression { 3 } else { 4 });
            assert_eq!(super::effect_tests::cues(&mut inputs), [profile.cue, 179]);
        }
    }
}

#[test]
fn rapid_shot_ground_and_occupied_cell_exits_cleanup_inside_and_outside_loops() {
    use super::super::world_occupancy::{
        MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle,
    };
    let catalog = authored_paths::catalog();
    let mut occupied = WorldOccupancy::default();
    occupied.apply(
        &MarkerCoverage::from_rectangle(WorldRectangle {
            x: 0,
            z: 0,
            width: 512,
            depth: 512,
        })
        .unwrap(),
        OccupancyChange::Mark,
    );
    for profile in [PROFILES[0], PROFILES[4]] {
        for ground in [false, true] {
            for completed_visits in [0, 1, 2, 4] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let player = prepare(&mut objects, owner, profile);
                let mut events = AudioState::default();
                let mut impact = ImpactState::default();
                let mut shots = ActiveShots::from_count(7);
                let mut flight = ProjectileFlightOverride::default();
                let mut inputs = world(&mut random);
                inputs.linked_shot_count = Some(LinkedShotCount {
                    owner: player,
                    state: &mut shots,
                });
                inputs.projectile_flight_override = Some(&mut flight);
                inputs.impact = Some(&mut impact);
                inputs.surface_mode = Some(super::super::collision_surface::SurfaceMode {
                    flags: u8::from(ground),
                });
                inputs.audio = Some(audio(&mut events));
                inputs.scene.player_configuration = Some(0);
                inputs.selected_occupancy_exempt = Some(true);
                for _ in 0..completed_visits {
                    assert_eq!(
                        runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 200)
                            .unwrap()
                            .step,
                        ControlStep::Movement
                    );
                    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs);
                }
                objects.get_mut(owner).unwrap().base.position = Vector3 {
                    x: 0,
                    y: if ground { -20 } else { -150 },
                    z: 0,
                };
                inputs.selected_occupancy_exempt = Some(ground);
                inputs.occupancy = Some(&occupied);
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 200)
                        .unwrap()
                        .step,
                    ControlStep::Ended
                );
                assert_eq!(inputs.linked_shot_count.as_ref().unwrap().state.count(), 7);
                assert_eq!(objects.len(), 2);
                assert_eq!(runtime.spawns.last_spawn, None);
                let mut cues = vec![profile.cue];
                if !ground {
                    cues.push(158);
                }
                assert_eq!(super::effect_tests::cues(&mut inputs), cues);
            }
        }
    }
}

#[test]
fn rapid_shot_special_character_height_callback_excludes_lower_and_includes_upper_bound() {
    let catalog = authored_paths::catalog();
    for profile in [PROFILES[0], PROFILES[4]] {
        for y in [i16::MIN, -301, -300, -299, -1, 0, 1, i16::MAX] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let player = prepare(&mut objects, owner, profile);
            let mut events = AudioState::default();
            let mut impact = ImpactState::default();
            let mut shots = ActiveShots::from_count(7);
            let mut flight = ProjectileFlightOverride { code: 1 };
            let mut inputs = world(&mut random);
            inputs.linked_shot_count = Some(LinkedShotCount {
                owner: player,
                state: &mut shots,
            });
            inputs.projectile_flight_override = Some(&mut flight);
            inputs.impact = Some(&mut impact);
            inputs.surface_mode = Some(Default::default());
            inputs.audio = Some(audio(&mut events));
            inputs.scene.player_configuration = Some(9);
            inputs.selected_occupancy_exempt = Some(true);
            for _ in 0..if profile.alternate { 2 } else { 3 } {
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, owner, &mut inputs, 200)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
            }
            objects.get_mut(owner).unwrap().base.position.y = y;
            assert_eq!(
                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                1
            );
            let in_range = (-299..=0).contains(&y);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 200)
                    .unwrap()
                    .step,
                if in_range {
                    ControlStep::Movement
                } else {
                    ControlStep::Ended
                }
            );
            assert_eq!(
                inputs.linked_shot_count.as_ref().unwrap().state.count(),
                if in_range { 8 } else { 7 }
            );
            assert_eq!(runtime.spawns.last_spawn, None);
            assert_eq!(super::effect_tests::cues(&mut inputs), [profile.cue]);
        }
    }
}
