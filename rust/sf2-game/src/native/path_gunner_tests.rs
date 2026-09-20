//! Static-source Gunner routes, health publication, feedback and lifecycles.
use super::super::path_fields::{ByteField, BytePart, WordField};
use super::super::path_player_control::{PlayerTargetControl, PrimaryControl};
use super::super::path_scene_state::{
    CoordinationCommand, EncounterCoordination, EncounterHealthDisplay, HealthDisplayField,
};
use super::super::player_hit_control::{PlayerHitControl, PrimaryFeedback};
use super::super::{
    authored_paths, Angle, AudioState, Behavior, ObjectKind, ObjectSpawnDefaults, PathId, ShapeId,
    Vector3,
};
use super::paired_patrol_tests::callbacks;
use super::projectile_tests::audio;
use super::tests::{setup, world};
use super::*;

const ROOTS: [PathCursor; 2] = [
    authored_paths::INNER_ARENA_KICK_GUNNER,
    authored_paths::OUTER_ARENA_KICK_GUNNER,
];
const POINTS: [[(i16, i16); 4]; 2] = [
    [(0, -1024), (0, 0), (-1024, 0), (-1024, -1024)],
    [(1280, -1280), (1280, 1280), (-1280, 1280), (-1280, -1280)],
];
const DESTINATIONS: [u8; 8] = [1, 3, 0, 2, 1, 3, 0, 2];
const HEADINGS: [u8; 8] = [0, 64, 128, 64, 192, 128, 192, 0];
const ENTRY_HEADINGS: [u8; 4] = [160, 224, 32, 96];

fn at(index: u16) -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: index,
    }
}

fn player(objects: &mut ObjectStore) -> ObjectId {
    let mut actor = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
    actor.base.position = Vector3 {
        x: 157,
        y: -79,
        z: 1000,
    };
    objects.allocate(actor).unwrap()
}

#[test]
fn display_commands_preserve_full_wrapping_bytes_without_premature_ui_clamping() {
    let byte = ByteField::WordPart {
        field: WordField::MotionPhase,
        part: BytePart::Low,
    };
    for field in [HealthDisplayField::Current, HealthDisplayField::Maximum] {
        for command in [
            CoordinationCommand::CopyTo(byte),
            CoordinationCommand::Assign(ByteOperand::Actor(byte)),
            CoordinationCommand::Assign(ByteOperand::Literal(255)),
            CoordinationCommand::Increment,
            CoordinationCommand::Decrement,
        ] {
            let catalog = PathCatalog::new(vec![vec![Statement::HealthDisplay {
                field,
                command,
                next: at(1),
            }]])
            .unwrap();
            let (mut runtime, mut objects, owner, mut random) = setup();
            let baseline = objects.clone();
            assert_eq!(
                runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
                Err(ProgramError::MissingHealthDisplay)
            );
            assert_eq!(objects, baseline);
            let random_before = random;
            for value in 0..=u8::MAX {
                for invert in [false, true] {
                    objects = baseline.clone();
                    objects
                        .get_mut(owner)
                        .unwrap()
                        .extension
                        .path_state
                        .motion_phase = 0xA500 | u16::from(!value);
                    let mut expected = objects.clone();
                    let mut display = EncounterHealthDisplay {
                        current: value,
                        maximum: value,
                        label: Some("retained"),
                    };
                    let mut desired_display = display;
                    let updated = match command {
                        CoordinationCommand::CopyTo(_) => {
                            expected
                                .get_mut(owner)
                                .unwrap()
                                .extension
                                .path_state
                                .motion_phase = 0xA500 | u16::from(value);
                            value
                        }
                        CoordinationCommand::Assign(ByteOperand::Actor(_)) => !value,
                        CoordinationCommand::Assign(_) => 255,
                        CoordinationCommand::Increment => value.wrapping_add(1),
                        CoordinationCommand::Decrement => value.wrapping_sub(1),
                    };
                    match field {
                        HealthDisplayField::Current => desired_display.current = updated,
                        HealthDisplayField::Maximum => desired_display.maximum = updated,
                    }
                    expected.get_mut(owner).unwrap().base.path = Some(at(1));
                    runtime.branch.invert_next = invert;
                    let mut inputs = world(&mut random);
                    inputs.health_display = Some(&mut display);
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: at(1),
                            executed: 1
                        })
                    );
                    assert_eq!(display, desired_display);
                    assert_eq!(objects, expected);
                    assert_eq!(runtime.branch.invert_next, invert);
                    assert_eq!(random, random_before);
                }
            }
        }
    }
    let (mut runtime, mut objects, owner, mut random) = setup();
    let catalog = PathCatalog::new(vec![vec![Statement::SetHealthDisplayLabel {
        label: "KICK GUNNER",
        next: at(1),
    }]])
    .unwrap();
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut world(&mut random), 1),
        Err(ProgramError::MissingHealthDisplay)
    );
    let mut display = EncounterHealthDisplay {
        current: 255,
        maximum: 129,
        label: Some("previous"),
    };
    let mut inputs = world(&mut random);
    inputs.health_display = Some(&mut display);
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::BudgetExceeded {
            cursor: at(1),
            executed: 1
        })
    );
    assert_eq!(
        display,
        EncounterHealthDisplay {
            current: 255,
            maximum: 129,
            label: Some("KICK GUNNER")
        }
    );
}

#[test]
fn encounter_feedback_gates_full_word_mode_and_preserves_every_other_hit_field() {
    let initial = PlayerHitControl {
        recovery: 199,
        feedback_duration: 255,
        feedback_flags: 0x81,
        camera_pitch_recoil: i16::MIN,
        reserve_shield: 37,
        secondary_protection: 173,
        hold_secondary_protection: true,
        impact_latched: true,
        impact_variant: true,
        bank_impulse: -119,
        deflection_sound_cooldown: 251,
    };
    for mode in 0..=u16::MAX {
        for state in [0, 1, 255] {
            let mut hit = initial;
            let mut expected = initial;
            if mode == 8 && state != 0 {
                expected.feedback_duration = 4;
                expected.feedback_flags |= 0x24;
            }
            hit.request_encounter_feedback(mode, state);
            assert_eq!(hit, expected);
        }
    }
    for state in 0..=u8::MAX {
        for flags in 0..=u8::MAX {
            let mut hit = PlayerHitControl {
                feedback_flags: flags,
                ..initial
            };
            let mut expected = hit;
            if state != 0 {
                expected.feedback_duration = 4;
                expected.feedback_flags |= 0x24;
            }
            hit.request_encounter_feedback(8, state);
            assert_eq!(hit, expected);
        }
    }
}

#[test]
fn feedback_uses_fresh_primary_control_not_selected_context_and_faults_only_when_read() {
    let catalog = PathCatalog::new(vec![vec![Statement::RequestPrimaryEncounterFeedback {
        next: at(1),
    }]])
    .unwrap();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let primary = player(&mut objects);
    let initial = objects.clone();
    let initial_random = random;
    let mut target = PlayerTargetControl {
        mode: 8,
        ..Default::default()
    };
    let mut inputs = world(&mut random);
    inputs.selected = Some(owner);
    runtime.branch.invert_next = true;
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::MissingPrimaryPlayer)
    );
    inputs.primary_player = Some(primary);
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::MissingPrimaryControl)
    );
    inputs.primary_control = Some(PrimaryControl {
        target: &mut target,
        linked_mode: false,
    });
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::MissingPrimaryFeedback)
    );
    assert_eq!(objects, initial);
    inputs.primary_control.as_mut().unwrap().target.mode = 0x108;
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::BudgetExceeded {
            cursor: at(1),
            executed: 1
        })
    );
    objects = initial.clone();
    inputs.primary_control.as_mut().unwrap().target.mode = 8;
    let mut hit = PlayerHitControl {
        feedback_flags: 0x81,
        feedback_duration: 29,
        ..Default::default()
    };
    inputs.primary_feedback = Some(PrimaryFeedback {
        state: 0,
        hit: &mut hit,
    });
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::BudgetExceeded {
            cursor: at(1),
            executed: 1
        })
    );
    assert_eq!(
        inputs
            .primary_feedback
            .as_ref()
            .unwrap()
            .hit
            .feedback_duration,
        29
    );
    objects = initial.clone();
    inputs.primary_feedback.as_mut().unwrap().state = 255;
    assert_eq!(
        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
        Err(ProgramError::BudgetExceeded {
            cursor: at(1),
            executed: 1
        })
    );
    assert_eq!(
        inputs
            .primary_feedback
            .as_ref()
            .unwrap()
            .hit
            .feedback_duration,
        4
    );
    assert_eq!(
        inputs.primary_feedback.as_ref().unwrap().hit.feedback_flags,
        0xA5
    );
    assert_eq!(inputs.random, &initial_random);
    assert!(runtime.branch.invert_next);
    let mut expected = initial;
    expected.get_mut(owner).unwrap().base.path = Some(at(1));
    assert_eq!(objects, expected);
}

#[test]
fn folded_route_blocks_consume_two_draws_and_preserve_all_other_actor_state() {
    let authored = authored_paths::catalog();
    let mut blocks = authored.paths[0]
        .iter()
        .filter_map(|statement| match statement {
            Statement::ChooseGunnerRoute { routes, .. } => Some(*routes),
            _ => None,
        });
    for arena in 0..2 {
        let routes = blocks.next().unwrap();
        let catalog = PathCatalog::new(vec![vec![Statement::ChooseGunnerRoute {
            routes,
            next: at(1),
        }]])
        .unwrap();
        let mut seen = [false; 8];
        for first in 0..=u8::MAX {
            for last in [0, 1, 37, 255] {
                for invert in [false, true] {
                    let (mut runtime, mut objects, owner, _) = setup();
                    let mut random = RandomState::new([first, last, 39, last]);
                    let mut expected_random = random;
                    let origin = usize::from(expected_random.next_byte() & 3);
                    let branch = usize::from(expected_random.next_byte() & 1);
                    let choice = origin * 2 + branch;
                    let destination = usize::from(DESTINATIONS[choice]);
                    seen[choice] = true;
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.position = Vector3 {
                        x: i16::MIN,
                        y: -171,
                        z: i16::MAX,
                    };
                    actor.extension.relative_position = Vector3 {
                        x: i16::MAX,
                        y: 317,
                        z: i16::MIN,
                    };
                    actor.extension.relative_rotation.pitch = Angle::from_units(79);
                    actor.extension.path_state.motion_phase = 0xABCD;
                    actor.extension.path_state.script_value = 517;
                    actor.extension.path_state.motion_delta = actor.base.position;
                    let original = objects.clone();
                    let mut expected = objects.clone();
                    let actor = expected.get_mut(owner).unwrap();
                    (actor.base.position.x, actor.base.position.z) = POINTS[arena][origin];
                    (
                        actor.extension.relative_position.x,
                        actor.extension.relative_position.z,
                    ) = POINTS[arena][destination];
                    actor.base.yaw = Angle::from_units(HEADINGS[choice]);
                    actor.extension.relative_rotation.yaw = actor.base.yaw;
                    if arena == 0 {
                        actor.extension.relative_rotation.pitch =
                            Angle::from_units(ENTRY_HEADINGS[origin]);
                    }
                    actor.extension.path_state.motion_phase =
                        (destination as u16) * 256 + choice as u16;
                    actor.base.path = Some(at(1));
                    runtime.branch.invert_next = invert;
                    let mut inputs = world(&mut random);
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 0),
                        Err(ProgramError::BudgetExceeded {
                            cursor: at(0),
                            executed: 0
                        })
                    );
                    assert_eq!(objects, original);
                    assert_eq!(
                        runtime.resume_program(&catalog, &mut objects, owner, &mut inputs, 1),
                        Err(ProgramError::BudgetExceeded {
                            cursor: at(1),
                            executed: 1
                        })
                    );
                    assert_eq!(objects, expected);
                    assert_eq!(random, expected_random);
                    assert_eq!(runtime.branch.invert_next, invert);
                }
            }
        }
        assert!(seen.into_iter().all(|seen| seen));
    }
    assert!(blocks.next().is_none());
}

#[test]
fn authored_gunners_publish_names_and_health_before_waiting_then_update_display_on_contact() {
    let catalog = authored_paths::catalog();
    for (arena, root) in ROOTS.into_iter().enumerate() {
        for health in 0..=u8::MAX {
            let (mut runtime, mut objects, owner, mut random) = setup();
            objects.get_mut(owner).unwrap().base.path = Some(root);
            let mut expected_random = random;
            let origin = usize::from(expected_random.next_byte() & 3);
            let connection = origin * 2 + usize::from(expected_random.next_byte() & 1);
            let mut shared = EncounterCoordination::default();
            let mut display = EncounterHealthDisplay::default();
            let mut inputs = world(&mut random);
            inputs.coordination = Some(&mut shared);
            inputs.health_display = Some(&mut display);
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.hit_points, if arena == 0 { 110 } else { 70 });
            assert_eq!(
                actor.base.position,
                Vector3 {
                    x: POINTS[arena][origin].0,
                    y: if arena == 0 { 200 } else { 100 },
                    z: POINTS[arena][origin].1
                }
            );
            assert_eq!(actor.base.yaw.units(), HEADINGS[connection]);
            assert_eq!(
                inputs.health_display.as_ref().unwrap().label,
                Some("KICK GUNNER")
            );
            assert_eq!(
                inputs.health_display.as_ref().unwrap().current,
                if arena == 0 { 35 } else { 15 }
            );
            assert_eq!(
                inputs.health_display.as_ref().unwrap().maximum,
                if arena == 0 { 35 } else { 15 }
            );
            assert_eq!(actor.extension.impact_materials.ordinary, Some(2));
            assert_eq!(inputs.random, &expected_random);
            let waiting = actor.base.path;
            let actor = objects.get_mut(owner).unwrap();
            actor.base.hit_points = health;
            actor.base.contacts.new_contact_latched = true;
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
            let adjusted = health.wrapping_sub(40);
            let adjusted = if adjusted == 1 { 2 } else { adjusted };
            assert_eq!(
                inputs.health_display.as_ref().unwrap().current,
                ((adjusted as i8) / 2) as u8
            );
            assert_eq!(
                inputs.health_display.as_ref().unwrap().maximum,
                if arena == 0 { 35 } else { 15 }
            );
            assert_eq!(objects.get(owner).unwrap().base.hit_points, health);
            assert_eq!(
                objects.get(owner).unwrap().base.path != waiting,
                (40_u8.wrapping_sub(health) as i8) >= 0
            );
            assert_eq!(inputs.random, &expected_random);
        }
    }
}

#[test]
fn encounter_sentinel_ends_both_gunners_before_display_or_player_dependencies() {
    for root in ROOTS {
        let (mut runtime, mut objects, owner, mut random) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(root);
        let before = random;
        let mut shared = EncounterCoordination {
            progress: 254,
            ..Default::default()
        };
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        assert_eq!(
            runtime
                .enter_program(
                    &authored_paths::catalog(),
                    &mut objects,
                    owner,
                    &mut inputs,
                    10
                )
                .unwrap()
                .step,
            ControlStep::Ended
        );
        assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
        assert_eq!(random, before);
    }
}

#[test]
fn death_requests_primary_feedback_flickers_fifteen_times_then_scores_and_preserves_health_bar() {
    use super::super::path_score::PlayerScore;
    let catalog = authored_paths::catalog();
    for root in ROOTS {
        for mode in [0, 8, 0x108, u16::MAX] {
            for state in [0, 1, 255] {
                for initial_score in [0_u16, 65400] {
                    let (mut runtime, mut objects, owner, mut random) = setup();
                    objects.get_mut(owner).unwrap().base.path = Some(root);
                    let primary = player(&mut objects);
                    let selected = player(&mut objects);
                    let mut shared = EncounterCoordination {
                        progress: 255,
                        ..Default::default()
                    };
                    let mut display = EncounterHealthDisplay::default();
                    let mut control = PlayerTargetControl {
                        mode,
                        ..Default::default()
                    };
                    let mut hit = PlayerHitControl {
                        feedback_duration: 91,
                        feedback_flags: 0x81,
                        recovery: 199,
                        ..Default::default()
                    };
                    let mut events = AudioState::default();
                    let mut score = PlayerScore::from_parts(initial_score, 171);
                    let mut inputs = world(&mut random);
                    inputs.health_display = Some(&mut display);
                    inputs.coordination = Some(&mut shared);
                    inputs.primary_player = Some(primary);
                    inputs.selected = Some(selected);
                    inputs.primary_control = Some(PrimaryControl {
                        target: &mut control,
                        linked_mode: false,
                    });
                    inputs.primary_feedback = Some(PrimaryFeedback {
                        state,
                        hit: &mut hit,
                    });
                    inputs.audio = Some(audio(&mut events));
                    inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
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
                    let actor = objects.get_mut(owner).unwrap();
                    actor.base.hit_points = 40;
                    actor.base.contacts.new_contact_latched = true;
                    let position = actor.base.position;
                    callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
                    let maximum = inputs.health_display.as_ref().unwrap().maximum;
                    let mut expected_random = *inputs.random;
                    for _ in 0..15 {
                        expected_random.next_byte();
                    }
                    // Fourteen NEXT yields, one fade yield, then the death tail.
                    for visit in 1..=16 {
                        let exit = runtime
                            .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                            .unwrap()
                            .step;
                        assert_eq!(
                            exit,
                            if visit == 16 {
                                ControlStep::MovementTail
                            } else {
                                ControlStep::Movement
                            }
                        );
                        assert_eq!(objects.get(owner).unwrap().base.position, position);
                        assert_eq!(
                            inputs
                                .primary_feedback
                                .as_ref()
                                .unwrap()
                                .hit
                                .feedback_duration,
                            if mode == 8 && state != 0 { 4 } else { 91 }
                        );
                        assert_eq!(
                            inputs.primary_feedback.as_ref().unwrap().hit.feedback_flags,
                            if mode == 8 && state != 0 { 0xA5 } else { 0x81 }
                        );
                        assert_eq!(inputs.primary_feedback.as_ref().unwrap().hit.recovery, 199);
                        assert!(objects.get(owner).unwrap().base.flags.collision_disabled);
                    }
                    assert_eq!(inputs.random, &expected_random);
                    assert_eq!(objects.len(), 5);
                    assert_eq!(objects.get(owner).unwrap().base.hit_points, 0);
                    assert!(!objects.get(owner).unwrap().base.flags.remove_after_tick);
                    assert_eq!(inputs.coordination.as_ref().unwrap().progress, 0);
                    assert_eq!(inputs.coordination.as_ref().unwrap().retired_actors, 0);
                    assert_eq!(
                        inputs.health_display.as_deref().unwrap(),
                        &EncounterHealthDisplay {
                            current: 0,
                            maximum,
                            label: Some("KICK GUNNER")
                        }
                    );
                    assert_eq!(
                        inputs.selected_score.as_ref().unwrap().points(),
                        171 * 65536 + u32::from(initial_score.saturating_add(500))
                    );
                    assert_eq!(inputs.primary_control.as_ref().unwrap().target.mode, 2);
                    assert_eq!(
                        inputs.primary_control.as_ref().unwrap().target.owner,
                        Some(owner)
                    );
                    assert_eq!(inputs.primary_control.as_ref().unwrap().target.range, 2);
                    let fade = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                    assert_eq!((fade.base.hit_points, fade.base.attack_power), (100, 8));
                }
            }
        }
    }
}

#[test]
fn outer_gunner_complete_cycle_keeps_all_five_jump_shots_arcs_and_wait_boundaries() {
    const ARC: [i16; 20] = [
        -100, -81, -64, -49, -36, -25, -16, -9, -4, -1, 1, 4, 9, 16, 25, 36, 49, 64, 81, 100,
    ];
    const BOUNCE: [i16; 5] = [8, 12, 4, -4, -20];
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Snapshot {
        position: Vector3,
        speed: u8,
        radar: u8,
        spawn: Option<(usize, u8, u8, i16)>,
    }
    fn emit(states: &mut Vec<Snapshot>, state: &mut Snapshot) {
        states.push(*state);
        state.spawn = None;
    }
    fn wait(states: &mut Vec<Snapshot>, state: &mut Snapshot, count: usize) {
        for _ in 0..count {
            emit(states, state);
        }
    }
    fn bounce(states: &mut Vec<Snapshot>, state: &mut Snapshot) {
        for delta in BOUNCE {
            state.position.y = state.position.y.wrapping_add(delta);
            emit(states, state);
        }
    }
    let catalog = authored_paths::catalog();
    for seed in 0..8 {
        let (mut runtime, mut objects, owner, _) = setup();
        let mut random = RandomState::new([0, seed, 39, 127]);
        let mut expected_random = random;
        let origin = usize::from(expected_random.next_byte() & 3);
        let connection = origin * 2 + usize::from(expected_random.next_byte() & 1);
        let destination = POINTS[1][usize::from(DESTINATIONS[connection])];
        let mut state = Snapshot {
            position: Vector3 {
                x: POINTS[1][origin].0,
                y: 100,
                z: POINTS[1][origin].1,
            },
            speed: 0,
            radar: 0,
            spawn: None,
        };
        let mut states = Vec::new();
        wait(&mut states, &mut state, 35);
        state.radar = 134;
        wait(&mut states, &mut state, 15);
        for y in [0, -100] {
            state.position.y = y;
            emit(&mut states, &mut state);
        }
        state.position.y = -200;
        state.spawn = Some((22, 8, 50, 10));
        state.speed = 35;
        for delta in ARC.into_iter().take(19) {
            state.position.y += delta;
            emit(&mut states, &mut state);
        }
        state.position.y = -100;
        state.speed = 0;
        emit(&mut states, &mut state);
        bounce(&mut states, &mut state);
        wait(&mut states, &mut state, 10);
        for hop in 0..5 {
            bounce(&mut states, &mut state);
            state.speed = 25;
            for index in 3..17 {
                state.position.y += 2 * ARC[index];
                if index == 16 {
                    state.speed = 0;
                }
                emit(&mut states, &mut state);
            }
            bounce(&mut states, &mut state);
            wait(&mut states, &mut state, 3);
            // Attached-spawn construction sets only the relative pose; the
            // common child visit subsequently publishes its world position.
            state.spawn = Some((20, 10, 10, 0));
            wait(&mut states, &mut state, 2);
            if hop < 4 {
                emit(&mut states, &mut state);
            }
        }
        bounce(&mut states, &mut state);
        for delta in ARC {
            state.position.y += delta;
            for (value, target) in [
                (&mut state.position.x, destination.0),
                (&mut state.position.z, destination.1),
            ] {
                let difference = target.wrapping_sub(*value);
                let step = if difference < 0 {
                    difference.min(-8)
                } else if difference > 0 {
                    difference.max(8)
                } else {
                    0
                };
                *value = value.wrapping_add(step / 8);
            }
            emit(&mut states, &mut state);
        }
        state.position.y = 100;
        state.spawn = Some((22, 8, 50, 10));
        wait(&mut states, &mut state, 15);
        state.radar = 0;
        wait(&mut states, &mut state, 35);
        emit(&mut states, &mut state);
        assert_eq!(states.len(), 312);
        objects.get_mut(owner).unwrap().base.path = Some(ROOTS[1]);
        let selected = player(&mut objects);
        let mut shared = EncounterCoordination::default();
        let mut display = EncounterHealthDisplay::default();
        let mut events = AudioState::default();
        let mut inputs = world(&mut random);
        inputs.coordination = Some(&mut shared);
        inputs.health_display = Some(&mut display);
        inputs.selected = Some(selected);
        inputs.audio = Some(audio(&mut events));
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        let mut shots = 0;
        for (index, expected) in states.into_iter().enumerate() {
            let last_spawn = runtime.spawns.last_spawn;
            let exit = runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap();
            assert_eq!(exit.step, ControlStep::Movement);
            let actor = objects.get(owner).unwrap();
            let spawned = if runtime.spawns.last_spawn != last_spawn {
                let spawn = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                if spawn.base.shape == ShapeId::from_catalog_index(20) {
                    shots += 1;
                    assert_eq!(spawn.base.child_number, 11);
                    assert_eq!(spawn.base.attachment, Some(owner));
                    assert_eq!(
                        spawn.extension.relative_position,
                        Vector3 { x: 0, y: 0, z: 50 }
                    );
                }
                Some((
                    spawn.base.shape.catalog_index(),
                    spawn.base.hit_points,
                    spawn.base.attack_power,
                    spawn.base.position.y,
                ))
            } else {
                None
            };
            assert_eq!(
                Snapshot {
                    position: actor.base.position,
                    speed: actor.base.speed,
                    radar: actor.extension.radar_marker.packed(),
                    spawn: spawned
                },
                expected,
                "seed {seed} visit {}",
                index + 1
            );
            assert_eq!(inputs.random, &expected_random);
        }
        assert_eq!(shots, 5);
        assert_eq!(objects.len(), 9);
        expected_random.next_byte();
        expected_random.next_byte();
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(inputs.random, &expected_random);
    }
}

#[test]
fn inner_gunner_selects_both_complete_attack_cycles_at_the_fiftieth_wait_boundary() {
    use super::super::weapon_dispatch::WeaponState;
    const ARC: [i16; 20] = [
        -100, -81, -64, -49, -36, -25, -16, -9, -4, -1, 1, 4, 9, 16, 25, 36, 49, 64, 81, 100,
    ];
    let catalog = authored_paths::catalog();
    let mut seen = [false; 2];
    for seed in 0..=u8::MAX {
        let (mut runtime, mut objects, owner, _) = setup();
        objects.get_mut(owner).unwrap().base.path = Some(ROOTS[0]);
        let mut random = RandomState::new([seed, 7, 39, 127]);
        let mut expected_random = random;
        expected_random.next_byte();
        expected_random.next_byte();
        let after_route = expected_random;
        let long_attack = expected_random.next_byte() < 127;
        let before_attack = expected_random;
        seen[usize::from(long_attack)] = true;
        let primary = player(&mut objects);
        let mut shared = EncounterCoordination::default();
        let mut display = EncounterHealthDisplay::default();
        let mut events = AudioState::default();
        let mut weapons = WeaponState::default();
        let mut inputs = world(&mut random);
        inputs.health_display = Some(&mut display);
        inputs.coordination = Some(&mut shared);
        inputs.selected = Some(primary);
        inputs.primary_player = Some(primary);
        inputs.primary_motion = Some(PrimaryMotionInput {
            auxiliary_mode: 16,
            displacement: Vector3::default(),
        });
        inputs.weapons = Some(&mut weapons);
        inputs.audio = Some(audio(&mut events));
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        let last_visit = if long_attack { 236 } else { 132 };
        let mut dust = Vec::new();
        let mut shots = Vec::new();
        let mut arc_y = -200;
        for visit in 1..=last_visit {
            let before_spawn = runtime.spawns.last_spawn;
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            let actor = objects.get(owner).unwrap();
            if visit <= 50 {
                assert_eq!(actor.base.position.y, 200);
                assert_eq!(
                    actor.extension.radar_marker.packed(),
                    // Unlike the outer arena, this entry retains the radar
                    // marker published by the shared enemy initializer.
                    134
                );
                assert_eq!(inputs.random, &after_route);
            } else if visit == 51 {
                assert_eq!(actor.base.position.y, 0);
                assert_eq!(inputs.random, &before_attack);
            } else if visit < 71 {
                arc_y += ARC[visit - 52];
                assert_eq!(actor.base.position.y, arc_y);
                assert_eq!(actor.base.speed, if long_attack { 45 } else { 50 });
            } else if visit == 71 {
                assert_eq!(actor.base.position.y, -100);
                // The short attack preserves its speed until the next visit.
                assert_eq!(actor.base.speed, if long_attack { 0 } else { 50 });
            }
            if before_spawn != runtime.spawns.last_spawn {
                let spawned = objects.get(runtime.spawns.last_spawn.unwrap()).unwrap();
                if spawned.base.shape == ShapeId::from_catalog_index(22) {
                    dust.push(visit);
                    assert_eq!(spawned.base.position.y, 10);
                    assert_eq!(
                        (spawned.base.hit_points, spawned.base.attack_power),
                        (8, 50)
                    );
                } else {
                    shots.push(visit);
                    if long_attack {
                        assert_eq!(spawned.base.shape, ShapeId::from_catalog_index(20));
                        assert_eq!(spawned.base.attachment, Some(owner));
                        assert_eq!(spawned.base.child_number, 11);
                        assert_eq!(
                            (spawned.base.hit_points, spawned.base.attack_power),
                            (10, 10)
                        );
                    } else {
                        assert_eq!(
                            spawned.base.path,
                            Some(authored_paths::OCCUPANCY_SURFACE_LIMITED)
                        );
                        assert_eq!(spawned.base.speed, 40);
                        // Source hostile launch consumes one draw precisely
                        // in the primary player's aligned half-plane.
                        if 192_u8.wrapping_sub(spawned.base.yaw.units()) >= 128 {
                            expected_random.next_byte();
                        }
                    }
                }
            }
            if visit >= 51 {
                assert_eq!(inputs.random, &expected_random);
            }
        }
        assert_eq!(
            dust,
            if long_attack {
                vec![52, 236]
            } else {
                vec![52, 72]
            }
        );
        assert_eq!(
            shots,
            if long_attack {
                vec![111, 136, 161, 186, 211]
            } else {
                vec![62]
            }
        );
        assert_eq!(objects.len(), if long_attack { 9 } else { 5 });
        assert_eq!(objects.get(owner).unwrap().base.position.y, 200);
        expected_random.next_byte();
        expected_random.next_byte();
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 100)
                .unwrap()
                .step,
            ControlStep::Movement
        );
        assert_eq!(inputs.random, &expected_random);
    }
    assert_eq!(seen, [true; 2]);
}
