//! Complete guidance/radio controller, with native main and callback phases.
use super::super::path_countdown::PathCountdown;
use super::super::path_radio::{DeferredMessage, PathRadio, RadioEvent, RadioLayout, RadioRequest};
use super::super::path_runtime::{CallbackStep, TriggerWorldInputs};
use super::super::{
    authored_paths, Behavior, Difficulty, FlightControlStyle, ObjectKind, ObjectSpawnDefaults,
    ShapeId, Vector3,
};
use super::tests::{setup, world};
use super::*;

struct Services {
    scene: ScenePathInputs,
    difficulty: Difficulty,
    style: FlightControlStyle,
    deferred: DeferredMessage,
    event: RadioEvent,
    request: RadioRequest,
    guidance: GuidanceHistory,
    countdown: PathCountdown,
    auxiliary: SelectedAuxiliaryState,
}

impl Default for Services {
    fn default() -> Self {
        Self {
            scene: ScenePathInputs {
                wingmate_pilot: Some(2),
                remaining_objectives: Some(1),
                player_configuration: Some(0),
                encounter_location: Some(0),
                map_region: Some(0),
                ..Default::default()
            },
            difficulty: Difficulty::Normal,
            style: FlightControlStyle::TypeA,
            deferred: DeferredMessage::default(),
            event: RadioEvent::default(),
            request: RadioRequest::default(),
            guidance: GuidanceHistory::default(),
            countdown: PathCountdown::default(),
            auxiliary: SelectedAuxiliaryState {
                mode: 0x10,
                action_flags: 0,
            },
        }
    }
}

impl Services {
    fn inputs<'a>(&'a mut self, random: &'a mut RandomState, selected: ObjectId) -> PathWorld<'a> {
        let mut inputs = world(random);
        inputs.scene = self.scene;
        inputs.campaign = Some(CampaignPathInputs {
            difficulty: self.difficulty,
            encounter_variant: 0,
        });
        inputs.control_style = Some(self.style);
        inputs.deferred_message = Some(&mut self.deferred);
        inputs.radio_event = Some(&mut self.event);
        inputs.radio = Some(PathRadio {
            request: &mut self.request,
            layout: RadioLayout {
                compact_panel: false,
                tracked_screen_y: 100,
            },
        });
        inputs.guidance = Some(&mut self.guidance);
        inputs.countdown = Some(&mut self.countdown);
        inputs.selected_auxiliary = Some(&mut self.auxiliary);
        inputs.selected = Some(selected);
        inputs.spawn_defaults = Some(ObjectSpawnDefaults::default());
        inputs
    }
}

fn selected(objects: &mut ObjectStore) -> ObjectId {
    let mut player = Object::new(ObjectKind::Player, ShapeId::EMPTY, Behavior::PlayerFlight);
    player.base.position = Vector3 {
        x: 100,
        y: 32700,
        z: -300,
    };
    objects.allocate(player).unwrap()
}

fn callbacks(
    runtime: &mut PathRuntime,
    catalog: &PathCatalog,
    objects: &mut ObjectStore,
    owner: ObjectId,
    inputs: &mut PathWorld<'_>,
    tick: u8,
) {
    assert!(runtime.begin_callbacks(objects, owner).unwrap());
    for _ in 0..32 {
        match runtime
            .step_callbacks(
                objects,
                owner,
                TriggerWorldInputs {
                    strategy_tick: tick,
                    ..Default::default()
                },
            )
            .unwrap()
        {
            CallbackStep::Run(_) => assert_eq!(
                runtime
                    .resume_program(catalog, objects, owner, inputs, 64)
                    .map(|exit| {
                        assert_eq!(exit.actor, owner);
                        exit.step
                    }),
                Ok(ControlStep::ResumeCallbacks)
            ),
            CallbackStep::Skipped | CallbackStep::Expired => {}
            CallbackStep::Complete => return,
        }
    }
    panic!("guidance callback pass did not complete");
}

fn visit(
    runtime: &mut PathRuntime,
    catalog: &PathCatalog,
    objects: &mut ObjectStore,
    owner: ObjectId,
    inputs: &mut PathWorld<'_>,
    tick: u8,
) {
    assert_eq!(
        runtime
            .enter_program(catalog, objects, owner, inputs, 64)
            .map(|exit| exit.step),
        Ok(ControlStep::Movement)
    );
    callbacks(runtime, catalog, objects, owner, inputs, tick);
}

fn trigger_count(runtime: &PathRuntime, objects: &ObjectStore, owner: ObjectId) -> usize {
    objects
        .get(owner)
        .unwrap()
        .extension
        .path_state
        .triggers
        .entries(&runtime.resources, owner)
        .unwrap()
        .len()
}

#[test]
fn controller_startup_preserves_deferred_zero_absence_and_both_tutorial_wait_routes() {
    let catalog = authored_paths::catalog();
    for pilot in [0, 2, 127, 128, 254, 255] {
        for deferred in [0, 1, 0x7FFF, 0x8000, 0xFFFF] {
            for (configuration, difficulty, history, hold_visit, triggers) in [
                (0, Difficulty::Normal, 0, 76, 4),
                (0, Difficulty::Hard, 0, 76, 4),
                (0, Difficulty::Expert, 0, 16, 2),
                (0, Difficulty::Normal, 0x0010, 16, 2),
                (9, Difficulty::Normal, 0, 36, 4),
                (9, Difficulty::Normal, 0x8000, 16, 2),
            ] {
                let (mut runtime, mut objects, owner, mut random) = setup();
                let player = selected(&mut objects);
                let before_random = random;
                objects.get_mut(owner).unwrap().base.path =
                    Some(authored_paths::GUIDANCE_RADIO_CONTROLLER);
                let mut services = Services::default();
                services.scene.wingmate_pilot = Some(pilot);
                services.scene.player_configuration = Some(configuration);
                services.scene.encounter_location = Some(8);
                services.difficulty = difficulty;
                services.guidance.flags = history;
                services.deferred.number = deferred;
                let last_visit = if pilot == 255 { 1 } else { hold_visit };
                for tick in 1..=last_visit {
                    services.request.pending = false;
                    let mut inputs = services.inputs(&mut random, player);
                    // Absence jumps directly to HOLD, without needing any
                    // deferred request, scene mode, difficulty or history.
                    if pilot == 255 {
                        inputs.deferred_message = None;
                        inputs.campaign = None;
                        inputs.guidance = None;
                        inputs.scene.player_configuration = None;
                    }
                    visit(
                        &mut runtime,
                        &catalog,
                        &mut objects,
                        owner,
                        &mut inputs,
                        tick,
                    );
                    let actor = objects.get(owner).unwrap();
                    assert_eq!(actor.extension.path_state.hold_latched, tick == last_visit);
                    let expected_request = pilot != 255 && deferred != 0 && tick == 1;
                    assert_eq!(services.request.pending, expected_request);
                    if expected_request {
                        let number = deferred.wrapping_add(pilot as i8 as i16 as u16) as u8;
                        assert_eq!(
                            services.request.message.index(),
                            (usize::from(number) + 255) % 256
                        );
                    }
                    if tick % 8 == 0 {
                        assert_eq!(
                            actor.base.position,
                            Vector3 {
                                x: 100,
                                y: -31836,
                                z: -300
                            }
                        );
                    }
                }
                assert_eq!(
                    services.deferred.number,
                    if pilot == 255 { deferred } else { 0 }
                );
                assert_eq!(
                    trigger_count(&runtime, &objects, owner),
                    if pilot == 255 { 2 } else { triggers }
                );
                assert_eq!(random, before_random);
            }
        }
    }
}

#[test]
fn normal_guidance_repeats_on_clock_gate_then_finishes_after_forty_consecutive_action_ticks() {
    let catalog = authored_paths::catalog();
    for style in [FlightControlStyle::TypeA, FlightControlStyle::TypeB] {
        for mode in [0x10, 0x20, 0x30] {
            let (mut runtime, mut objects, owner, mut random) = setup();
            let player = selected(&mut objects);
            let before_random = random;
            objects.get_mut(owner).unwrap().base.path =
                Some(authored_paths::GUIDANCE_RADIO_CONTROLLER);
            let mut services = Services {
                style,
                ..Default::default()
            };
            services.auxiliary.mode = mode;
            for tick in 1..=128 {
                services.request.pending = false;
                visit(
                    &mut runtime,
                    &catalog,
                    &mut objects,
                    owner,
                    &mut services.inputs(&mut random, player),
                    tick,
                );
                assert_eq!(services.request.pending, tick == 128 && mode == 0x10);
            }
            assert_eq!(trigger_count(&runtime, &objects, owner), 4);
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .relative_rotation
                    .pitch
                    .units(),
                2
            );
            if mode == 0x10 {
                assert_eq!(
                    services.request.message.index(),
                    if style == FlightControlStyle::TypeA {
                        27
                    } else {
                        123
                    }
                );
            }
            // Partial progress is discarded when action bit 40 disappears.
            services.auxiliary.action_flags = 0x40;
            for tick in 129..=140 {
                visit(
                    &mut runtime,
                    &catalog,
                    &mut objects,
                    owner,
                    &mut services.inputs(&mut random, player),
                    tick,
                );
            }
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .script_value,
                12
            );
            services.auxiliary.action_flags = 0;
            visit(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut services.inputs(&mut random, player),
                141,
            );
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .script_value,
                0
            );
            services.auxiliary.action_flags = 0x40;
            for progress in 1..=40 {
                visit(
                    &mut runtime,
                    &catalog,
                    &mut objects,
                    owner,
                    &mut services.inputs(&mut random, player),
                    141 + progress,
                );
                assert_eq!(
                    services.guidance.flags,
                    if progress == 40 { 0x10 } else { 0 }
                );
            }
            // FORCE resumes at the cancellation tail on the next main turn.
            visit(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut services.inputs(&mut random, player),
                182,
            );
            assert_eq!(trigger_count(&runtime, &objects, owner), 2);
            assert_eq!(random, before_random);
        }
    }
}

#[test]
fn alternate_guidance_waits_for_the_selected_mode_then_cancels_both_tutorial_callbacks() {
    let catalog = authored_paths::catalog();
    for location in [8, 2] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let player = selected(&mut objects);
        let before_random = random;
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::GUIDANCE_RADIO_CONTROLLER);
        let mut services = Services::default();
        services.scene.player_configuration = Some(9);
        services.scene.encounter_location = Some(location);
        services.auxiliary.mode = if location == 8 { 0x10 } else { 0x20 };
        for tick in 1..=128 {
            services.request.pending = false;
            visit(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut services.inputs(&mut random, player),
                tick,
            );
            assert_eq!(services.guidance.flags, 0);
            assert_eq!(services.request.pending, tick == 128);
        }
        assert_eq!(services.request.message.index(), 57);
        assert_eq!(services.countdown.remaining, 60);
        assert_eq!(trigger_count(&runtime, &objects, owner), 4);
        services.auxiliary.mode = 0x30;
        visit(
            &mut runtime,
            &catalog,
            &mut objects,
            owner,
            &mut services.inputs(&mut random, player),
            129,
        );
        assert_eq!(services.guidance.flags, 0x8000);
        visit(
            &mut runtime,
            &catalog,
            &mut objects,
            owner,
            &mut services.inputs(&mut random, player),
            130,
        );
        assert_eq!(trigger_count(&runtime, &objects, owner), 2);
        assert_eq!(random, before_random);
    }
}

#[test]
fn event_callback_spawns_a_delayed_reply_with_borrowed_argument_and_fresh_wingmate_snapshot() {
    let catalog = authored_paths::catalog();
    for event_number in [16, 17] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let player = selected(&mut objects);
        let before_random = random;
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::GUIDANCE_RADIO_CONTROLLER);
        let mut services = Services {
            difficulty: Difficulty::Expert,
            ..Default::default()
        };
        services.countdown.remaining = 23;
        for tick in 1..=16 {
            visit(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut services.inputs(&mut random, player),
                tick,
            );
        }
        services.event.number = 0xA500 | event_number;
        visit(
            &mut runtime,
            &catalog,
            &mut objects,
            owner,
            &mut services.inputs(&mut random, player),
            17,
        );
        let reply = runtime.spawns.last_spawn.unwrap();
        assert_ne!(reply, owner);
        assert_eq!(runtime.spawns.parameter, Some(event_number as u8));
        let child = objects.get(reply).unwrap();
        assert_eq!(child.extension.path_state.motion_phase, event_number);
        assert!(child.extension.path_state.needs_path_initialization);
        assert_eq!(child.extension.parent, None);
        assert_eq!(
            objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .script_parameter,
            119
        );
        assert_eq!(services.countdown.remaining, 23);
        assert_eq!(
            services.request.message.index(),
            usize::from(event_number) - 1
        );
        // Reply samples its own published pilot once, not the parent's cached
        // pilot or a later publication during its sixty-count wait.
        services.scene.wingmate_pilot = Some(7);
        services.request.pending = false;
        for turn in 1..=61 {
            let mut inputs = services.inputs(&mut random, player);
            if turn > 1 {
                inputs.scene.wingmate_pilot = None;
                inputs.scene.map_region = None;
            }
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, reply, &mut inputs, 24)
                    .map(|exit| exit.step),
                Ok(if turn == 61 {
                    ControlStep::Ended
                } else {
                    ControlStep::Movement
                })
            );
            assert_eq!(services.request.pending, turn == 61);
        }
        assert_eq!(
            services.request.message.index(),
            if event_number == 16 { 44 } else { 50 }
        );
        assert!(objects.get(reply).unwrap().base.flags.remove_after_tick);
        // Parent's cooldown does not use the shared countdown. It clears
        // only the low event byte on the final decrement.
        services.request.pending = false;
        for remaining in (0..119).rev() {
            callbacks(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut services.inputs(&mut random, player),
                1,
            );
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .script_parameter,
                remaining
            );
            assert_eq!(
                services.event.number,
                if remaining == 0 {
                    0xA500
                } else {
                    0xA500 | event_number
                }
            );
            assert!(!services.request.pending);
        }
        assert_eq!(services.countdown.remaining, 23);
        assert_eq!(random, before_random);
    }
}

#[test]
fn event_service_short_circuits_inactive_scene_and_uses_shared_sixty_count_cooldown_without_reply()
{
    let catalog = authored_paths::catalog();
    for (pilot, region, number) in [(2, Some(8), 16), (2, Some(0), 18), (255, None, 17)] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let player = selected(&mut objects);
        let before_random = random;
        objects.get_mut(owner).unwrap().base.path = Some(authored_paths::GUIDANCE_RADIO_CONTROLLER);
        let mut services = Services {
            difficulty: Difficulty::Expert,
            ..Default::default()
        };
        services.scene.wingmate_pilot = Some(pilot);
        for tick in 1..=if pilot == 255 { 1 } else { 16 } {
            visit(
                &mut runtime,
                &catalog,
                &mut objects,
                owner,
                &mut services.inputs(&mut random, player),
                tick,
            );
        }
        let before_count = objects.active_ids().len();
        services.scene.remaining_objectives = Some(0);
        let mut inputs = services.inputs(&mut random, player);
        inputs.radio_event = None;
        inputs.radio = None;
        inputs.scene.map_region = None;
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
        assert_eq!(
            objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .script_parameter,
            0
        );
        services.scene.remaining_objectives = Some(1);
        services.scene.map_region = region;
        services.event.number = 0xB300 | number;
        callbacks(
            &mut runtime,
            &catalog,
            &mut objects,
            owner,
            &mut services.inputs(&mut random, player),
            1,
        );
        assert_eq!(objects.active_ids().len(), before_count);
        assert_eq!(runtime.spawns.last_spawn, None);
        assert_eq!(services.countdown.remaining, 60);
        assert_eq!(
            objects
                .get(owner)
                .unwrap()
                .extension
                .path_state
                .script_parameter,
            59
        );
        assert_eq!(services.request.message.index(), usize::from(number) - 1);
        services.request.pending = false;
        for remaining in (0..59).rev() {
            let mut inputs = services.inputs(&mut random, player);
            inputs.scene.remaining_objectives = None;
            inputs.scene.map_region = None;
            inputs.radio = None;
            callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs, 1);
            assert_eq!(
                objects
                    .get(owner)
                    .unwrap()
                    .extension
                    .path_state
                    .script_parameter,
                remaining
            );
            assert_eq!(
                services.event.number,
                if remaining == 0 {
                    0xB300
                } else {
                    0xB300 | number
                }
            );
            assert!(!services.request.pending);
        }
        assert_eq!(random, before_random);
    }
}

#[test]
fn reply_resamples_region_before_requiring_pilot_or_waiting() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let player = selected(&mut objects);
    let before_random = random;
    objects.get_mut(owner).unwrap().base.path = Some(authored_paths::GUIDANCE_RADIO_CONTROLLER);
    let mut services = Services {
        difficulty: Difficulty::Expert,
        ..Default::default()
    };
    for tick in 1..=16 {
        visit(
            &mut runtime,
            &catalog,
            &mut objects,
            owner,
            &mut services.inputs(&mut random, player),
            tick,
        );
    }
    services.event.number = 16;
    callbacks(
        &mut runtime,
        &catalog,
        &mut objects,
        owner,
        &mut services.inputs(&mut random, player),
        1,
    );
    let reply = runtime.spawns.last_spawn.unwrap();
    services.scene.map_region = Some(8);
    services.scene.wingmate_pilot = None;
    services.request.pending = false;
    assert_eq!(
        runtime
            .enter_program(
                &catalog,
                &mut objects,
                reply,
                &mut services.inputs(&mut random, player),
                8
            )
            .unwrap()
            .step,
        ControlStep::Ended
    );
    assert!(!services.request.pending);
    assert_eq!(objects.get(reply).unwrap().base.wait_timer, 0);
    // This early exit precedes InvisibleOn; retirement does not invent it.
    assert!(objects.get(reply).unwrap().base.flags.visible);
    assert!(!objects.get(reply).unwrap().base.flags.collision_disabled);
    assert_eq!(random, before_random);
}
