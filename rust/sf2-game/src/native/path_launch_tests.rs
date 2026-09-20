//! Static-source launch paths and exhaustive byte-domain leaf tests.
use super::super::path_launch::PilotCraftAppearance;
use super::super::path_motion::PublishedPlayerMotion;
use super::super::path_relationships::find_child;
use super::super::path_scene_state::CameraTrackingTarget;
use super::super::render::MaterialSetId;
use super::super::{authored_paths, Angle, Behavior, ObjectKind, ShapeId, Vector3};
use super::super::{AudioState, ObjectSpawnDefaults};
use super::paired_patrol_tests::callbacks;
use super::projectile_tests::audio;
use super::tests::{setup, world};
use super::*;

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: super::super::PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn authored_leaf(select: impl Fn(Statement) -> bool) -> Statement {
    let catalog = authored_paths::catalog();
    (0..authored_paths::LOWERED_COMMAND_COUNT as u16)
        .map(|index| catalog.statement(at(index)).unwrap())
        .find(|statement| select(*statement))
        .unwrap()
}

fn appearances() -> &'static [PilotCraftAppearance; 6] {
    match authored_leaf(|s| matches!(s, Statement::SelectActivePilotCraft { .. })) {
        Statement::SelectActivePilotCraft { appearances, .. } => appearances,
        _ => unreachable!(),
    }
}

#[test]
fn launch_leaves_require_explicit_inputs_and_do_not_mutate_on_missing_input() {
    for (statement, expected_error) in [
        (
            Statement::SelectActivePilotCraft {
                appearances: appearances(),
                next: at(1),
            },
            ProgramError::MissingSceneByte(SceneByte::ActivePilot),
        ),
        (
            Statement::UpdateLowShieldVisual { next: at(1) },
            ProgramError::MissingSceneByte(SceneByte::ActiveShield),
        ),
        (
            Statement::AlignCameraHeading { next: at(1) },
            ProgramError::MissingCameraHeading,
        ),
        (
            Statement::PublishCameraTrackingTarget { next: at(1) },
            ProgramError::MissingCameraTrackingTarget,
        ),
    ] {
        let catalog = PathCatalog::new(vec![vec![statement]]).unwrap();
        let (mut runtime, mut objects, owner, mut random) = setup();
        let before = objects.clone();
        let before_random = random;
        runtime.branch.invert_next = true;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 0),
            Err(ProgramError::BudgetExceeded {
                cursor: at(0),
                executed: 0
            })
        );
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
            Err(expected_error)
        );
        assert_eq!(objects, before);
        assert_eq!(random, before_random);
        assert!(runtime.branch.invert_next);
    }
}

#[test]
fn pilot_appearance_masks_then_clamps_all_bytes_without_resetting_shape_state() {
    let catalog = PathCatalog::new(vec![vec![Statement::SelectActivePilotCraft {
        appearances: appearances(),
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let selected = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap();
    let before_random = random;
    let table = [
        (52, 0x81F4),
        (52, 0x82FE),
        (53, 0x81F4),
        (53, 0x82FE),
        (85, 0x81F4),
        (85, 0x82FE),
    ];
    for pilot in 0..=u8::MAX {
        for invert in [false, true] {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(at(0));
            actor.extension.depth_offset = 0xD39A;
            actor.extension.path_state.motion_phase = 0x8173;
            actor.base.hit_points = pilot ^ 255;
            actor.base.position = Vector3 {
                x: -317,
                y: i16::MIN,
                z: 99,
            };
            let mut expected = objects.clone();
            let entry = table[usize::from((pilot & 15).min(5))];
            let actor = expected.get_mut(owner).unwrap();
            actor.base.path = Some(at(1));
            actor.base.shape = ShapeId::from_catalog_index(entry.0);
            actor.extension.material_set = Some(MaterialSetId::from_catalog_token(entry.1));
            let mut inputs = world(&mut random);
            inputs.scene.active_pilot = Some(pilot);
            inputs.scene.wingmate_pilot = Some(pilot ^ 255);
            inputs.selected = Some(selected);
            runtime.branch.invert_next = invert;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
            assert_eq!(objects, expected);
            assert_eq!(runtime.branch.invert_next, invert);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn camera_heading_uses_signed_floor_for_every_heading_and_yaw() {
    let catalog =
        PathCatalog::new(vec![vec![Statement::AlignCameraHeading { next: at(1) }]]).unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let before_random = random;
    for camera in 0..=u8::MAX {
        for yaw in 0..=u8::MAX {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(at(0));
            actor.base.yaw = Angle::from_units(yaw);
            actor.base.pitch = Angle::from_units(camera);
            actor.base.roll = Angle::from_units(yaw ^ 255);
            let mut expected = objects.clone();
            let difference = ((-i16::from(camera) - i16::from(yaw) + 128).rem_euclid(256)) - 128;
            let actor = expected.get_mut(owner).unwrap();
            actor.base.yaw = Angle::from_units(
                (i16::from(yaw) + difference.div_euclid(8)).rem_euclid(256) as u8,
            );
            actor.base.path = Some(at(1));
            runtime.branch.invert_next = yaw & 1 != 0;
            let mut inputs = world(&mut random);
            inputs.camera_heading = Some(Angle::from_units(camera));
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
            assert_eq!(objects, expected, "camera={camera} yaw={yaw}");
            assert_eq!(runtime.branch.invert_next, yaw & 1 != 0);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn shield_visual_replaces_depth_word_and_only_phase_low_for_every_shield_and_clock() {
    let catalog =
        PathCatalog::new(vec![vec![Statement::UpdateLowShieldVisual { next: at(1) }]]).unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let before_random = random;
    for shield in 0..=u8::MAX {
        for clock in 0..=u8::MAX {
            let actor = objects.get_mut(owner).unwrap();
            actor.base.path = Some(at(0));
            actor.extension.path_state.motion_phase = u16::from(clock) * 256 + u16::from(shield);
            actor.extension.depth_offset = 0xD39A;
            actor.base.hit_points = shield ^ 255;
            let mut expected = objects.clone();
            let actor = expected.get_mut(owner).unwrap();
            actor.base.path = Some(at(1));
            actor.extension.path_state.motion_phase =
                u16::from(clock) * 256 + u16::from(shield <= 12);
            actor.extension.depth_offset = if shield <= 12 && [0, 2].contains(&(clock % 8)) {
                3
            } else {
                0
            };
            let mut inputs = world(&mut random);
            inputs.animation_clock = clock;
            inputs.scene.active_shield = Some(shield);
            runtime.branch.invert_next = shield & 1 != 0;
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                Err(ProgramError::BudgetExceeded {
                    cursor: at(1),
                    executed: 1
                })
            );
            assert_eq!(objects, expected, "shield={shield} clock={clock}");
            assert_eq!(runtime.branch.invert_next, shield & 1 != 0);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn camera_tracking_publishes_current_owner_not_selection_without_copying_pose() {
    let catalog = PathCatalog::new(vec![vec![Statement::PublishCameraTrackingTarget {
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let selected = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::PlayerFlight,
        ))
        .unwrap();
    for invert in [false, true] {
        objects.get_mut(owner).unwrap().base.path = Some(at(0));
        let mut expected = objects.clone();
        expected.get_mut(owner).unwrap().base.path = Some(at(1));
        let before_random = random;
        let mut tracking = CameraTrackingTarget {
            actor: Some(selected),
        };
        let mut inputs = world(&mut random);
        inputs.selected = Some(selected);
        inputs.camera_tracking = Some(&mut tracking);
        runtime.branch.invert_next = invert;
        assert_eq!(
            runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
            Err(ProgramError::BudgetExceeded {
                cursor: at(1),
                executed: 1
            })
        );
        assert_eq!(tracking.actor, Some(owner));
        assert_eq!(objects, expected);
        assert_eq!(random, before_random);
        assert_eq!(runtime.branch.invert_next, invert);
    }
}

#[test]
fn launch_runs_all_three_pitch_loops_and_switches_banking_to_captured_horizontal_drift() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.path = Some(authored_paths::CRAFT_LAUNCH_TRANSITION);
    actor.base.position = Vector3 {
        x: i16::MAX - 3,
        y: -91,
        z: i16::MIN + 5,
    };
    actor.base.pitch = Angle::from_units(5);
    actor.base.yaw = Angle::from_units(77);
    actor.base.roll = Angle::from_units(250);
    actor.base.speed = 3;
    actor.extension.path_state.motion_phase = 0xACB7;
    let origin = actor.base.position;
    let before_random = random;
    let mut events = AudioState::default();
    let mut inputs = world(&mut random);
    inputs.scene.active_pilot = Some(18);
    inputs.scene.active_shield = Some(100);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    inputs.published_motion = Some(PublishedPlayerMotion {
        delta: Vector3 {
            x: 17,
            y: 99,
            z: -23,
        },
        ..Default::default()
    });
    inputs.audio = Some(audio(&mut events));
    let mut expected_roll = 250_u8;
    let mut helper = None;
    let mut initial_triggers = Vec::new();
    let bank_steps = [48_u8, 42, 37, 33, 29, 26, 23];
    for visit in 1..=30 {
        if visit > 1 {
            inputs.animation_clock = visit;
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut inputs,
                visit,
            );
            if visit <= 8 {
                expected_roll = expected_roll.wrapping_add(bank_steps[usize::from(visit - 2)]);
            } else {
                let delta = i16::from(0_u8.wrapping_sub(expected_roll) as i8);
                let step = if delta == 0 {
                    0
                } else {
                    delta.signum() * (delta.abs() / 8).max(1)
                };
                expected_roll = expected_roll.wrapping_add(step as u8);
            }
        }
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        if visit == 1 {
            helper = runtime.spawns.last_spawn;
            assert!(helper.is_some());
            initial_triggers = objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .triggers
                .entries(&runtime.resources, owner)
                .unwrap()
                .to_vec();
            assert_eq!(initial_triggers.len(), 2);
        }
        let actor = objects.get(owner).unwrap();
        let decrements = match visit {
            1..=7 => visit,
            8 => 9,
            9..=27 => visit + 2,
            _ => 30,
        };
        assert_eq!(
            actor.base.pitch.units(),
            5_u8.wrapping_sub(decrements),
            "visit {visit}"
        );
        assert_eq!(actor.base.yaw.units(), 77);
        assert_eq!(actor.base.roll.units(), expected_roll, "visit {visit}");
        assert_eq!(actor.base.shape, ShapeId::from_catalog_index(53));
        assert_eq!(actor.extension.path_state.motion_phase, 183);
        assert!(actor.base.flags.collision_disabled);
        assert!(
            actor
                .extension
                .path_state
                .motion
                .generate_velocity_each_step
        );
        assert_eq!(
            (
                actor.base.speed,
                actor.base.target_speed,
                actor.base.acceleration
            ),
            if visit < 8 { (3, 25, 5) } else { (35, 0, 0) }
        );
        // Common velocity integration is deliberately outside this path test.
        let drift_visits = i16::from(visit.saturating_sub(8));
        assert_eq!(
            actor.base.position,
            Vector3 {
                x: origin.x.wrapping_add(17 * drift_visits),
                y: origin.y,
                z: origin.z.wrapping_sub(23 * drift_visits)
            }
        );
        let triggers = actor
            .extension
            .path_state
            .triggers
            .entries(&runtime.resources, owner)
            .unwrap();
        assert_eq!(triggers.len(), 2);
        assert_eq!(triggers[0], initial_triggers[0]);
        assert_eq!(triggers[1] == initial_triggers[1], visit < 8);
        if visit >= 8 {
            assert_eq!(actor.extension.relative_position.x, 17);
            assert_eq!(actor.extension.relative_position.z, -23);
            assert_eq!(actor.extension.relative_rotation.roll.units(), 21);
            let child = find_child(&objects, owner, 32).unwrap().unwrap();
            let child = objects.get(child).unwrap();
            assert_eq!((child.base.hit_points, child.base.attack_power), (100, 100));
            assert_eq!(child.base.shape, ShapeId::from_catalog_index(19));
            assert_eq!(
                child.extension.relative_position,
                Vector3 { x: 0, y: 0, z: -20 }
            );
            assert_eq!(child.extension.parent, Some(owner));
            // Subsequent snapshots cannot change the already captured drift.
            inputs.published_motion.as_mut().unwrap().delta = Vector3 {
                x: -900,
                y: -901,
                z: 902,
            };
        } else {
            assert_eq!(find_child(&objects, owner, 32).unwrap(), None);
        }
    }
    let helper = objects.get(helper.unwrap()).unwrap();
    assert_eq!(helper.base.shape, ShapeId::EMPTY);
    assert_eq!(helper.base.position, origin);
    assert_eq!((helper.base.hit_points, helper.base.attack_power), (1, 1));
    assert_eq!(helper.extension.parent, None);
    assert_eq!(random, before_random);
    assert_eq!(
        events
            .take_events()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
        vec![super::super::SoundEvent::Authored(
            super::super::path_sound::AuthoredCue::new(
                26,
                0,
                super::super::path_control::PlayerTarget::Primary
            )
        )]
    );
}

#[test]
fn launch_camera_helper_waits_eight_passes_then_publishes_itself_without_resampling_heading_or_motion(
) {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    objects.get_mut(owner).unwrap().base.path = Some(authored_paths::CRAFT_LAUNCH_TRANSITION);
    objects.get_mut(owner).unwrap().base.yaw = Angle::from_units(77);
    let before_random = random;
    let mut inputs = world(&mut random);
    inputs.scene.active_pilot = Some(0);
    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    let helper = runtime.spawns.last_spawn.unwrap();
    let origin = objects.get(helper).unwrap().base.position;
    inputs.camera_heading = Some(Angle::from_units(31));
    inputs.published_motion = Some(PublishedPlayerMotion {
        delta: Vector3 {
            x: -317,
            y: 900,
            z: 79,
        },
        ..Default::default()
    });
    let mut tracking = CameraTrackingTarget { actor: Some(owner) };
    inputs.camera_tracking = Some(&mut tracking);
    for visit in 1..=12_u8 {
        if visit > 1 {
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                helper,
                &mut inputs,
                visit,
            );
        }
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, helper, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let actor = objects.get(helper).unwrap();
        assert!(actor.base.flags.collision_disabled);
        assert_eq!(actor.base.yaw.units(), 63);
        assert_eq!(
            actor.base.position,
            Vector3 {
                x: origin.x.wrapping_sub(317 * i16::from(visit - 1)),
                y: origin.y,
                z: origin.z.wrapping_add(79 * i16::from(visit - 1))
            }
        );
        assert_eq!(
            actor.extension.relative_position,
            Vector3 {
                x: -317,
                y: 0,
                z: 79
            }
        );
        assert_eq!(
            inputs.camera_tracking.as_ref().unwrap().actor,
            Some(if visit <= 8 { owner } else { helper })
        );
        assert_eq!(actor.base.wait_timer, if visit <= 8 { visit } else { 0 });
        if visit >= 9 {
            assert_eq!(actor.base.pitch.units(), 246);
            assert_eq!(actor.base.speed, 30);
        }
        inputs.camera_heading = None;
        inputs.published_motion = None;
    }
    assert_eq!(random, before_random);
}

#[test]
fn shield_callback_restores_saved_phase_and_spawns_only_on_low_even_ticks() {
    let catalog = authored_paths::catalog();
    for shield in 0..=u8::MAX {
        for clock in 0..8 {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::CRAFT_LAUNCH_TRANSITION);
            let mut inputs = world(&mut random);
            inputs.scene.active_pilot = Some(0);
            inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let actor = objects.get_mut(owner).unwrap();
            actor.extension.path_state.motion_phase = u16::from(shield) * 256 + 0xA7;
            actor.base.hit_points = shield ^ 255;
            actor.extension.depth_offset = 0xD39A;
            inputs.scene.active_shield = Some(shield);
            inputs.animation_clock = clock;
            let before_random = *inputs.random;
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut inputs,
                clock,
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(
                actor.extension.path_state.motion_phase,
                u16::from(shield) * 256 + 0xA7
            );
            assert_eq!(
                actor.extension.depth_offset,
                if shield < 13 && clock % 8 < 4 && clock % 2 == 0 {
                    3
                } else {
                    0
                }
            );
            assert_eq!(*inputs.random, before_random);
            let child = find_child(&objects, owner, 99).unwrap();
            assert_eq!(child.is_some(), shield < 13 && clock % 2 == 0);
            if let Some(child) = child {
                let actor = objects.get(child).unwrap();
                assert_eq!(actor.base.shape, ShapeId::from_catalog_index(36));
                assert_eq!((actor.base.hit_points, actor.base.attack_power), (1, 1));
                assert_eq!(actor.extension.relative_position, Vector3::default());
                let mut expected_random = before_random;
                let offsets = [0; 3].map(|_| {
                    let high = expected_random.next_byte();
                    let low = expected_random.next_byte();
                    (u16::from_be_bytes([high, low]) & 31) as i16 - 15
                });
                assert_eq!(
                    runtime
                        .enter_program(&catalog, &mut objects, child, &mut inputs, 100)
                        .unwrap()
                        .step,
                    ControlStep::Movement
                );
                let actor = objects.get(child).unwrap();
                assert_eq!(
                    actor.extension.relative_position,
                    Vector3 {
                        x: offsets[0],
                        y: offsets[1],
                        z: offsets[2]
                    }
                );
                assert_eq!(actor.extension.texture_scroll_x, 252);
                assert_eq!(*inputs.random, expected_random);
            }
        }
    }
}

#[test]
fn launch_exhaust_grows_for_three_yields_then_resets_and_cycles_while_paused() {
    let catalog = authored_paths::catalog();
    let path = match authored_leaf(
        |s| matches!(s, Statement::SpawnChild { parameters, .. } if parameters.number == 32 && parameters.hit_points == 100 && parameters.attack_power == 100),
    ) {
        Statement::SpawnChild { parameters, .. } => parameters.path.unwrap(),
        _ => unreachable!(),
    };
    let (mut runtime, mut objects, owner, mut random) = setup();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.path = Some(path);
    actor.extension.depth_offset = 0xCD71;
    actor.extension.path_state.motion_phase = 0xABCD;
    actor.extension.texture_scroll_y = 231;
    let before_random = random;
    for visit in 1..=16 {
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut world(&mut random), 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        let actor = objects.get(owner).unwrap();
        assert!(actor.base.contacts.run_when_paused);
        assert!(actor.base.flags.scaled_sprite);
        assert!(actor.base.flags.collision_disabled);
        assert_eq!(actor.extension.depth_offset, 0xCD00);
        assert_eq!(
            actor.extension.texture_scroll_x,
            if visit < 4 { 4 * visit } else { 0 }
        );
        assert_eq!(actor.extension.texture_scroll_y, 231);
        assert_eq!(
            actor.extension.path_state.motion_phase,
            if visit < 4 { 0xABCD } else { 0xAB00 }
        );
        assert_eq!(
            actor.extension.path_state.animation.color.fixed_frame(),
            Some(if visit < 4 { visit } else { (visit + 1) % 2 })
        );
        assert!(!actor.base.flags.remove_after_tick);
        assert_eq!(random, before_random);
    }
}
